use std::fmt::{Display, Formatter};

use thiserror::Error;

pub(crate) type DemResult<T> = Result<T, DemError>;

/// DEM-sampling resource dimension whose configured limit was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DemResourceKind {
    ActiveBatchBytes,
}

/// Typed resource-admission failure owned by the DEM sampling engine.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DemResourceLimitError {
    kind: DemResourceKind,
    actual: u64,
    limit: u64,
}

impl DemResourceLimitError {
    pub(crate) const fn active_batch_bytes(actual: u64, limit: u64) -> Self {
        Self::new(DemResourceKind::ActiveBatchBytes, actual, limit)
    }

    const fn new(kind: DemResourceKind, actual: u64, limit: u64) -> Self {
        Self {
            kind,
            actual,
            limit,
        }
    }

    pub const fn kind(&self) -> DemResourceKind {
        self.kind
    }

    pub const fn actual(&self) -> u64 {
        self.actual
    }

    pub const fn limit(&self) -> u64 {
        self.limit
    }
}

impl Display for DemResourceLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            DemResourceKind::ActiveBatchBytes => write!(
                formatter,
                "cannot compile DEM sampler: the sampling session would require at least {} bytes, exceeding the {}-byte memory budget; raise the memory budget to admit it",
                self.actual, self.limit
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

    #[error(transparent)]
    Cancelled(#[from] stab_model::Cancelled),
}

impl DemError {
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
