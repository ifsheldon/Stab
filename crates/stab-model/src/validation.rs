use thiserror::Error;

use crate::DiagnosticSeverity;

/// Stable machine-readable structural-validation failure classes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ValidationErrorCode {
    UnknownGate,
    InvalidDomainValue,
    InvalidArgumentCount,
    InvalidArgument,
    InvalidTarget,
    InvalidTargetCount,
    CircuitCountOverflow,
    CoordinateShiftDimensionMissing,
    CoordinateShiftOverflow,
    DetectorCountOverflow,
    DetectorIndexOutOfRange,
    DetectorCoordinateLookupFailed,
}

impl ValidationErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownGate => "unknown-gate",
            Self::InvalidDomainValue => "invalid-domain-value",
            Self::InvalidArgumentCount => "invalid-argument-count",
            Self::InvalidArgument => "invalid-argument",
            Self::InvalidTarget => "invalid-target",
            Self::InvalidTargetCount => "invalid-target-count",
            Self::CircuitCountOverflow => "circuit-count-overflow",
            Self::CoordinateShiftDimensionMissing => "coordinate-shift-dimension-missing",
            Self::CoordinateShiftOverflow => "coordinate-shift-overflow",
            Self::DetectorCountOverflow => "detector-count-overflow",
            Self::DetectorIndexOutOfRange => "detector-index-out-of-range",
            Self::DetectorCoordinateLookupFailed => "detector-coordinate-lookup-failed",
        }
    }
}

/// A typed structural-validation failure for a Stim model value.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ValidationError {
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

    #[error("circuit count overflowed")]
    CircuitCountOverflow,

    #[error("coordinate shift dimension missing")]
    CoordinateShiftDimensionMissing,

    #[error("coordinate shift overflowed")]
    CoordinateShiftOverflow,

    #[error("detector count overflowed")]
    DetectorCountOverflow,

    #[error("Detector index {index} is too big. The circuit has {detector_count} detectors")]
    DetectorIndexOutOfRange { index: u64, detector_count: u64 },

    #[error("detector coordinate lookup failed")]
    DetectorCoordinateLookupFailed,
}

impl ValidationError {
    pub(crate) fn invalid_domain_value(kind: &'static str, value: impl ToString) -> Self {
        Self::InvalidDomainValue {
            kind,
            value: value.to_string(),
        }
    }

    pub const fn code(&self) -> ValidationErrorCode {
        match self {
            Self::UnknownGate(_) => ValidationErrorCode::UnknownGate,
            Self::InvalidDomainValue { .. } => ValidationErrorCode::InvalidDomainValue,
            Self::InvalidArgumentCount { .. } => ValidationErrorCode::InvalidArgumentCount,
            Self::InvalidArgument { .. } => ValidationErrorCode::InvalidArgument,
            Self::InvalidTarget { .. } => ValidationErrorCode::InvalidTarget,
            Self::InvalidTargetCount { .. } => ValidationErrorCode::InvalidTargetCount,
            Self::CircuitCountOverflow => ValidationErrorCode::CircuitCountOverflow,
            Self::CoordinateShiftDimensionMissing => {
                ValidationErrorCode::CoordinateShiftDimensionMissing
            }
            Self::CoordinateShiftOverflow => ValidationErrorCode::CoordinateShiftOverflow,
            Self::DetectorCountOverflow => ValidationErrorCode::DetectorCountOverflow,
            Self::DetectorIndexOutOfRange { .. } => ValidationErrorCode::DetectorIndexOutOfRange,
            Self::DetectorCoordinateLookupFailed => {
                ValidationErrorCode::DetectorCoordinateLookupFailed
            }
        }
    }

    pub const fn severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::Error
    }
}
