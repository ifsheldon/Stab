#![feature(portable_simd)]

//! Core circuit, detector error model, and simulator primitives for Stab.

pub mod bits;
mod circuit;
mod circuit_flow;
mod circuit_generation;
mod circuit_inverse;
mod circuit_simplify;
mod circuit_tableau;
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
mod sampling;
mod sparse_rev_frame_tracker;
pub mod stabilizers;
mod target;

pub use bits::{BitBlock, BitError, BitLen, BitMatrix, BitResult, BitSlice, BitVec, SparseXorVec};
pub use circuit::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock};
pub use circuit_flow::{check_if_circuit_has_unsigned_stabilizer_flows, circuit_flow_generators};
pub use circuit_generation::{
    CodeDistance, ColorCodeParams, ColorCodeTask, GeneratedCircuit, RepetitionCodeParams,
    RepetitionCodeTask, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
};
pub use circuit_inverse::{circuit_inverse_qec, circuit_inverse_unitary};
pub use circuit_simplify::simplified_circuit;
pub use circuit_tableau::circuit_to_tableau;
pub use dem::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemObservableId, DemRepeatBlock,
    DemTarget, DetectorErrorModel, DisjointPauliProbabilities, ErrorAnalyzerOptions,
    IndependentPauliProbabilities, circuit_to_detector_error_model,
    find_undetectable_logical_error, independent_to_disjoint_xyz_errors,
    likeliest_error_sat_problem, shortest_error_sat_problem,
    shortest_graphlike_undetectable_logical_error, try_disjoint_to_independent_xyz_errors,
};
pub use dem_sampler::CompiledDemSampler;
pub use detection::{
    DetectionConversionOptions, DetectionConversionOutput, DetectionEventRecord,
    DetectionObservableOutputMode, convert_measurements_to_detection_events,
    detection_record_width, measurement_record_count, sample_detection_events,
    validate_detection_sampling_circuit, write_detection_records, write_observable_records,
    write_ptb64_detection_records, write_ptb64_observable_records,
};
pub use error::{CircuitError, CircuitResult};
pub use error_matcher::explain_errors_from_circuit;
pub use gate::{Gate, GateCategory};
pub use ids::{MeasureRecordOffset, ObservableId, Probability, QubitId, RepeatCount};
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
