//! Stable pure transforms and semantic analysis over Stab models.
//!
//! This crate owns operations that combine model syntax with stabilizer algebra without creating
//! mutable execution sessions or importing result codecs, filesystem policy, CLI types, or ops
//! contracts.

pub mod circuit;
mod circuit_detecting_regions;
mod circuit_feedback;
mod circuit_flow;
mod circuit_generation;
mod circuit_inverse;
mod circuit_missing_detectors;
mod circuit_pass;
mod circuit_pauli;
mod circuit_simplify;
mod circuit_tableau;
mod circuit_to_dem;
mod circuit_transforms;
mod dem;
mod error;
mod error_matcher;
pub mod gate;
mod matched_error;
mod resources;
mod sparse_rev_frame_tracker;
mod tableau_circuit;

pub use circuit::circuit_without_tags;
pub use circuit_detecting_regions::{
    DetectingRegionMap, DetectingRegionOptions, DetectingRegionTargetMap,
    DetectingRegionTargetOptions, all_detecting_region_targets, all_detecting_region_ticks,
    circuit_detecting_regions, circuit_detecting_regions_for_targets,
};
pub use circuit_feedback::circuit_with_inlined_feedback;
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
pub use circuit_inverse::{
    InverseQecOptions, TimeReversedForFlowsOptions, circuit_inverse_qec,
    circuit_inverse_qec_with_options, circuit_inverse_unitary, circuit_time_reversed_for_flows,
    circuit_time_reversed_for_flows_with_options,
};
pub use circuit_missing_detectors::{MissingDetectorOptions, missing_detectors};
pub use circuit_pass::{
    CircuitPass, CircuitPassContext, CircuitPassError, CircuitPassInput, CircuitPassLimits,
    CircuitPassOutput, CircuitPassProjectionError, CircuitPassResources, run_circuit_pass,
};
pub use circuit_pauli::{pauli_after_circuit, pauli_before_circuit};
pub use circuit_simplify::decomposed_circuit;
pub use circuit_tableau::circuit_to_tableau;
pub use circuit_to_dem::{
    DisjointPauliProbabilities, ErrorAnalyzerOptions, IndependentPauliProbabilities,
    circuit_to_detector_error_model, independent_to_disjoint_xyz_errors,
    try_disjoint_to_independent_xyz_errors,
};
pub use circuit_transforms::{
    CircuitFlattenLimits, WithoutNoiseOptions, WithoutNoisePass, WithoutNoiseReport,
    circuit_without_noise, flattened_circuit, flattened_circuit_operations,
    flattened_circuit_operations_with_limits, flattened_circuit_with_limits,
};
pub use dem::{
    DemFlattenLimits, LogicalErrorSearchLimits, SatMaterializationLimits,
    detector_error_model_without_tags, find_undetectable_logical_error,
    find_undetectable_logical_error_with_limits, flattened_detector_error_model,
    flattened_detector_error_model_with_limits, likeliest_error_sat_problem,
    likeliest_error_sat_problem_with_limits, rounded_detector_error_model,
    shortest_error_sat_problem, shortest_error_sat_problem_with_limits,
    shortest_graphlike_undetectable_logical_error,
    shortest_graphlike_undetectable_logical_error_with_limits,
};
pub use error::{AnalysisError, AnalysisResult};
pub use error_matcher::explain_errors_from_circuit;
pub use gate::{
    GateUnitaryMatrix, gate_decomposition_to_circuit, gate_flows, gate_h_s_cx_m_r_decomposition,
    gate_has_flows, gate_has_h_s_cx_m_r_decomposition, gate_has_tableau, gate_has_unitary_matrix,
    gate_tableau, gate_unitary_matrix, single_qubit_clifford_for_gate,
};
pub use matched_error::{
    CircuitErrorLocation, CircuitErrorLocationStackFrame, CircuitTargetsInsideInstruction,
    DemTargetWithCoords, ExplainedError, FlippedMeasurement, GateTargetWithCoords,
    canonicalize_circuit_error_location_parts, canonicalize_dem_error_terms,
};
pub use resources::{CircuitPassStage, ResourceKind, ResourceLimitError, ResourceOperation};
pub use tableau_circuit::tableau_to_circuit;

/// Low-level lowering operations shared with compilation engines.
pub mod advanced {
    pub use crate::circuit_flow::flow_record_index;
    pub use crate::circuit_simplify::{
        decomposed_single_instruction, visit_decomposed_spp_instructions,
    };
    pub use crate::matched_error::{
        CircuitErrorLocationView, CircuitTargetsInsideInstructionView, write_explained_error,
    };
}
