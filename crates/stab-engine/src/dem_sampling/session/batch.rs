use stab_records::{DemSampleBatchView, DetectionBatchView, PackedShotBatch};

use stab_model::MemoryBudget;

use super::{
    DemSamplingExecutionError, MAX_REPLAY_BATCH_SHOTS, ReplayRecord, internal_format_error,
};
use crate::DetectionRecordBuffer;
use crate::bernoulli::INDEXED_BLOCK_SHOTS;
use crate::dem_sampling::{DemResourceLimitError, DemSamplerLimits, DemSamplingPlan};

/// Active-batch size that batch selection aims for; larger budgets admit more but batch alike.
const PREFERRED_ACTIVE_BATCH_BYTES: u64 = 64 * 1024 * 1024;

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
        error_record: ReplayRecord<'_>,
    ) -> Result<(), DemSamplingExecutionError> {
        self.detectors
            .copy_shot_from_bools(shot_index, &record.detectors)
            .map_err(internal_format_error)?;
        self.observables
            .copy_shot_from_bools(shot_index, &record.observables)
            .map_err(internal_format_error)?;
        let sampled_errors = self.sampled_errors.as_mut().ok_or_else(|| {
            DemSamplingExecutionError::InternalInvariant {
                message: "DEM replay omitted its reusable sampled-error batch".to_owned(),
            }
        })?;
        match error_record {
            ReplayRecord::Dense(bits) => sampled_errors
                .copy_shot_from_bools(shot_index, bits)
                .map_err(internal_format_error),
            ReplayRecord::Packed(bits) => sampled_errors
                .copy_shot_from_bits(shot_index, bits)
                .map_err(internal_format_error),
            ReplayRecord::Sparse(sparse) => {
                let mut words = sampled_errors
                    .shot_words_mut(shot_index)
                    .map_err(internal_format_error)?;
                words.fill(0);
                for &hit in sparse.hits() {
                    let word = words.get_mut(hit / (u64::BITS as usize)).ok_or_else(|| {
                        DemSamplingExecutionError::InternalInvariant {
                            message: format!(
                                "sparse replay hit {hit} escaped its sampled-error row"
                            ),
                        }
                    })?;
                    *word |= 1_u64 << (hit % (u64::BITS as usize));
                }
                Ok(())
            }
        }
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
                    .map_err(internal_format_error)?
                    .into(),
            )
        } else {
            None
        };
        DemSampleBatchView::try_new(detection, sampled_errors).map_err(internal_format_error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SessionCapacities {
    pub(super) output: usize,
    pub(super) sample: usize,
}

pub(super) fn initial_capacities(
    plan: &DemSamplingPlan,
    limits: &DemSamplerLimits,
) -> Result<SessionCapacities, DemSamplingExecutionError> {
    // Batch shape follows the historical 64 MiB active-batch target, which keeps seeded streams
    // independent of the admission budget; only a smaller budget shrinks it further.
    let preferred = limits
        .memory_budget()
        .min(MemoryBudget::from_bytes(PREFERRED_ACTIVE_BATCH_BYTES));
    let fits = |output, sample, include_sampled_errors| {
        preferred.admits(session_storage_bytes(
            plan,
            output,
            sample,
            include_sampled_errors,
        ))
    };
    let include_sampled_errors = fits(1, 1, true);
    for output in (1..=MAX_REPLAY_BATCH_SHOTS).rev() {
        if !fits(output, 1, include_sampled_errors) {
            continue;
        }
        if output < MAX_REPLAY_BATCH_SHOTS {
            return Ok(SessionCapacities {
                output,
                sample: output,
            });
        }
        for sample in (1..=INDEXED_BLOCK_SHOTS).rev() {
            if fits(output, sample, include_sampled_errors) {
                return Ok(SessionCapacities { output, sample });
            }
        }
    }
    // Even one shot exceeds the preferred batch size; admit a one-shot batch within the budget.
    validate_session_storage(plan, 1, 1, false, limits)?;
    Ok(SessionCapacities {
        output: 1,
        sample: 1,
    })
}

pub(super) fn validate_session_storage(
    plan: &DemSamplingPlan,
    output_shots: usize,
    sample_shots: usize,
    include_sampled_errors: bool,
    limits: &DemSamplerLimits,
) -> Result<(), DemSamplingExecutionError> {
    let estimated_bytes =
        session_storage_bytes(plan, output_shots, sample_shots, include_sampled_errors);
    let budget = limits.memory_budget();
    if !budget.admits(estimated_bytes) {
        let actual = u64::try_from(estimated_bytes).unwrap_or(u64::MAX);
        return Err(DemSamplingExecutionError::InvalidRequest(
            DemResourceLimitError::active_batch_bytes(actual, budget.bytes()).into(),
        ));
    }
    Ok(())
}

pub(super) fn session_storage_bytes(
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
