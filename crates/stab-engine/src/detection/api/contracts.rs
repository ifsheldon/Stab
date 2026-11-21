use stab_model::MemoryBudgetExceeded;
use thiserror::Error;

use crate::SamplingExecutionError;

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
    /// as [`crate::RunStatus::Cancelled`] instead, so a run does not return this variant;
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
