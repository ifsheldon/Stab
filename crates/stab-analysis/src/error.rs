use stab_model::ModelError;
use thiserror::Error;

/// Result type for pure Stab model analysis.
pub type AnalysisResult<T> = Result<T, AnalysisError>;

/// A typed failure while transforming or semantically analyzing a Stab model.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AnalysisError {
    #[error("{0}")]
    Model(#[from] ModelError),

    #[error("cannot convert circuit to tableau: {message}")]
    InvalidTableauConversion { message: String },

    #[error("cannot simplify circuit: {message}")]
    InvalidCircuitSimplification { message: String },
}

impl AnalysisError {
    pub(crate) fn invalid_tableau_conversion(message: impl Into<String>) -> Self {
        Self::InvalidTableauConversion {
            message: message.into(),
        }
    }

    pub(crate) fn invalid_circuit_simplification(message: impl Into<String>) -> Self {
        Self::InvalidCircuitSimplification {
            message: message.into(),
        }
    }
}
