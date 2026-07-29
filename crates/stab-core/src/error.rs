use crate::{FormatError, ParseError, ResourceLimitError};
pub use stab_model::{ModelError, ModelResult, ValidationError, ValidationErrorCode};
use thiserror::Error;

pub type CircuitResult<T> = Result<T, CircuitError>;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CircuitError {
    #[error("{0}")]
    Parse(#[from] ParseError),

    #[error("unknown gate {0}")]
    UnknownGate(String),

    #[error("invalid {kind} value {value}")]
    InvalidDomainValue { kind: &'static str, value: String },

    #[error("failed to parse line {line}: {message}")]
    ParseLine { line: usize, message: String },

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

    #[error("cannot convert circuit to tableau: {message}")]
    InvalidTableauConversion { message: String },

    #[error("cannot simplify circuit: {message}")]
    InvalidCircuitSimplification { message: String },

    #[error("cannot compile circuit sampler: {message}")]
    InvalidSamplerCompilation { message: String },

    #[error("invalid result format data: {0}")]
    InvalidResultFormat(#[source] FormatError),

    #[error(transparent)]
    ResourceLimit(#[from] ResourceLimitError),

    #[error("failed to {operation} circuit file: {message}")]
    CircuitIo {
        operation: &'static str,
        kind: std::io::ErrorKind,
        message: String,
    },

    #[error("invalid detector error model: {message}")]
    InvalidDetectorErrorModel { message: String },

    #[error("unterminated repeat block")]
    UnterminatedRepeatBlock,

    #[error("unexpected repeat block terminator")]
    UnexpectedRepeatTerminator,
}

impl From<stab_model::ModelError> for CircuitError {
    fn from(error: stab_model::ModelError) -> Self {
        match error {
            stab_model::ModelError::Parse(error) => Self::Parse(error),
            stab_model::ModelError::ResourceLimit(error) => {
                Self::ResourceLimit(ResourceLimitError::from(error))
            }
            stab_model::ModelError::Validation(error) => Self::from(error),
        }
    }
}

impl From<stab_model::ValidationError> for CircuitError {
    fn from(error: stab_model::ValidationError) -> Self {
        match error {
            stab_model::ValidationError::UnknownGate(gate) => Self::UnknownGate(gate),
            stab_model::ValidationError::InvalidDomainValue { kind, value } => {
                Self::InvalidDomainValue { kind, value }
            }
            stab_model::ValidationError::InvalidArgumentCount {
                gate,
                expected,
                actual,
            } => Self::InvalidArgumentCount {
                gate,
                expected,
                actual,
            },
            stab_model::ValidationError::InvalidArgument { gate, argument } => {
                Self::InvalidArgument { gate, argument }
            }
            stab_model::ValidationError::InvalidTarget { gate, target } => {
                Self::InvalidTarget { gate, target }
            }
            stab_model::ValidationError::InvalidTargetCount { gate, count } => {
                Self::InvalidTargetCount { gate, count }
            }
            stab_model::ValidationError::CircuitCountOverflow
            | stab_model::ValidationError::CoordinateShiftDimensionMissing
            | stab_model::ValidationError::CoordinateShiftOverflow => {
                Self::invalid_result_format(error.to_string())
            }
            stab_model::ValidationError::DetectorCountOverflow
            | stab_model::ValidationError::DetectorIndexOutOfRange { .. }
            | stab_model::ValidationError::DetectorCoordinateLookupFailed => {
                Self::invalid_detector_error_model(error.to_string())
            }
            stab_model::ValidationError::InvalidDetectorErrorModel { message } => {
                Self::invalid_detector_error_model(message)
            }
        }
    }
}

impl From<stab_analysis::AnalysisError> for CircuitError {
    fn from(error: stab_analysis::AnalysisError) -> Self {
        match error {
            stab_analysis::AnalysisError::Model(error) => error.into(),
            stab_analysis::AnalysisError::InvalidDomainValue { kind, value } => {
                Self::InvalidDomainValue { kind, value }
            }
            stab_analysis::AnalysisError::InvalidTableauConversion { message } => {
                Self::InvalidTableauConversion { message }
            }
            stab_analysis::AnalysisError::InvalidCircuitSimplification { message } => {
                Self::InvalidCircuitSimplification { message }
            }
            stab_analysis::AnalysisError::InvalidResultFormat { message } => {
                Self::invalid_result_format(message)
            }
            stab_analysis::AnalysisError::InvalidDetectorErrorModel { message } => {
                Self::invalid_detector_error_model(message)
            }
            stab_analysis::AnalysisError::ResourceLimit(error) => Self::ResourceLimit(error.into()),
        }
    }
}

impl From<stab_engine::SamplingCompileError> for CircuitError {
    fn from(error: stab_engine::SamplingCompileError) -> Self {
        match error {
            stab_engine::SamplingCompileError::Model(error) => error.into(),
            stab_engine::SamplingCompileError::Analysis(error) => error.into(),
            stab_engine::SamplingCompileError::InvalidCircuit { message } => {
                Self::invalid_sampler_compilation(message)
            }
            stab_engine::SamplingCompileError::BackendUnavailable { requested } => {
                Self::invalid_sampler_compilation(format!(
                    "sampling backend {} is unavailable",
                    requested.as_str()
                ))
            }
        }
    }
}

impl From<stab_engine::SamplingExecutionError> for CircuitError {
    fn from(error: stab_engine::SamplingExecutionError) -> Self {
        match error {
            stab_engine::SamplingExecutionError::InvalidSweepRecordWidth { expected, actual } => {
                Self::invalid_result_format(format!(
                    "sweep record expected {expected} bits, got {actual}"
                ))
            }
            other => Self::invalid_sampler_compilation(other.to_string()),
        }
    }
}

impl CircuitError {
    pub(crate) fn invalid_domain_value(kind: &'static str, value: impl ToString) -> Self {
        Self::InvalidDomainValue {
            kind,
            value: value.to_string(),
        }
    }

    pub(crate) fn invalid_sampler_compilation(message: impl Into<String>) -> Self {
        Self::InvalidSamplerCompilation {
            message: message.into(),
        }
    }

    pub fn invalid_result_format(message: impl Into<String>) -> Self {
        Self::InvalidResultFormat(FormatError::invalid_data(message))
    }

    pub const fn format_error(&self) -> Option<&FormatError> {
        match self {
            Self::InvalidResultFormat(error) => Some(error),
            _ => None,
        }
    }

    pub const fn parse_error(&self) -> Option<&ParseError> {
        match self {
            Self::Parse(error) => Some(error),
            _ => None,
        }
    }

    pub const fn resource_limit_error(&self) -> Option<&ResourceLimitError> {
        match self {
            Self::ResourceLimit(error) => Some(error),
            _ => None,
        }
    }

    pub(crate) fn circuit_io(operation: &'static str, error: std::io::Error) -> Self {
        Self::CircuitIo {
            operation,
            kind: error.kind(),
            message: error.to_string(),
        }
    }

    pub(crate) fn invalid_detector_error_model(message: impl Into<String>) -> Self {
        Self::InvalidDetectorErrorModel {
            message: message.into(),
        }
    }
}

impl From<FormatError> for CircuitError {
    fn from(error: FormatError) -> Self {
        Self::InvalidResultFormat(error)
    }
}

impl From<stab_records::FormatError> for CircuitError {
    fn from(error: stab_records::FormatError) -> Self {
        Self::InvalidResultFormat(error.into())
    }
}
