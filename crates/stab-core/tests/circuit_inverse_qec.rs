#![allow(
    clippy::expect_used,
    reason = "M6 QEC inverse parity tests mirror compact upstream examples"
)]

use std::str::FromStr;

use stab_core::{Circuit, Flow, circuit_inverse_qec, circuit_time_reversed_for_flows};

#[test]
fn circuit_inverse_qec_unitary_matches_stim() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_inverse_qec.test.cc.
    let input = circuit(
        "
        H 0
        ISWAP 0 1 1 2 3 2
        S 0 3 4
    ",
    );
    let expected = circuit(
        "
        S_DAG 4 3 0
        ISWAP_DAG 3 2 1 2 0 1
        H 0
    ",
    );

    assert_eq!(circuit_inverse_qec(&input).expect("inverse QEC"), expected);
    assert_eq!(input.inverse_qec().expect("method inverse QEC"), expected);
}

#[test]
fn circuit_inverse_qec_rejects_measurement_rewrite_cases_for_later_slices() {
    let input = circuit(
        "
        R 0
        M 0
        DETECTOR rec[-1]
    ",
    );

    assert!(circuit_inverse_qec(&input).is_err());
}

#[test]
fn time_reversed_for_flows_unitary_subset_matches_qec_inverse() {
    let input = circuit(
        "
        H 0
        ISWAP 0 1 1 2 3 2
        S 0 3 4
    ",
    );
    let expected_circuit = circuit(
        "
        S_DAG 4 3 0
        ISWAP_DAG 3 2 1 2 0 1
        H 0
    ",
    );

    let (actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&input, &[]).expect("time reverse empty-flow unitary");

    assert_eq!(actual_circuit, expected_circuit);
    assert_eq!(actual_flows, Vec::<Flow>::new());
}

#[test]
fn time_reversed_for_flows_unitary_subset_supports_flow_past_end() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec flow-past-end coverage.
    let input = circuit("H 0\n");
    let flows = [flow("X300*Z0 -> X300*X0")];

    let (actual_circuit, actual_flows) = input
        .time_reversed_for_flows(&flows)
        .expect("time reverse flows");

    assert_eq!(actual_circuit, input);
    assert_eq!(actual_flows, vec![flow("X300*X0 -> X300*Z0")]);
}

#[test]
fn time_reversed_for_flows_unitary_subset_supports_extra_idle_qubits() {
    // Adapted from Stim v1.16.0 Python time_reversed_for_flows examples.
    let input = circuit("H 2\n");
    let flows = [flow("X300 -> X300"), flow("X2*Z301 -> Z2*Z301")];

    let (actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&input, &flows).expect("time reverse extra qubits");

    assert_eq!(actual_circuit, input);
    assert_eq!(
        actual_flows,
        vec![flow("X300 -> X300"), flow("Z2*Z301 -> X2*Z301")]
    );
}

#[test]
fn time_reversed_for_flows_unitary_subset_folds_large_repeats() {
    let input = circuit(
        "
        REPEAT 1000001 {
            H 0
        }
    ",
    );
    let flows = [flow("X -> Z")];

    let (actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&input, &flows).expect("time reverse repeated unitary");

    assert_eq!(actual_circuit, input);
    assert_eq!(actual_flows, vec![flow("Z -> X")]);
}

#[test]
fn time_reversed_for_flows_unitary_subset_folds_large_cy_repeats() {
    let input = circuit(
        "
        REPEAT 1000001 {
            CY 0 1
        }
    ",
    );
    let flows = [flow("X0 -> X0*Y1")];

    let (actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&input, &flows).expect("time reverse repeated CY");

    assert_eq!(actual_circuit, input);
    assert_eq!(actual_flows, vec![flow("X0*Y1 -> X0")]);
}

#[test]
fn time_reversed_for_flows_unitary_subset_folds_large_sqrt_x_repeats() {
    let input = circuit(
        "
        REPEAT 1000001 {
            SQRT_X 0
        }
    ",
    );
    let expected_circuit = circuit(
        "
        REPEAT 1000001 {
            SQRT_X_DAG 0
        }
    ",
    );
    let flows = [flow("Y0 -> Z0")];

    let (actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&input, &flows).expect("time reverse repeated SQRT_X");

    assert_eq!(actual_circuit, expected_circuit);
    assert_eq!(actual_flows, vec![flow("Z0 -> Y0")]);
}

#[test]
fn time_reversed_for_flows_unitary_subset_validates_general_unitaries_with_tableau() {
    let swap = circuit("SWAP 0 1\n");
    let swap_flows = [flow("X0 -> X1"), flow("Z1 -> Z0")];
    let (_actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&swap, &swap_flows).expect("time reverse swap flows");
    assert_eq!(actual_flows, vec![flow("X1 -> X0"), flow("Z0 -> Z1")]);

    let sqrt_x = circuit("SQRT_X 0\n");
    let sqrt_x_flows = [flow("X0 -> X0"), flow("Z0 -> -Y0")];
    let (_actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&sqrt_x, &sqrt_x_flows).expect("time reverse sqrt_x flows");
    assert_eq!(actual_flows, vec![flow("X0 -> X0"), flow("Y0 -> Z0")]);

    let iswap = circuit("ISWAP 0 1\n");
    let iswap_flows = [flow("X0 -> Z0*Y1"), flow("X1 -> Y0*Z1")];
    let (_actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&iswap, &iswap_flows).expect("time reverse iswap flows");
    assert_eq!(actual_flows, vec![flow("Z0*Y1 -> X0"), flow("Y0*Z1 -> X1")]);
}

#[test]
fn time_reversed_for_flows_measurement_rich_subset_reverses_single_measurement() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec flow_reverse coverage.
    let input = circuit("M 0\n");
    let flows = [flow("1 -> Z0 xor rec[-1]")];

    let (actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&input, &flows).expect("time reverse measurement flow");

    assert_eq!(actual_circuit, input);
    assert_eq!(actual_flows, vec![flow("Z0 -> rec[-1]")]);
}

#[test]
fn time_reversed_for_flows_measurement_rich_subset_reverses_pair_measurement() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec flow_through_mzz coverage.
    let input = circuit("MZZ 0 1\n");
    let flows = [
        flow("X0*X1 -> Y0*Y1 xor rec[-1]"),
        flow("X0*X1 -> X0*X1"),
        flow("Z0 -> Z1 xor rec[-1]"),
        flow("Z0 -> Z0"),
    ];

    let (actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&input, &flows).expect("time reverse MZZ flows");

    assert_eq!(actual_circuit, input);
    assert_eq!(
        actual_flows,
        vec![
            flow("Y0*Y1 -> X0*X1 xor rec[-1]"),
            flow("X0*X1 -> X0*X1"),
            flow("Z1 -> Z0 xor rec[-1]"),
            flow("Z0 -> Z0"),
        ]
    );
}

#[test]
fn time_reversed_for_flows_measurement_rich_subset_covers_selected_bases() {
    for (circuit_text, input_flow, expected_flow) in [
        ("MX 0\n", "1 -> X0 xor rec[-1]", "X0 -> rec[-1]"),
        ("MY 0\n", "1 -> Y0 xor rec[-1]", "Y0 -> rec[-1]"),
        ("MXX 0 1\n", "1 -> X0*X1 xor rec[-1]", "X0*X1 -> rec[-1]"),
        ("MYY 0 1\n", "1 -> Y0*Y1 xor rec[-1]", "Y0*Y1 -> rec[-1]"),
    ] {
        let input = circuit(circuit_text);
        let (actual_circuit, actual_flows) =
            circuit_time_reversed_for_flows(&input, &[flow(input_flow)])
                .expect("time reverse selected measurement basis");

        assert_eq!(actual_circuit, input, "{circuit_text}");
        assert_eq!(actual_flows, vec![flow(expected_flow)], "{circuit_text}");
    }
}

#[test]
fn time_reversed_for_flows_measurement_rich_subset_turns_measurements_into_resets() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec measurement-to-reset reversal behavior.
    for (circuit_text, input_flow, expected_circuit, expected_flow) in [
        ("M 0\n", "Z0 -> rec[-1]", "R 0\n", "1 -> Z0"),
        ("M 0\n", "Z0 -> _ xor rec[-1]", "R 0\n", "_ -> Z0"),
        ("MX 0\n", "X0 -> rec[-1]", "RX 0\n", "1 -> X0"),
        ("MY 0\n", "Y0 -> rec[-1]", "RY 0\n", "1 -> Y0"),
    ] {
        let (actual_circuit, actual_flows) =
            circuit_time_reversed_for_flows(&circuit(circuit_text), &[flow(input_flow)])
                .expect("time reverse reset-convertible measurement");

        assert_eq!(actual_circuit, circuit(expected_circuit), "{circuit_text}");
        assert_eq!(actual_flows, vec![flow(expected_flow)], "{circuit_text}");
    }
}

#[test]
fn time_reversed_for_flows_measurement_rich_subset_reverses_measure_resets() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec measure-reset reversal behavior.
    for (circuit_text, reset_flow, record_flow, expected_reset, expected_record) in [
        (
            "MR 0\n",
            "1 -> Z0",
            "Z0 -> rec[-1]",
            "Z0 -> rec[-1]",
            "1 -> Z0",
        ),
        (
            "MRX 0\n",
            "1 -> X0",
            "X0 -> rec[-1]",
            "X0 -> rec[-1]",
            "1 -> X0",
        ),
        (
            "MRY 0\n",
            "1 -> Y0",
            "Y0 -> rec[-1]",
            "Y0 -> rec[-1]",
            "1 -> Y0",
        ),
    ] {
        let input = circuit(circuit_text);
        let input_flows = [flow(reset_flow), flow(record_flow)];

        let (actual_circuit, actual_flows) = circuit_time_reversed_for_flows(&input, &input_flows)
            .expect("time reverse selected measure-reset basis");

        assert_eq!(actual_circuit, input, "{circuit_text}");
        assert_eq!(
            actual_flows,
            vec![flow(expected_reset), flow(expected_record)],
            "{circuit_text}"
        );
    }
}

#[test]
fn time_reversed_for_flows_unitary_subset_rejects_unsatisfied_general_unitary_flow() {
    let error = circuit_time_reversed_for_flows(&circuit("SWAP 0 1\n"), &[flow("X0 -> X0")])
        .expect_err("swap does not preserve X0")
        .to_string();

    assert!(
        error.contains("requires input circuit to satisfy flow 0"),
        "{error}"
    );
}

#[test]
fn time_reversed_for_flows_unitary_subset_rejects_large_repeated_unitary_outside_folded_subset() {
    let input = circuit(
        "
        REPEAT 1000001 {
            SWAP 0 1
        }
    ",
    );
    let error = circuit_time_reversed_for_flows(&input, &[flow("X0 -> X1")])
        .expect_err("large repeated SWAP is not folded by the scoped validator")
        .to_string();

    assert!(error.contains("folded sparse validation"), "{error}");
}

#[test]
fn time_reversed_for_flows_unitary_subset_rejects_unsatisfied_flow() {
    let error = circuit_time_reversed_for_flows(&circuit("H 0\n"), &[flow("Z0 -> Z0")])
        .expect_err("flow is not satisfied")
        .to_string();

    assert!(
        error.contains("requires input circuit to satisfy flow 0"),
        "{error}"
    );
}

#[test]
fn time_reversed_for_flows_measurement_rich_subset_rejects_unsatisfied_flows() {
    let error = circuit_time_reversed_for_flows(&circuit("M 0\n"), &[flow("X0 -> rec[-1]")])
        .expect_err("measurement-rich flow is not satisfied")
        .to_string();

    assert!(
        error.contains("requires selected measurement-rich circuit to satisfy flow 0"),
        "{error}"
    );
}

#[test]
fn time_reversed_for_flows_measurement_rich_subset_rejects_unpromoted_terms() {
    let error = circuit_time_reversed_for_flows(
        &circuit(
            "
            M 0
            M 1
        ",
        ),
        &[flow("Z0 -> rec[-2]")],
    )
    .expect_err("multi-instruction measurement-rich flow rewrites are not in the scoped subset")
    .to_string();

    assert!(
        error.contains("measurement-rich subset supports only one noiseless plain measurement"),
        "{error}"
    );
}

#[test]
fn time_reversed_for_flows_measurement_rich_subset_rejects_noisy_measurements() {
    let error = circuit_time_reversed_for_flows(&circuit("M(0.125) 0\n"), &[flow("Z0 -> rec[-1]")])
        .expect_err("noisy measurement-rich flow rewrites are not in the scoped subset")
        .to_string();

    assert!(
        error.contains("measurement-rich subset supports only one noiseless plain measurement"),
        "{error}"
    );
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn flow(text: &str) -> Flow {
    Flow::from_str(text).expect("parse flow")
}
