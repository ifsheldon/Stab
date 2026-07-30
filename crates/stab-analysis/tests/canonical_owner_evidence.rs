#![allow(
    clippy::expect_used,
    reason = "canonical owner tests use fixed compatibility fixtures and inspect exact values"
)]

use num_complex::Complex32;
use stab_analysis::{
    GateUnitaryMatrix, circuit_to_tableau, circuit_without_noise, decomposed_circuit,
    flattened_circuit, gate_decomposition_to_circuit, gate_h_s_cx_m_r_decomposition,
    gate_has_h_s_cx_m_r_decomposition, gate_has_tableau, gate_has_unitary_matrix, gate_tableau,
    gate_unitary_matrix,
};
use stab_model::{Circuit, CircuitItem, Gate};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("valid circuit fixture")
}

#[test]
fn pf1_gate_tableau_metadata() {
    let h = Gate::from_name("H").expect("H");
    let h_tableau = gate_tableau(h).expect("H tableau");
    assert_eq!(h_tableau.x_output(0).expect("X output").to_string(), "+Z");
    assert_eq!(h_tableau.z_output(0).expect("Z output").to_string(), "+X");

    let cx = Gate::from_name("CX").expect("CX");
    let cx_tableau = gate_tableau(cx).expect("CX tableau");
    assert_eq!(
        [
            cx_tableau.x_output(0).expect("X0").to_string(),
            cx_tableau.z_output(0).expect("Z0").to_string(),
            cx_tableau.x_output(1).expect("X1").to_string(),
            cx_tableau.z_output(1).expect("Z1").to_string(),
        ],
        ["+XX", "+Z_", "+_X", "+ZZ"]
    );
    for gate in Gate::all() {
        assert_eq!(gate_has_tableau(gate), gate_tableau(gate).is_ok());
    }
}

#[test]
fn cq2_circuit_api_flattened_transform() {
    let source = circuit(
        "SHIFT_COORDS(5, 0)\n\
         QUBIT_COORDS(1, 2, 3) 0\n\
         REPEAT[tag] 2 {\n\
             M 0\n\
             DETECTOR(1, 0) rec[-1]\n\
             SHIFT_COORDS(0, 1)\n\
         }\n",
    );
    assert_eq!(
        flattened_circuit(&source)
            .expect("flatten")
            .to_stim_string(),
        concat!(
            "QUBIT_COORDS(6, 2, 3) 0\n",
            "M 0\n",
            "DETECTOR(6, 0) rec[-1]\n",
            "M 0\n",
            "DETECTOR(6, 1) rec[-1]\n",
        )
    );
    assert!(
        flattened_circuit(&circuit("REPEAT 1000001 {\n    H 0\n}\n")).is_err(),
        "materializing above the source-owned expansion limit must fail"
    );
}

#[test]
fn cq2_gate_unitary_matrix_contract() {
    let h = Gate::from_name("H").expect("H");
    let amplitude = f32::sqrt(0.5);
    let h_matrix = gate_unitary_matrix(h).expect("H matrix");
    assert_eq!(
        h_matrix,
        GateUnitaryMatrix::One([
            [
                Complex32::new(amplitude, 0.0),
                Complex32::new(amplitude, 0.0),
            ],
            [
                Complex32::new(amplitude, 0.0),
                Complex32::new(-amplitude, 0.0),
            ],
        ])
    );
    assert_eq!(h_matrix.dimension(), 2);
    assert_eq!(h_matrix.num_qubits(), 1);
    assert_eq!(h_matrix.entry_count(), 4);

    let cx_matrix = gate_unitary_matrix(Gate::from_name("CX").expect("CX")).expect("CX matrix");
    assert!(matches!(cx_matrix, GateUnitaryMatrix::Two(_)));
    assert_eq!(cx_matrix.dimension(), 4);
    assert_eq!(cx_matrix.num_qubits(), 2);
    assert_eq!(cx_matrix.entry_count(), 16);

    for gate in Gate::all() {
        assert_eq!(
            gate_has_unitary_matrix(gate),
            gate_unitary_matrix(gate).is_ok()
        );
    }
}

#[test]
fn cq2_circuit_api_without_noise_transform() {
    let source = circuit(
        "H[tag] 0\n\
         X_ERROR(0.25) 0\n\
         HERALDED_ERASE[herald](0.5) 1\n\
         REPEAT[loop] 2 {\n\
             DEPOLARIZE1(0.1) 0\n\
             M[measure](0.2) 0\n\
             DETECTOR[det] rec[-1]\n\
         }\n",
    );
    assert_eq!(
        circuit_without_noise(&source)
            .expect("remove noise")
            .to_stim_string(),
        concat!(
            "H[tag] 0\n",
            "MPAD[herald] 0\n",
            "REPEAT[loop] 2 {\n",
            "    M[measure] 0\n",
            "    DETECTOR[det] rec[-1]\n",
            "}\n",
        )
    );
    assert!(
        source.to_stim_string().contains("X_ERROR"),
        "the transform must not mutate its source"
    );
}

#[test]
fn cq2_gate_decomposition_metadata_contract() {
    let sqrt_xx = Gate::from_name("SQRT_XX").expect("SQRT_XX");
    let decomposition = gate_h_s_cx_m_r_decomposition(sqrt_xx).expect("decomposition metadata");
    let decomposed =
        gate_decomposition_to_circuit(decomposition).expect("valid decomposition circuit");
    assert!(!decomposed.is_empty());
    assert!(
        decomposed
            .items()
            .iter()
            .filter_map(CircuitItem::as_instruction)
            .all(|instruction| matches!(
                instruction.gate().canonical_name(),
                "H" | "S" | "CX" | "M" | "R"
            ))
    );
    assert_eq!(
        circuit_to_tableau(&decomposed, false, false, false).expect("decomposition tableau"),
        gate_tableau(sqrt_xx).expect("gate tableau")
    );
    for gate in Gate::all() {
        assert_eq!(
            gate_has_h_s_cx_m_r_decomposition(gate),
            gate_h_s_cx_m_r_decomposition(gate).is_ok()
        );
    }
}

#[test]
fn cq2_circuit_api_decomposed_transform() {
    let source = circuit(
        "ISWAP[tag] 0 1\n\
         MPP[measure] X0*X1 Y0*Y1\n\
         DETECTOR[det] rec[-1]\n",
    );
    let decomposed = decomposed_circuit(&source).expect("decompose");
    assert!(
        decomposed
            .items()
            .iter()
            .filter_map(CircuitItem::as_instruction)
            .all(|instruction| !matches!(
                instruction.gate().canonical_name(),
                "ISWAP" | "MPP" | "MXX" | "MYY" | "MZZ"
            ))
    );
    assert!(decomposed.to_stim_string().contains("[measure]"));
    assert!(decomposed.to_stim_string().contains("DETECTOR[det]"));
    assert!(
        decomposed_circuit(&circuit("SPP X0*Z0\n")).is_err(),
        "anti-Hermitian Pauli products must remain rejected"
    );
}
