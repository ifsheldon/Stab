#![allow(
    clippy::expect_used,
    reason = "analysis contract tests use direct fixture assertions"
)]

use std::assert_matches;

use stab_algebra::SingleQubitClifford;
use stab_analysis::{
    AnalysisError, GateData, circuit_without_tags, gate_decomposition_to_circuit,
    gate_h_s_cx_m_r_decomposition, gate_has_h_s_cx_m_r_decomposition, gate_has_tableau,
    gate_tableau, single_qubit_clifford_for_gate,
};
use stab_model::{Circuit, Gate, GateKind};

#[test]
fn single_qubit_clifford_gate_conversion_covers_the_complete_algebra_set() {
    for clifford in SingleQubitClifford::all() {
        let gate = Gate::from_name(clifford.canonical_name()).expect("canonical Clifford gate");
        assert_eq!(single_qubit_clifford_for_gate(gate), Ok(clifford));
    }

    let cx = Gate::from_name("CX").expect("two-qubit Clifford gate");
    assert!(single_qubit_clifford_for_gate(cx).is_err());
}

#[test]
fn gate_decomposition_is_a_valid_model_without_implying_tableau_support() {
    let sqrt_xx = Gate::from_name("SQRT_XX").expect("SQRT_XX gate");
    let decomposition =
        gate_h_s_cx_m_r_decomposition(sqrt_xx).expect("SQRT_XX decomposition metadata");
    let circuit = gate_decomposition_to_circuit(decomposition).expect("valid decomposition model");
    assert!(!circuit.is_empty());

    let measurement = Gate::from_name("M").expect("M gate");
    assert!(gate_has_h_s_cx_m_r_decomposition(measurement));
    assert!(!gate_has_tableau(measurement));
    let error = gate_tableau(measurement).expect_err("measurement has no unitary tableau");
    assert_matches!(
        error,
        AnalysisError::GateDataUnavailable {
            gate: GateKind::M,
            data: GateData::Tableau,
        }
    );
    assert!(error.to_string().contains("does not have tableau data"));
}

#[test]
fn circuit_without_tags_is_recursive_non_mutating_and_preserves_boundaries() {
    let original = Circuit::from_stim_str(
        "H[top] 0\nREPEAT[loop] 2 {\n    M[measure](0.125) 0\n    DETECTOR[det] rec[-1]\n}\n",
    )
    .expect("tagged circuit");
    let stripped = circuit_without_tags(&original);

    assert_eq!(
        stripped.to_stim_string(),
        "H 0\nREPEAT 2 {\n    M(0.125) 0\n    DETECTOR rec[-1]\n}\n"
    );
    assert!(original.to_stim_string().contains("[loop]"));

    let distinct =
        Circuit::from_stim_str("H[first] 0\nH[second] 1\n").expect("distinct tagged operations");
    assert_eq!(
        circuit_without_tags(&distinct).to_stim_string(),
        "H 0\nH 1\n",
        "tag removal must not fuse source instruction boundaries"
    );
}
