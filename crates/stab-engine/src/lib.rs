//! Backend-neutral engine foundations for Stab.

pub mod fingerprint;
pub mod probability;
pub mod sampling;

pub use fingerprint::{CompilationOperation, CompilationRequestFingerprint};
pub use probability::biased_randomize_bits;
pub use sampling::{
    BackendPreference, PlanFingerprint, RandomPolicy, ReferenceSampleMode, RunError,
    SamplingBackend, SamplingCancellation, SamplingCompileError, SamplingCompileErrorCode,
    SamplingCompiler, SamplingExecutionError, SamplingPlan, SamplingRunProgress, SamplingRunStatus,
    SamplingRunSummary, SamplingSession, Seed, ShotCount, SinkFailurePhase,
    count_determined_measurements,
};
