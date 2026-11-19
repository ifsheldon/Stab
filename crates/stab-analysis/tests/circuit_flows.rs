#![allow(
    clippy::expect_used,
    reason = "M6 circuit-flow parity tests mirror compact upstream examples"
)]

use std::assert_matches;
use std::str::FromStr;

use stab_algebra::{Flow, FlowMeasurementIndex, PauliString};
use stab_analysis::{
    UnsignedStabilizerFlowCheck, UnsignedStabilizerFlowFailure,
    check_if_circuit_has_unsigned_stabilizer_flows,
    check_unsigned_stabilizer_flows_with_diagnostics, circuit_flow_generators,
    circuit_has_all_unsigned_stabilizer_flows, circuit_has_unsigned_stabilizer_flow,
    solve_for_flow_measurements,
};
use stab_model::Circuit;

#[test]
fn flow_generation_ignores_coordinate_overflow_irrelevant_to_semantics() {
    let circuit = circuit(
        "
        REPEAT 2 {
            SHIFT_COORDS(1e308)
            M 0
        }
    ",
    );
    let flows = circuit_flow_generators(&circuit).expect("generate flows without flattening");
    assert_eq!(
        flows.iter().map(ToString::to_string).collect::<Vec<_>>(),
        vec!["1 -> rec[0] xor rec[1]", "1 -> Z xor rec[1]", "Z -> rec[1]",]
    );
}

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
fn unsigned_stabilizer_flow_diagnostics_report_output_mismatches_for_wide_unitary_circuits() {
    // Whichever algorithm checks a wide circuit against narrower flows, a unitary mismatch reports
    // the circuit's image of the input, padded to the circuit width.
    let circuit = circuit("H 0\nCX 0 1\nTICK\nI 999\n");
    let checks = check_unsigned_stabilizer_flows_with_diagnostics(
        &circuit,
        &[flow("Z_ -> XX"), flow("Z -> Z")],
    );
    assert!(
        checks
            .first()
            .is_some_and(UnsignedStabilizerFlowCheck::has_flow)
    );
    let mut actual_output = "XX".to_owned();
    actual_output.push_str(&"_".repeat(998));
    assert_eq!(
        checks.get(1).and_then(UnsignedStabilizerFlowCheck::failure),
        Some(&UnsignedStabilizerFlowFailure::OutputMismatch {
            expected_output: pauli("Z"),
            actual_output: pauli(&actual_output),
        })
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
    assert_matches!(
        failure,
        UnsignedStabilizerFlowFailure::UnsupportedCircuit { .. },
        "{text}: {failure:?}",
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
            .is_some_and(UnsignedStabilizerFlowCheck::has_flow)
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
                .map(UnsignedStabilizerFlowCheck::has_flow)
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
fn check_if_circuit_has_unsigned_stabilizer_flows_marker_skips_circuit_observables() {
    // The circuit's own observables 0 and 1 must not be mistaken for the flows' tracking labels.
    let circuit = circuit(
        "
        M 0
        OBSERVABLE_INCLUDE(0) X0
        OBSERVABLE_INCLUDE(1) rec[-1]
        ",
    );
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(
            &circuit,
            &[flow("Z -> rec[-1]"), flow("Z -> Z"), flow("X -> X")]
        ),
        vec![true, true, false]
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

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn flow(text: &str) -> Flow {
    Flow::from_str(text).expect("parse flow")
}

fn pauli(text: &str) -> PauliString {
    PauliString::from_str(text).expect("parse pauli")
}
