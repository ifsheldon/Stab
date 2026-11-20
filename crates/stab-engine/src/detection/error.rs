use std::fmt::{Display, Formatter};

use thiserror::Error;

use crate::{ResourceAmount, SamplingCompileError, SamplingExecutionError};

pub(crate) type DetectionResult<T> = Result<T, DetectionError>;

/// Detection-conversion resource dimension whose configured limit was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DetectionResourceKind {
    RepeatNesting,
    CompiledBytes,
    /// Bounded storage of a measurement-sampling session or reference run that a detection
    /// operation creates.
    SessionStorageBytes,
}

/// Typed resource-admission failure owned by the detection engine.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DetectionResourceLimitError {
    kind: DetectionResourceKind,
    actual: ResourceAmount,
    limit: u64,
}

impl DetectionResourceLimitError {
    pub(crate) const fn detection_repeat_nesting(actual: usize, limit: usize) -> Self {
        Self {
            kind: DetectionResourceKind::RepeatNesting,
            actual: ResourceAmount::exact(actual as u64),
            limit: limit as u64,
        }
    }

    pub(crate) const fn detection_compiled_bytes(actual: u64, limit: u64) -> Self {
        Self {
            kind: DetectionResourceKind::CompiledBytes,
            actual: ResourceAmount::exact(actual),
            limit,
        }
    }

    pub(crate) fn session_storage_bytes(actual: u128, limit: u64) -> Self {
        Self {
            kind: DetectionResourceKind::SessionStorageBytes,
            actual: ResourceAmount::saturating_from_u128(actual),
            limit,
        }
    }

    pub const fn kind(&self) -> DetectionResourceKind {
        self.kind
    }

    pub const fn actual(&self) -> u64 {
        self.actual.value()
    }

    pub const fn actual_is_lower_bound(&self) -> bool {
        self.actual.is_lower_bound()
    }

    pub const fn limit(&self) -> u64 {
        self.limit
    }
}

impl Display for DetectionResourceLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            DetectionResourceKind::RepeatNesting => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion repeat nesting {} exceeds fixed safety limit {}",
                self.actual, self.limit
            ),
            DetectionResourceKind::CompiledBytes => write!(
                formatter,
                "cannot compile circuit sampler: the compiled plan and its records need at least {} bytes, exceeding the {}-byte memory budget; raise the memory budget to admit it",
                self.actual, self.limit
            ),
            DetectionResourceKind::SessionStorageBytes => write!(
                formatter,
                "sampling session needs an estimated {} bytes of bounded storage, exceeding the {}-byte memory budget; raise the memory budget to admit it",
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

    /// The allocator refused storage that admission had already accepted.
    #[error("cannot compile circuit sampler: {message}")]
    StorageAllocation { message: String },

    #[error(transparent)]
    Cancelled(#[from] stab_model::Cancelled),
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

    pub(crate) fn storage_allocation(message: impl Into<String>) -> Self {
        Self::StorageAllocation {
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
            SamplingCompileError::CompiledBytesLimit { required, budget } => Self::ResourceLimit(
                DetectionResourceLimitError::detection_compiled_bytes(required, budget),
            ),
            SamplingCompileError::StorageAllocation { message } => {
                Self::StorageAllocation { message }
            }
            SamplingCompileError::Cancelled(cancelled) => Self::Cancelled(cancelled),
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
            SamplingExecutionError::Cancelled(cancelled) => Self::Cancelled(cancelled),
            SamplingExecutionError::SessionStorageLimit {
                estimated_bytes,
                limit_bytes,
            } => Self::ResourceLimit(DetectionResourceLimitError::session_storage_bytes(
                estimated_bytes,
                limit_bytes,
            )),
            error @ SamplingExecutionError::SessionStorageAllocation { .. } => {
                Self::storage_allocation(error.to_string())
            }
            other => Self::invalid_sampler_compilation(other.to_string()),
        }
    }
}
