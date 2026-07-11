#![allow(
    clippy::expect_used,
    reason = "M6 circuit-flow parity tests mirror compact upstream examples"
)]

use std::str::FromStr;

use stab_core::{
    Circuit, Flow, FlowMeasurementIndex, PauliString, UnsignedStabilizerFlowFailure,
    check_if_circuit_has_unsigned_stabilizer_flows,
    check_unsigned_stabilizer_flows_with_diagnostics, circuit_has_all_unsigned_stabilizer_flows,
    circuit_has_unsigned_stabilizer_flow, sample_if_circuit_has_stabilizer_flows,
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
fn unsigned_stabilizer_flow_diagnostics_explain_unitary_mismatches() {
    let circuit = circuit("H 0\n");
    let checks = check_unsigned_stabilizer_flows_with_diagnostics(
        &circuit,
        &[flow("X -> Z"), flow("X -> X")],
    );
    let mut checks = checks.iter();
    let first = checks.next().expect("first diagnostic check");
    let second = checks.next().expect("second diagnostic check");
    assert!(checks.next().is_none());

    assert!(first.has_flow());
    assert_eq!(first.failure(), None);
    assert!(!second.has_flow());
    assert_eq!(
        second.failure(),
        Some(&UnsignedStabilizerFlowFailure::OutputMismatch {
            expected_output: pauli("X"),
            actual_output: pauli("Z"),
        })
    );

    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &[flow("X -> Z"), flow("X -> X")]),
        vec![true, false]
    );
}

#[test]
fn unsigned_stabilizer_flow_diagnostics_explain_sparse_tracker_failures() {
    let circuit = circuit("M 0\n");
    let checks = check_unsigned_stabilizer_flows_with_diagnostics(
        &circuit,
        &[
            flow("Z -> rec[-1]"),
            flow("X -> rec[-1]"),
            flow("Z -> rec[-2]"),
        ],
    );
    let mut checks = checks.iter();
    let first = checks.next().expect("first diagnostic check");
    let second = checks.next().expect("second diagnostic check");
    let third = checks.next().expect("third diagnostic check");
    assert!(checks.next().is_none());

    assert!(first.has_flow());
    assert_eq!(first.failure(), None);
    assert!(!second.has_flow());
    assert_eq!(
        second.failure(),
        Some(&UnsignedStabilizerFlowFailure::InputMismatch {
            expected_input: pauli("X"),
            actual_input: pauli("Z"),
        })
    );
    assert!(!third.has_flow());
    assert_eq!(
        third.failure(),
        Some(
            &UnsignedStabilizerFlowFailure::MeasurementRecordOutOfRange {
                record: FlowMeasurementIndex::new(-2),
                measurement_count: 1,
            }
        )
    );
}

#[test]
fn unsigned_stabilizer_flow_diagnostics_keep_unsupported_circuits_fail_closed() {
    let text = "SPP X0*Z0\n";
    let circuit = circuit(text);
    let query = flow("Z -> Z");
    let checks =
        check_unsigned_stabilizer_flows_with_diagnostics(&circuit, std::slice::from_ref(&query));
    let mut checks = checks.iter();
    let check = checks.next().expect("diagnostic check");
    assert!(checks.next().is_none());

    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &[query]),
        vec![false],
        "{text}"
    );
    assert!(!check.has_flow(), "{text}");
    let failure = check.failure().expect("unsupported diagnostic");
    assert!(
        matches!(
            failure,
            UnsignedStabilizerFlowFailure::UnsupportedCircuit { .. }
        ),
        "{text}: {failure:?}"
    );
    if let UnsignedStabilizerFlowFailure::UnsupportedCircuit { reason } = failure {
        assert!(!reason.is_empty(), "{text}");
    }
}

#[test]
fn unsigned_stabilizer_flow_diagnostics_match_bool_checker() {
    let cases = [
        (
            circuit("H 0\n"),
            vec![flow("X -> Z"), flow("Y -> Y"), flow("X -> X")],
        ),
        (
            circuit(
                "
                M 0
                OBSERVABLE_INCLUDE(0) rec[-1]
                ",
            ),
            vec![
                flow("Z -> rec[-1]"),
                flow("1 -> rec[-1] xor obs[0]"),
                flow("X -> rec[-1]"),
                flow("Z -> rec[-2]"),
            ],
        ),
        (circuit("SPP X0*Z0\n"), vec![flow("Z -> Z")]),
    ];

    for (circuit, flows) in cases {
        let bool_checks = check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows);
        let diagnostic_checks = check_unsigned_stabilizer_flows_with_diagnostics(&circuit, &flows)
            .iter()
            .map(|check| check.has_flow())
            .collect::<Vec<_>>();
        assert_eq!(diagnostic_checks, bool_checks);
    }
}

#[test]
fn unsigned_checker_combines_observable_effects_before_collapse_checks() {
    let circuit = circuit(
        "
        M 0
        OBSERVABLE_INCLUDE(0) X0
        ",
    );
    let flows = [flow("1 -> X xor obs[0]"), flow("1 -> X")];

    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows),
        vec![true, false]
    );
    assert_eq!(
        check_unsigned_stabilizer_flows_with_diagnostics(&circuit, &flows)
            .iter()
            .map(|check| check.has_flow())
            .collect::<Vec<_>>(),
        vec![true, false]
    );
}

#[test]
fn unsigned_checker_xor_cancels_absolute_relative_record_aliases() {
    let circuit = circuit("M 0\n");
    let aliased = flow("1 -> rec[-1] xor rec[0]");

    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&circuit, std::slice::from_ref(&aliased)),
        vec![true]
    );
    assert!(
        check_unsigned_stabilizer_flows_with_diagnostics(&circuit, &[aliased])
            .first()
            .is_some_and(stab_core::UnsignedStabilizerFlowCheck::has_flow)
    );
}

#[test]
fn unsigned_checker_empty_batch_skips_dense_validation() {
    let circuit = circuit("H 8191\n");

    assert!(check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &[]).is_empty());
    assert!(check_unsigned_stabilizer_flows_with_diagnostics(&circuit, &[]).is_empty());
}

#[test]
fn unsigned_checker_preserves_flow_qubits_idle_outside_circuit_width() {
    let circuit = circuit("H 1\n");
    let shorter = [flow("Z0 -> Z0"), flow("X0 -> Z0")];
    let empty = Circuit::new();
    let longer = [flow("X1000000 -> X1000000"), flow("X1000000 -> Z1000000")];

    for (circuit, queries) in [(&circuit, &shorter), (&empty, &longer)] {
        assert_eq!(
            check_if_circuit_has_unsigned_stabilizer_flows(circuit, queries),
            vec![true, false]
        );
        assert_eq!(
            check_unsigned_stabilizer_flows_with_diagnostics(circuit, queries)
                .iter()
                .map(stab_core::UnsignedStabilizerFlowCheck::has_flow)
                .collect::<Vec<_>>(),
            vec![true, false]
        );
    }
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
fn sample_if_circuit_has_stabilizer_flows_checks_signed_unitary_flows() {
    // Adapted from Stim v1.16.0 src/stim/util_top/has_flow.test.cc
    // sample_if_circuit_has_stabilizer_flows_signed_checked.
    let circuit = circuit(
        "
        R 2 3
        X 1 3
        ",
    );
    let flows = [
        flow("Z0 -> Z0"),
        flow("Z1 -> -Z1"),
        flow("1 -> Z2"),
        flow("1 -> -Z3"),
        flow("Z0 -> -Z0"),
        flow("Z1 -> Z1"),
        flow("1 -> -Z2"),
        flow("1 -> Z3"),
    ];

    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(&circuit, &flows, 256, Some(5))
            .expect("sample signed flows"),
        vec![true, true, true, true, false, false, false, false]
    );
}

#[test]
fn sample_if_circuit_has_stabilizer_flows_checks_signed_measurement_records() {
    // Adapted from Stim v1.16.0 src/stim/util_top/has_flow.test.cc
    // sample_if_circuit_has_stabilizer_flows_measurements_signed_checked.
    let circuit = circuit(
        "
        X 1
        M 0 1 2
        X 2
        ",
    );
    let flows = [
        flow("Z0 -> Z0"),
        flow("Z1 -> -Z1"),
        flow("Z2 -> -Z2"),
        flow("Z0 -> rec[-3]"),
        flow("-Z1 -> rec[-2]"),
        flow("Z2 -> rec[-1]"),
        flow("1 -> Z0 xor rec[-3]"),
        flow("1 -> Z1 xor rec[-2]"),
        flow("1 -> -Z2 xor rec[-1]"),
        flow("Z0 -> -Z0"),
        flow("Z1 -> Z1"),
        flow("Z2 -> Z2"),
        flow("-Z0 -> rec[-3]"),
        flow("Z1 -> rec[-2]"),
        flow("-Z2 -> rec[-1]"),
        flow("1 -> -Z0 xor rec[-3]"),
        flow("1 -> -Z1 xor rec[-2]"),
        flow("1 -> Z2 xor rec[-1]"),
    ];

    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(&circuit, &flows, 256, Some(7))
            .expect("sample signed measurement flows"),
        vec![
            true, true, true, true, true, true, true, true, true, false, false, false, false,
            false, false, false, false, false
        ]
    );
}

#[test]
fn sample_if_circuit_has_stabilizer_flows_checks_signed_observables() {
    // Adapted from Stim v1.16.0 src/stim/util_top/has_flow.test.cc
    // sample_if_circuit_has_stabilizer_flows_signed_obs and observable target variants.
    let observable_record_circuit = circuit(
        "
        X 1
        M 0 1 2
        X 2
        OBSERVABLE_INCLUDE(0) rec[-3]
        OBSERVABLE_INCLUDE(1) rec[-2]
        OBSERVABLE_INCLUDE(2) rec[-1]
        ",
    );
    let observable_record_flows = [
        flow("Z0 -> obs[0]"),
        flow("-Z1 -> obs[1]"),
        flow("Z2 -> obs[2]"),
        flow("1 -> Z0 xor obs[0]"),
        flow("1 -> Z1 xor obs[1]"),
        flow("1 -> -Z2 xor obs[2]"),
        flow("-Z0 -> obs[0]"),
        flow("Z1 -> obs[1]"),
        flow("-Z2 -> obs[2]"),
    ];
    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(
            &observable_record_circuit,
            &observable_record_flows,
            256,
            Some(11),
        )
        .expect("sample signed observable record flows"),
        vec![true, true, true, true, true, true, false, false, false]
    );

    let observable_pauli_circuit = circuit(
        "
        OBSERVABLE_INCLUDE(3) X0
        OBSERVABLE_INCLUDE(2) Y0
        OBSERVABLE_INCLUDE(4) Z1
        ",
    );
    let observable_pauli_flows = [
        flow("X0 -> obs[3]"),
        flow("Y0 -> obs[2]"),
        flow("Z1 -> obs[4]"),
        flow("-X0 -> obs[3]"),
        flow("X0 -> obs[2]"),
        flow("Y0 -> obs[3]"),
        flow("-Z1 -> obs[4]"),
    ];
    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(
            &observable_pauli_circuit,
            &observable_pauli_flows,
            256,
            Some(13),
        )
        .expect("sample signed observable Pauli flows"),
        vec![true, true, true, false, false, false, false]
    );

    let inverted_observable_pauli_circuit = circuit(
        "
        OBSERVABLE_INCLUDE(3) X0
        OBSERVABLE_INCLUDE(2) !X0
        ",
    );
    let inverted_observable_pauli_flows = [
        flow("X0 -> obs[3]"),
        flow("-X0 -> obs[2]"),
        flow("-X0 -> obs[3]"),
        flow("X0 -> obs[2]"),
    ];
    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(
            &inverted_observable_pauli_circuit,
            &inverted_observable_pauli_flows,
            256,
            Some(17),
        )
        .expect("sample signed inverted observable Pauli flows"),
        vec![true, true, false, false]
    );
}

#[test]
fn sample_if_circuit_has_stabilizer_flows_checks_inverted_record_observables() {
    // Adapted from Stim v1.16.0 src/stim/util_top/has_flow.test.cc
    // sample_if_circuit_has_stabilizer_flows_inverted_obs_rec.
    let circuit = circuit(
        "
        M !0
        OBSERVABLE_INCLUDE(3) rec[-1]
        ",
    );
    let flows = [flow("-Z0 -> obs[3]"), flow("Z0 -> obs[3]")];

    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(&circuit, &flows, 256, Some(23))
            .expect("sample signed inverted record-backed observable flows"),
        vec![true, false]
    );
}

#[test]
fn sample_if_circuit_has_stabilizer_flows_rejects_malformed_measurement_refs() {
    let error = sample_if_circuit_has_stabilizer_flows(
        &circuit("M 0\n"),
        &[flow("Z -> rec[-2]")],
        8,
        Some(19),
    )
    .expect_err("reject out-of-range sampled flow record");

    assert!(
        error.to_string().contains("outside sampled flow circuit"),
        "{error}"
    );
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
fn solve_for_flow_measurements_treats_sweep_controlled_paulis_as_noops() {
    let queries = [flow("1 -> Z0"), flow("Z0 -> 1"), flow("Z0 -> Z0")];
    let expected = vec![Some(vec![0]), Some(vec![0]), Some(vec![])];
    for suffix in [
        "CX sweep[0] 0",
        "CX 0 sweep[0]",
        "CY sweep[0] 0",
        "CY 0 sweep[0]",
        "CZ sweep[0] 0",
        "CZ 0 sweep[0]",
        "XCZ sweep[0] 0",
        "XCZ 0 sweep[0]",
        "YCZ sweep[0] 0",
        "YCZ 0 sweep[0]",
    ] {
        let text = format!("M 0\n{suffix}\n");
        assert_eq!(
            solve_for_flow_measurements(&circuit(&text), &queries).expect(&text),
            expected.clone(),
            "{suffix}"
        );
    }
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

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn flow(text: &str) -> Flow {
    Flow::from_str(text).expect("parse flow")
}

fn pauli(text: &str) -> PauliString {
    PauliString::from_str(text).expect("parse pauli")
}
