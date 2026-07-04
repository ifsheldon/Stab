#![feature(portable_simd)]

//! Core circuit, detector error model, and simulator primitives for Stab.

pub mod bits;
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
mod dem;
mod dem_sampler;
mod detection;
mod error;
mod error_matcher;
mod gate;
mod ids;
mod matched_error;
mod mbqc_decomposition;
mod probability_util;
mod reference_sample_tree;
pub mod result_formats;
pub mod result_streaming;
mod sampling;
mod sparse_rev_frame_tracker;
pub mod stabilizers;
mod target;

pub use bits::{BitBlock, BitError, BitLen, BitMatrix, BitResult, BitSlice, BitVec, SparseXorVec};
pub use circuit::{
    Circuit, CircuitFlattenedInstructionIter, CircuitFlattenedInstructionRevIter,
    CircuitInstruction, CircuitItem, RepeatBlock,
};
pub use circuit_detecting_regions::{
    DetectingRegionMap, DetectingRegionOptions, circuit_detecting_regions,
};
pub use circuit_feedback::circuit_with_inlined_feedback;
pub use circuit_flow::{
    check_if_circuit_has_unsigned_stabilizer_flows, circuit_flow_generators,
    solve_for_flow_measurements,
};
pub use circuit_generation::{
    CodeDistance, ColorCodeParams, ColorCodeTask, GeneratedCircuit, RepetitionCodeParams,
    RepetitionCodeTask, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
};
pub use circuit_inverse::{circuit_inverse_qec, circuit_inverse_unitary};
pub use circuit_missing_detectors::{MissingDetectorOptions, missing_detectors};
pub use circuit_simplify::{decomposed_circuit, simplified_circuit};
pub use circuit_tableau::circuit_to_tableau;
pub use dem::{
    DemDetectorId, DemFlattenedInstructionIter, DemInstruction, DemInstructionKind, DemItem,
    DemObservableId, DemRepeatBlock, DemTarget, DetectorErrorModel, DisjointPauliProbabilities,
    ErrorAnalyzerOptions, IndependentPauliProbabilities, circuit_to_detector_error_model,
    find_undetectable_logical_error, independent_to_disjoint_xyz_errors,
    likeliest_error_sat_problem, shortest_error_sat_problem,
    shortest_graphlike_undetectable_logical_error, try_disjoint_to_independent_xyz_errors,
};
pub use dem_sampler::CompiledDemSampler;
pub use detection::{
    CompiledDetectionConverter, DetectionConversionOptions, DetectionConversionOutput,
    DetectionEventRecord, DetectionObservableOutputMode, convert_measurements_to_detection_events,
    convert_measurements_to_detection_events_with_sweep, detection_record_width,
    measurement_record_count, sample_detection_events, try_for_each_sampled_detection_event,
    validate_detection_sampling_circuit, write_detection_records, write_observable_records,
    write_ptb64_detection_records, write_ptb64_observable_records,
};
pub use error::{CircuitError, CircuitResult};
pub use error_matcher::explain_errors_from_circuit;
pub use gate::{
    Gate, GateArgumentRule, GateCategory, GateDecomposition, GateTargetGroupKind, GateTargetRule,
    GateUnitaryMatrix,
};
pub use ids::{
    CircuitDetectorId, MeasureRecordOffset, ObservableId, Probability, QubitId, RepeatCount,
};
pub use matched_error::{
    CircuitErrorLocation, CircuitErrorLocationStackFrame, CircuitTargetsInsideInstruction,
    DemTargetWithCoords, ExplainedError, FlippedMeasurement, GateTargetWithCoords,
};
pub use mbqc_decomposition::mbqc_decomposition;
pub use probability_util::biased_randomize_bits;
pub use reference_sample_tree::ReferenceSampleTree;
pub use result_formats::SampleFormat;
pub use sampling::{CompiledSampler, count_determined_measurements};
pub use stabilizers::{
    CliffordString, CommutingPauliStringIterator, FlexPauliString, Flow, PauliBasis, PauliPhase,
    PauliSign, PauliString, PauliStringIterator, SingleQubitClifford, StabilizerError,
    StabilizerResult, Tableau, TableauIterator, stabilizers_to_tableau, unitary_to_tableau,
};
pub use target::{Pauli, Target};
