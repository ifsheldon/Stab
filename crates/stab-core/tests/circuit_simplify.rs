#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "M6 simplified-circuit parity tests mirror compact upstream examples"
)]

use stab_core::{Circuit, CircuitItem, simplified_circuit};

#[test]
fn simplified_circuit_rewrites_single_qubit_cliffords_to_h_s_base() {
    let circuit = circuit(
        "
        I 0
        X 0
        Y 1
        Z 2
        H_XY 0
        H_YZ 1
        H_NXY 2
        H_NXZ 0
        H_NYZ 1
        SQRT_X 0
        SQRT_X_DAG 1
        SQRT_Y 2
        SQRT_Y_DAG 0
        S_DAG 1
        C_XYZ 0
        C_NXYZ 1
        C_XNYZ 2
        C_XYNZ 0
        C_ZYX 1
        C_NZYX 2
        C_ZNYX 0
        C_ZYNX 1
        H 2
        S 2
    ",
    );

    let simplified = simplified_circuit(&circuit).expect("simplify");
    assert_eq!(
        circuit.simplified().expect("simplify through method"),
        simplified
    );
    assert_h_s_cx_base(&simplified);
    assert_tableau_equivalent(&circuit, &simplified);
    assert!(!simplified.to_stim_string().contains("H_XY"));
    assert!(!simplified.to_stim_string().contains("SQRT_X"));
    assert!(!simplified.to_stim_string().contains("C_XYZ"));
}

#[test]
fn simplified_circuit_rewrites_simple_two_qubit_cliffords_to_base() {
    let circuit = circuit(
        "
        CZ 0 1
        CY 1 2
        SWAP 0 2
        CX 2 1
    ",
    );

    let simplified = simplified_circuit(&circuit).expect("simplify");
    assert_h_s_cx_base(&simplified);
    assert_tableau_equivalent(&circuit, &simplified);
    assert!(!simplified.to_stim_string().contains("CZ"));
    assert!(!simplified.to_stim_string().contains("CY"));
    assert!(!simplified.to_stim_string().contains("SWAP"));
}

#[test]
fn simplified_circuit_recurses_into_repeat_blocks() {
    let circuit = circuit(
        "
        REPEAT 3 {
            H_XY 0
            CZ 0 1
        }
    ",
    );

    let simplified = simplified_circuit(&circuit).expect("simplify");
    assert_h_s_cx_base(&simplified);
    assert_tableau_equivalent(&circuit, &simplified);
    assert!(!simplified.to_stim_string().contains("H_XY"));
    assert!(!simplified.to_stim_string().contains("CZ"));
}

#[test]
fn simplified_circuit_preserves_unsupported_gates_for_later_slices() {
    let circuit = circuit("SQRT_XX 0 1\n");
    let simplified = simplified_circuit(&circuit).expect("simplify");
    assert_eq!(simplified, circuit);
}

#[test]
fn simplified_circuit_preserves_classical_controlled_pairs() {
    let circuit = circuit(
        "
        M 0
        CY rec[-1] 1
        CZ sweep[0] 2
        CX rec[-1] 2
    ",
    );
    let simplified = simplified_circuit(&circuit).expect("simplify");
    assert_eq!(simplified, circuit);
}

#[test]
fn cq2_circuit_api_simplified_contract_matches_selected_stim_scope() {
    simplified_circuit_rewrites_single_qubit_cliffords_to_h_s_base();
    simplified_circuit_rewrites_simple_two_qubit_cliffords_to_base();
    simplified_circuit_recurses_into_repeat_blocks();
    simplified_circuit_preserves_unsupported_gates_for_later_slices();
    simplified_circuit_preserves_classical_controlled_pairs();
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn assert_tableau_equivalent(original: &Circuit, simplified: &Circuit) {
    assert_eq!(
        original
            .to_tableau(false, true, true)
            .expect("original tableau"),
        simplified
            .to_tableau(false, true, true)
            .expect("simplified tableau")
    );
}

fn assert_h_s_cx_base(circuit: &Circuit) {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                assert!(
                    matches!(instruction.gate().canonical_name(), "H" | "S" | "CX"),
                    "{}",
                    instruction.gate().canonical_name()
                );
            }
            CircuitItem::RepeatBlock(repeat) => assert_h_s_cx_base(repeat.body()),
        }
    }
}
