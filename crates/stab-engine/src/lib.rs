//! Backend-neutral engine foundations for Stab.

mod bernoulli;
mod dem_sampling;
mod descriptor;
mod detection;
mod fingerprint;
mod lane_width;
mod reference_sample_tree;
mod run;
mod sampled_flow;
mod sampling;
mod sampling_estimate;
mod work_poll;

pub use dem_sampling::{
    DEM_SAMPLING_COMPILATION_DESCRIPTOR, DemError, DemReplayRecordStore, DemReplayRetention,
    DemReplaySession, DemReplaySessionOptions, DemReplayStoreState, DemReplayTransaction,
    DemResourceKind, DemResourceLimitError, DemSamplingCompiler, DemSamplingExecutionError,
    DemSamplingPlan, DemSamplingSession, DemSamplingSessionOptions,
};
pub use descriptor::CompilationDescriptor;
pub(crate) use detection::DetectionRecordBuffer;
pub use detection::{
    CircuitReferenceSigns, DETECTION_SAMPLING_COMPILATION_DESCRIPTOR, DetectionCompileError,
    DetectionConversionLimits, DetectionError, DetectionExecutionError, DetectionResourceKind,
    DetectionResourceLimitError, DetectionSamplingCompiler, DetectionSamplingPlan,
    DetectionSamplingSession, DetectionSamplingSessionOptions,
    MEASUREMENT_TO_DETECTION_COMPILATION_DESCRIPTOR, MeasurementToDetectionCompiler,
    MeasurementToDetectionPlan, MeasurementToDetectionSession,
    MeasurementToDetectionSessionOptions, MeasurementToDetectionTransaction, StreamingDetectionRun,
    circuit_reference_signs, detection_record_width,
};
pub use fingerprint::{CompilationOperation, CompilationRequestFingerprint};
pub use lane_width::LaneWidth;
pub use reference_sample_tree::{ReferenceSampleTree, ReferenceSampleTreeError};
pub use run::{RunError, RunProgress, RunStatus, RunSummary, SinkFailurePhase};
pub use sampled_flow::{
    SampledFlowError, SampledFlowOptions, sample_if_circuit_has_stabilizer_flows,
};
pub use sampling::COMPILATION_DESCRIPTOR;
pub use sampling::{
    InitialState, PlanFingerprint, RandomPolicy, ReferenceSample, ReferenceSampleLoopPolicy,
    ReferenceSampleMode, SamplingBackend, SamplingCompileError, SamplingCompiler,
    SamplingExecutionError, SamplingPlan, SamplingSession, SamplingSessionOptions, Seed, ShotCount,
    StreamingSamplingRun, SweepRecord,
};
pub use sampling_estimate::estimate_sampling_request;

/// Most shots any session delivers to a sink in one batch, and most shots one
/// measurement-to-detection call accepts.
///
/// Circuit sampling, detection sampling, and DEM sampling deliver bit-plane batches of at most
/// this many shots, the widest [`LaneWidth`]; a sink that preallocates per-batch storage, such as
/// decoder predictions, can size it once from this bound.
pub const MAX_BATCH_SHOTS: usize = detection::frame::FRAME_BATCH_SHOTS;

/// Compiler registrations exposed through product capability discovery.
pub const COMPILATION_DESCRIPTORS: &[CompilationDescriptor] = &[
    COMPILATION_DESCRIPTOR,
    MEASUREMENT_TO_DETECTION_COMPILATION_DESCRIPTOR,
    DETECTION_SAMPLING_COMPILATION_DESCRIPTOR,
    DEM_SAMPLING_COMPILATION_DESCRIPTOR,
];
