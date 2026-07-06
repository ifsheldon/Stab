#![allow(
    clippy::expect_used,
    reason = "M6 circuit-flow parity tests mirror compact upstream examples"
)]

use std::str::FromStr;

use stab_core::{
    Circuit, Flow, check_if_circuit_has_unsigned_stabilizer_flows,
    circuit_has_all_unsigned_stabilizer_flows, circuit_has_unsigned_stabilizer_flow,
    solve_for_flow_measurements,
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
fn circuit_has_unsigned_stabilizer_flow_helpers_match_supported_batch_semantics() {
    let h_circuit = circuit("H 0\n");
    let h_flows = [
        flow("X -> Z"),
        flow("Y -> Y"),
        flow("Z -> X"),
        flow("X -> X"),
    ];
    assert!(circuit_has_unsigned_stabilizer_flow(
        &h_circuit,
        &h_flows[1]
    ));
    assert!(!circuit_has_unsigned_stabilizer_flow(
        &h_circuit,
        &h_flows[3]
    ));
    assert!(circuit_has_all_unsigned_stabilizer_flows(
        &h_circuit,
        &h_flows[..3]
    ));
    assert!(!circuit_has_all_unsigned_stabilizer_flows(
        &h_circuit, &h_flows
    ));
    assert!(circuit_has_all_unsigned_stabilizer_flows(&h_circuit, &[]));

    let mzz_observable_circuit = circuit(
        "
        MZZ 0 1
        OBSERVABLE_INCLUDE(2) rec[-1]
    ",
    );
    let observable_flows = [flow("Z0*Z1 -> obs[2]"), flow("X0*X1 -> Y0*Y1 xor obs[2]")];
    assert!(circuit_has_all_unsigned_stabilizer_flows(
        &mzz_observable_circuit,
        &observable_flows
    ));
    assert!(!circuit_has_all_unsigned_stabilizer_flows(
        &mzz_observable_circuit,
        &[
            observable_flows[0].clone(),
            flow("X0*X1 -> Y0*Y1 xor obs[1]")
        ]
    ));

    let folded_measurement_circuit = circuit(
        "
        REPEAT 1000001 {
            H 0
        }
        M 0
        ",
    );
    assert!(circuit_has_all_unsigned_stabilizer_flows(
        &folded_measurement_circuit,
        &[flow("X -> rec[-1]")]
    ));
    assert!(!circuit_has_all_unsigned_stabilizer_flows(
        &folded_measurement_circuit,
        &[flow("Z -> rec[-1]")]
    ));
}

#[test]
fn check_if_circuit_has_unsigned_stabilizer_flows_folds_shifted_measurement_repeats() {
    let circuit = circuit(
        "
        REPEAT 17 {
            CX 0 1 1 2 2 3 3 0
            M 0 0 1
            DETECTOR rec[-2] rec[-3]
            OBSERVABLE_INCLUDE(3) rec[-1]
        }
        ",
    );
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(
            &circuit,
            &[
                flow("Z___ -> rec[-7] xor rec[-4]"),
                flow("X___ -> rec[-7] xor rec[-4]")
            ]
        ),
        vec![true, false]
    );
}

#[test]
fn check_if_circuit_has_unsigned_stabilizer_flows_marker_avoids_flow_observables() {
    let circuit = circuit("M 0\n");
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(
            &circuit,
            &[flow("Z -> rec[-1]"), flow("1 -> rec[-1] xor obs[0]")]
        ),
        vec![true, false]
    );
}

#[test]
fn check_if_circuit_has_unsigned_stabilizer_flows_marker_handles_max_observable() {
    let circuit = circuit(
        "
        M 0
        OBSERVABLE_INCLUDE(4294967295) rec[-1]
        ",
    );
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(
            &circuit,
            &[
                flow("Z -> rec[-1]"),
                flow("1 -> rec[-1] xor obs[4294967295]")
            ]
        ),
        vec![true, true]
    );
}

#[test]
fn pf6_sparse_rev_spp_circuit_has_unsigned_stabilizer_flow_helpers_support_unsigned_semantics() {
    let spp_circuit = circuit("SPP X0*Y1*Z2\n");
    let spp_dag_circuit = circuit("SPP_DAG X0*Y1*Z2\n");
    let true_unsigned_flows = [flow("Z__ -> YYZ"), flow("__Z -> __Z")];
    let false_identity_flow = flow("Z__ -> Z__");
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&spp_circuit, &true_unsigned_flows),
        vec![true, true]
    );
    assert!(circuit_has_all_unsigned_stabilizer_flows(
        &spp_circuit,
        &true_unsigned_flows
    ));
    assert!(circuit_has_all_unsigned_stabilizer_flows(
        &spp_dag_circuit,
        &true_unsigned_flows
    ));
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(
            &spp_circuit,
            std::slice::from_ref(&false_identity_flow),
        ),
        vec![false]
    );
    assert!(!circuit_has_unsigned_stabilizer_flow(
        &spp_circuit,
        &false_identity_flow
    ));
    assert!(!circuit_has_all_unsigned_stabilizer_flows(
        &spp_circuit,
        &[false_identity_flow]
    ));

    let anti_hermitian_circuit = circuit("SPP X0*Z0\n");
    assert!(!circuit_has_unsigned_stabilizer_flow(
        &anti_hermitian_circuit,
        &flow("Z -> Z")
    ));
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
fn solve_for_flow_measurements_cpp_empty_and_simple_examples() {
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
fn solve_for_flow_measurements_python_measured_idle_examples() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_flow_generators_test.py.
    let measured_idle_circuit = circuit("M 2\n");
    assert_eq!(
        solve_for_flow_measurements(&measured_idle_circuit, &[flow("X2 -> X2")])
            .expect("solve measured X identity"),
        vec![None]
    );
    assert_eq!(
        solve_for_flow_measurements(
            &measured_idle_circuit,
            &[
                flow("X2 -> X2"),
                flow("Y2 -> Y2"),
                flow("Z2 -> Z2"),
                flow("Z2 -> 1"),
            ],
        )
        .expect("solve measured idle batch"),
        vec![None, None, Some(vec![]), Some(vec![0])]
    );
}

#[test]
fn solve_for_flow_measurements_python_multi_target_examples() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_flow_generators_test.py.
    assert_eq!(
        solve_for_flow_measurements(
            &circuit("MXX 0 1\n"),
            &[flow("YY -> ZZ"), flow("YY -> YY"), flow("YZ -> ZY"),]
        )
        .expect("solve MXX multi-target batch"),
        vec![Some(vec![0]), Some(vec![]), Some(vec![0])]
    );
    assert_eq!(
        solve_for_flow_measurements(&circuit("M 1 2\n"), &[flow("_Z -> 1")])
            .expect("solve multi-target M"),
        vec![Some(vec![0])]
    );
    assert_eq!(
        solve_for_flow_measurements(&circuit("MX 1 2\n"), &[flow("_X -> 1")])
            .expect("solve multi-target MX"),
        vec![Some(vec![0])]
    );
    assert_eq!(
        solve_for_flow_measurements(&circuit("MYY 1 2 3 4\n"), &[flow("_YY__ -> 1")])
            .expect("solve multi-target MYY"),
        vec![Some(vec![0])]
    );
    assert_eq!(
        solve_for_flow_measurements(&circuit("MPP Y1*Y2 Y3*Y4\n"), &[flow("_YY__ -> 1")])
            .expect("solve multi-target MPP"),
        vec![Some(vec![0])]
    );
}

#[test]
fn solve_for_flow_measurements_python_fewer_measurements_heuristic_examples() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_flow_generators_test.py.
    let mpp_then_single_measurements = circuit(
        "
        MPP Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8
        M 0 1 2 3 4 5 6 7 8
        ",
    );
    assert_eq!(
        solve_for_flow_measurements(
            &mpp_then_single_measurements,
            &[flow("1 -> Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8")]
        )
        .expect("solve MPP before single measurements output"),
        vec![Some(vec![0])]
    );
    assert_eq!(
        solve_for_flow_measurements(
            &mpp_then_single_measurements,
            &[flow("Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8 -> 1")]
        )
        .expect("solve MPP before single measurements input"),
        vec![Some(vec![0])]
    );

    let single_measurements_then_mpp = circuit(
        "
        M 0 1 2 3 4 5 6 7 8
        MPP Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8
        ",
    );
    assert_eq!(
        solve_for_flow_measurements(
            &single_measurements_then_mpp,
            &[flow("1 -> Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8")]
        )
        .expect("solve MPP after single measurements output"),
        vec![Some(vec![9])]
    );
    assert_eq!(
        solve_for_flow_measurements(
            &single_measurements_then_mpp,
            &[flow("Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8 -> 1")]
        )
        .expect("solve MPP after single measurements input"),
        vec![Some(vec![9])]
    );
}

#[test]
fn solve_for_flow_measurements_cpp_repetition_code_example() {
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
        CX sweep[0] 0
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
