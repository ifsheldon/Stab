//! Backend-neutral engine foundations for Stab.

mod dem_sampling;
mod detection;
pub mod fingerprint;
pub mod probability;
mod sampling;

pub use dem_sampling::{
    DemError, DemReplayBatchStatus, DemReplaySession, DemResourceKind, DemResourceLimitError,
    DemSamplerLimits, DemSamplingCancellation, DemSamplingCompiler, DemSamplingExecutionError,
    DemSamplingPlan, DemSamplingRunError, DemSamplingRunProgress, DemSamplingRunStatus,
    DemSamplingRunSummary, DemSamplingSession,
};
pub use detection::{
    CompiledDetectionConverter, DetectionCompileError, DetectionConversionLimits,
    DetectionConversionOptions, DetectionError, DetectionEventRecord, DetectionExecutionError,
    DetectionRecordLimitSubject, DetectionResourceKind, DetectionResourceLimitError,
    DetectionRunError, DetectionRunProgress, DetectionRunStatus, DetectionRunSummary,
    DetectionSamplingCompiler, DetectionSamplingPlan, DetectionSamplingSession,
    MeasurementToDetectionCompiler, MeasurementToDetectionPlan, MeasurementToDetectionSession,
    MeasurementToDetectionSinkAdapter, detection_record_width, detection_record_width_with_limits,
    measurement_record_count, measurement_record_count_with_limits,
    validate_detection_sampling_circuit, validate_detection_sampling_circuit_with_limits,
};
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
