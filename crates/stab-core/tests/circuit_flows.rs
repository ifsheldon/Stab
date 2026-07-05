#![allow(
    clippy::expect_used,
    reason = "M6 circuit-flow parity tests mirror compact upstream examples"
)]

use std::str::FromStr;

use stab_core::{
    Circuit, Flow, check_if_circuit_has_unsigned_stabilizer_flows, solve_for_flow_measurements,
};

#[test]
fn check_if_circuit_has_unsigned_stabilizer_flows_historical_failure() {
    // Adapted from Stim v1.16.0 src/stim/util_top/has_flow.test.cc.
    let circuit = circuit(
        "
        CX 0 1
        S 0
    ",
    );
    let flows = [flow("X_ -> YX"), flow("Y_ -> XX"), flow("X_ -> XX")];
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows),
        vec![true, true, false]
    );
}

#[test]
fn check_if_circuit_has_unsigned_stabilizer_flows_ignores_signs() {
    let circuit = circuit(
        "
        X 0
        S 0
    ",
    );
    let flows = [flow("+X -> +Y"), flow("-X -> -Y"), flow("Z -> -Z")];
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows),
        vec![true, true, true]
    );
}

#[test]
fn check_if_circuit_has_unsigned_stabilizer_flows_supports_measurement_records() {
    // Adapted from Stim v1.16.0 src/stim/util_top/has_flow.test.cc.
    let circuit = circuit(
        "
        R 4
        CX 0 4 1 4 2 4 3 4
        M 4
    ",
    );
    let flows = [
        flow("Z___ -> Z____"),
        flow("_Z__ -> _Z__"),
        flow("__Z_ -> __Z_"),
        flow("___Z -> ___Z"),
        flow("XX__ -> XX__"),
        flow("XXXX -> XXXX"),
        flow("XYZ_ -> XYZ_"),
        flow("XXX_ -> XXX_"),
        flow("ZZZZ -> ____ xor rec[-1]"),
        flow("+___Z -> -___Z"),
        flow("-___Z -> -___Z"),
        flow("-___Z -> +___Z"),
    ];
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows),
        vec![
            true, true, true, true, true, true, true, false, true, true, true, true
        ]
    );
}

#[test]
fn check_if_circuit_has_unsigned_stabilizer_flows_supports_pair_measurement_records() {
    // Adapted from Stim v1.16.0 src/stim/util_top/has_flow.test.cc.
    let circuit = circuit("MZZ 0 1\n");
    let flows = [
        flow("X0*X1 -> Y0*Y1 xor rec[-1]"),
        flow("X0*X1 -> Z0*Z1 xor rec[-1]"),
        flow("X0*X1 -> X0*X1"),
        flow("Z0 -> Z1 xor rec[-1]"),
        flow("Z0 -> Z0"),
    ];
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows),
        vec![true, false, true, true, true]
    );
}

#[test]
fn check_if_circuit_has_unsigned_stabilizer_flows_supports_observable_dependencies() {
    // Adapted from Stim v1.16.0 src/stim/util_top/has_flow.test.cc.
    let mzz_observable_circuit = circuit(
        "
        MZZ 0 1
        OBSERVABLE_INCLUDE(2) rec[-1]
    ",
    );
    let flows = [
        flow("Z0*Z1 -> obs[2]"),
        flow("1 -> Z0*Z1 xor obs[2]"),
        flow("X0*X1 -> X0*X1 xor obs[0]"),
        flow("X0*X1 -> Y0*Y1 xor obs[2]"),
        flow("X0*X1 -> Y0*Y1 xor obs[1]"),
        flow("X0*X1 -> Y0*Y1 xor rec[-1]"),
    ];
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&mzz_observable_circuit, &flows),
        vec![true, true, true, true, false, true]
    );

    let observable_pauli_circuit = circuit(
        "
        OBSERVABLE_INCLUDE(3) X0 Y1 Z2
        OBSERVABLE_INCLUDE(2) Y0
    ",
    );
    let observable_pauli_flows = [
        flow("X0*Y1*Z2 -> obs[3]"),
        flow("-Y0 -> obs[2]"),
        flow("Y0 -> obs[3]"),
        flow("1 -> X0*Y1*Z2 xor obs[3]"),
    ];
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(
            &observable_pauli_circuit,
            &observable_pauli_flows,
        ),
        vec![true, true, false, true]
    );
}

#[test]
fn check_if_circuit_has_unsigned_stabilizer_flows_folds_unitary_repeats() {
    let h_repeat_circuit = circuit(
        "
        REPEAT 1000001 {
            H 0
        }
        M 0
        ",
    );
    let flows = [flow("X -> rec[-1]"), flow("Z -> rec[-1]")];
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&h_repeat_circuit, &flows),
        vec![true, false]
    );

    let fixed_two_qubit_circuit = circuit(
        "
        REPEAT 1000001 {
            SWAP 0 1
        }
        M 1
        ",
    );
    let fixed_two_qubit_flows = [flow("Z_ -> rec[-1]"), flow("_Z -> rec[-1]")];
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(
            &fixed_two_qubit_circuit,
            &fixed_two_qubit_flows,
        ),
        vec![true, false]
    );
}

#[test]
fn solve_for_flow_measurements_matches_stim_empty_and_simple_examples() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_flow_generators.test.cc.
    assert_eq!(
        solve_for_flow_measurements(&circuit(""), &[]).expect("empty solve"),
        Vec::<Option<Vec<i32>>>::new()
    );

    let mx_circuit = circuit("MX 0\n");
    assert_eq!(
        solve_for_flow_measurements(&mx_circuit, &[flow("1 -> X0")]).expect("solve 1 -> X0"),
        vec![Some(vec![0])]
    );
    assert_eq!(
        solve_for_flow_measurements(&mx_circuit, &[flow("1 -> Y0")]).expect("solve 1 -> Y0"),
        vec![None]
    );
    assert_eq!(
        solve_for_flow_measurements(
            &mx_circuit,
            &[
                flow("1 -> X0"),
                flow("Y0 -> Y0"),
                flow("X0 -> 1"),
                flow("X0 -> Z0"),
                flow("Y1 -> Y1"),
            ],
        )
        .expect("solve simple batch"),
        vec![Some(vec![0]), None, Some(vec![0]), None, Some(vec![]),]
    );

    let error = solve_for_flow_measurements(&circuit(""), &[flow("1 -> 1")])
        .expect_err("empty Pauli flow is unsupported")
        .to_string();
    assert!(
        error.contains("only supports flows with non-empty Pauli input or output"),
        "{error}"
    );
}

#[test]
fn solve_for_flow_measurements_matches_stim_repetition_code_example() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_flow_generators.test.cc.
    let circuit = circuit(
        "
        R 1 3
        CX 0 1 2 3
        CX 4 3 2 1
        M 1 3
    ",
    );
    let flows = [
        flow("Z0*Z2 -> 1"),
        flow("1 -> Z2*Z4"),
        flow("1 -> Z0*Z4"),
        flow("Z0*Z4 -> Z0*Z2"),
        flow("Z0 -> Z0"),
        flow("Z0 -> Z1"),
        flow("Z0 -> Z2"),
        flow("X0*X2*X4 -> X0*X2*X4"),
        flow("X0 -> X0"),
        flow("X0 -> Z0"),
    ];
    assert_eq!(
        solve_for_flow_measurements(&circuit, &flows).expect("solve rep code"),
        vec![
            Some(vec![0]),
            Some(vec![1]),
            Some(vec![0, 1]),
            Some(vec![1]),
            Some(vec![]),
            None,
            Some(vec![0]),
            Some(vec![]),
            None,
            None,
        ]
    );
}

#[test]
fn solve_for_flow_measurements_has_documented_fallback_resource_limit() {
    let circuit = circuit(
        "
        M 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16
        H 0
    ",
    );
    let error = solve_for_flow_measurements(&circuit, &[flow("Z0 -> Z0")])
        .expect_err("fallback solver is bounded")
        .to_string();
    assert!(
        error.contains("fallback supports at most 16 measurements"),
        "{error}"
    );
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn flow(text: &str) -> Flow {
    Flow::from_str(text).expect("parse flow")
}
