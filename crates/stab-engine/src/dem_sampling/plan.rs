use std::fmt;
use std::sync::Arc;

use stab_model::advanced::{FoldedDemTraversal, MAX_DEM_REPEAT_NESTING};
use stab_model::{Cancellation, DetectorErrorModel, MemoryBudget};
use stab_records::{DetectorWidth, ObservableWidth, SampledErrorWidth};

use super::bit_plane::{SampledErrorPlanes, sample_into_planes};
use super::buffers::try_false_vec;
use super::program::{DemCompileMeter, DemPoll, DemSampleBlock, compile_block, usize_from_u64};
use super::replay::{ErrorRecordBits, apply_error_record, reset_detection_record};
use super::session::{DemReplaySession, DemSamplingExecutionError, DemSamplingSession};
use super::{DemError, DemResult, DemSamplerLimits};
use crate::{DetectionRecordBuffer, RandomPolicy, ShotCount};

/// Compiler for immutable detector-error-model sampling plans.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DemSamplingCompiler {
    limits: DemSamplerLimits,
}

impl DemSamplingCompiler {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn limits(mut self, limits: DemSamplerLimits) -> Self {
        self.limits = limits;
        self
    }

    #[must_use]
    pub fn memory_budget(mut self, memory_budget: MemoryBudget) -> Self {
        self.limits = self.limits.with_memory_budget(memory_budget);
        self
    }

    #[must_use]
    pub fn cancellation(mut self, cancellation: Cancellation) -> Self {
        self.limits = self.limits.with_cancellation(cancellation);
        self
    }

    pub fn compile(&self, model: &DetectorErrorModel) -> DemResult<DemSamplingPlan> {
        self.limits.cancellation().check()?;
        let traversal = FoldedDemTraversal::new(model)?;
        let repeat_depth = traversal.root().summary().max_repeat_depth();
        if repeat_depth > MAX_DEM_REPEAT_NESTING {
            return Err(DemError::invalid_sampler_compilation(format!(
                "DEM repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}, got {repeat_depth}"
            )));
        }
        let mut meter =
            DemCompileMeter::new(self.limits.memory_budget(), self.limits.cancellation());
        let operations = compile_block(traversal.root(), &mut meter)?;
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
                retained_bytes: meter.retained_bytes(),
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

    /// Conservative heap bytes retained by this compiled plan.
    pub fn retained_bytes(&self) -> u64 {
        self.inner.retained_bytes
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

    /// Validates that a replay of `shots` shots is representable, without constructing a
    /// session or touching a sink. Replay work is not capped.
    pub fn validate_replay(&self, shots: ShotCount) -> DemResult<()> {
        usize::try_from(shots.get()).map_err(|_| {
            DemError::invalid_sampler_compilation(
                "DEM sampler replay shot count does not fit in usize",
            )
        })?;
        self.replay_work_units_per_shot().map(drop)
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
        cancellation: &Cancellation,
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
            &mut DemPoll::new(cancellation),
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
        cancellation: &Cancellation,
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
            &mut DemPoll::new(cancellation),
        )
    }

    pub(super) fn validate_error_record_width<B: ErrorRecordBits + ?Sized>(
        &self,
        error_record: &B,
        shot_index: Option<usize>,
    ) -> DemResult<()> {
        if error_record.bit_len() == self.error_count() {
            return Ok(());
        }
        if let Some(shot_index) = shot_index {
            return Err(DemError::invalid_result_format(format!(
                "DEM error record {shot_index} expected {} bits, got {}",
                self.error_count(),
                error_record.bit_len()
            )));
        }
        Err(DemError::invalid_result_format(format!(
            "DEM error record expected {} bits, got {}",
            self.error_count(),
            error_record.bit_len()
        )))
    }

    pub(super) fn detection_record_from_error_record_into<B: ErrorRecordBits + ?Sized>(
        &self,
        error_record: &B,
        record: &mut DetectionRecordBuffer,
        cancellation: &Cancellation,
    ) -> DemResult<()> {
        self.validate_error_record_width(error_record, None)?;
        reset_detection_record(record, self.detector_count(), self.observable_count());
        let consumed = apply_error_record(
            &self.inner.operations,
            error_record,
            record,
            &mut DemPoll::new(cancellation),
        )?;
        if consumed != error_record.bit_len() {
            return Err(DemError::invalid_result_format(
                "DEM error record had unused trailing bits",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct DemSamplingPlanInner {
    pub(super) detector_count: usize,
    pub(super) observable_count: usize,
    pub(super) operations: DemSampleBlock,
    pub(super) retained_bytes: u64,
}
