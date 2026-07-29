//! Stable pure transforms and semantic analysis over Stab models.
//!
//! This crate owns operations that combine model syntax with stabilizer algebra without creating
//! mutable execution sessions or importing result codecs, filesystem policy, CLI types, or ops
//! contracts.

pub mod circuit;
mod circuit_simplify;
mod circuit_tableau;
mod error;
pub mod gate;

pub use circuit::circuit_without_tags;
pub use circuit_simplify::{decomposed_circuit, simplified_circuit};
pub use circuit_tableau::circuit_to_tableau;
pub use error::{AnalysisError, AnalysisResult};
pub use gate::{
    GateUnitaryMatrix, gate_decomposition_to_circuit, gate_flows, gate_h_s_cx_m_r_decomposition,
    gate_has_flows, gate_has_h_s_cx_m_r_decomposition, gate_has_tableau, gate_has_unitary_matrix,
    gate_tableau, gate_unitary_matrix, single_qubit_clifford_for_gate,
};

/// Low-level lowering operations shared with compilation engines.
pub mod advanced {
    pub use crate::circuit_simplify::decomposed_single_instruction;
}
