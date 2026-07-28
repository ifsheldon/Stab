use thiserror::Error;

use crate::{ParseError, ResourceLimitError, ValidationError};

/// Result type for stable model construction and validation.
pub type ModelResult<T> = Result<T, ModelError>;

/// A typed failure while constructing or validating a Stim model value.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ModelError {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    ResourceLimit(#[from] ResourceLimitError),

    #[error(transparent)]
    Validation(#[from] ValidationError),
}

impl ModelError {
    pub(crate) fn invalid_domain_value(kind: &'static str, value: impl ToString) -> Self {
        ValidationError::invalid_domain_value(kind, value).into()
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
