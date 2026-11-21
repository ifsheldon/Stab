use stab_model::MemoryBudgetExceeded;
use thiserror::Error;

/// Failure to compile an immutable sampling plan.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SamplingCompileError {
    #[error(transparent)]
    Model(#[from] stab_model::ModelError),

    #[error(transparent)]
    Analysis(#[from] stab_analysis::AnalysisError),

    #[error("cannot compile circuit sampler: {message}")]
    InvalidCircuit { message: String },

    /// The memory budget refused the compiled program before any of it was reserved.
    #[error(
        "cannot compile circuit sampler: the compiled program needs at least {} bytes, exceeding the {}-byte memory budget; raise the memory budget to admit it",
        exceeded.required().value(),
        exceeded.budget().bytes()
    )]
    CompiledBytesLimit { exceeded: MemoryBudgetExceeded },

    /// The allocator refused storage that admission had already accepted.
    #[error("cannot compile circuit sampler: {message}")]
    StorageAllocation { message: String },

    #[error(transparent)]
    Cancelled(#[from] stab_model::Cancelled),
}

impl SamplingCompileError {
    /// The memory-budget refusal this error reports, including one from the model or analysis
    /// layer.
    pub fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match self {
            Self::Model(error) => error.memory_budget_exceeded(),
            Self::Analysis(error) => error.memory_budget_exceeded(),
            Self::CompiledBytesLimit { exceeded } => Some(exceeded),
            Self::InvalidCircuit { .. } | Self::StorageAllocation { .. } | Self::Cancelled(_) => {
                None
            }
        }
    }

    pub(crate) fn invalid_circuit(message: impl Into<String>) -> Self {
        Self::InvalidCircuit {
            message: message.into(),
        }
    }
}
