use std::fmt;
use std::sync::Arc;

use stab_model::DetectorErrorModel;
use stab_model::advanced::{FoldedDemTraversal, MAX_DEM_REPEAT_NESTING};
use stab_records::{DetectorWidth, ObservableWidth, SampledErrorWidth};

use super::bit_plane::{SampledErrorPlanes, sample_into_planes};
use super::buffers::try_false_vec;
use super::program::{
    DemSampleBlock, apply_error_record_block, compile_block, reset_detection_record, usize_from_u64,
};
use super::session::{DemReplaySession, DemSamplingExecutionError, DemSamplingSession};
use super::{DemError, DemResourceLimitError, DemResult, DemSamplerLimits};
use crate::{DetectionRecordBuffer, RandomPolicy, ShotCount};

/// Compiler for immutable detector-error-model sampling plans.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DemSamplingCompiler;

impl DemSamplingCompiler {
    pub const fn new() -> Self {
        Self
    }

    pub fn compile(self, model: &DetectorErrorModel) -> DemResult<DemSamplingPlan> {
        let traversal = FoldedDemTraversal::new(model)?;
        let repeat_depth = traversal.root().summary().max_repeat_depth();
        if repeat_depth > MAX_DEM_REPEAT_NESTING {
            return Err(DemError::invalid_sampler_compilation(format!(
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

    /// Creates owned mutable state for one incremental sampled-error replay.
    pub fn replay_session(
        &self,
        expected_shots: ShotCount,
    ) -> Result<DemReplaySession, DemSamplingExecutionError> {
        self.replay_session_with_limits(expected_shots, DemSamplerLimits::default())
    }

    /// Creates an owned replay session under explicit caller-selected resource limits.
    pub fn replay_session_with_limits(
        &self,
        expected_shots: ShotCount,
        limits: DemSamplerLimits,
    ) -> Result<DemReplaySession, DemSamplingExecutionError> {
        DemReplaySession::new(self.clone(), expected_shots, limits)
    }

    /// Validates replay traversal work without constructing a session or touching a sink.
    pub fn validate_replay(&self, shots: ShotCount) -> DemResult<()> {
        self.validate_replay_with_limits(shots, DemSamplerLimits::default())
    }

    /// Validates replay traversal work against explicit caller-owned limits.
    pub fn validate_replay_with_limits(
        &self,
        shots: ShotCount,
        limits: DemSamplerLimits,
    ) -> DemResult<()> {
        let shots = usize::try_from(shots.get()).map_err(|_| {
            DemError::invalid_sampler_compilation(
                "DEM sampler replay shot count does not fit in usize",
            )
        })?;
        self.validate_replay_work_units_with_limits(shots, limits)
    }

    pub fn validate_replay_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> DemResult<()> {
        let units_per_shot = self.replay_work_units_per_shot()?;
        let total_units = shots.checked_mul(units_per_shot).ok_or_else(|| {
            DemError::invalid_sampler_compilation("DEM sampler replay work overflowed")
        })?;
        if total_units > limits.max_replay_work_units() {
            return Err(DemResourceLimitError::replay_work_units(
                total_units,
                limits.max_replay_work_units(),
            )
            .into());
        }
        Ok(())
    }

    pub fn replay_work_units_per_shot(&self) -> DemResult<usize> {
        self.detector_count()
            .checked_add(self.observable_count())
            .and_then(|width| width.checked_add(self.error_count()))
            .map(|width| width.max(1))
            .ok_or_else(|| {
                DemError::invalid_sampler_compilation(
                    "DEM sampler replay work overflowed while validating buffer size",
                )
            })
    }

    pub fn validate_detector_sample_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> DemResult<()> {
        self.validate_sample_work_units(
            shots,
            self.inner.operations.direct_sample_work_count,
            limits,
        )
    }

    pub fn validate_sampled_error_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> DemResult<()> {
        self.validate_sample_work_units(shots, self.error_count(), limits)
    }

    pub(super) fn try_reusable_detection_record(&self) -> DemResult<DetectionRecordBuffer> {
        Ok(DetectionRecordBuffer {
            detectors: try_false_vec(self.detector_count(), "DEM detector record")?,
            observables: try_false_vec(self.observable_count(), "DEM observable record")?,
        })
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the deterministic shot range and reusable output planes are independent inputs"
    )]
    pub(super) fn sample_detection_planes_into(
        &self,
        seed: u64,
        first_shot: u64,
        shot_count: usize,
        detector_planes: &mut Vec<u64>,
        observable_planes: &mut Vec<u64>,
    ) -> DemResult<()> {
        sample_into_planes(
            &self.inner.operations,
            self.detector_count(),
            self.observable_count(),
            seed,
            first_shot,
            shot_count,
            detector_planes,
            observable_planes,
            SampledErrorPlanes::Discard,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the deterministic shot range and three reusable output planes are independent inputs"
    )]
    pub(super) fn sample_detection_and_error_planes_into(
        &self,
        seed: u64,
        first_shot: u64,
        shot_count: usize,
        detector_planes: &mut Vec<u64>,
        observable_planes: &mut Vec<u64>,
        error_planes: &mut Vec<u64>,
    ) -> DemResult<()> {
        sample_into_planes(
            &self.inner.operations,
            self.detector_count(),
            self.observable_count(),
            seed,
            first_shot,
            shot_count,
            detector_planes,
            observable_planes,
            SampledErrorPlanes::Record(error_planes),
        )
    }

    pub(super) fn validate_error_record_width(
        &self,
        error_record: &[bool],
        shot_index: Option<usize>,
    ) -> DemResult<()> {
        if error_record.len() == self.error_count() {
            return Ok(());
        }
        if let Some(shot_index) = shot_index {
            return Err(DemError::invalid_result_format(format!(
                "DEM error record {shot_index} expected {} bits, got {}",
                self.error_count(),
                error_record.len()
            )));
        }
        Err(DemError::invalid_result_format(format!(
            "DEM error record expected {} bits, got {}",
            self.error_count(),
            error_record.len()
        )))
    }

    pub(super) fn detection_record_from_error_record_into(
        &self,
        error_record: &[bool],
        record: &mut DetectionRecordBuffer,
    ) -> DemResult<()> {
        self.validate_error_record_width(error_record, None)?;
        reset_detection_record(record, self.detector_count(), self.observable_count());
        let mut cursor = 0;
        apply_error_record_block(&self.inner.operations, 0, error_record, &mut cursor, record)?;
        if cursor != error_record.len() {
            return Err(DemError::invalid_result_format(
                "DEM error record had unused trailing bits",
            ));
        }
        Ok(())
    }

    fn validate_sample_work_units(
        &self,
        shots: usize,
        error_applications_per_shot: usize,
        limits: DemSamplerLimits,
    ) -> DemResult<()> {
        if error_applications_per_shot == 0 || shots == 0 {
            return Ok(());
        }
        let work_units = shots
            .checked_mul(error_applications_per_shot)
            .ok_or_else(|| {
                DemError::invalid_sampler_compilation("DEM sampler sample work overflowed")
            })?;
        if work_units > limits.max_sampled_error_applications() {
            return Err(DemResourceLimitError::sampled_error_applications(
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
