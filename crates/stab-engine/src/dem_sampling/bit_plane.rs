use super::program::{DemSampleBlock, DemSampleError, DemSampleOperation};
use super::{DemError, DemResult};
use crate::bernoulli::{
    INDEXED_BLOCK_SHOTS, INDEXED_BLOCK_WORDS, IndexedRangeError, sample_indexed_range_into,
};

pub(super) enum SampledErrorPlanes<'a> {
    Discard,
    Record(&'a mut Vec<u64>),
}

impl SampledErrorPlanes<'_> {
    fn is_discard(&self) -> bool {
        matches!(self, Self::Discard)
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the compiled program, deterministic stream identity, and three reusable outputs are independent inputs"
)]
pub(super) fn sample_into_planes(
    block: &DemSampleBlock,
    detector_count: usize,
    observable_count: usize,
    seed: u64,
    first_shot: u64,
    shot_count: usize,
    detector_planes: &mut Vec<u64>,
    observable_planes: &mut Vec<u64>,
    mut error_output: SampledErrorPlanes<'_>,
) -> DemResult<()> {
    if shot_count > INDEXED_BLOCK_SHOTS {
        return Err(DemError::invalid_sampler_compilation(format!(
            "DEM bit-plane sampler supports at most {} shots per batch, got {shot_count}",
            INDEXED_BLOCK_SHOTS
        )));
    }
    if shot_count == 0 {
        detector_planes.clear();
        observable_planes.clear();
        if let SampledErrorPlanes::Record(error_planes) = error_output {
            error_planes.clear();
        }
        return Ok(());
    }
    let word_count = shot_count.div_ceil(u64::BITS as usize);
    reset_planes(detector_planes, detector_count, word_count)?;
    reset_planes(observable_planes, observable_count, word_count)?;
    if let SampledErrorPlanes::Record(error_planes) = &mut error_output {
        reset_planes(error_planes, block.error_count, word_count)?;
    }
    let expected_streams = if error_output.is_discard() {
        block.direct_sample_work_count
    } else {
        block.error_count
    };
    let mut stream_cursor = 0_usize;
    sample_block(
        block,
        0,
        seed,
        first_shot,
        shot_count,
        detector_planes,
        observable_planes,
        &mut error_output,
        &mut stream_cursor,
    )?;
    if stream_cursor != expected_streams {
        return Err(DemError::invalid_sampler_compilation(format!(
            "DEM bit-plane sampler consumed {stream_cursor} random streams but {expected_streams} were compiled"
        )));
    }
    if let SampledErrorPlanes::Record(error_planes) = error_output
        && error_planes.len()
            != block
                .error_count
                .checked_mul(word_count)
                .ok_or_else(plane_storage_overflow_error)?
    {
        return Err(DemError::invalid_sampler_compilation(format!(
            "DEM bit-plane sampler produced {} error planes but {} were compiled",
            error_planes.len(),
            block.error_count
        )));
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "recursive DEM traversal carries immutable stream coordinates and reusable plane outputs"
)]
fn sample_block(
    block: &DemSampleBlock,
    detector_shift: u64,
    seed: u64,
    first_shot: u64,
    shot_count: usize,
    detector_planes: &mut [u64],
    observable_planes: &mut [u64],
    error_output: &mut SampledErrorPlanes<'_>,
    stream_cursor: &mut usize,
) -> DemResult<()> {
    for operation in &block.operations {
        match operation {
            DemSampleOperation::Error(error) => sample_error(
                error,
                detector_shift,
                seed,
                first_shot,
                shot_count,
                detector_planes,
                observable_planes,
                error_output,
                stream_cursor,
            )?,
            DemSampleOperation::Repeat(repeat) => {
                if error_output.is_discard() {
                    if repeat.body.direct_sample_effect_count == 0 {
                        continue;
                    }
                    if repeat.body.detector_shift == 0
                        && !repeat.body.direct_sample_has_stochastic_error
                    {
                        if repeat.repeat_count.is_multiple_of(2) {
                            continue;
                        }
                        let shift = detector_shift
                            .checked_add(repeat.start_detector_shift)
                            .ok_or_else(detector_shift_overflow_error)?;
                        sample_block(
                            &repeat.body,
                            shift,
                            seed,
                            first_shot,
                            shot_count,
                            detector_planes,
                            observable_planes,
                            error_output,
                            stream_cursor,
                        )?;
                        continue;
                    }
                    if let Some(folded_errors) = repeat.folded_zero_shift_errors.as_deref() {
                        let shift = detector_shift
                            .checked_add(repeat.start_detector_shift)
                            .ok_or_else(detector_shift_overflow_error)?;
                        for error in folded_errors {
                            sample_error(
                                error,
                                shift,
                                seed,
                                first_shot,
                                shot_count,
                                detector_planes,
                                observable_planes,
                                error_output,
                                stream_cursor,
                            )?;
                        }
                        continue;
                    }
                }
                let mut shift = detector_shift
                    .checked_add(repeat.start_detector_shift)
                    .ok_or_else(detector_shift_overflow_error)?;
                for _ in 0..repeat.repeat_count {
                    sample_block(
                        &repeat.body,
                        shift,
                        seed,
                        first_shot,
                        shot_count,
                        detector_planes,
                        observable_planes,
                        error_output,
                        stream_cursor,
                    )?;
                    shift = shift
                        .checked_add(repeat.body.detector_shift)
                        .ok_or_else(detector_shift_overflow_error)?;
                }
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "one sampled error combines stream coordinates, shifted targets, and reusable outputs"
)]
fn sample_error(
    error: &DemSampleError,
    detector_shift: u64,
    seed: u64,
    first_shot: u64,
    shot_count: usize,
    detector_planes: &mut [u64],
    observable_planes: &mut [u64],
    error_output: &mut SampledErrorPlanes<'_>,
    stream_cursor: &mut usize,
) -> DemResult<()> {
    let stream_position = *stream_cursor;
    let stream_index = u64::try_from(stream_position).map_err(|_| {
        DemError::invalid_sampler_compilation("DEM sample stream index does not fit in u64")
    })?;
    *stream_cursor = stream_cursor.checked_add(1).ok_or_else(|| {
        DemError::invalid_sampler_compilation("DEM sample stream index overflowed")
    })?;
    let word_count = shot_count.div_ceil(u64::BITS as usize);
    let mut masks = [0_u64; INDEXED_BLOCK_WORDS];
    let active_masks = masks.get_mut(..word_count).ok_or_else(|| {
        DemError::invalid_sampler_compilation("DEM sample window exceeds indexed mask storage")
    })?;
    sample_indexed_range_into(
        error.probability,
        seed,
        stream_index,
        first_shot,
        shot_count,
        active_masks,
    )
    .map_err(indexed_range_error)?;
    if let SampledErrorPlanes::Record(error_planes) = error_output {
        for (word_index, mask) in active_masks.iter().copied().enumerate() {
            let index = word_index
                .checked_mul(error_planes.len() / word_count)
                .and_then(|offset| offset.checked_add(stream_position))
                .ok_or_else(plane_storage_overflow_error)?;
            let plane = error_planes.get_mut(index).ok_or_else(|| {
                DemError::invalid_sampler_compilation(
                    "DEM sampled-error plane index escaped compiled storage",
                )
            })?;
            *plane = mask;
        }
    }
    if active_masks.iter().all(|mask| *mask == 0) {
        return Ok(());
    }
    for detector in &error.detectors {
        let shifted = detector_shift
            .checked_add(*detector)
            .ok_or_else(detector_shift_overflow_error)?;
        let index = usize::try_from(shifted).map_err(|_| {
            DemError::invalid_sampler_compilation(format!(
                "detector index {shifted} does not fit in usize"
            ))
        })?;
        xor_plane_masks(detector_planes, index, active_masks, word_count, "detector")?;
    }
    for observable in &error.observables {
        xor_plane_masks(
            observable_planes,
            *observable,
            active_masks,
            word_count,
            "observable",
        )?;
    }
    Ok(())
}

fn xor_plane_masks(
    planes: &mut [u64],
    plane_index: usize,
    masks: &[u64],
    word_count: usize,
    kind: &'static str,
) -> DemResult<()> {
    let width = planes
        .len()
        .checked_div(word_count)
        .ok_or_else(plane_storage_overflow_error)?;
    if plane_index >= width {
        return Err(DemError::invalid_sampler_compilation(format!(
            "{kind} index {plane_index} is out of range"
        )));
    }
    for (word_index, mask) in masks.iter().copied().enumerate() {
        let index = word_index
            .checked_mul(width)
            .and_then(|offset| offset.checked_add(plane_index))
            .ok_or_else(plane_storage_overflow_error)?;
        let plane = planes
            .get_mut(index)
            .ok_or_else(plane_storage_overflow_error)?;
        *plane ^= mask;
    }
    Ok(())
}

fn reset_planes(planes: &mut Vec<u64>, width: usize, word_count: usize) -> DemResult<()> {
    let len = width
        .checked_mul(word_count)
        .ok_or_else(plane_storage_overflow_error)?;
    planes.resize(len, 0);
    planes.fill(0);
    Ok(())
}

fn indexed_range_error(error: IndexedRangeError) -> DemError {
    DemError::invalid_sampler_compilation(format!(
        "DEM indexed Bernoulli range is invalid: {error:?}"
    ))
}

fn plane_storage_overflow_error() -> DemError {
    DemError::invalid_sampler_compilation("DEM bit-plane storage size overflowed")
}

fn detector_shift_overflow_error() -> DemError {
    DemError::invalid_sampler_compilation("DEM sampler detector shift overflowed")
}
