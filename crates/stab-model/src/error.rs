use std::borrow::Cow;

use thiserror::Error;

use crate::{Cancelled, ModelDialect, ParseError, ResourceLimitError, ValidationError};

/// Result type for stable model construction and validation.
pub type ModelResult<T> = Result<T, ModelError>;

/// A typed failure while constructing or validating a Stim model value.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ModelError {
    #[error("{0}")]
    Parse(#[from] ParseError),

    #[error("{0}")]
    ResourceLimit(#[from] ResourceLimitError),

    #[error("{0}")]
    Validation(#[from] ValidationError),

    /// The allocator refused storage for an already admitted model operation.
    #[error("{}{detail}", allocation_message_prefix(*.dialect))]
    AllocationFailed {
        dialect: ModelDialect,
        detail: Cow<'static, str>,
    },

    /// A traversal given a cancellation signal observed it.
    #[error(transparent)]
    Cancelled(#[from] Cancelled),
}

/// Keeps allocation refusals worded as the validation failures they used to be reported as.
const fn allocation_message_prefix(dialect: ModelDialect) -> &'static str {
    match dialect {
        ModelDialect::StimCircuit => "invalid circuit: ",
        ModelDialect::DetectorErrorModel => "invalid detector error model: ",
    }
}

impl ModelError {
    pub(crate) fn invalid_domain_value(kind: &'static str, value: impl ToString) -> Self {
        ValidationError::invalid_domain_value(kind, value).into()
    }

    /// The allocator refused storage a detector error model operation needed.
    pub(crate) fn dem_allocation_failed(detail: impl Into<Cow<'static, str>>) -> Self {
        Self::AllocationFailed {
            dialect: ModelDialect::DetectorErrorModel,
            detail: detail.into(),
        }
    }

    pub(crate) fn invalid_detector_error_model(message: impl Into<String>) -> Self {
        ValidationError::InvalidDetectorErrorModel {
            message: message.into(),
        }
        .into()
    }

    pub fn parse_error(&self) -> Option<&ParseError> {
        match self {
            Self::Parse(error) => Some(error),
            _ => None,
        }
    }

    pub fn resource_limit_error(&self) -> Option<&ResourceLimitError> {
        match self {
            Self::ResourceLimit(error) => Some(error),
            _ => None,
        }
    }

    pub fn validation_error(&self) -> Option<&ValidationError> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}
