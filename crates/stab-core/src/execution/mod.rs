//! Simulator-backed compilation and execution APIs.
//!
//! Execution consumes model values and analysis-owned lowering. It does not own textual codecs,
//! filesystem paths, or CLI policy.

mod circuit_adapters;
mod reference_sample_tree;
mod sampled_flow;

pub use crate::sampling::{
    BackendPreference, CompiledSampler, PlanFingerprint, RandomPolicy, ReferenceSampleMode,
    RunError, SamplingBackend, SamplingCancellation, SamplingCompileError,
    SamplingCompileErrorCode, SamplingCompiler, SamplingExecutionError, SamplingPlan,
    SamplingRunProgress, SamplingRunStatus, SamplingRunSummary, SamplingSession, Seed, ShotCount,
    SinkFailurePhase, count_determined_measurements,
};
pub use circuit_adapters::{circuit_reference_sample, circuit_reference_sample_tree};
pub use reference_sample_tree::ReferenceSampleTree;
pub use sampled_flow::sample_if_circuit_has_stabilizer_flows;
