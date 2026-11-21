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
mod controlled_pauli;
mod dem;
mod error;
mod folded_walk;
pub mod gate;
mod instruction_shape;
mod resources;
mod sparse_rev_frame_tracker;
mod tableau_circuit;
mod target_xor_set;

pub use circuit::circuit_without_tags;
pub use circuit_detecting_regions::{
    DetectingRegionListOptions, DetectingRegionMap, DetectingRegionOptions,
    DetectingRegionTargetMap, all_detecting_region_targets, all_detecting_region_ticks,
    circuit_detecting_regions, circuit_detecting_regions_for_targets,
};
pub use circuit_feedback::{FeedbackInliningOptions, circuit_with_inlined_feedback};
pub use circuit_flow::{
    FlowGeneratorOptions, FlowMeasurementOptions, FlowVerdict, FlowViolation,
    UnsignedFlowCheckOptions, check_unsigned_stabilizer_flows, circuit_flow_generators,
    solve_for_flow_measurements,
};
pub use circuit_generation::{
    CodeCircuitParams, CodeDistance, CodeFamily, CodeTask, ColorCodeTask, GeneratedCircuit,
    RepetitionCodeTask, RoundCount, SurfaceCodeTask, UnknownCodeFamily, UnknownCodeTask,
};
pub use circuit_inverse::{
    InverseQecOptions, TimeReversedForFlowsOptions, circuit_inverse_qec, circuit_inverse_unitary,
    circuit_time_reversed_for_flows,
};
pub use circuit_missing_detectors::{MissingDetectorOptions, missing_detectors};
pub use circuit_pass::{
    CircuitPass, CircuitPassContext, CircuitPassError, CircuitPassInput, CircuitPassOutput,
    CircuitPassProjectionError, CircuitPassResources, run_circuit_pass,
};
pub use circuit_pauli::{PauliConjugationOptions, pauli_after_circuit, pauli_before_circuit};
pub use circuit_simplify::decomposed_circuit;
pub use circuit_tableau::{CircuitToTableauOptions, circuit_to_tableau};
pub use circuit_to_dem::{
    DecompositionFailure, DisjointPauliProbabilities, ErrorAnalyzerOptions, ErrorDecomposition,
    GaugeDetectors, IndependentPauliProbabilities, LoopFolding, RemnantEdges,
    circuit_to_detector_error_model, independent_to_disjoint_xyz_errors,
    try_disjoint_to_independent_xyz_errors,
};
pub use circuit_transforms::{
    CircuitFlattenOptions, WithoutNoiseOptions, WithoutNoisePass, WithoutNoiseReport,
    circuit_without_noise, flattened_circuit, flattened_circuit_operations,
};
pub use dem::{
    DemFlattenOptions, GraphlikeSearchOptions, HypergraphSearchOptions, LogicalErrorSearch,
    LogicalErrorSearchKind, LogicalErrorSearchLimits, NoLogicalErrorReport,
    SatMaterializationLimits, SymptomGrowth, UngraphlikeErrors, detector_error_model_without_tags,
    find_undetectable_logical_error, flattened_detector_error_model, likeliest_error_sat_problem,
    rounded_detector_error_model, shortest_error_sat_problem,
    shortest_graphlike_undetectable_logical_error,
};
pub use error::{
    AnalysisError, AnalysisOperation, AnalysisRequestError, AnalysisResult, GeneratorParameter,
    NonDeterministicTargets, NonUnitaryReason, PauliCollapse, ThresholdExcess, UnanalyzableNoise,
};
pub use gate::{
    GateUnitaryMatrix, gate_flows, gate_tableau, gate_unitary_matrix,
    single_qubit_clifford_for_gate, two_qubit_clifford_for_gate,
};
pub use resources::{CircuitPassStage, ResourceKind, ResourceLimitError};
pub use tableau_circuit::tableau_to_circuit;

/// Low-level lowering operations shared with compilation engines.
pub mod advanced {
    pub use crate::circuit_flow::flow_record_index;
    pub use crate::circuit_simplify::visit_decomposed_spp_instructions;
}
