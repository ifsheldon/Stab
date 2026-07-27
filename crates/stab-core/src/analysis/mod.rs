//! Pure semantic lowering, transforms, and analysis over Stab models and algebra.
//!
//! This namespace does not own mutable simulator sessions. Gate projections live here because
//! they combine the closed Stim gate model with algebra values.

mod circuit_adapters;
mod dem_adapters;
mod gate_adapters;
mod gate_semantics;

pub use crate::circuit_feedback::circuit_with_inlined_feedback;
pub use crate::circuit_inverse::{
    InverseQecOptions, TimeReversedForFlowsOptions, circuit_inverse_qec,
    circuit_inverse_qec_with_options, circuit_inverse_unitary, circuit_time_reversed_for_flows,
    circuit_time_reversed_for_flows_with_options,
};
pub(crate) use crate::circuit_simplify::decomposed_single_instruction;
pub use crate::circuit_simplify::{decomposed_circuit, simplified_circuit};
pub use crate::circuit_tableau::circuit_to_tableau;
pub use crate::circuit_transforms::{
    circuit_without_noise, flattened_circuit, flattened_circuit_operations,
    flattened_circuit_operations_with_limits, flattened_circuit_with_limits,
};
pub use circuit_adapters::circuit_without_tags;
pub use dem_adapters::{
    detector_error_model_without_tags, flattened_detector_error_model,
    flattened_detector_error_model_with_limits, rounded_detector_error_model,
};
pub use gate_adapters::{
    GateUnitaryMatrix, gate_decomposition_to_circuit, gate_flows, gate_h_s_cx_m_r_decomposition,
    gate_has_flows, gate_has_h_s_cx_m_r_decomposition, gate_has_tableau, gate_has_unitary_matrix,
    gate_tableau, gate_unitary_matrix,
};
pub use gate_semantics::single_qubit_clifford_for_gate;
