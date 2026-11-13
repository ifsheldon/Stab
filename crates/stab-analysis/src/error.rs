use stab_model::ModelError;
use thiserror::Error;

use crate::ResourceLimitError;

/// Result type for pure Stab model analysis.
pub type AnalysisResult<T> = Result<T, AnalysisError>;

/// A typed failure while transforming or semantically analyzing a Stab model.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AnalysisError {
    #[error("{0}")]
    Model(ModelError),

    #[error("invalid {kind} value {value}")]
    InvalidDomainValue { kind: &'static str, value: String },

    #[error("cannot convert circuit to tableau: {message}")]
    InvalidTableauConversion { message: String },

    #[error("cannot simplify circuit: {message}")]
    InvalidCircuitSimplification { message: String },

    #[error("invalid result format data: {message}")]
    InvalidResultFormat { message: String },

    #[error("invalid detector error model: {message}")]
    InvalidDetectorErrorModel { message: String },

    #[error(transparent)]
    ResourceLimit(#[from] ResourceLimitError),

    /// The operation's cancellation signal was observed.
    #[error(transparent)]
    Cancelled(#[from] stab_model::Cancelled),
}

impl From<ModelError> for AnalysisError {
    /// Keeps one representation of cancellation: a model traversal stopped by the operation's
    /// signal becomes [`AnalysisError::Cancelled`].
    fn from(error: ModelError) -> Self {
        match error {
            ModelError::Cancelled(cancelled) => Self::Cancelled(cancelled),
            error => Self::Model(error),
        }
    }
}

impl AnalysisError {
    pub(crate) fn invalid_domain_value(kind: &'static str, value: impl ToString) -> Self {
        Self::InvalidDomainValue {
            kind,
            value: value.to_string(),
        }
    }

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

    pub(crate) fn invalid_result_format(message: impl Into<String>) -> Self {
        Self::InvalidResultFormat {
            message: message.into(),
        }
    }

    pub(crate) fn invalid_detector_error_model(message: impl Into<String>) -> Self {
        Self::InvalidDetectorErrorModel {
            message: message.into(),
        }
    }

    pub const fn resource_limit_error(&self) -> Option<&ResourceLimitError> {
        match self {
            Self::ResourceLimit(error) => Some(error),
            _ => None,
        }
    }
}
