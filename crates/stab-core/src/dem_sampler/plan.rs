use std::fmt;

use crate::{
    CircuitResult, DetectionEventRecord, DetectorErrorModel, DetectorWidth, ObservableWidth,
    RandomPolicy, SampledErrorWidth, ShotCount,
};

use super::{DemSamplerLimits, DemSamplingExecutionError, DemSamplingSession};

/// Compiler for immutable detector-error-model sampling plans.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DemSamplingCompiler;

impl DemSamplingCompiler {
    pub const fn new() -> Self {
        Self
    }

    pub fn compile(self, model: &DetectorErrorModel) -> CircuitResult<DemSamplingPlan> {
        stab_engine::DemSamplingCompiler::new()
            .compile(model)
            .map(DemSamplingPlan::from_engine)
            .map_err(Into::into)
    }
}

/// Compatibility wrapper over the engine-owned DEM sampling plan.
#[derive(Clone, PartialEq)]
pub struct DemSamplingPlan {
    inner: stab_engine::DemSamplingPlan,
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
    pub(super) const fn from_engine(inner: stab_engine::DemSamplingPlan) -> Self {
        Self { inner }
    }

    pub fn detector_width(&self) -> DetectorWidth {
        self.inner.detector_width()
    }

    pub fn observable_width(&self) -> ObservableWidth {
        self.inner.observable_width()
    }

    pub fn sampled_error_width(&self) -> SampledErrorWidth {
        self.inner.sampled_error_width()
    }

    pub fn detector_count(&self) -> usize {
        self.inner.detector_count()
    }

    pub fn observable_count(&self) -> usize {
        self.inner.observable_count()
    }

    pub fn error_count(&self) -> usize {
        self.inner.error_count()
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
        self.inner
            .session_with_limits(random_policy, limits)
            .map(DemSamplingSession::from_engine)
            .map_err(DemSamplingExecutionError::from_engine)
    }

    pub fn validate_replay(&self, shots: ShotCount) -> CircuitResult<()> {
        self.inner.validate_replay(shots).map_err(Into::into)
    }

    pub fn validate_replay_with_limits(
        &self,
        shots: ShotCount,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.inner
            .validate_replay_with_limits(shots, limits)
            .map_err(Into::into)
    }

    pub(super) fn validate_sample_buffer_units_with_limits(
        &self,
        shots: usize,
        include_error_records: bool,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.inner
            .validate_sample_buffer_units_with_limits(shots, include_error_records, limits)
            .map_err(Into::into)
    }

    pub(super) fn validate_materialized_bytes_with_limits(
        &self,
        shots: usize,
        include_error_records: bool,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.inner
            .validate_materialized_bytes_with_limits(shots, include_error_records, limits)
            .map_err(Into::into)
    }

    pub(super) fn validate_replay_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.inner
            .validate_replay_work_units_with_limits(shots, limits)
            .map_err(Into::into)
    }

    pub(super) fn replay_work_units_per_shot(&self) -> CircuitResult<usize> {
        self.inner.replay_work_units_per_shot().map_err(Into::into)
    }

    pub(super) fn validate_detector_sample_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.inner
            .validate_detector_sample_work_units_with_limits(shots, limits)
            .map_err(Into::into)
    }

    pub(super) fn validate_sampled_error_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.inner
            .validate_sampled_error_work_units_with_limits(shots, limits)
            .map_err(Into::into)
    }

    pub(super) fn try_reusable_detection_record(&self) -> CircuitResult<DetectionEventRecord> {
        self.inner
            .try_reusable_detection_record()
            .map_err(Into::into)
    }

    pub(super) fn try_reusable_error_record(&self) -> CircuitResult<Vec<bool>> {
        self.inner.try_reusable_error_record().map_err(Into::into)
    }

    pub(super) fn limits_after_compatibility_sink(
        &self,
        include_error_records: bool,
        limits: DemSamplerLimits,
    ) -> CircuitResult<DemSamplerLimits> {
        let reserved_bytes = self
            .inner
            .materialized_bytes_per_shot(include_error_records)
            .map_err(crate::CircuitError::from)?;
        if reserved_bytes > limits.max_materialized_bytes() {
            return Err(crate::ResourceLimitError::dem_materialized_bytes(
                reserved_bytes,
                limits.max_materialized_bytes(),
            )
            .into());
        }
        Ok(limits.with_max_materialized_bytes(limits.max_materialized_bytes() - reserved_bytes))
    }

    pub(super) fn validate_error_record_width(
        &self,
        error_record: &[bool],
        shot_index: Option<usize>,
    ) -> CircuitResult<()> {
        self.inner
            .validate_error_record_width(error_record, shot_index)
            .map_err(Into::into)
    }

    pub(super) fn detection_record_from_error_record_into(
        &self,
        error_record: &[bool],
        record: &mut DetectionEventRecord,
    ) -> CircuitResult<()> {
        self.inner
            .detection_record_from_error_record_into(error_record, record)
            .map_err(Into::into)
    }
}
