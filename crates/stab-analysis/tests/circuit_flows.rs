#![allow(
    clippy::expect_used,
    reason = "M6 circuit-flow parity tests mirror compact upstream examples"
)]

use std::assert_matches;
use std::str::FromStr;

use stab_algebra::{Flow, FlowMeasurementIndex, PauliString};
use stab_analysis::{
    AnalysisError, AnalysisOperation, FlowVerdict, FlowViolation, ResourceKind,
    UnsignedFlowCheckOptions, check_unsigned_stabilizer_flows, circuit_flow_generators,
    solve_for_flow_measurements,
};
use stab_model::{Cancellation, Circuit, GateKind, MemoryBudget};

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
    assert_eq!(decided(&circuit, &flows), vec![true, true, false]);
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
    assert_eq!(decided(&circuit, &flows), vec![true, true, true]);
}

#[test]
fn unsigned_stabilizer_flow_diagnostics_explain_unitary_mismatches() {
    let circuit = circuit("H 0\n");
    assert_eq!(
        verdicts(&circuit, &[flow("X -> Z"), flow("X -> X")]),
        vec![
            FlowVerdict::Holds,
            FlowVerdict::Violated(FlowViolation::OutputMismatch {
                expected_output: pauli("X"),
                actual_output: pauli("Z"),
            }),
        ]
    );
}

#[test]
fn unsigned_stabilizer_flow_diagnostics_report_output_mismatches_for_wide_unitary_circuits() {
    // Whichever algorithm checks a wide circuit against narrower flows, a unitary mismatch reports
    // the circuit's image of the input, padded to the circuit width.
    let circuit = circuit("H 0\nCX 0 1\nTICK\nI 999\n");
    let mut actual_output = "XX".to_owned();
    actual_output.push_str(&"_".repeat(998));
    assert_eq!(
        verdicts(&circuit, &[flow("Z_ -> XX"), flow("Z -> Z")]),
        vec![
            FlowVerdict::Holds,
            FlowVerdict::Violated(FlowViolation::OutputMismatch {
                expected_output: pauli("Z"),
                actual_output: pauli(&actual_output),
            }),
        ]
    );
}

#[test]
fn unsigned_stabilizer_flow_diagnostics_explain_sparse_tracker_failures() {
    let circuit = circuit("M 0\n");
    assert_eq!(
        verdicts(
            &circuit,
            &[
                flow("Z -> rec[-1]"),
                flow("X -> rec[-1]"),
                flow("Z -> rec[-2]"),
            ],
        ),
        vec![
            FlowVerdict::Holds,
            FlowVerdict::Violated(FlowViolation::InputMismatch {
                expected_input: pauli("X"),
                actual_input: pauli("Z"),
            }),
            FlowVerdict::Violated(FlowViolation::MeasurementRecordOutOfRange {
                record: FlowMeasurementIndex::new(-2),
                measurement_count: 1,
            }),
        ]
    );
}

#[test]
fn unsigned_stabilizer_flow_diagnostics_reject_circuits_without_flow_semantics() {
    // A circuit without well-defined flow semantics is an error for the whole batch, never a
    // verdict: an anti-Hermitian product, and a record lookback before the first measurement.
    for (text, gate) in [
        ("SPP X0*Z0\n", GateKind::SPP),
        ("SPP_DAG X0*Z0\nH 1\n", GateKind::SPP_DAG),
        ("MPP X0*Z0*Y1\n", GateKind::MPP),
    ] {
        let error = check_unsigned_stabilizer_flows(
            &circuit(text),
            &[flow("Z -> Z"), flow("Z1 -> Z1")],
            &UnsignedFlowCheckOptions::default(),
        )
        .expect_err(text);
        assert_matches!(
            error,
            AnalysisError::AntiHermitianPauliProduct { gate: actual, .. } if actual == gate,
            "{text}"
        );
    }
    for text in [
        "DETECTOR rec[-1]\n",
        "M 0\nCX rec[-2] 1\n",
        "M 0\nREPEAT 3 {\n    OBSERVABLE_INCLUDE(0) rec[-2]\n    M 0\n}\n",
    ] {
        let error = check_unsigned_stabilizer_flows(
            &circuit(text),
            &[flow("Z -> Z")],
            &UnsignedFlowCheckOptions::default(),
        )
        .expect_err(text);
        assert_matches!(
            error,
            AnalysisError::MeasurementRecordOutOfRange { .. },
            "{text}"
        );
    }
}

#[test]
fn unsigned_checker_decides_flows_beyond_the_target_range_as_idle() {
    // No circuit target names qubit 2^24 or above, so every circuit is the identity there: a flow
    // holds only if its input and output agree on those qubits, and the reported mismatch strings
    // carry the flow's own terms at those positions.
    let limit = stab_model::QubitId::MAX.get() + 1;
    let unitary = circuit("H 0\n");
    assert_eq!(
        verdicts(
            &unitary,
            &[
                flow("X -> Z"),
                flow(&format!("X{} -> X{}", limit + 1, limit + 1)),
                flow(&format!("Z0*X{limit} -> X0*X{limit}")),
                flow(&format!("X{} -> X{limit}", limit + 1)),
            ],
        ),
        vec![
            FlowVerdict::Holds,
            FlowVerdict::Holds,
            FlowVerdict::Holds,
            FlowVerdict::Violated(FlowViolation::OutputMismatch {
                expected_output: sparse_pauli(&format!("X{limit}")),
                actual_output: sparse_pauli(&format!("X{}", limit + 1)),
            }),
        ]
    );
    let measured = circuit("M 0\n");
    assert_eq!(
        verdicts(
            &measured,
            &[
                flow(&format!("Z0*Y{limit} -> Y{limit} xor rec[-1]")),
                flow(&format!("Z0 -> Y{limit} xor rec[-1]")),
            ],
        ),
        vec![
            FlowVerdict::Holds,
            FlowVerdict::Violated(FlowViolation::InputMismatch {
                expected_input: pauli("Z"),
                actual_input: sparse_pauli(&format!("Z0*Y{limit}")),
            }),
        ]
    );

    // The reported string is admitted against the caller's budget before it is built.
    let refused = check_unsigned_stabilizer_flows(
        &measured,
        &[flow(&format!("X{limit} -> X{limit}"))],
        &UnsignedFlowCheckOptions {
            memory_budget: MemoryBudget::from_bytes(1 << 20),
            ..UnsignedFlowCheckOptions::default()
        },
    )
    .expect_err("a wide pulled-back string exceeds one MiB");
    assert_eq!(
        refused
            .resource_limit_error()
            .map(|limit| (limit.operation(), limit.resource())),
        Some((
            AnalysisOperation::FlowCheck,
            ResourceKind::MaterializedBytes
        )),
        "{refused:?}"
    );
}

#[test]
fn unsigned_checker_reports_budget_refusal_and_cancellation_as_errors() {
    // Neither stopping condition may read as a verdict about the flows.
    let circuit = circuit("M 0\nCX 0 1 1 2 2 3\nM 3\n");
    let flows = [flow("Z___ -> rec[-2]"), flow("X___ -> X___")];
    let refused = check_unsigned_stabilizer_flows(
        &circuit,
        &flows,
        &UnsignedFlowCheckOptions {
            memory_budget: MemoryBudget::from_bytes(1),
            ..UnsignedFlowCheckOptions::default()
        },
    )
    .expect_err("a one-byte budget refuses the sparse tracker");
    assert!(refused.memory_budget_exceeded().is_some(), "{refused:?}");

    let cancellation = Cancellation::new();
    cancellation.cancel();
    assert_matches!(
        check_unsigned_stabilizer_flows(
            &circuit,
            &flows,
            &UnsignedFlowCheckOptions {
                cancellation,
                ..UnsignedFlowCheckOptions::default()
            },
        ),
        Err(AnalysisError::Cancelled(_))
    );
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
        verdicts(&circuit, &flows),
        vec![
            FlowVerdict::Holds,
            FlowVerdict::Violated(FlowViolation::CollapseAnticommutation),
        ]
    );
}

#[test]
fn unsigned_checker_xor_cancels_absolute_relative_record_aliases() {
    let circuit = circuit("M 0\n");
    assert_eq!(
        verdicts(&circuit, &[flow("1 -> rec[-1] xor rec[0]")]),
        vec![FlowVerdict::Holds]
    );
}

#[test]
fn unsigned_checker_empty_batch_skips_dense_validation() {
    let circuit = circuit("H 8191\n");
    assert!(verdicts(&circuit, &[]).is_empty());
}

#[test]
fn unsigned_checker_preserves_flow_qubits_idle_outside_circuit_width() {
    let circuit = circuit("H 1\n");
    let shorter = [flow("Z0 -> Z0"), flow("X0 -> Z0")];
    let empty = Circuit::new();
    let longer = [flow("X1000000 -> X1000000"), flow("X1000000 -> Z1000000")];

    for (circuit, queries) in [(&circuit, &shorter), (&empty, &longer)] {
        assert_eq!(decided(circuit, queries), vec![true, false]);
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
    assert_eq!(decided(&h_circuit, &h_flows), vec![true, true, true, false]);

    let mzz_observable_circuit = circuit(
        "
        MZZ 0 1
        OBSERVABLE_INCLUDE(2) rec[-1]
    ",
    );
    let observable_flows = [
        flow("Z0*Z1 -> obs[2]"),
        flow("X0*X1 -> Y0*Y1 xor obs[2]"),
        flow("X0*X1 -> Y0*Y1 xor obs[1]"),
    ];
    assert_eq!(
        decided(&mzz_observable_circuit, &observable_flows),
        vec![true, true, false]
    );

    let folded_measurement_circuit = circuit(
        "
        REPEAT 1000001 {
            H 0
        }
        M 0
        ",
    );
    assert_eq!(
        decided(
            &folded_measurement_circuit,
            &[flow("X -> rec[-1]"), flow("Z -> rec[-1]")]
        ),
        vec![true, false]
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
        decided(
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
        decided(
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
        decided(
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
        decided(
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
    let flows = [flow("Z__ -> YYZ"), flow("__Z -> __Z"), flow("Z__ -> Z__")];
    assert_eq!(decided(&spp_circuit, &flows), vec![true, true, false]);
    assert_eq!(decided(&spp_dag_circuit, &flows), vec![true, true, false]);
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
        decided(&circuit, &flows),
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
        decided(&circuit, &flows),
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
        decided(&mzz_observable_circuit, &flows),
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
        decided(&observable_pauli_circuit, &observable_pauli_flows),
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
    assert_eq!(decided(&h_repeat_circuit, &flows), vec![true, false]);

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
        decided(&fixed_two_qubit_circuit, &fixed_two_qubit_flows),
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

fn verdicts(circuit: &Circuit, flows: &[Flow]) -> Vec<FlowVerdict> {
    check_unsigned_stabilizer_flows(circuit, flows, &UnsignedFlowCheckOptions::default())
        .expect("default options decide every flow")
}

/// Each flow's verdict as `true` when it holds and `false` when it is violated.
fn decided(circuit: &Circuit, flows: &[Flow]) -> Vec<bool> {
    verdicts(circuit, flows)
        .iter()
        .map(FlowVerdict::holds)
        .collect()
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

/// A Pauli string in the sparse `X0*Z5` notation of flows, as wide as its last term.
fn sparse_pauli(text: &str) -> PauliString {
    flow(&format!("{text} -> 1")).input().clone()
}
