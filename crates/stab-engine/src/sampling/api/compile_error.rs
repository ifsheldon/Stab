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

    #[error(
        "cannot compile circuit sampler: the compiled program needs at least {required} bytes, exceeding the {budget}-byte memory budget; raise the memory budget to admit it"
    )]
    CompiledBytesLimit { required: u64, budget: u64 },

    /// The allocator refused storage that admission had already accepted.
    #[error("cannot compile circuit sampler: {message}")]
    StorageAllocation { message: String },

    #[error(transparent)]
    Cancelled(#[from] stab_model::Cancelled),
}

impl SamplingCompileError {
    pub(crate) fn invalid_circuit(message: impl Into<String>) -> Self {
        Self::InvalidCircuit {
            message: message.into(),
        }
    }
}
