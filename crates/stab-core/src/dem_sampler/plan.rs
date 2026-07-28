use std::fmt;
use std::sync::Arc;

use rand::Rng;

use super::buffers::{try_false_vec, try_vec_with_capacity, validate_vector_capacity};
use super::{
    DemSampleBlock, SampledErrorOutput, apply_error_record_block, compile_block,
    reset_detection_record, sample_block_into, usize_from_u64,
};
use crate::{
    CircuitError, CircuitResult, DetectionEventRecord, DetectorErrorModel, DetectorWidth,
    ObservableWidth, RandomPolicy, ResourceLimitError, SampledErrorWidth, ShotCount,
    dem::{FoldedDemTraversal, MAX_DEM_REPEAT_NESTING},
};

use super::limits::DemSamplerLimits;
use super::session::{DemSamplingExecutionError, DemSamplingSession};

/// Compiler for immutable detector-error-model sampling plans.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DemSamplingCompiler;

impl DemSamplingCompiler {
    pub const fn new() -> Self {
        Self
    }

    pub fn compile(self, model: &DetectorErrorModel) -> CircuitResult<DemSamplingPlan> {
        let traversal = FoldedDemTraversal::new(model)?;
        let repeat_depth = traversal.root().summary().max_repeat_depth();
        if repeat_depth > MAX_DEM_REPEAT_NESTING {
            return Err(CircuitError::invalid_sampler_compilation(format!(
                "DEM repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}, got {repeat_depth}"
            )));
        }
        let operations = compile_block(traversal.root())?;
        let detector_count = usize_from_u64(
            traversal.root().summary().detector_count()?,
            "detector count",
        )?;
        let observable_count = usize_from_u64(
            traversal.root().summary().observable_count(),
            "observable count",
        )?;
        Ok(DemSamplingPlan {
            inner: Arc::new(DemSamplingPlanInner {
                detector_count,
                observable_count,
                operations,
            }),
        })
    }
}

/// Immutable, shareable lowered detector-error-model sampling program.
#[derive(Clone, PartialEq)]
pub struct DemSamplingPlan {
    pub(super) inner: Arc<DemSamplingPlanInner>,
}

impl fmt::Debug for DemSamplingPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DemSamplingPlan")
            .field("detector_width", &self.detector_width())
            .field("observable_width", &self.observable_width())
            .field("sampled_error_width", &self.sampled_error_width())
            .finish_non_exhaustive()
    }
}

impl DemSamplingPlan {
    pub fn detector_width(&self) -> DetectorWidth {
        DetectorWidth::new(self.detector_count())
    }

    pub fn observable_width(&self) -> ObservableWidth {
        ObservableWidth::new(self.observable_count())
    }

    pub fn sampled_error_width(&self) -> SampledErrorWidth {
        SampledErrorWidth::new(self.error_count())
    }

    pub fn detector_count(&self) -> usize {
        self.inner.detector_count
    }

    pub fn observable_count(&self) -> usize {
        self.inner.observable_count
    }

    pub fn error_count(&self) -> usize {
        self.inner.operations.error_count
    }

    pub fn session(
        &self,
        random_policy: RandomPolicy,
    ) -> Result<DemSamplingSession, DemSamplingExecutionError> {
        self.session_with_limits(random_policy, DemSamplerLimits::default())
    }

    pub fn session_with_limits(
        &self,
        random_policy: RandomPolicy,
        limits: DemSamplerLimits,
    ) -> Result<DemSamplingSession, DemSamplingExecutionError> {
        DemSamplingSession::new(self.clone(), random_policy, limits)
    }

    /// Validates replay traversal work without constructing a session or touching a sink.
    pub fn validate_replay(&self, shots: ShotCount) -> CircuitResult<()> {
        self.validate_replay_with_limits(shots, DemSamplerLimits::default())
    }

    /// Validates replay traversal work against explicit caller-owned limits.
    pub fn validate_replay_with_limits(
        &self,
        shots: ShotCount,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        let shots = usize::try_from(shots.get()).map_err(|_| {
            CircuitError::invalid_sampler_compilation(
                "DEM sampler replay shot count does not fit in usize",
            )
        })?;
        self.validate_replay_work_units_with_limits(shots, limits)
    }

    pub(super) fn validate_sample_buffer_units_with_limits(
        &self,
        shots: usize,
        include_error_records: bool,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        let mut units_per_shot = self
            .detector_count()
            .checked_add(self.observable_count())
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler output width overflowed while validating buffer size",
                )
            })?;
        if include_error_records {
            units_per_shot = units_per_shot
                .checked_add(self.error_count())
                .ok_or_else(|| {
                    CircuitError::invalid_sampler_compilation(
                        "DEM sampler output and error width overflowed while validating buffer size",
                    )
                })?;
        }
        let units_per_shot = units_per_shot.max(1);
        let total_units = shots.checked_mul(units_per_shot).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation("DEM sampler buffer size overflowed")
        })?;
        if total_units > limits.max_materialized_units() {
            return Err(ResourceLimitError::dem_materialized_units(
                total_units,
                limits.max_materialized_units(),
            )
            .into());
        }
        self.validate_materialized_bytes_with_limits(shots, include_error_records, limits)?;
        validate_vector_capacity::<DetectionEventRecord>(shots, "DEM detection record container")?;
        validate_vector_capacity::<bool>(self.detector_count(), "DEM detector record")?;
        validate_vector_capacity::<bool>(self.observable_count(), "DEM observable record")?;
        if include_error_records {
            validate_vector_capacity::<Vec<bool>>(shots, "DEM sampled-error record container")?;
            validate_vector_capacity::<bool>(self.error_count(), "DEM sampled-error record")?;
        }
        Ok(())
    }

    pub(super) fn validate_materialized_bytes_with_limits(
        &self,
        shots: usize,
        include_error_records: bool,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        let bytes_per_shot = self.materialized_bytes_per_shot(include_error_records)?;
        let total_bytes = shots.checked_mul(bytes_per_shot).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation("DEM sampler buffer byte size overflowed")
        })?;
        if total_bytes > limits.max_materialized_bytes() {
            return Err(ResourceLimitError::dem_materialized_bytes(
                total_bytes,
                limits.max_materialized_bytes(),
            )
            .into());
        }
        Ok(())
    }

    pub(super) fn validate_replay_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        let units_per_shot = self.replay_work_units_per_shot()?;
        let total_units = shots.checked_mul(units_per_shot).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation("DEM sampler replay work overflowed")
        })?;
        if total_units > limits.max_replay_work_units() {
            return Err(ResourceLimitError::dem_replay_work_units(
                total_units,
                limits.max_replay_work_units(),
            )
            .into());
        }
        Ok(())
    }

    pub(super) fn replay_work_units_per_shot(&self) -> CircuitResult<usize> {
        self.detector_count()
            .checked_add(self.observable_count())
            .and_then(|width| width.checked_add(self.error_count()))
            .map(|width| width.max(1))
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler replay work overflowed while validating buffer size",
                )
            })
    }

    pub(super) fn validate_detector_sample_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.validate_sample_work_units(
            shots,
            self.inner.operations.direct_sample_work_count,
            limits,
        )
    }

    pub(super) fn validate_sampled_error_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.validate_sample_work_units(shots, self.error_count(), limits)
    }

    pub(super) fn try_reusable_detection_record(&self) -> CircuitResult<DetectionEventRecord> {
        Ok(DetectionEventRecord {
            detectors: try_false_vec(self.detector_count(), "DEM detector record")?,
            observables: try_false_vec(self.observable_count(), "DEM observable record")?,
        })
    }

    pub(super) fn try_reusable_error_record(&self) -> CircuitResult<Vec<bool>> {
        try_vec_with_capacity(self.error_count(), "DEM sampled-error record")
    }

    pub(super) fn limits_after_compatibility_sink(
        &self,
        include_error_records: bool,
        limits: DemSamplerLimits,
    ) -> CircuitResult<DemSamplerLimits> {
        let reserved_bytes = self.materialized_bytes_per_shot(include_error_records)?;
        if reserved_bytes > limits.max_materialized_bytes() {
            return Err(ResourceLimitError::dem_materialized_bytes(
                reserved_bytes,
                limits.max_materialized_bytes(),
            )
            .into());
        }
        Ok(limits.with_max_materialized_bytes(limits.max_materialized_bytes() - reserved_bytes))
    }

    pub(super) fn sample_detection_record_into<R>(
        &self,
        rng: &mut R,
        record: &mut DetectionEventRecord,
    ) -> CircuitResult<()>
    where
        R: Rng,
    {
        reset_detection_record(record, self.detector_count(), self.observable_count());
        sample_block_into(
            &self.inner.operations,
            0,
            rng,
            record,
            SampledErrorOutput::Discard,
        )
    }

    pub(super) fn sample_detection_record_and_error_record_into<R>(
        &self,
        rng: &mut R,
        record: &mut DetectionEventRecord,
        error_record: &mut Vec<bool>,
    ) -> CircuitResult<()>
    where
        R: Rng,
    {
        reset_detection_record(record, self.detector_count(), self.observable_count());
        error_record.clear();
        sample_block_into(
            &self.inner.operations,
            0,
            rng,
            record,
            SampledErrorOutput::Record(error_record),
        )
    }

    pub(super) fn validate_error_record_width(
        &self,
        error_record: &[bool],
        shot_index: Option<usize>,
    ) -> CircuitResult<()> {
        if error_record.len() == self.error_count() {
            return Ok(());
        }
        if let Some(shot_index) = shot_index {
            return Err(CircuitError::invalid_result_format(format!(
                "DEM error record {shot_index} expected {} bits, got {}",
                self.error_count(),
                error_record.len()
            )));
        }
        Err(CircuitError::invalid_result_format(format!(
            "DEM error record expected {} bits, got {}",
            self.error_count(),
            error_record.len()
        )))
    }

    pub(super) fn detection_record_from_error_record_into(
        &self,
        error_record: &[bool],
        record: &mut DetectionEventRecord,
    ) -> CircuitResult<()> {
        self.validate_error_record_width(error_record, None)?;
        reset_detection_record(record, self.detector_count(), self.observable_count());
        let mut cursor = 0;
        apply_error_record_block(&self.inner.operations, 0, error_record, &mut cursor, record)?;
        if cursor != error_record.len() {
            return Err(CircuitError::invalid_result_format(
                "DEM error record had unused trailing bits",
            ));
        }
        Ok(())
    }

    fn materialized_bytes_per_shot(&self, include_error_records: bool) -> CircuitResult<usize> {
        let detector_observable_bytes = self
            .detector_count()
            .checked_add(self.observable_count())
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler output width overflowed while validating buffer bytes",
                )
            })?;
        let mut bytes = std::mem::size_of::<DetectionEventRecord>()
            .checked_add(detector_observable_bytes)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler per-shot output byte size overflowed",
                )
            })?;
        if include_error_records {
            bytes = bytes
                .checked_add(std::mem::size_of::<Vec<bool>>())
                .and_then(|value| value.checked_add(self.error_count()))
                .ok_or_else(|| {
                    CircuitError::invalid_sampler_compilation(
                        "DEM sampler per-shot error byte size overflowed",
                    )
                })?;
        }
        Ok(bytes.max(1))
    }

    fn validate_sample_work_units(
        &self,
        shots: usize,
        error_applications_per_shot: usize,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        if error_applications_per_shot == 0 || shots == 0 {
            return Ok(());
        }
        let work_units = shots
            .checked_mul(error_applications_per_shot)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation("DEM sampler sample work overflowed")
            })?;
        if work_units > limits.max_sampled_error_applications() {
            return Err(ResourceLimitError::dem_sampled_error_applications(
                work_units,
                limits.max_sampled_error_applications(),
            )
            .into());
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct DemSamplingPlanInner {
    pub(super) detector_count: usize,
    pub(super) observable_count: usize,
    pub(super) operations: DemSampleBlock,
}
