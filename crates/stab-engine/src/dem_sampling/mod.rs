mod bit_plane;
mod buffers;
mod error;
mod limits;
mod plan;
mod program;
mod session;

pub use error::{DemError, DemResourceKind, DemResourceLimitError};
pub use limits::DemSamplerLimits;
pub use plan::{DemSamplingCompiler, DemSamplingPlan};
pub use session::{
    DemReplayBatchStatus, DemReplaySession, DemReplayTransaction, DemSamplingCancellation,
    DemSamplingExecutionError, DemSamplingRunError, DemSamplingRunProgress, DemSamplingRunStatus,
    DemSamplingRunSummary, DemSamplingSession,
};

pub(crate) use error::DemResult;

use crate::{CompilationDescriptor, CompilationOperation};

/// Detector-error-model sampling compiler registration.
pub const DEM_SAMPLING_COMPILATION_DESCRIPTOR: CompilationDescriptor = CompilationDescriptor::new(
    CompilationOperation::DemSampling,
    stab_model::ModelDialect::DetectorErrorModel,
    1,
    None,
    false,
);

#[cfg(test)]
mod tests;
