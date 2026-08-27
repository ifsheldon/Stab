use thiserror::Error;

use crate::ResourceAmount;

/// Stable code classifying a sampling compilation failure.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SamplingCompileErrorCode {
    InvalidCircuit,
    ResourceLimit,
}

impl SamplingCompileErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidCircuit => "invalid-circuit",
            Self::ResourceLimit => "resource-limit",
        }
    }
}

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
        "cannot compile circuit sampler: expanded operation work {actual} exceeds per-shot limit {limit}"
    )]
    ExpandedOperationLimit { actual: ResourceAmount, limit: u64 },
}

impl SamplingCompileError {
    pub(crate) fn invalid_circuit(message: impl Into<String>) -> Self {
        Self::InvalidCircuit {
            message: message.into(),
        }
    }

    pub const fn code(&self) -> SamplingCompileErrorCode {
        match self {
            Self::ExpandedOperationLimit { .. } => SamplingCompileErrorCode::ResourceLimit,
            Self::Model(_) | Self::Analysis(_) | Self::InvalidCircuit { .. } => {
                SamplingCompileErrorCode::InvalidCircuit
            }
        }
    }
}
