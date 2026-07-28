//! Core circuit, detector error model, and simulator primitives for Stab.

pub mod analysis;
pub mod bits;
mod capabilities;
mod circuit;
mod circuit_detecting_regions;
mod circuit_feedback;
mod circuit_flow;
mod circuit_generation;
mod circuit_inverse;
mod circuit_missing_detectors;
mod circuit_simplify;
mod circuit_tableau;
mod circuit_transforms;
mod compilation_fingerprint;
mod dem;
mod dem_sampler;
mod detection;
mod diagnostics;
mod error;
mod error_matcher;
pub mod execution;
mod fingerprint;
mod gate;
mod ids;
mod matched_error;
mod mbqc_decomposition;
mod model_bytes;
mod model_parse;
mod model_tag;
mod parse_limits;
mod probability_util;
mod resources;
pub mod result_formats;
pub mod result_streaming;
mod sampling;
mod sampling_estimate;
mod sampling_output_compat;
mod source_text;
mod sparse_rev_frame_tracker;
pub mod stabilizers;
mod target;

pub use analysis::{
    GateUnitaryMatrix, InverseQecOptions, TimeReversedForFlowsOptions, circuit_inverse_qec,
    circuit_inverse_qec_with_options, circuit_inverse_unitary, circuit_time_reversed_for_flows,
    circuit_time_reversed_for_flows_with_options, circuit_to_tableau,
    circuit_with_inlined_feedback, decomposed_circuit, simplified_circuit,
    single_qubit_clifford_for_gate,
};
pub use bits::{
    BitBlock, BitError, BitLen, BitMatrix, BitResult, BitSlice, BitVec, BitWordsMut, SparseXorVec,
};
pub use capabilities::{CapabilitySet, CompilationCapability};
pub use circuit::{
    Circuit, CircuitFlattenedInstructionIter, CircuitFlattenedInstructionRevIter,
    CircuitInstruction, CircuitItem, RepeatBlock,
};
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
pub use compilation_fingerprint::{CompilationOperation, CompilationRequestFingerprint};
#[cfg(feature = "ops-contracts")]
#[doc(hidden)]
pub use dem::{__circuit_to_detector_error_model_with_diagnostics, ErrorAnalyzerDiagnostics};
pub use dem::{
    DemDetectorId, DemFlattenLimits, DemFlattenedInstructionIter, DemInstruction,
    DemInstructionKind, DemItem, DemObservableId, DemRepeatBlock, DemTarget, DetectorErrorModel,
    DisjointPauliProbabilities, ErrorAnalyzerOptions, IndependentPauliProbabilities,
    LogicalErrorSearchLimits, SatMaterializationLimits, circuit_to_detector_error_model,
    find_undetectable_logical_error, find_undetectable_logical_error_with_limits,
    independent_to_disjoint_xyz_errors, likeliest_error_sat_problem,
    likeliest_error_sat_problem_with_limits, shortest_error_sat_problem,
    shortest_error_sat_problem_with_limits, shortest_graphlike_undetectable_logical_error,
    shortest_graphlike_undetectable_logical_error_with_limits,
    try_disjoint_to_independent_xyz_errors,
};
pub use dem_sampler::{CompiledDemSampler, DemSamplerLimits};
pub use detection::{
    CompiledDetectionConverter, DetectionConversionLimits, DetectionConversionOptions,
    DetectionConversionOutput, DetectionEventRecord, DetectionObservableOutputMode,
    convert_measurements_to_detection_events, convert_measurements_to_detection_events_with_limits,
    convert_measurements_to_detection_events_with_sweep,
    convert_measurements_to_detection_events_with_sweep_and_limits, detection_record_width,
    detection_record_width_with_limits, measurement_record_count,
    measurement_record_count_with_limits, sample_detection_events,
    sample_detection_events_with_limits, try_for_each_sampled_detection_event,
    try_for_each_sampled_detection_event_with_limits, validate_detection_sampling_circuit,
    validate_detection_sampling_circuit_with_limits, write_detection_records,
    write_observable_records, write_ptb64_detection_records, write_ptb64_observable_records,
};
pub use diagnostics::{
    ByteSpan, DiagnosticSeverity, FormatError, FormatErrorCode, FormatErrorContext, ParseError,
    ParseErrorCode, ParseErrorContext,
};
pub use error::{CircuitError, CircuitResult, ModelError, ModelResult};
pub use error_matcher::explain_errors_from_circuit;
pub use execution::{
    BackendPreference, CompiledSampler, PlanFingerprint, RandomPolicy, ReferenceSampleMode,
    ReferenceSampleTree, RunError, SamplingBackend, SamplingCancellation, SamplingCompileError,
    SamplingCompileErrorCode, SamplingCompiler, SamplingExecutionError, SamplingPlan,
    SamplingRunProgress, SamplingRunStatus, SamplingRunSummary, SamplingSession, Seed, ShotCount,
    SinkFailurePhase, count_determined_measurements, sample_if_circuit_has_stabilizer_flows,
};
pub use fingerprint::{ModelDialect, ModelFingerprint};
#[cfg(feature = "ops-contracts")]
#[doc(hidden)]
pub use gate::{
    __gate_contract_family_names, __gate_contract_statistical_plans,
    __gate_contract_statistical_rejection_boundaries, __gate_contract_surface_names,
    GateContractStatisticalBucket, GateContractStatisticalPlan,
};
pub use gate::{
    Gate, GateArgumentRule, GateCategory, GateDecomposition, GateTargetGroupKind, GateTargetRule,
};
pub use ids::{
    CircuitDetectorId, DemRepeatCount, MeasureRecordOffset, ObservableId, Probability, QubitId,
    RepeatCount,
};
pub use matched_error::{
    CircuitErrorLocation, CircuitErrorLocationStackFrame, CircuitTargetsInsideInstruction,
    DemTargetWithCoords, ExplainedError, FlippedMeasurement, GateTargetWithCoords,
};
pub use mbqc_decomposition::mbqc_decomposition;
pub use parse_limits::{ParseLimits, RepeatNestingLimit, RepeatNestingLimitError, SourceLineLimit};
pub use probability_util::biased_randomize_bits;
pub use resources::{
    Estimate, EstimateClass, ResourceEstimate, ResourceKind, ResourceLimitError, ResourceOperation,
};
pub use result_formats::{
    BitPlane64Batch, BitPlane64BatchView, CodecCapability, CorrectionWidth, DemSampleBatchView,
    DemSampleCodecSink, DemSampleEncodedRecords, DemSampleSink, DetectionBatchView,
    DetectionCodecSink, DetectionSink, DetectorWidth, DetsLayout, DetsResultType, DetsToken,
    EncodedSizeEstimate, MeasurementBatchView, MeasurementCodecSink, MeasurementSink,
    MeasurementWidth, ObservablePredictionBatch, ObservableWidth, PackedShotBatch,
    PackedShotBatchView, RecordEncoding, RecordFormat, SampleFormat, SampledErrorWidth,
};
pub use sampling_estimate::estimate_sampling_request;
pub use stabilizers::{
    CliffordString, CommutingPauliStringIterator, FlexPauliString, Flow, FlowMeasurementIndex,
    PauliBasis, PauliPhase, PauliSign, PauliString, PauliStringIterator, SingleQubitClifford,
    StabilizerError, StabilizerResource, StabilizerResult, Tableau, TableauIterator,
    stabilizers_to_tableau, unitary_to_tableau,
};
pub use target::{Pauli, Target};
