use std::fmt::{Display, Formatter};

use thiserror::Error;

use crate::{SamplingCompileError, SamplingExecutionError};

pub(crate) type DetectionResult<T> = Result<T, DetectionError>;

/// Detection record dimension whose configured limit was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DetectionRecordLimitSubject {
    DetectionRecord,
    MeasurementRecord,
    SweepRecord,
    ObservableCount,
}

/// Detection-conversion resource dimension whose configured limit was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DetectionResourceKind {
    RecordBits(DetectionRecordLimitSubject),
    RepeatNesting,
    ExpandedInstructions,
    RepeatIterations,
    CompiledTerms,
    CompiledBytes,
}

/// Typed resource-admission failure owned by the detection engine.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DetectionResourceLimitError {
    kind: DetectionResourceKind,
    actual: u64,
    limit: u64,
}

impl DetectionResourceLimitError {
    pub(crate) const fn detection_record_bits(
        subject: DetectionRecordLimitSubject,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self::record_bits(subject, actual, limit)
    }

    pub(crate) const fn detection_repeat_nesting(actual: usize, limit: usize) -> Self {
        Self::repeat_nesting(actual, limit)
    }

    pub(crate) const fn detection_expanded_instructions(actual: u64, limit: u64) -> Self {
        Self::expanded_instructions(actual, limit)
    }

    pub(crate) const fn detection_repeat_iterations(actual: u64, limit: u64) -> Self {
        Self::repeat_iterations(actual, limit)
    }

    pub(crate) const fn detection_compiled_terms(actual: u64, limit: u64) -> Self {
        Self::compiled_terms(actual, limit)
    }

    pub(crate) const fn detection_compiled_bytes(actual: u64, limit: u64) -> Self {
        Self::compiled_bytes(actual, limit)
    }

    pub(crate) const fn record_bits(
        subject: DetectionRecordLimitSubject,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self {
            kind: DetectionResourceKind::RecordBits(subject),
            actual,
            limit,
        }
    }

    pub(crate) const fn repeat_nesting(actual: usize, limit: usize) -> Self {
        Self {
            kind: DetectionResourceKind::RepeatNesting,
            actual: actual as u64,
            limit: limit as u64,
        }
    }

    pub(crate) const fn expanded_instructions(actual: u64, limit: u64) -> Self {
        Self {
            kind: DetectionResourceKind::ExpandedInstructions,
            actual,
            limit,
        }
    }

    pub(crate) const fn repeat_iterations(actual: u64, limit: u64) -> Self {
        Self {
            kind: DetectionResourceKind::RepeatIterations,
            actual,
            limit,
        }
    }

    pub(crate) const fn compiled_terms(actual: u64, limit: u64) -> Self {
        Self {
            kind: DetectionResourceKind::CompiledTerms,
            actual,
            limit,
        }
    }

    pub(crate) const fn compiled_bytes(actual: u64, limit: u64) -> Self {
        Self {
            kind: DetectionResourceKind::CompiledBytes,
            actual,
            limit,
        }
    }

    pub const fn kind(&self) -> DetectionResourceKind {
        self.kind
    }

    pub const fn actual(&self) -> u64 {
        self.actual
    }

    pub const fn limit(&self) -> u64 {
        self.limit
    }
}

impl Display for DetectionResourceLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            DetectionResourceKind::RecordBits(subject) => match subject {
                DetectionRecordLimitSubject::DetectionRecord => write!(
                    formatter,
                    "invalid result format data: detection record width {} exceeds current limit {}",
                    self.actual, self.limit
                ),
                DetectionRecordLimitSubject::MeasurementRecord => write!(
                    formatter,
                    "invalid result format data: measurement record width {} exceeds current detection conversion limit {}",
                    self.actual, self.limit
                ),
                DetectionRecordLimitSubject::SweepRecord => write!(
                    formatter,
                    "invalid result format data: sweep bit width {} exceeds current detection conversion limit {}",
                    self.actual, self.limit
                ),
                DetectionRecordLimitSubject::ObservableCount => write!(
                    formatter,
                    "invalid result format data: observable id {} exceeds current detection record limit {}",
                    self.actual.saturating_sub(1),
                    self.limit
                ),
            },
            DetectionResourceKind::RepeatNesting => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion repeat nesting {} exceeds fixed safety limit {}",
                self.actual, self.limit
            ),
            DetectionResourceKind::ExpandedInstructions => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion would execute {} expanded instructions; current limit is {}",
                self.actual, self.limit
            ),
            DetectionResourceKind::RepeatIterations => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion would execute {} repeat iterations; current limit is {}",
                self.actual, self.limit
            ),
            DetectionResourceKind::CompiledTerms => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion would retain {} measurement-reference terms; current limit is {}",
                self.actual, self.limit
            ),
            DetectionResourceKind::CompiledBytes => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion would require at least {} compiled bytes; current limit is {}",
                self.actual, self.limit
            ),
        }
    }
}

impl std::error::Error for DetectionResourceLimitError {}

/// Semantic failure while compiling or executing detection conversion.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DetectionError {
    #[error(transparent)]
    Model(#[from] stab_model::ModelError),

    #[error(transparent)]
    Analysis(#[from] stab_analysis::AnalysisError),

    #[error("cannot compile circuit sampler: {message}")]
    InvalidSamplerCompilation { message: String },

    #[error("invalid result format data: {message}")]
    InvalidResultFormat { message: String },

    #[error(transparent)]
    ResourceLimit(#[from] DetectionResourceLimitError),
}

impl DetectionError {
    pub(crate) fn invalid_sampler_compilation(message: impl Into<String>) -> Self {
        Self::InvalidSamplerCompilation {
            message: message.into(),
        }
    }

    pub(crate) fn invalid_result_format(message: impl Into<String>) -> Self {
        Self::InvalidResultFormat {
            message: message.into(),
        }
    }
}

impl From<SamplingCompileError> for DetectionError {
    fn from(error: SamplingCompileError) -> Self {
        match error {
            SamplingCompileError::Model(error) => Self::Model(error),
            SamplingCompileError::Analysis(error) => Self::Analysis(error),
            SamplingCompileError::InvalidCircuit { message } => {
                Self::invalid_sampler_compilation(message)
            }
        }
    }
}

impl From<SamplingExecutionError> for DetectionError {
    fn from(error: SamplingExecutionError) -> Self {
        match error {
            SamplingExecutionError::InvalidSweepRecordWidth { expected, actual } => {
                Self::invalid_result_format(format!(
                    "sweep record expected {expected} bits, got {actual}"
                ))
            }
            other => Self::invalid_sampler_compilation(other.to_string()),
        }
    }
}
