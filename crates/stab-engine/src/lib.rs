//! Backend-neutral engine foundations for Stab.

pub mod fingerprint;
pub mod probability;
mod sampling;

pub use fingerprint::{CompilationOperation, CompilationRequestFingerprint};
pub use probability::biased_randomize_bits;
pub use sampling::{
    BackendPreference, PlanFingerprint, RandomPolicy, ReferenceSampleMode, RunError,
    SamplingBackend, SamplingCancellation, SamplingCompilationDescriptor, SamplingCompileError,
    SamplingCompileErrorCode, SamplingCompiler, SamplingExecutionError, SamplingPlan,
    SamplingRunProgress, SamplingRunStatus, SamplingRunSummary, SamplingSession, Seed, ShotCount,
    SinkFailurePhase, count_determined_measurements,
};
pub use sampling::{COMPILATION_DESCRIPTOR, REGISTERED_BACKENDS};

#[doc(hidden)]
pub use sampling::{ReferenceSampleScratch, normalize_pauli_product_terms_for_core_detection};
