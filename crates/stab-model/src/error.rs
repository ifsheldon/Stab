use thiserror::Error;

/// Result type for stable model construction and validation.
pub type ModelResult<T> = Result<T, ModelError>;

/// A typed failure while constructing or validating a Stim model value.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ModelError {
    #[error("unknown gate {0}")]
    UnknownGate(String),

    #[error("invalid {kind} value {value}")]
    InvalidDomainValue { kind: &'static str, value: String },

    #[error("gate {gate} expected {expected} argument(s), got {actual}")]
    InvalidArgumentCount {
        gate: &'static str,
        expected: &'static str,
        actual: usize,
    },

    #[error("gate {gate} received invalid argument {argument}")]
    InvalidArgument {
        gate: &'static str,
        argument: String,
    },

    #[error("gate {gate} received invalid target {target}")]
    InvalidTarget { gate: &'static str, target: String },

    #[error("gate {gate} received invalid target count {count}")]
    InvalidTargetCount { gate: &'static str, count: usize },
}

impl ModelError {
    pub(crate) fn invalid_domain_value(kind: &'static str, value: impl ToString) -> Self {
        Self::InvalidDomainValue {
            kind,
            value: value.to_string(),
        }
    }
}
