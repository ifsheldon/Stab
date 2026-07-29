#![allow(
    clippy::expect_used,
    reason = "analysis contract tests use direct fixture assertions"
)]

use stab_analysis::{
    AnalysisError, GateUnitaryMatrix, circuit_without_tags, gate_decomposition_to_circuit,
    gate_flows, gate_h_s_cx_m_r_decomposition, gate_has_flows, gate_has_h_s_cx_m_r_decomposition,
    gate_has_tableau, gate_has_unitary_matrix, gate_tableau, gate_unitary_matrix,
    single_qubit_clifford_for_gate,
};
use stab_model::{Circuit, Gate};

#[test]
fn gate_semantics_bridge_model_metadata_into_algebra_values() {
    let h = Gate::from_name("H").expect("H gate");
    let h_tableau = gate_tableau(h).expect("H tableau");
    assert_eq!(h_tableau.x_output(0).expect("H X output").to_string(), "+Z");
    assert_eq!(h_tableau.z_output(0).expect("H Z output").to_string(), "+X");
    assert_eq!(
        single_qubit_clifford_for_gate(h)
            .expect("H Clifford")
            .canonical_name(),
        "H"
    );

    let h_matrix = gate_unitary_matrix(h).expect("H unitary matrix");
    assert!(matches!(h_matrix, GateUnitaryMatrix::One(_)));
    assert_eq!(h_matrix.dimension(), 2);
    assert_eq!(h_matrix.num_qubits(), 1);
    assert_eq!(h_matrix.entry_count(), 4);
    assert_eq!(h_matrix.to_vecs().len(), 2);

    let cx = Gate::from_name("CX").expect("CX gate");
    let cx_tableau = gate_tableau(cx).expect("CX tableau");
    assert_eq!(
        [
            cx_tableau.x_output(0).expect("CX X0").to_string(),
            cx_tableau.z_output(0).expect("CX Z0").to_string(),
            cx_tableau.x_output(1).expect("CX X1").to_string(),
            cx_tableau.z_output(1).expect("CX Z1").to_string(),
        ],
        ["+XX", "+Z_", "+_X", "+ZZ"]
    );
    let cx_matrix = gate_unitary_matrix(cx).expect("CX unitary matrix");
    assert!(matches!(cx_matrix, GateUnitaryMatrix::Two(_)));
    assert_eq!(cx_matrix.dimension(), 4);
    assert_eq!(cx_matrix.num_qubits(), 2);
    assert_eq!(cx_matrix.entry_count(), 16);

    let measurement = Gate::from_name("M").expect("M gate");
    let measurement_flows = gate_flows(measurement)
        .expect("measurement flows")
        .into_iter()
        .map(|flow| flow.to_string())
        .collect::<Vec<_>>();
    assert_eq!(measurement_flows, ["Z -> rec[-1]", "Z -> Z"]);

    for gate in Gate::all() {
        assert_eq!(gate_has_tableau(gate), gate_tableau(gate).is_ok());
        assert_eq!(gate_has_flows(gate), gate_flows(gate).is_ok());
        assert_eq!(
            gate_has_unitary_matrix(gate),
            gate_unitary_matrix(gate).is_ok()
        );
        assert_eq!(
            gate_has_h_s_cx_m_r_decomposition(gate),
            gate_h_s_cx_m_r_decomposition(gate).is_ok()
        );
    }
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
    assert!(matches!(
        error,
        AnalysisError::InvalidTableauConversion { ref message }
            if message.contains("does not have tableau data")
    ));
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
