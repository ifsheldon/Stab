use std::fmt;

use stab_model::MemoryBudgetExceeded;
use thiserror::Error;

use crate::{SamplingExecutionError, ShotCount, SinkFailurePhase};

use super::super::error::DetectionError;

/// Failure to compile a measurement-conversion or detection-sampling plan.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DetectionCompileError {
    #[error(transparent)]
    InvalidCircuit(#[from] DetectionError),
}

/// Engine-side failure from a detection conversion or sampling session.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DetectionExecutionError {
    #[error("detection session is poisoned")]
    SessionPoisoned,

    #[error("detection delivery is already finalized")]
    DeliveryFinished,

    #[error("a detection transaction is already active")]
    TransactionActive,

    #[error("detection session shot counter overflowed")]
    ShotCounterOverflow,

    /// The memory budget refused the session's bounded storage before any of it was reserved.
    #[error(
        "detection session needs an estimated {} bytes of bounded storage, exceeding the {}-byte memory budget; raise the memory budget to admit it",
        exceeded.required(),
        exceeded.budget().bytes()
    )]
    SessionStorageLimit { exceeded: MemoryBudgetExceeded },

    #[error("detection session could not allocate bounded storage: {message}")]
    SessionStorageAllocation { message: String },

    #[error("detection conversion failed: {0}")]
    Conversion(#[source] DetectionError),

    #[error("measurement sampling failed: {0}")]
    Sampling(#[source] SamplingExecutionError),

    #[error("detection conversion was cancelled inside a measurement-sink composition")]
    CancelledComposition,

    #[error("detection execution violated an internal batch invariant: {message}")]
    InternalInvariant { message: String },

    /// Cancellation was observed by an engine step. Sessions report cooperative cancellation
    /// as [`DetectionRunStatus::Cancelled`] instead, so a run does not return this variant;
    /// creating a streaming run returns it when the plan's signal is observed while the run's
    /// records are zeroed.
    #[error(transparent)]
    Cancelled(#[from] stab_model::Cancelled),
}

impl DetectionCompileError {
    /// The memory-budget refusal this error reports, if it is one.
    pub fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match self {
            Self::InvalidCircuit(error) => error.memory_budget_exceeded(),
        }
    }
}

impl DetectionExecutionError {
    /// The memory-budget refusal this error reports, including one from the wrapped conversion
    /// or measurement-sampling failure.
    pub fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match self {
            Self::SessionStorageLimit { exceeded } => Some(exceeded),
            Self::Conversion(error) => error.memory_budget_exceeded(),
            Self::Sampling(error) => error.memory_budget_exceeded(),
            _ => None,
        }
    }
}

impl From<DetectionError> for DetectionExecutionError {
    fn from(error: DetectionError) -> Self {
        match error {
            DetectionError::Cancelled(cancelled) => Self::Cancelled(cancelled),
            error => Self::Conversion(error),
        }
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
    /// The engine's memory-budget refusal; a sink failure is the caller's own type and is not
    /// inspected.
    pub fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match self {
            Self::Engine { source, .. } => source.memory_budget_exceeded(),
            Self::Sink { .. } => None,
        }
    }

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
