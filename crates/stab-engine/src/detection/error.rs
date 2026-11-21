use std::fmt::{Display, Formatter};

use thiserror::Error;

use stab_model::{MemoryBudget, MemoryBudgetExceeded, ResourceAmount};

use crate::{SamplingCompileError, SamplingExecutionError};

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
///
/// Repeat nesting is a fixed safety limit; compiled bytes and session storage are refused by
/// the caller's memory budget and answer [`Self::memory_budget_exceeded`].
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DetectionResourceLimitError {
    refusal: DetectionRefusal,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum DetectionRefusal {
    RepeatNesting {
        actual: u64,
        limit: u64,
    },
    MemoryBudget {
        kind: DetectionResourceKind,
        exceeded: MemoryBudgetExceeded,
    },
}

impl DetectionResourceLimitError {
    pub(crate) const fn detection_repeat_nesting(actual: usize, limit: usize) -> Self {
        Self {
            refusal: DetectionRefusal::RepeatNesting {
                actual: actual as u64,
                limit: limit as u64,
            },
        }
    }

    pub(crate) const fn detection_compiled_bytes(actual: u64, budget: MemoryBudget) -> Self {
        Self::refused_by_budget(
            DetectionResourceKind::CompiledBytes,
            MemoryBudgetExceeded::new(ResourceAmount::exact(actual), budget),
        )
    }

    pub(crate) const fn refused_by_budget(
        kind: DetectionResourceKind,
        exceeded: MemoryBudgetExceeded,
    ) -> Self {
        Self {
            refusal: DetectionRefusal::MemoryBudget { kind, exceeded },
        }
    }

    pub const fn kind(&self) -> DetectionResourceKind {
        match self.refusal {
            DetectionRefusal::RepeatNesting { .. } => DetectionResourceKind::RepeatNesting,
            DetectionRefusal::MemoryBudget { kind, .. } => kind,
        }
    }

    /// The memory-budget refusal, for the kinds a caller-selected budget limits.
    pub const fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match &self.refusal {
            DetectionRefusal::MemoryBudget { exceeded, .. } => Some(exceeded),
            DetectionRefusal::RepeatNesting { .. } => None,
        }
    }

    const fn amount(&self) -> ResourceAmount {
        match self.refusal {
            DetectionRefusal::RepeatNesting { actual, .. } => ResourceAmount::exact(actual),
            DetectionRefusal::MemoryBudget { exceeded, .. } => exceeded.required(),
        }
    }

    pub const fn actual(&self) -> u64 {
        self.amount().value()
    }

    pub const fn actual_is_lower_bound(&self) -> bool {
        self.amount().is_lower_bound()
    }

    pub const fn limit(&self) -> u64 {
        match self.refusal {
            DetectionRefusal::RepeatNesting { limit, .. } => limit,
            DetectionRefusal::MemoryBudget { exceeded, .. } => exceeded.budget().bytes(),
        }
    }
}

impl Display for DetectionResourceLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let (actual, limit) = (self.amount(), self.limit());
        match self.kind() {
            DetectionResourceKind::RepeatNesting => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion repeat nesting {actual} exceeds fixed safety limit {limit}"
            ),
            DetectionResourceKind::CompiledBytes => write!(
                formatter,
                "cannot compile circuit sampler: the compiled plan and its records need at least {actual} bytes, exceeding the {limit}-byte memory budget; raise the memory budget to admit it"
            ),
            DetectionResourceKind::SessionStorageBytes => write!(
                formatter,
                "sampling session needs an estimated {actual} bytes of bounded storage, exceeding the {limit}-byte memory budget; raise the memory budget to admit it"
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
    /// The memory-budget refusal this error reports, from the detection engine or from the
    /// model or analysis layer it wraps.
    pub fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match self {
            Self::Model(error) => error.memory_budget_exceeded(),
            Self::Analysis(error) => error.memory_budget_exceeded(),
            Self::ResourceLimit(error) => error.memory_budget_exceeded(),
            Self::InvalidSamplerCompilation { .. }
            | Self::InvalidResultFormat { .. }
            | Self::StorageAllocation { .. }
            | Self::Cancelled(_) => None,
        }
    }

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
            SamplingCompileError::CompiledBytesLimit { exceeded } => {
                Self::ResourceLimit(DetectionResourceLimitError::refused_by_budget(
                    DetectionResourceKind::CompiledBytes,
                    exceeded,
                ))
            }
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
            SamplingExecutionError::SessionStorageLimit { exceeded } => {
                Self::ResourceLimit(DetectionResourceLimitError::refused_by_budget(
                    DetectionResourceKind::SessionStorageBytes,
                    exceeded,
                ))
            }
            error @ SamplingExecutionError::SessionStorageAllocation { .. } => {
                Self::storage_allocation(error.to_string())
            }
            other => Self::invalid_sampler_compilation(other.to_string()),
        }
    }
}
