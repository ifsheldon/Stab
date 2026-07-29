use std::fmt;

use thiserror::Error;

use crate::{CircuitError, DemSampleSink, ShotCount, SinkFailurePhase};

pub use stab_engine::{
    DemReplayBatchStatus, DemSamplingCancellation, DemSamplingRunProgress, DemSamplingRunStatus,
    DemSamplingRunSummary,
};

/// Engine-side DEM sampling execution failure with facade-compatible errors.
#[derive(Debug, Error)]
pub enum DemSamplingExecutionError {
    #[error("DEM sampling session is poisoned")]
    SessionPoisoned,

    #[error("DEM sampling session shot counter overflowed")]
    ShotCounterOverflow,

    #[error(
        "DEM sampling session needs an estimated {estimated_bytes} bytes of bounded storage, exceeding the {limit_bytes}-byte safety limit"
    )]
    SessionStorageLimit {
        estimated_bytes: u128,
        limit_bytes: u64,
    },

    #[error("DEM sampling session could not allocate bounded storage: {message}")]
    SessionStorageAllocation { message: String },

    #[error(transparent)]
    InvalidRequest(#[from] CircuitError),

    #[error("DEM sampling execution violated an internal batch invariant: {message}")]
    InternalInvariant { message: String },
}

impl DemSamplingExecutionError {
    pub fn into_circuit_error(self) -> CircuitError {
        match self {
            Self::InvalidRequest(error) => error,
            other => CircuitError::invalid_sampler_compilation(other.to_string()),
        }
    }

    pub(super) fn from_engine(error: stab_engine::DemSamplingExecutionError) -> Self {
        match error {
            stab_engine::DemSamplingExecutionError::SessionPoisoned => Self::SessionPoisoned,
            stab_engine::DemSamplingExecutionError::ShotCounterOverflow => {
                Self::ShotCounterOverflow
            }
            stab_engine::DemSamplingExecutionError::SessionStorageLimit {
                estimated_bytes,
                limit_bytes,
            } => Self::SessionStorageLimit {
                estimated_bytes,
                limit_bytes,
            },
            stab_engine::DemSamplingExecutionError::SessionStorageAllocation { message } => {
                Self::SessionStorageAllocation { message }
            }
            stab_engine::DemSamplingExecutionError::InvalidRequest(error) => {
                Self::InvalidRequest(error.into())
            }
            stab_engine::DemSamplingExecutionError::InternalInvariant { message } => {
                Self::InternalInvariant { message }
            }
        }
    }
}

/// Non-lossy composition of DEM engine and sink failures.
#[derive(Debug)]
pub enum DemSamplingRunError<SinkError> {
    Engine {
        source: DemSamplingExecutionError,
        progress: DemSamplingRunProgress,
    },
    Sink {
        phase: SinkFailurePhase,
        source: SinkError,
        progress: DemSamplingRunProgress,
    },
}

impl<SinkError> DemSamplingRunError<SinkError> {
    pub const fn progress(&self) -> DemSamplingRunProgress {
        match self {
            Self::Engine { progress, .. } | Self::Sink { progress, .. } => *progress,
        }
    }
}

impl<SinkError: fmt::Display> fmt::Display for DemSamplingRunError<SinkError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine { source, progress } => write!(
                formatter,
                "{source} after {} committed shots",
                progress.committed_shots().get()
            ),
            Self::Sink {
                phase,
                source,
                progress,
            } => write!(
                formatter,
                "DEM sampling sink {} failed after {} committed shots while attempting {} shots: {source}",
                phase.as_str(),
                progress.committed_shots().get(),
                progress.attempted_batch_shots().get()
            ),
        }
    }
}

impl<SinkError> std::error::Error for DemSamplingRunError<SinkError>
where
    SinkError: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine { source, .. } => Some(source),
            Self::Sink { source, .. } => Some(source),
        }
    }
}

/// Mutable compatibility wrapper over an engine-owned DEM sampling session.
pub struct DemSamplingSession {
    inner: stab_engine::DemSamplingSession,
}

impl fmt::Debug for DemSamplingSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DemSamplingSession")
            .field("inner", &self.inner)
            .field("total_committed_shots", &self.total_committed_shots())
            .field("poisoned", &self.is_poisoned())
            .finish_non_exhaustive()
    }
}

impl DemSamplingSession {
    pub(super) const fn from_engine(inner: stab_engine::DemSamplingSession) -> Self {
        Self { inner }
    }

    pub fn cancellation(&self) -> DemSamplingCancellation {
        self.inner.cancellation()
    }

    pub const fn is_poisoned(&self) -> bool {
        self.inner.is_poisoned()
    }

    pub const fn total_committed_shots(&self) -> ShotCount {
        self.inner.total_committed_shots()
    }

    pub fn run<Sink>(
        &mut self,
        shots: ShotCount,
        sink: &mut Sink,
    ) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.inner.run(shots, sink).map_err(run_error_from_engine)
    }

    pub fn run_with_sampled_errors<Sink>(
        &mut self,
        shots: ShotCount,
        sink: &mut Sink,
    ) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.inner
            .run_with_sampled_errors(shots, sink)
            .map_err(run_error_from_engine)
    }

    pub fn replay<Sink>(
        &mut self,
        error_records: &[Vec<bool>],
        sink: &mut Sink,
    ) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.inner
            .replay(error_records, sink)
            .map_err(run_error_from_engine)
    }

    pub fn start_replay<'session, 'sink, Sink>(
        &'session mut self,
        expected_shots: ShotCount,
        sink: &'sink mut Sink,
    ) -> Result<DemReplaySession<'session, 'sink, Sink>, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.inner
            .start_replay(expected_shots, sink)
            .map(DemReplaySession::from_engine)
            .map_err(run_error_from_engine)
    }
}

/// Incremental facade wrapper over an engine-owned DEM replay delivery.
pub struct DemReplaySession<'session, 'sink, Sink>
where
    Sink: DemSampleSink,
{
    inner: stab_engine::DemReplaySession<'session, 'sink, Sink>,
}

impl<Sink> fmt::Debug for DemReplaySession<'_, '_, Sink>
where
    Sink: DemSampleSink,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DemReplaySession")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<'session, 'sink, Sink> DemReplaySession<'session, 'sink, Sink>
where
    Sink: DemSampleSink,
{
    const fn from_engine(inner: stab_engine::DemReplaySession<'session, 'sink, Sink>) -> Self {
        Self { inner }
    }

    pub fn write_batch(
        &mut self,
        error_records: &[Vec<bool>],
    ) -> Result<DemReplayBatchStatus, DemSamplingRunError<Sink::Error>> {
        self.inner
            .write_batch(error_records)
            .map_err(run_error_from_engine)
    }

    pub fn finish(self) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>> {
        self.inner.finish().map_err(run_error_from_engine)
    }
}

pub(super) fn run_error_from_engine<SinkError>(
    error: stab_engine::DemSamplingRunError<SinkError>,
) -> DemSamplingRunError<SinkError> {
    match error {
        stab_engine::DemSamplingRunError::Engine { source, progress } => {
            DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::from_engine(source),
                progress,
            }
        }
        stab_engine::DemSamplingRunError::Sink {
            phase,
            source,
            progress,
        } => DemSamplingRunError::Sink {
            phase,
            source,
            progress,
        },
    }
}

impl From<stab_engine::DemSamplingExecutionError> for DemSamplingExecutionError {
    fn from(error: stab_engine::DemSamplingExecutionError) -> Self {
        Self::from_engine(error)
    }
}
