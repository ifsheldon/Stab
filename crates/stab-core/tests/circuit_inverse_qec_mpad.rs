#![allow(
    clippy::expect_used,
    reason = "PFM2 QEC inverse MPAD parity tests mirror compact pinned probes"
)]

use std::str::FromStr;

use stab_core::{
    Circuit, Flow, InverseQecOptions, circuit_inverse_qec, circuit_inverse_qec_with_options,
    circuit_time_reversed_for_flows,
};

#[test]
fn circuit_inverse_qec_supports_selected_mpad_record_tail() {
    // Adapted from Stim v1.16.0 time_reversed_for_flows([]) MPAD behavior.
    for (input_text, expected_text) in [
        ("MPAD\n", "MPAD\n"),
        ("MPAD 0\n", "MPAD 0\n"),
        ("MPAD 0 1\n", "MPAD 1 0\n"),
        (
            "
            MPAD 0 1
            DETECTOR rec[-2]
            OBSERVABLE_INCLUDE(0) rec[-1]
            ",
            "
            MPAD 1 0
            DETECTOR rec[-1]
            OBSERVABLE_INCLUDE(0) rec[-2]
            ",
        ),
        (
            "
            MPAD[test](0.125) 0 1
            DETECTOR[d](1, 2) rec[-2] rec[-1]
            ",
            "
            MPAD[test](0.125) 1 0
            DETECTOR[d](1, 2) rec[-2] rec[-1]
            ",
        ),
        (
            "
            MPAD 0 1
            DETECTOR rec[-2] rec[-2]
            OBSERVABLE_INCLUDE(2) rec[-1] rec[-2] rec[-1]
            ",
            "
            MPAD 1 0
            OBSERVABLE_INCLUDE(2) rec[-1]
            ",
        ),
    ] {
        let input = circuit(input_text);
        let expected = circuit(expected_text);

        assert_eq!(
            circuit_inverse_qec(&input).expect("inverse selected MPAD record-tail flow"),
            expected,
            "{input_text}"
        );
        assert_eq!(
            input
                .inverse_qec()
                .expect("method inverse selected MPAD record-tail flow"),
            expected,
            "{input_text}"
        );
    }
}

#[test]
fn time_reversed_for_flows_supports_selected_mpad_empty_flow() {
    let input = circuit(
        "
        MPAD 0 1
        OBSERVABLE_INCLUDE(0) rec[-1]
        DETECTOR rec[-2]
        ",
    );
    let expected = circuit(
        "
        MPAD 1 0
        DETECTOR rec[-1]
        OBSERVABLE_INCLUDE(0) rec[-2]
        ",
    );

    let (actual, flows) =
        circuit_time_reversed_for_flows(&input, &[]).expect("time reverse selected MPAD");

    assert_eq!(actual, expected);
    assert!(flows.is_empty());
}

#[test]
fn time_reversed_for_flows_supports_selected_mpad_pauli_only_flows() {
    for (circuit_text, flows, expected_circuit_text, expected_flows) in [
        ("MPAD 0\n", vec!["X0 -> X0"], "MPAD 0\n", vec!["X0 -> X0"]),
        (
            "
            MPAD 0 1
            DETECTOR rec[-2]
            OBSERVABLE_INCLUDE(0) rec[-1]
            ",
            vec!["X0 -> X0", "Z2 -> Z2"],
            "
            MPAD 1 0
            DETECTOR rec[-1]
            OBSERVABLE_INCLUDE(0) rec[-2]
            ",
            vec!["X0 -> X0", "Z2 -> Z2"],
        ),
    ] {
        let input = circuit(circuit_text);
        let expected_circuit = circuit(expected_circuit_text);
        let input_flows = flows.into_iter().map(flow).collect::<Vec<_>>();
        let expected_flows = expected_flows.into_iter().map(flow).collect::<Vec<_>>();

        let (actual_circuit, actual_flows) = circuit_time_reversed_for_flows(&input, &input_flows)
            .expect("time reverse selected MPAD Pauli-only flows");

        assert_eq!(actual_circuit, expected_circuit, "{circuit_text}");
        assert_eq!(actual_flows, expected_flows, "{circuit_text}");
    }
}

#[test]
fn time_reversed_for_flows_rejects_selected_mpad_unpromoted_flow_terms() {
    for (circuit_text, flow_text, expected_error) in [
        ("MPAD 0\n", "1 -> rec[-1]", "measurement-rich subset"),
        ("MPAD 0\n", "X0 -> X0 xor obs[0]", "measurement-rich subset"),
        (
            "MPAD 0\n",
            "X0 -> Z0",
            "unitary subset requires input circuit to satisfy flow",
        ),
    ] {
        let error = circuit_time_reversed_for_flows(&circuit(circuit_text), &[flow(flow_text)])
            .expect_err("selected MPAD unpromoted flow terms are rejected")
            .to_string();

        assert!(
            error.contains(expected_error),
            "{circuit_text} {flow_text}: {error}"
        );
    }
}

#[test]
fn time_reversed_for_flows_rejects_unpromoted_mpad_shapes_with_pauli_flows() {
    let error = circuit_time_reversed_for_flows(&circuit("MPAD 0\nH 0\n"), &[flow("X0 -> Z0")])
        .expect_err("interleaved MPAD time reversal remains outside the selected packet")
        .to_string();

    assert!(
        error.contains("inverse_qec selected MPAD record-tail subset")
            || error.contains("operation MPAD is not unitary"),
        "{error}"
    );
}

#[test]
fn circuit_inverse_qec_rejects_unpromoted_mpad_shapes() {
    for circuit_text in [
        "MPAD 0 1\nDETECTOR rec[-3]\n",
        "MPAD 0 1\nOBSERVABLE_INCLUDE(0) X0\n",
        "MPAD 0 1\nOBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]\n",
        "MPAD 0 1\nH 0\n",
        "MPAD 0 1\nREPEAT 2 {\n    DETECTOR rec[-1]\n}\n",
    ] {
        let error = circuit_inverse_qec(&circuit(circuit_text))
            .expect_err("unpromoted MPAD inverse-QEC shape is rejected")
            .to_string();

        assert!(
            error.contains("inverse_qec selected MPAD record-tail subset")
                || error.contains("operation MPAD is not unitary"),
            "{circuit_text}: {error}"
        );
    }
}

#[test]
fn circuit_inverse_qec_keep_measurements_rejects_selected_mpad_packet() {
    let error = circuit_inverse_qec_with_options(
        &circuit("MPAD 0\n"),
        InverseQecOptions {
            keep_measurements: true,
        },
    )
    .expect_err("keep_measurements rejects selected MPAD packet")
    .to_string();

    assert!(error.contains("inverse_qec keep_measurements"));
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect(text)
}

fn flow(text: &str) -> Flow {
    Flow::from_str(text).expect("parse flow")
}
