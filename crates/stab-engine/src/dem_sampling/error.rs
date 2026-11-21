use std::fmt::{Display, Formatter};

use stab_model::{MemoryBudget, MemoryBudgetExceeded, ResourceAmount};
use thiserror::Error;

pub(crate) type DemResult<T> = Result<T, DemError>;

/// DEM-sampling resource dimension whose configured limit was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DemResourceKind {
    ActiveBatchBytes,
    CompiledBytes,
}

/// Typed memory-budget refusal owned by the DEM sampling engine.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DemResourceLimitError {
    kind: DemResourceKind,
    exceeded: MemoryBudgetExceeded,
}

impl DemResourceLimitError {
    pub(crate) const fn active_batch_bytes(actual: u64, budget: MemoryBudget) -> Self {
        Self::new(DemResourceKind::ActiveBatchBytes, actual, budget)
    }

    pub(crate) const fn compiled_bytes(actual: u64, budget: MemoryBudget) -> Self {
        Self::new(DemResourceKind::CompiledBytes, actual, budget)
    }

    const fn new(kind: DemResourceKind, actual: u64, budget: MemoryBudget) -> Self {
        Self {
            kind,
            exceeded: MemoryBudgetExceeded::new(ResourceAmount::exact(actual), budget),
        }
    }

    pub const fn kind(&self) -> DemResourceKind {
        self.kind
    }

    /// The memory-budget refusal; every DEM sampling resource limit is a memory budget.
    pub const fn memory_budget_exceeded(&self) -> &MemoryBudgetExceeded {
        &self.exceeded
    }

    pub const fn actual(&self) -> u64 {
        self.exceeded.required().value()
    }

    pub const fn limit(&self) -> u64 {
        self.exceeded.budget().bytes()
    }
}

impl Display for DemResourceLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let (actual, limit) = (self.actual(), self.limit());
        match self.kind {
            DemResourceKind::ActiveBatchBytes => write!(
                formatter,
                "cannot compile DEM sampler: the sampling session would require at least {actual} bytes, exceeding the {limit}-byte memory budget; raise the memory budget to admit it"
            ),
            DemResourceKind::CompiledBytes => write!(
                formatter,
                "cannot compile DEM sampler: compiled DEM program requires {actual} bytes, exceeding the {limit}-byte memory budget; raise the memory budget to admit it"
            ),
        }
    }
}

impl std::error::Error for DemResourceLimitError {}

/// Semantic failure while compiling, validating, or replaying DEM samples.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum DemError {
    #[error(transparent)]
    Model(#[from] stab_model::ModelError),

    #[error("cannot compile DEM sampler: {message}")]
    InvalidSamplerCompilation { message: String },

    #[error("invalid result format data: {message}")]
    InvalidResultFormat { message: String },

    #[error(transparent)]
    ResourceLimit(#[from] DemResourceLimitError),

    /// The allocator refused storage that admission had already accepted.
    #[error("cannot compile DEM sampler: {message}")]
    StorageAllocation { message: String },

    #[error(transparent)]
    Cancelled(#[from] stab_model::Cancelled),
}

impl DemError {
    /// The memory-budget refusal this error reports, from the DEM sampling engine or the model
    /// layer.
    pub fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match self {
            Self::Model(error) => error.memory_budget_exceeded(),
            Self::ResourceLimit(error) => Some(error.memory_budget_exceeded()),
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
