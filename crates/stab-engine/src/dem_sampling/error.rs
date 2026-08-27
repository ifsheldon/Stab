use std::fmt::{Display, Formatter};

use thiserror::Error;

pub(crate) type DemResult<T> = Result<T, DemError>;

/// DEM-sampling resource dimension whose configured limit was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DemResourceKind {
    SampledErrorApplications,
    ReplayWorkUnits,
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
    pub(crate) const fn sampled_error_applications(actual: usize, limit: usize) -> Self {
        Self::new(
            DemResourceKind::SampledErrorApplications,
            actual as u64,
            limit as u64,
        )
    }

    pub(crate) const fn replay_work_units(actual: usize, limit: usize) -> Self {
        Self::new(
            DemResourceKind::ReplayWorkUnits,
            actual as u64,
            limit as u64,
        )
    }

    pub(crate) const fn active_batch_bytes(actual: usize, limit: usize) -> Self {
        Self::new(
            DemResourceKind::ActiveBatchBytes,
            actual as u64,
            limit as u64,
        )
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
            DemResourceKind::SampledErrorApplications => write!(
                formatter,
                "cannot compile circuit sampler: DEM sampler would apply {} sampled errors; current limit is {}",
                self.actual, self.limit
            ),
            DemResourceKind::ReplayWorkUnits => write!(
                formatter,
                "cannot compile circuit sampler: DEM sampler would require {} replay work units; current limit is {}",
                self.actual, self.limit
            ),
            DemResourceKind::ActiveBatchBytes => write!(
                formatter,
                "cannot compile circuit sampler: DEM sampling session would require at least {} active batch bytes; current limit is {}",
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

    #[error("cannot compile circuit sampler: {message}")]
    InvalidSamplerCompilation { message: String },

    #[error("invalid result format data: {message}")]
    InvalidResultFormat { message: String },

    #[error(transparent)]
    ResourceLimit(#[from] DemResourceLimitError),
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
