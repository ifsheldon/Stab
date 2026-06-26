use thiserror::Error;

pub type CircuitResult<T> = Result<T, CircuitError>;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CircuitError {
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

    #[error("unterminated repeat block")]
    UnterminatedRepeatBlock,

    #[error("unexpected repeat block terminator")]
    UnexpectedRepeatTerminator,
}

impl CircuitError {
    pub(crate) fn parse_line(line: usize, message: impl Into<String>) -> Self {
        Self::ParseLine {
            line,
            message: message.into(),
        }
    }

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

    pub(crate) fn invalid_sampler_compilation(message: impl Into<String>) -> Self {
        Self::InvalidSamplerCompilation {
            message: message.into(),
        }
    }
}
