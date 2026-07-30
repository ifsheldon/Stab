//! Simulator-backed compilation and execution APIs.
//!
//! Execution consumes model values and analysis-owned lowering. It does not own textual codecs,
//! filesystem paths, or CLI policy.

mod circuit_adapters;
mod sampled_flow;

pub use crate::sampling::{
    PlanFingerprint, RandomPolicy, ReferenceSampleMode, RunError, SamplingCancellation,
    SamplingCompileError, SamplingCompileErrorCode, SamplingCompiler, SamplingExecutionError,
    SamplingPlan, SamplingRunProgress, SamplingRunStatus, SamplingRunSummary, SamplingSession,
    Seed, ShotCount, SinkFailurePhase, count_determined_measurements,
};
pub use circuit_adapters::{circuit_reference_sample, circuit_reference_sample_tree};
pub use sampled_flow::sample_if_circuit_has_stabilizer_flows;
pub use stab_engine::{
    DemReplayBatchStatus, DemReplaySession, DemSamplingCancellation, DemSamplingCompiler,
    DemSamplingExecutionError, DemSamplingPlan, DemSamplingRunError, DemSamplingRunProgress,
    DemSamplingRunStatus, DemSamplingRunSummary, DemSamplingSession, DetectionCompileError,
    DetectionExecutionError, DetectionRunError, DetectionRunProgress, DetectionRunStatus,
    DetectionRunSummary, DetectionSamplingCompiler, DetectionSamplingPlan,
    DetectionSamplingSession, MeasurementToDetectionCompiler, MeasurementToDetectionPlan,
    MeasurementToDetectionSession, MeasurementToDetectionSinkAdapter, ReferenceSampleTree,
};

use crate::sampling::CompiledSampler;
