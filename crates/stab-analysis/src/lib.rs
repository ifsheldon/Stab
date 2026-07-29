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
mod circuit_simplify;
mod circuit_tableau;
mod circuit_transforms;
mod error;
pub mod gate;
mod mbqc_decomposition;
mod resources;
mod sparse_rev_frame_tracker;

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
pub use circuit_simplify::{decomposed_circuit, simplified_circuit};
pub use circuit_tableau::circuit_to_tableau;
pub use circuit_transforms::{
    CircuitFlattenLimits, circuit_without_noise, flattened_circuit, flattened_circuit_operations,
    flattened_circuit_operations_with_limits, flattened_circuit_with_limits,
};
pub use error::{AnalysisError, AnalysisResult};
pub use gate::{
    GateUnitaryMatrix, gate_decomposition_to_circuit, gate_flows, gate_h_s_cx_m_r_decomposition,
    gate_has_flows, gate_has_h_s_cx_m_r_decomposition, gate_has_tableau, gate_has_unitary_matrix,
    gate_tableau, gate_unitary_matrix, single_qubit_clifford_for_gate,
};
pub use mbqc_decomposition::mbqc_decomposition;
pub use resources::{ResourceKind, ResourceLimitError, ResourceOperation};

/// Low-level lowering operations shared with compilation engines.
pub mod advanced {
    pub use crate::circuit_flow::transitions::{ReverseFlowTransition, reverse_flow_transition};
    pub use crate::circuit_flow::{check_unsigned_flows_with_sparse_tracker, flow_record_index};
    pub use crate::circuit_simplify::decomposed_single_instruction;
    pub use crate::sparse_rev_frame_tracker::{
        AnalyzerProbeBudget, ShiftedRecurrence, ShiftedRecurrenceSearch, SparseReverseFrameTracker,
        search_shifted_recurrence,
    };
}
