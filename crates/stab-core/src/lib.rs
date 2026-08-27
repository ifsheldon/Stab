//! Core circuit, detector error model, and simulator primitives for Stab.

pub mod advanced;
pub mod analysis;
mod capabilities;
mod circuit_detecting_regions;
mod circuit_feedback;
mod circuit_flow;
mod circuit_generation;
mod circuit_inverse;
mod circuit_missing_detectors;
mod circuit_simplify;
mod circuit_tableau;
mod circuit_transforms;
mod dem;
mod dem_sampler;
mod diagnostics;
mod error;
mod error_matcher;
pub mod execution;
pub mod experimental;
mod gate;
mod matched_error;
mod mbqc_decomposition;
mod resources;
mod result_formats;
mod result_streaming;
mod sampling_estimate;

pub use analysis::{
    GateUnitaryMatrix, InverseQecOptions, TimeReversedForFlowsOptions, circuit_inverse_qec,
    circuit_inverse_qec_with_options, circuit_inverse_unitary, circuit_time_reversed_for_flows,
    circuit_time_reversed_for_flows_with_options, circuit_to_tableau,
    circuit_with_inlined_feedback, decomposed_circuit, simplified_circuit,
    single_qubit_clifford_for_gate,
};
pub use capabilities::{CapabilitySet, CompilationCapability};
pub use circuit_detecting_regions::{
    DetectingRegionMap, DetectingRegionOptions, DetectingRegionTargetMap,
    DetectingRegionTargetOptions, all_detecting_region_targets, all_detecting_region_ticks,
    circuit_detecting_regions, circuit_detecting_regions_for_targets,
};
pub use circuit_flow::{
    UnsignedStabilizerFlowCheck, UnsignedStabilizerFlowFailure,
    check_if_circuit_has_unsigned_stabilizer_flows,
    check_unsigned_stabilizer_flows_with_diagnostics, circuit_flow_generators,
    circuit_has_all_unsigned_stabilizer_flows, circuit_has_unsigned_stabilizer_flow,
    solve_for_flow_measurements,
};
pub use circuit_generation::{
    CodeDistance, ColorCodeParams, ColorCodeTask, GeneratedCircuit, RepetitionCodeParams,
    RepetitionCodeTask, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
};
pub use circuit_missing_detectors::{MissingDetectorOptions, missing_detectors};
pub use circuit_transforms::CircuitFlattenLimits;
pub use dem::{
    DemDetectorId, DemFlattenLimits, DemInstruction, DemInstructionKind, DemItem, DemObservableId,
    DemRepeatBlock, DemTarget, DetectorErrorModel, DisjointPauliProbabilities,
    ErrorAnalyzerOptions, IndependentPauliProbabilities, LogicalErrorSearchLimits,
    SatMaterializationLimits, circuit_to_detector_error_model, find_undetectable_logical_error,
    find_undetectable_logical_error_with_limits, independent_to_disjoint_xyz_errors,
    likeliest_error_sat_problem, likeliest_error_sat_problem_with_limits,
    shortest_error_sat_problem, shortest_error_sat_problem_with_limits,
    shortest_graphlike_undetectable_logical_error,
    shortest_graphlike_undetectable_logical_error_with_limits,
    try_disjoint_to_independent_xyz_errors,
};
pub use dem_sampler::DemSamplerLimits;
pub use diagnostics::{
    ByteSpan, DiagnosticSeverity, FormatError, FormatErrorCode, FormatErrorContext, ParseError,
    ParseErrorCode, ParseErrorContext,
};
pub use error::{
    CircuitError, CircuitResult, ModelError, ModelResult, ValidationError, ValidationErrorCode,
};
pub use error_matcher::explain_errors_from_circuit;
pub use execution::{CircuitReferenceSampleError, CountDeterminedMeasurementsError};
pub use execution::{
    DetectionConversionLimits, PlanFingerprint, RandomPolicy, ReferenceSampleMode,
    ReferenceSampleTree, ReferenceSampleTreeError, RunError, SamplingCancellation,
    SamplingCompileError, SamplingCompileErrorCode, SamplingCompiler, SamplingExecutionError,
    SamplingPlan, SamplingRunProgress, SamplingRunStatus, SamplingRunSummary, SamplingSession,
    Seed, ShotCount, SinkFailurePhase, circuit_reference_sample, count_determined_measurements,
    detection_record_width, detection_record_width_with_limits, measurement_record_count,
    measurement_record_count_with_limits, sample_if_circuit_has_stabilizer_flows,
    validate_detection_sampling_circuit, validate_detection_sampling_circuit_with_limits,
};
pub use matched_error::{
    CircuitErrorLocation, CircuitErrorLocationStackFrame, CircuitTargetsInsideInstruction,
    DemTargetWithCoords, ExplainedError, FlippedMeasurement, GateTargetWithCoords,
};
pub use mbqc_decomposition::mbqc_decomposition;
pub use resources::{
    Estimate, EstimateClass, ResourceEstimate, ResourceKind, ResourceLimitError, ResourceOperation,
};
pub use result_formats::{
    BitPlane64Batch, BitPlane64BatchView, CodecCapability, CorrectionWidth, DemSampleBatchView,
    DemSampleSink, DetectionBatchView, DetectionSink, DetectorWidth, EncodedSizeEstimate,
    MeasurementBatchView, MeasurementSink, MeasurementWidth, ObservablePredictionBatch,
    ObservableWidth, PackedShotBatch, PackedShotBatchView, RecordEncoding, RecordFormat,
    SampledErrorWidth,
};
pub use sampling_estimate::estimate_sampling_request;
pub use stab_algebra::{
    CliffordString, FlexPauliString, Flow, FlowMeasurementIndex, PauliBasis, PauliPhase, PauliSign,
    PauliString, SingleQubitClifford, StabilizerError, StabilizerResource, StabilizerResult,
    Tableau, stabilizers_to_tableau, unitary_to_tableau,
};
pub use stab_decoder::{
    DecodeBatchError, DecodeBatchStatus, DecodeBatchSummary, DecodeCancellation,
    DecodeContractError, DecodePreflightError, DecodeSessionFailure, DecoderInputBatchView,
    DecoderLayout, DecoderModelView, DecoderModelViewError, DecoderSession, ValidatedDecodeBatch,
    decode_batch,
};
pub use stab_engine::{CompilationOperation, CompilationRequestFingerprint, biased_randomize_bits};
pub use stab_model::{
    Circuit, CircuitDetectorId, CircuitInstruction, CircuitItem, DemRepeatCount, Gate,
    GateArgumentRule, GateCategory, GateDecomposition, GateTargetGroupKind, GateTargetRule,
    MeasureRecordOffset, ModelDialect, ModelFingerprint, ObservableId, ParseLimits, Pauli,
    Probability, ProbabilityStimText, QubitId, RepeatBlock, RepeatCount, RepeatNestingLimit,
    RepeatNestingLimitError, SourceLineLimit, Target,
};

pub(crate) use dem_sampler::DetectionEventRecord;
pub(crate) use result_formats::{DetsLayout, DetsToken};
pub(crate) use stab_bits::BitSlice;
