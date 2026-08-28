use stab_records::{BitPlane64BatchView, DemSampleBatchView, DetectionBatchView, PackedShotBatch};

use super::{
    DemSamplingExecutionError, MAX_BATCH_SHOTS, MAX_DEM_SESSION_STORAGE_BYTES,
    internal_format_error,
};
use crate::DetectionRecordBuffer;
use crate::bernoulli::INDEXED_BLOCK_SHOTS;
use crate::dem_sampling::{DemError, DemResourceLimitError, DemSamplerLimits, DemSamplingPlan};

#[derive(Debug)]
pub(super) struct SessionBatch {
    detectors: PackedShotBatch,
    observables: PackedShotBatch,
    sampled_errors: Option<PackedShotBatch>,
    capacity: usize,
}

impl SessionBatch {
    pub(super) fn try_new(
        plan: &DemSamplingPlan,
        capacity: usize,
    ) -> Result<Self, DemSamplingExecutionError> {
        let detectors = PackedShotBatch::zeros(capacity, plan.detector_count())
            .map_err(storage_format_error)?;
        let observables = PackedShotBatch::zeros(capacity, plan.observable_count())
            .map_err(storage_format_error)?;
        Ok(Self {
            detectors,
            observables,
            sampled_errors: None,
            capacity,
        })
    }

    pub(super) const fn capacity(&self) -> usize {
        self.capacity
    }

    pub(super) const fn has_sampled_errors(&self) -> bool {
        self.sampled_errors.is_some()
    }

    pub(super) fn ensure_sampled_errors(
        &mut self,
        error_count: usize,
    ) -> Result<(), DemSamplingExecutionError> {
        if self.sampled_errors.is_none() {
            self.sampled_errors = Some(
                PackedShotBatch::zeros(self.capacity, error_count).map_err(storage_format_error)?,
            );
        }
        Ok(())
    }

    pub(super) fn copy_replay_record(
        &mut self,
        shot_index: usize,
        record: &DetectionRecordBuffer,
        error_record: &[bool],
    ) -> Result<(), DemSamplingExecutionError> {
        self.detectors
            .copy_shot_from_bools(shot_index, &record.detectors)
            .map_err(internal_format_error)?;
        self.observables
            .copy_shot_from_bools(shot_index, &record.observables)
            .map_err(internal_format_error)?;
        self.sampled_errors
            .as_mut()
            .ok_or_else(|| DemSamplingExecutionError::InternalInvariant {
                message: "DEM replay omitted its reusable sampled-error batch".to_owned(),
            })?
            .copy_shot_from_bools(shot_index, error_record)
            .map_err(internal_format_error)
    }

    pub(super) fn view(
        &self,
        shot_count: usize,
        include_sampled_errors: bool,
    ) -> Result<DemSampleBatchView<'_>, DemSamplingExecutionError> {
        let detectors = self
            .detectors
            .view_prefix(shot_count)
            .map_err(internal_format_error)?;
        let observables = self
            .observables
            .view_prefix(shot_count)
            .map_err(internal_format_error)?;
        let detection =
            DetectionBatchView::try_new(detectors, observables).map_err(internal_format_error)?;
        let sampled_errors = if include_sampled_errors {
            Some(
                self.sampled_errors
                    .as_ref()
                    .ok_or_else(|| DemSamplingExecutionError::InternalInvariant {
                        message: "DEM batch omitted requested sampled-error storage".to_owned(),
                    })?
                    .view_prefix(shot_count)
                    .map_err(internal_format_error)?,
            )
        } else {
            None
        };
        DemSampleBatchView::try_new(detection, sampled_errors).map_err(internal_format_error)
    }

    pub(super) fn copy_from_plane_chunk(
        &mut self,
        detector_planes: &[u64],
        observable_planes: &[u64],
        error_planes: Option<&[u64]>,
        plane_word_index: usize,
        shot_count: usize,
        include_sampled_errors: bool,
    ) -> Result<(), DemSamplingExecutionError> {
        copy_plane_words(
            &mut self.detectors,
            detector_planes,
            plane_word_index,
            shot_count,
        )?;
        copy_plane_words(
            &mut self.observables,
            observable_planes,
            plane_word_index,
            shot_count,
        )?;
        if include_sampled_errors {
            let source =
                error_planes.ok_or_else(|| DemSamplingExecutionError::InternalInvariant {
                    message: "DEM sample planes omitted requested sampled-error output".to_owned(),
                })?;
            let target = self.sampled_errors.as_mut().ok_or_else(|| {
                DemSamplingExecutionError::InternalInvariant {
                    message: "DEM batch omitted requested sampled-error storage".to_owned(),
                }
            })?;
            copy_plane_words(target, source, plane_word_index, shot_count)?;
        }
        Ok(())
    }
}

fn copy_plane_words(
    target: &mut PackedShotBatch,
    words: &[u64],
    plane_word_index: usize,
    shot_count: usize,
) -> Result<(), DemSamplingExecutionError> {
    if shot_count > target.shot_count() {
        return Err(DemSamplingExecutionError::InternalInvariant {
            message: "DEM plane chunk exceeds the reusable output batch".to_owned(),
        });
    }
    let width = target.bits_per_shot();
    let start = plane_word_index.checked_mul(width).ok_or_else(|| {
        DemSamplingExecutionError::InternalInvariant {
            message: "DEM plane chunk offset overflowed".to_owned(),
        }
    })?;
    let end =
        start
            .checked_add(width)
            .ok_or_else(|| DemSamplingExecutionError::InternalInvariant {
                message: "DEM plane chunk end overflowed".to_owned(),
            })?;
    let chunk =
        words
            .get(start..end)
            .ok_or_else(|| DemSamplingExecutionError::InternalInvariant {
                message: "DEM plane chunk escaped sampled storage".to_owned(),
            })?;
    let source = BitPlane64BatchView::try_from_words(chunk, target.shot_count(), width)
        .map_err(internal_format_error)?;
    target
        .copy_from_bit_planes(source)
        .map_err(internal_format_error)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SessionCapacities {
    pub(super) output: usize,
    pub(super) sample: usize,
}

pub(super) fn initial_capacities(
    plan: &DemSamplingPlan,
    limits: DemSamplerLimits,
) -> Result<SessionCapacities, DemSamplingExecutionError> {
    let include_sampled_errors = validate_session_storage(plan, 1, 1, true, limits).is_ok();
    for output in (1..=MAX_BATCH_SHOTS).rev() {
        if validate_session_storage(plan, output, 1, include_sampled_errors, limits).is_err() {
            continue;
        }
        if output < MAX_BATCH_SHOTS {
            return Ok(SessionCapacities {
                output,
                sample: output,
            });
        }
        for sample in (1..=INDEXED_BLOCK_SHOTS).rev() {
            if validate_session_storage(plan, output, sample, include_sampled_errors, limits)
                .is_ok()
            {
                return Ok(SessionCapacities { output, sample });
            }
        }
    }
    validate_session_storage(plan, 1, 1, false, limits)?;
    Err(DemSamplingExecutionError::InternalInvariant {
        message: "one-shot DEM session storage passed admission but no batch capacity was selected"
            .to_owned(),
    })
}

pub(super) fn validate_session_storage(
    plan: &DemSamplingPlan,
    output_shots: usize,
    sample_shots: usize,
    include_sampled_errors: bool,
    limits: DemSamplerLimits,
) -> Result<(), DemSamplingExecutionError> {
    let estimated_bytes =
        session_storage_bytes(plan, output_shots, sample_shots, include_sampled_errors);
    if estimated_bytes > limits.max_active_batch_bytes() as u128 {
        let actual = usize::try_from(estimated_bytes).map_err(|_| {
            DemSamplingExecutionError::InvalidRequest(DemError::invalid_sampler_compilation(
                "DEM sampling session active byte estimate overflowed usize",
            ))
        })?;
        return Err(DemSamplingExecutionError::InvalidRequest(
            DemResourceLimitError::active_batch_bytes(actual, limits.max_active_batch_bytes())
                .into(),
        ));
    }
    if estimated_bytes > u128::from(MAX_DEM_SESSION_STORAGE_BYTES) {
        return Err(DemSamplingExecutionError::SessionStorageLimit {
            estimated_bytes,
            limit_bytes: MAX_DEM_SESSION_STORAGE_BYTES,
        });
    }
    Ok(())
}

fn session_storage_bytes(
    plan: &DemSamplingPlan,
    output_shots: usize,
    sample_shots: usize,
    include_sampled_errors: bool,
) -> u128 {
    let detector_width = plan.detector_count() as u128;
    let observable_width = plan.observable_count() as u128;
    let sampled_error_width = if include_sampled_errors {
        plan.error_count() as u128
    } else {
        0
    };
    let plane_words = sample_shots.div_ceil(u64::BITS as usize) as u128;
    let scratch = (std::mem::size_of::<DetectionRecordBuffer>() as u128)
        .saturating_add(detector_width)
        .saturating_add(observable_width)
        .saturating_add(
            detector_width
                .saturating_add(observable_width)
                .saturating_mul(plane_words)
                .saturating_mul(std::mem::size_of::<u64>() as u128),
        )
        .saturating_add(if include_sampled_errors {
            sampled_error_width
                .saturating_mul(plane_words)
                .saturating_mul(std::mem::size_of::<u64>() as u128)
        } else {
            0
        });
    let packed_rows = packed_row_bytes(plan.detector_count())
        .saturating_add(packed_row_bytes(plan.observable_count()))
        .saturating_add(if include_sampled_errors {
            packed_row_bytes(plan.error_count())
        } else {
            0
        });
    scratch.saturating_add(packed_rows.saturating_mul(output_shots as u128))
}

fn packed_row_bytes(width: usize) -> u128 {
    (width.div_ceil(u64::BITS as usize) as u128).saturating_mul(std::mem::size_of::<u64>() as u128)
}

fn storage_format_error(source: stab_records::FormatError) -> DemSamplingExecutionError {
    DemSamplingExecutionError::SessionStorageAllocation {
        message: source.to_string(),
    }
}
