use crate::{Circuit, CircuitResult, Flow, Gate, GateDecomposition, Tableau};

pub use stab_analysis::GateUnitaryMatrix;

pub fn gate_tableau(gate: Gate) -> CircuitResult<Tableau> {
    stab_analysis::gate_tableau(gate).map_err(Into::into)
}

pub fn gate_has_tableau(gate: Gate) -> bool {
    stab_analysis::gate_has_tableau(gate)
}

pub fn gate_flows(gate: Gate) -> CircuitResult<Vec<Flow>> {
    stab_analysis::gate_flows(gate).map_err(Into::into)
}

pub fn gate_has_flows(gate: Gate) -> bool {
    stab_analysis::gate_has_flows(gate)
}

pub fn gate_unitary_matrix(gate: Gate) -> CircuitResult<GateUnitaryMatrix> {
    stab_analysis::gate_unitary_matrix(gate).map_err(Into::into)
}

pub fn gate_has_unitary_matrix(gate: Gate) -> bool {
    stab_analysis::gate_has_unitary_matrix(gate)
}

pub fn gate_h_s_cx_m_r_decomposition(gate: Gate) -> CircuitResult<GateDecomposition> {
    stab_analysis::gate_h_s_cx_m_r_decomposition(gate).map_err(Into::into)
}

pub fn gate_has_h_s_cx_m_r_decomposition(gate: Gate) -> bool {
    stab_analysis::gate_has_h_s_cx_m_r_decomposition(gate)
}

pub fn gate_decomposition_to_circuit(decomposition: GateDecomposition) -> CircuitResult<Circuit> {
    stab_analysis::gate_decomposition_to_circuit(decomposition).map_err(Into::into)
}
