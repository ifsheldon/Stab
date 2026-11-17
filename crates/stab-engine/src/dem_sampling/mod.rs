mod bit_plane;
mod buffers;
mod error;
mod limits;
mod plan;
mod program;
mod record;
mod replay;
mod replay_planes;
mod replay_store;
mod session;

pub use error::{DemError, DemResourceKind, DemResourceLimitError};
pub use limits::DemSamplerLimits;
pub use plan::{DemSamplingCompiler, DemSamplingPlan};
pub use replay_store::{DemReplayRecordStore, DemReplayRetention};
pub use session::{
    DemReplayBatchStatus, DemReplaySession, DemReplayTransaction, DemSamplingExecutionError,
    DemSamplingRunError, DemSamplingRunProgress, DemSamplingRunStatus, DemSamplingRunSummary,
    DemSamplingSession,
};

pub(crate) use error::DemResult;

use crate::{CompilationDescriptor, CompilationOperation};

/// Detector-error-model sampling compiler registration.
///
/// Schema `2` marks the sampled-error stream that leaves seeded detector and observable output
/// unchanged by recording sampled errors.
pub const DEM_SAMPLING_COMPILATION_DESCRIPTOR: CompilationDescriptor = CompilationDescriptor::new(
    CompilationOperation::DemSampling,
    stab_model::ModelDialect::DetectorErrorModel,
    2,
    None,
    false,
);

#[cfg(test)]
mod tests;
