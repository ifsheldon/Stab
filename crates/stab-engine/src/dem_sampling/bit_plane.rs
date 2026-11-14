use super::program::{DemPoll, DemSampleBlock, DemSampleError, DemSampleOperation};
use super::{DemError, DemResult};
use crate::bernoulli::{
    INDEXED_BLOCK_SHOTS, INDEXED_BLOCK_WORDS, IndexedRangeError, IndexedShotRange, SparseBernoulli,
    sample_indexed_range_into,
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
    poll: &mut DemPoll<'_>,
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
    // An invalid range is reported by the first sampled error, as the per-error sampler did.
    let range = IndexedShotRange::try_new(first_shot, shot_count);
    let mut stream_cursor = 0_usize;
    sample_block(
        block,
        0,
        seed,
        range,
        detector_planes,
        observable_planes,
        &mut error_output,
        &mut stream_cursor,
        poll,
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
    range: Result<IndexedShotRange, IndexedRangeError>,
    detector_planes: &mut [u64],
    observable_planes: &mut [u64],
    error_output: &mut SampledErrorPlanes<'_>,
    stream_cursor: &mut usize,
    poll: &mut DemPoll<'_>,
) -> DemResult<()> {
    for operation in &block.operations {
        match operation {
            DemSampleOperation::Error(error) => {
                poll.tick()?;
                sample_error(
                    error,
                    detector_shift,
                    seed,
                    range,
                    detector_planes,
                    observable_planes,
                    error_output,
                    stream_cursor,
                )?;
            }
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
                            range,
                            detector_planes,
                            observable_planes,
                            error_output,
                            stream_cursor,
                            poll,
                        )?;
                        continue;
                    }
                    if let Some(folded_errors) = repeat.folded_zero_shift_errors.as_deref() {
                        let shift = detector_shift
                            .checked_add(repeat.start_detector_shift)
                            .ok_or_else(detector_shift_overflow_error)?;
                        for error in folded_errors {
                            poll.tick()?;
                            sample_error(
                                error,
                                shift,
                                seed,
                                range,
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
                    poll.tick()?;
                    sample_block(
                        &repeat.body,
                        shift,
                        seed,
                        range,
                        detector_planes,
                        observable_planes,
                        error_output,
                        stream_cursor,
                        poll,
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
    range: Result<IndexedShotRange, IndexedRangeError>,
    detector_planes: &mut [u64],
    observable_planes: &mut [u64],
    error_output: &mut SampledErrorPlanes<'_>,
    stream_cursor: &mut usize,
) -> DemResult<()> {
    let stream_position = *stream_cursor;
    *stream_cursor = stream_cursor.checked_add(1).ok_or_else(|| {
        DemError::invalid_sampler_compilation("DEM sample stream index overflowed")
    })?;
    let stream = ErrorStream {
        seed,
        position: stream_position,
        range: range.map_err(indexed_range_error)?,
    };
    match error.sparse() {
        Some(sparse) => sample_sparse_error(
            error,
            sparse,
            detector_shift,
            stream,
            detector_planes,
            observable_planes,
            error_output,
        ),
        None => sample_masked_error(
            error,
            detector_shift,
            stream,
            detector_planes,
            observable_planes,
            error_output,
        ),
    }
}

/// The indexed random stream and shot range of one sampled error.
#[derive(Clone, Copy)]
struct ErrorStream {
    seed: u64,
    position: usize,
    range: IndexedShotRange,
}

impl ErrorStream {
    fn index(self) -> DemResult<u64> {
        u64::try_from(self.position).map_err(|_| {
            DemError::invalid_sampler_compilation("DEM sample stream index does not fit in u64")
        })
    }

    fn word_count(self) -> usize {
        self.range.shot_count().div_ceil(u64::BITS as usize)
    }
}

/// Toggles each sampled hit directly into the target planes, without a block mask.
fn sample_sparse_error(
    error: &DemSampleError,
    sparse: &SparseBernoulli,
    detector_shift: u64,
    stream: ErrorStream,
    detector_planes: &mut [u64],
    observable_planes: &mut [u64],
    error_output: &mut SampledErrorPlanes<'_>,
) -> DemResult<()> {
    let word_count = stream.word_count();
    let mut error_plane = match error_output {
        SampledErrorPlanes::Discard => None,
        SampledErrorPlanes::Record(error_planes) => Some(
            stream
                .position
                .checked_mul(word_count)
                .and_then(|start| error_planes.get_mut(start..start.checked_add(word_count)?))
                .ok_or_else(|| {
                    DemError::invalid_sampler_compilation(
                        "DEM sampled-error plane index escaped compiled storage",
                    )
                })?,
        ),
    };
    sparse.try_for_each_hit(stream.seed, stream.index()?, stream.range, |position| {
        let word = position / u64::BITS as usize;
        let bit = 1_u64 << (position % u64::BITS as usize);
        if let Some(plane) = error_plane.as_deref_mut() {
            *plane.get_mut(word).ok_or_else(|| {
                DemError::invalid_sampler_compilation(
                    "DEM sampled-error hit escaped its shot range",
                )
            })? |= bit;
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
            toggle_plane_bit(detector_planes, index, word_count, word, bit, "detector")?;
        }
        for observable in &error.observables {
            toggle_plane_bit(
                observable_planes,
                *observable,
                word_count,
                word,
                bit,
                "observable",
            )?;
        }
        Ok(())
    })
}

/// Samples a whole mask for dense, complemented, and boundary probabilities.
fn sample_masked_error(
    error: &DemSampleError,
    detector_shift: u64,
    stream: ErrorStream,
    detector_planes: &mut [u64],
    observable_planes: &mut [u64],
    error_output: &mut SampledErrorPlanes<'_>,
) -> DemResult<()> {
    let word_count = stream.word_count();
    let mut masks = [0_u64; INDEXED_BLOCK_WORDS];
    let active_masks = masks.get_mut(..word_count).ok_or_else(|| {
        DemError::invalid_sampler_compilation("DEM sample window exceeds indexed mask storage")
    })?;
    sample_indexed_range_into(
        error.probability(),
        stream.seed,
        stream.index()?,
        stream.range.first_shot(),
        stream.range.shot_count(),
        active_masks,
    )
    .map_err(indexed_range_error)?;
    if let SampledErrorPlanes::Record(error_planes) = error_output {
        let plane = stream
            .position
            .checked_mul(word_count)
            .and_then(|start| error_planes.get_mut(start..start.checked_add(word_count)?))
            .ok_or_else(|| {
                DemError::invalid_sampler_compilation(
                    "DEM sampled-error plane index escaped compiled storage",
                )
            })?;
        plane.copy_from_slice(active_masks);
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

/// Toggles `bit` of word `word` in plane `plane_index` of planes holding `word_count` words each.
#[inline]
fn toggle_plane_bit(
    planes: &mut [u64],
    plane_index: usize,
    word_count: usize,
    word: usize,
    bit: u64,
    kind: &'static str,
) -> DemResult<()> {
    let target = plane_index
        .checked_mul(word_count)
        .filter(|_| word < word_count)
        .and_then(|start| planes.get_mut(start.checked_add(word)?))
        .ok_or_else(|| {
            DemError::invalid_sampler_compilation(format!(
                "{kind} index {plane_index} is out of range"
            ))
        })?;
    *target ^= bit;
    Ok(())
}

/// XORs `masks` into plane `plane_index` of bit-major planes holding `word_count` words each.
fn xor_plane_masks(
    planes: &mut [u64],
    plane_index: usize,
    masks: &[u64],
    word_count: usize,
    kind: &'static str,
) -> DemResult<()> {
    let plane = plane_index
        .checked_mul(word_count)
        .and_then(|start| planes.get_mut(start..start.checked_add(word_count)?))
        .ok_or_else(|| {
            DemError::invalid_sampler_compilation(format!(
                "{kind} index {plane_index} is out of range"
            ))
        })?;
    for (plane, mask) in plane.iter_mut().zip(masks) {
        *plane ^= *mask;
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        reason = "focused sampler tests compare fixed pairs and fail immediately on fixture errors"
    )]

    use super::*;

    /// Planes start from a fixed nonzero pattern so the test sees XOR rather than overwrite.
    fn patterned_planes(len: usize, salt: u64) -> Vec<u64> {
        (0..len as u64)
            .map(|index| (index.wrapping_add(salt)).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .collect()
    }

    #[test]
    fn sparse_hits_toggle_the_same_planes_as_the_mask_path() {
        const DETECTORS: usize = 5;
        const OBSERVABLES: usize = 2;
        const ERRORS: usize = 3;
        let ranges: [(u64, usize); 11] = [
            (0, 1024),
            (0, 1),
            (0, 63),
            (0, 64),
            (0, 1000),
            (1, 1),
            (17, 47),
            (100, 924),
            (1000, 25),
            (1000, 1024),
            (5 * 1024 + 513, 1024),
        ];
        let mut toggled_hits = 0_u32;
        for probability in [1e-9, 6.67e-5, 1e-3, 0.004, 0.0199] {
            let error = DemSampleError::new(probability, vec![0, 3], vec![1]);
            let sparse = *error.sparse().expect("sparse probability");
            for (first_shot, shot_count) in ranges {
                for seed in [0, 13, u64::MAX] {
                    let word_count = shot_count.div_ceil(64);
                    let stream = ErrorStream {
                        seed,
                        position: 1,
                        range: IndexedShotRange::try_new(first_shot, shot_count)
                            .expect("valid range"),
                    };
                    let mut outputs = Vec::new();
                    for use_sparse in [false, true] {
                        let mut detector_planes = patterned_planes(DETECTORS * word_count, 1);
                        let mut observable_planes = patterned_planes(OBSERVABLES * word_count, 2);
                        let mut error_planes = vec![0; ERRORS * word_count];
                        let mut error_output = SampledErrorPlanes::Record(&mut error_planes);
                        if use_sparse {
                            sample_sparse_error(
                                &error,
                                &sparse,
                                1,
                                stream,
                                &mut detector_planes,
                                &mut observable_planes,
                                &mut error_output,
                            )
                        } else {
                            sample_masked_error(
                                &error,
                                1,
                                stream,
                                &mut detector_planes,
                                &mut observable_planes,
                                &mut error_output,
                            )
                        }
                        .expect("sample one error");
                        outputs.push((detector_planes, observable_planes, error_planes));
                    }
                    let context =
                        format!("p={probability} range={first_shot}+{shot_count} seed={seed}");
                    assert_eq!(outputs[0], outputs[1], "{context}");
                    toggled_hits += outputs[0]
                        .2
                        .iter()
                        .map(|word| word.count_ones())
                        .sum::<u32>();
                }
            }
        }
        assert!(toggled_hits > 200, "fixture must exercise many hits");
    }

    #[test]
    fn sparse_hits_report_out_of_range_targets_like_the_mask_path() {
        let error = DemSampleError::new(0.019, vec![7], Vec::new());
        let sparse = *error.sparse().expect("sparse probability");
        let stream = ErrorStream {
            seed: 3,
            position: 0,
            range: IndexedShotRange::try_new(0, 1024).expect("valid range"),
        };
        let mut detector_planes = vec![0; 16];
        let failure = sample_sparse_error(
            &error,
            &sparse,
            0,
            stream,
            &mut detector_planes,
            &mut [],
            &mut SampledErrorPlanes::Discard,
        )
        .expect_err("detector 7 is outside one plane");
        assert!(
            failure
                .to_string()
                .contains("detector index 7 is out of range"),
            "{failure}"
        );
    }
}
