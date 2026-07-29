use std::fmt;

use stab_records::{
    DetectionSink, DetectorWidth, MeasurementBatchView, MeasurementWidth, ObservableWidth,
};

use super::DetectionConversionLimits;
use crate::{Circuit, RandomPolicy, ReferenceSampleMode, SamplingCancellation, ShotCount};

mod compat;
mod contracts;
mod delivery;

pub(super) use compat::{sample_materialized, try_for_each};
pub use contracts::{
    DetectionCompileError, DetectionExecutionError, DetectionRunError, DetectionRunProgress,
    DetectionRunStatus, DetectionRunSummary,
};
pub use delivery::MeasurementToDetectionSinkAdapter;

/// Builder for immutable measurement-to-detection conversion plans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeasurementToDetectionCompiler {
    inner: stab_engine::MeasurementToDetectionCompiler,
}

impl MeasurementToDetectionCompiler {
    pub fn new() -> Self {
        Self {
            inner: stab_engine::MeasurementToDetectionCompiler::new(),
        }
    }

    #[must_use]
    pub const fn limits(mut self, limits: DetectionConversionLimits) -> Self {
        self.inner = self.inner.limits(limits);
        self
    }

    #[must_use]
    pub const fn reference_sample_mode(mut self, mode: ReferenceSampleMode) -> Self {
        self.inner = self.inner.reference_sample_mode(mode);
        self
    }

    pub fn compile(
        self,
        circuit: &Circuit,
    ) -> Result<MeasurementToDetectionPlan, DetectionCompileError> {
        self.inner
            .compile(circuit)
            .map(MeasurementToDetectionPlan::from_engine)
            .map_err(DetectionCompileError::from_engine)
    }
}

impl Default for MeasurementToDetectionCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable, shareable measurement-to-detection conversion plan.
#[derive(Clone)]
pub struct MeasurementToDetectionPlan {
    inner: stab_engine::MeasurementToDetectionPlan,
}

impl fmt::Debug for MeasurementToDetectionPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MeasurementToDetectionPlan")
            .field("measurement_width", &self.measurement_width())
            .field("sweep_width", &self.sweep_width())
            .field("detector_width", &self.detector_width())
            .field("observable_width", &self.observable_width())
            .finish_non_exhaustive()
    }
}

impl MeasurementToDetectionPlan {
    const fn from_engine(inner: stab_engine::MeasurementToDetectionPlan) -> Self {
        Self { inner }
    }

    pub fn measurement_width(&self) -> MeasurementWidth {
        self.inner.measurement_width()
    }

    pub fn sweep_width(&self) -> MeasurementWidth {
        self.inner.sweep_width()
    }

    pub fn detector_width(&self) -> DetectorWidth {
        self.inner.detector_width()
    }

    pub fn observable_width(&self) -> ObservableWidth {
        self.inner.observable_width()
    }

    pub fn session(&self) -> Result<MeasurementToDetectionSession, DetectionExecutionError> {
        self.inner
            .session()
            .map(MeasurementToDetectionSession::from_engine)
            .map_err(DetectionExecutionError::from_engine)
    }
}

/// Mutable reusable state for measurement-to-detection conversion.
pub struct MeasurementToDetectionSession {
    inner: stab_engine::MeasurementToDetectionSession,
}

impl fmt::Debug for MeasurementToDetectionSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MeasurementToDetectionSession")
            .field("inner", &self.inner)
            .field("total_committed_shots", &self.total_committed_shots())
            .field("poisoned", &self.is_poisoned())
            .finish_non_exhaustive()
    }
}

impl MeasurementToDetectionSession {
    const fn from_engine(inner: stab_engine::MeasurementToDetectionSession) -> Self {
        Self { inner }
    }

    pub fn cancellation(&self) -> SamplingCancellation {
        self.inner.cancellation()
    }

    pub fn is_poisoned(&self) -> bool {
        self.inner.is_poisoned()
    }

    pub const fn total_committed_shots(&self) -> ShotCount {
        self.inner.total_committed_shots()
    }

    pub fn start_delivery<'session, 'sink, Sink>(
        &'session mut self,
        sink: &'sink mut Sink,
    ) -> Result<MeasurementToDetectionSinkAdapter<'session, 'sink, Sink>, DetectionExecutionError>
    where
        Sink: DetectionSink,
    {
        self.inner
            .start_delivery(sink)
            .map(MeasurementToDetectionSinkAdapter::from_engine)
            .map_err(DetectionExecutionError::from_engine)
    }

    pub fn run<Sink>(
        &mut self,
        measurements: MeasurementBatchView<'_>,
        sweeps: Option<MeasurementBatchView<'_>>,
        sink: &mut Sink,
    ) -> Result<DetectionRunSummary, DetectionRunError<Sink::Error>>
    where
        Sink: DetectionSink,
    {
        self.inner
            .run(measurements, sweeps, sink)
            .map(summary_from_engine)
            .map_err(run_error_from_engine)
    }
}

/// Builder for immutable circuit detection-sampling plans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetectionSamplingCompiler {
    inner: stab_engine::DetectionSamplingCompiler,
}

impl DetectionSamplingCompiler {
    pub fn new() -> Self {
        Self {
            inner: stab_engine::DetectionSamplingCompiler::new(),
        }
    }

    #[must_use]
    pub const fn limits(mut self, limits: DetectionConversionLimits) -> Self {
        self.inner = self.inner.limits(limits);
        self
    }

    pub fn compile(
        self,
        circuit: &Circuit,
    ) -> Result<DetectionSamplingPlan, DetectionCompileError> {
        self.inner
            .compile(circuit)
            .map(DetectionSamplingPlan::from_engine)
            .map_err(DetectionCompileError::from_engine)
    }
}

impl Default for DetectionSamplingCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable, shareable circuit detection-sampling plan.
#[derive(Clone)]
pub struct DetectionSamplingPlan {
    inner: stab_engine::DetectionSamplingPlan,
}

impl fmt::Debug for DetectionSamplingPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DetectionSamplingPlan")
            .field("measurement_width", &self.measurement_width())
            .field("detector_width", &self.detector_width())
            .field("observable_width", &self.observable_width())
            .finish_non_exhaustive()
    }
}

impl DetectionSamplingPlan {
    const fn from_engine(inner: stab_engine::DetectionSamplingPlan) -> Self {
        Self { inner }
    }

    pub fn measurement_width(&self) -> MeasurementWidth {
        self.inner.measurement_width()
    }

    pub fn detector_width(&self) -> DetectorWidth {
        self.inner.detector_width()
    }

    pub fn observable_width(&self) -> ObservableWidth {
        self.inner.observable_width()
    }

    pub fn session(
        &self,
        random_policy: RandomPolicy,
    ) -> Result<DetectionSamplingSession, DetectionExecutionError> {
        self.inner
            .session(random_policy)
            .map(DetectionSamplingSession::from_engine)
            .map_err(DetectionExecutionError::from_engine)
    }
}

/// Mutable reusable state for circuit detection sampling.
pub struct DetectionSamplingSession {
    inner: stab_engine::DetectionSamplingSession,
}

impl fmt::Debug for DetectionSamplingSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DetectionSamplingSession")
            .field("total_committed_shots", &self.total_committed_shots())
            .field("poisoned", &self.is_poisoned())
            .finish_non_exhaustive()
    }
}

impl DetectionSamplingSession {
    const fn from_engine(inner: stab_engine::DetectionSamplingSession) -> Self {
        Self { inner }
    }

    pub fn cancellation(&self) -> SamplingCancellation {
        self.inner.cancellation()
    }

    pub fn is_poisoned(&self) -> bool {
        self.inner.is_poisoned()
    }

    pub const fn total_committed_shots(&self) -> ShotCount {
        self.inner.total_committed_shots()
    }

    pub fn run<Sink>(
        &mut self,
        shots: ShotCount,
        sink: &mut Sink,
    ) -> Result<DetectionRunSummary, DetectionRunError<Sink::Error>>
    where
        Sink: DetectionSink,
    {
        self.inner
            .run(shots, sink)
            .map(summary_from_engine)
            .map_err(run_error_from_engine)
    }
}

pub(super) const fn progress_from_engine(
    progress: stab_engine::DetectionRunProgress,
) -> DetectionRunProgress {
    DetectionRunProgress::new(
        progress.committed_shots().get(),
        progress.attempted_batch_shots().get(),
    )
}

pub(super) const fn summary_from_engine(
    summary: stab_engine::DetectionRunSummary,
) -> DetectionRunSummary {
    DetectionRunSummary {
        status: match summary.status() {
            stab_engine::DetectionRunStatus::Completed => DetectionRunStatus::Completed,
            stab_engine::DetectionRunStatus::Cancelled => DetectionRunStatus::Cancelled,
        },
        requested_shots: summary.requested_shots(),
        committed_shots: summary.committed_shots(),
        total_committed_shots: summary.total_committed_shots(),
    }
}

pub(super) fn run_error_from_engine<SinkError>(
    error: stab_engine::DetectionRunError<SinkError>,
) -> DetectionRunError<SinkError> {
    match error {
        stab_engine::DetectionRunError::Engine { source, progress } => DetectionRunError::Engine {
            source: DetectionExecutionError::from_engine(source),
            progress: progress_from_engine(progress),
        },
        stab_engine::DetectionRunError::Sink {
            phase,
            source,
            progress,
        } => DetectionRunError::Sink {
            phase,
            source,
            progress: progress_from_engine(progress),
        },
    }
}
