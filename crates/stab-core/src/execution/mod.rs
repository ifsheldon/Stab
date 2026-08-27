//! Simulator-backed compilation and execution APIs.
//!
//! Execution consumes model values and analysis-owned lowering. It does not own textual codecs,
//! filesystem paths, or CLI policy.

pub use stab_engine::{
    CircuitReferenceSampleError, CountDeterminedMeasurementsError, DemReplayBatchStatus,
    DemReplaySession, DemSamplingCancellation, DemSamplingCompiler, DemSamplingExecutionError,
    DemSamplingPlan, DemSamplingRunError, DemSamplingRunProgress, DemSamplingRunStatus,
    DemSamplingRunSummary, DemSamplingSession, DetectionCompileError, DetectionExecutionError,
    DetectionRunError, DetectionRunProgress, DetectionRunStatus, DetectionRunSummary,
    DetectionSamplingCompiler, DetectionSamplingPlan, DetectionSamplingSession,
    MeasurementToDetectionCompiler, MeasurementToDetectionPlan, MeasurementToDetectionSession,
    MeasurementToDetectionSinkAdapter, PlanFingerprint, RandomPolicy, ReferenceSampleMode,
    ReferenceSampleTree, ReferenceSampleTreeError, RunError, SamplingCancellation,
    SamplingCompileError, SamplingCompileErrorCode, SamplingCompiler, SamplingExecutionError,
    SamplingPlan, SamplingRunProgress, SamplingRunStatus, SamplingRunSummary, SamplingSession,
    Seed, ShotCount, SinkFailurePhase, circuit_reference_sample, count_determined_measurements,
    sample_if_circuit_has_stabilizer_flows,
};
