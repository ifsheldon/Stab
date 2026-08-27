//! Simulator-backed compilation and execution APIs.
//!
//! Execution consumes model values and analysis-owned lowering. It does not own textual codecs,
//! filesystem paths, or CLI policy.

pub use stab_engine::{
    CircuitReferenceSampleError, CountDeterminedMeasurementsError, DemReplayBatchStatus,
    DemReplaySession, DemSamplingCancellation, DemSamplingCompiler, DemSamplingExecutionError,
    DemSamplingPlan, DemSamplingRunError, DemSamplingRunProgress, DemSamplingRunStatus,
    DemSamplingRunSummary, DemSamplingSession, DetectionCompileError, DetectionConversionLimits,
    DetectionError, DetectionExecutionError, DetectionRecordLimitSubject, DetectionResourceKind,
    DetectionResourceLimitError, DetectionRunError, DetectionRunProgress, DetectionRunStatus,
    DetectionRunSummary, DetectionSamplingCompiler, DetectionSamplingPlan,
    DetectionSamplingSession, MeasurementToDetectionCompiler, MeasurementToDetectionPlan,
    MeasurementToDetectionSession, MeasurementToDetectionSinkAdapter, PlanFingerprint,
    RandomPolicy, ReferenceSampleMode, ReferenceSampleTree, ReferenceSampleTreeError, RunError,
    SamplingCancellation, SamplingCompileError, SamplingCompileErrorCode, SamplingCompiler,
    SamplingExecutionError, SamplingPlan, SamplingRunProgress, SamplingRunStatus,
    SamplingRunSummary, SamplingSession, Seed, ShotCount, SinkFailurePhase,
    circuit_reference_sample, count_determined_measurements, detection_record_width,
    detection_record_width_with_limits, measurement_record_count,
    measurement_record_count_with_limits, sample_if_circuit_has_stabilizer_flows,
    validate_detection_sampling_circuit, validate_detection_sampling_circuit_with_limits,
};
