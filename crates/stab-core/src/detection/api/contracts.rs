use std::fmt;

use thiserror::Error;

use crate::{CircuitError, SamplingExecutionError, ShotCount, SinkFailurePhase};

/// Failure to compile a measurement-conversion or detection-sampling plan.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DetectionCompileError {
    #[error(transparent)]
    InvalidCircuit(#[from] CircuitError),
}

impl DetectionCompileError {
    pub fn into_circuit_error(self) -> CircuitError {
        match self {
            Self::InvalidCircuit(error) => error,
        }
    }
}

/// Engine-side failure from a detection conversion or sampling session.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DetectionExecutionError {
    #[error("detection session is poisoned")]
    SessionPoisoned,

    #[error("detection delivery is already finalized")]
    DeliveryFinished,

    #[error("detection session shot counter overflowed")]
    ShotCounterOverflow,

    #[error(
        "detection session needs an estimated {estimated_bytes} bytes of bounded storage, exceeding the {limit_bytes}-byte safety limit"
    )]
    SessionStorageLimit {
        estimated_bytes: u128,
        limit_bytes: u64,
    },

    #[error("detection session could not allocate bounded storage: {message}")]
    SessionStorageAllocation { message: String },

    #[error("detection conversion failed: {0}")]
    Conversion(#[source] CircuitError),

    #[error("measurement sampling failed: {0}")]
    Sampling(#[source] SamplingExecutionError),

    #[error("detection conversion was cancelled inside a measurement-sink composition")]
    CancelledComposition,

    #[error("detection execution violated an internal batch invariant: {message}")]
    InternalInvariant { message: String },
}

impl DetectionExecutionError {
    pub fn into_circuit_error(self) -> CircuitError {
        match self {
            Self::Conversion(error) => error,
            Self::Sampling(error) => error.into_circuit_error(),
            other => CircuitError::invalid_sampler_compilation(other.to_string()),
        }
    }
}

impl From<CircuitError> for DetectionExecutionError {
    fn from(error: CircuitError) -> Self {
        Self::Conversion(error)
    }
}

/// Exact progress at a detection execution failure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DetectionRunProgress {
    committed_shots: ShotCount,
    attempted_batch_shots: ShotCount,
}

impl DetectionRunProgress {
    pub(super) const fn new(committed_shots: u64, attempted_batch_shots: u64) -> Self {
        Self {
            committed_shots: ShotCount::new(committed_shots),
            attempted_batch_shots: ShotCount::new(attempted_batch_shots),
        }
    }

    pub const fn committed_shots(self) -> ShotCount {
        self.committed_shots
    }

    pub const fn attempted_batch_shots(self) -> ShotCount {
        self.attempted_batch_shots
    }
}

/// Non-lossy composition of detection-engine and sink failures.
#[derive(Debug)]
pub enum DetectionRunError<SinkError> {
    Engine {
        source: DetectionExecutionError,
        progress: DetectionRunProgress,
    },
    Sink {
        phase: SinkFailurePhase,
        source: SinkError,
        progress: DetectionRunProgress,
    },
}

impl<SinkError> DetectionRunError<SinkError> {
    pub const fn progress(&self) -> DetectionRunProgress {
        match self {
            Self::Engine { progress, .. } | Self::Sink { progress, .. } => *progress,
        }
    }
}

impl<SinkError: fmt::Display> fmt::Display for DetectionRunError<SinkError> {
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
                "detection sink {} failed after {} committed shots while attempting {} shots: {source}",
                phase.as_str(),
                progress.committed_shots().get(),
                progress.attempted_batch_shots().get()
            ),
        }
    }
}

impl<SinkError> std::error::Error for DetectionRunError<SinkError>
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

/// Completion state of one detection execution call.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DetectionRunStatus {
    Completed,
    Cancelled,
}

/// Summary of one completed or cooperatively cancelled detection call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetectionRunSummary {
    pub(super) status: DetectionRunStatus,
    pub(super) requested_shots: ShotCount,
    pub(super) committed_shots: ShotCount,
    pub(super) total_committed_shots: ShotCount,
}

impl DetectionRunSummary {
    pub const fn status(self) -> DetectionRunStatus {
        self.status
    }

    pub const fn requested_shots(self) -> ShotCount {
        self.requested_shots
    }

    pub const fn committed_shots(self) -> ShotCount {
        self.committed_shots
    }

    pub const fn total_committed_shots(self) -> ShotCount {
        self.total_committed_shots
    }
}
