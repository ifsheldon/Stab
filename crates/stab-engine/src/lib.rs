//! Backend-neutral engine foundations for Stab.

mod dem_sampling;
mod descriptor;
mod detection;
mod fingerprint;
mod probability;
mod reference_sample_tree;
mod sampled_flow;
mod sampling;

pub use dem_sampling::{
    DEM_SAMPLING_COMPILATION_DESCRIPTOR, DemError, DemReplayBatchStatus, DemReplaySession,
    DemResourceKind, DemResourceLimitError, DemSamplerLimits, DemSamplingCancellation,
    DemSamplingCompiler, DemSamplingExecutionError, DemSamplingPlan, DemSamplingRunError,
    DemSamplingRunProgress, DemSamplingRunStatus, DemSamplingRunSummary, DemSamplingSession,
};
pub use descriptor::CompilationDescriptor;
pub use detection::{
    CompiledDetectionConverter, DETECTION_SAMPLING_COMPILATION_DESCRIPTOR, DetectionCompileError,
    DetectionConversionLimits, DetectionConversionOptions, DetectionError, DetectionEventRecord,
    DetectionExecutionError, DetectionRecordLimitSubject, DetectionResourceKind,
    DetectionResourceLimitError, DetectionRunError, DetectionRunProgress, DetectionRunStatus,
    DetectionRunSummary, DetectionSamplingCompiler, DetectionSamplingPlan,
    DetectionSamplingSession, MEASUREMENT_TO_DETECTION_COMPILATION_DESCRIPTOR,
    MeasurementToDetectionCompiler, MeasurementToDetectionPlan, MeasurementToDetectionSession,
    MeasurementToDetectionSinkAdapter, detection_record_width, detection_record_width_with_limits,
    measurement_record_count, measurement_record_count_with_limits,
    validate_detection_sampling_circuit, validate_detection_sampling_circuit_with_limits,
};
pub use fingerprint::{CompilationOperation, CompilationRequestFingerprint};
pub use probability::biased_randomize_bits;
pub use reference_sample_tree::{ReferenceSampleTree, ReferenceSampleTreeError};
pub use sampled_flow::{SampledFlowError, sample_if_circuit_has_stabilizer_flows};
pub use sampling::COMPILATION_DESCRIPTOR;
pub use sampling::{
    CountDeterminedMeasurementsError, PlanFingerprint, RandomPolicy, ReferenceSampleMode, RunError,
    SamplingBackend, SamplingCancellation, SamplingCompileError, SamplingCompileErrorCode,
    SamplingCompiler, SamplingExecutionError, SamplingPlan, SamplingRunProgress, SamplingRunStatus,
    SamplingRunSummary, SamplingSession, Seed, ShotCount, SinkFailurePhase,
    count_determined_measurements,
};

/// Compiler registrations exposed through product capability discovery.
pub const COMPILATION_DESCRIPTORS: &[CompilationDescriptor] = &[
    COMPILATION_DESCRIPTOR,
    MEASUREMENT_TO_DETECTION_COMPILATION_DESCRIPTOR,
    DETECTION_SAMPLING_COMPILATION_DESCRIPTOR,
    DEM_SAMPLING_COMPILATION_DESCRIPTOR,
];
