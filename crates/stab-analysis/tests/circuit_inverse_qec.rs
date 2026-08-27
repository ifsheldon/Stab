#![allow(
    clippy::expect_used,
    reason = "M6 QEC inverse parity tests mirror compact upstream examples"
)]

use std::str::FromStr;

use stab_algebra::Flow;
use stab_analysis::{
    InverseQecOptions, TimeReversedForFlowsOptions, circuit_has_all_unsigned_stabilizer_flows,
    circuit_inverse_qec, circuit_inverse_qec_with_options, circuit_time_reversed_for_flows,
    circuit_time_reversed_for_flows_with_options,
};
use stab_model::Circuit;

#[test]
fn inverse_qec_common_semantic_matrix_matches_stim() {
    let cases = [
        (
            "unitary packet",
            "H 0\nISWAP 0 1 1 2 3 2\nS 0 3 4\n",
            "S_DAG 4 3 0\nISWAP_DAG 3 2 1 2 0 1\nH 0\n",
        ),
        (
            "reset measurement detector packet",
            "RX 1\nMX 1\nDETECTOR[tag](2, 3) rec[-1]\n",
            "RX 1\nMX 1\nDETECTOR[tag](2, 3) rec[-1]\n",
        ),
        (
            "two-to-one detector flow",
            "R 0 1\nCX 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n",
            "R 1 0\nCX 0 1\nM 1 0\nDETECTOR rec[-2]\n",
        ),
        (
            "tagged two-to-one detector flow",
            "R[r] 0 1\nCX[c] 0 1\nM[m] 0 1\nDETECTOR[d](7) rec[-1] rec[-2]\n",
            "R[m] 1 0\nCX[c] 0 1\nM[r] 1 0\nDETECTOR[d](7) rec[-2]\n",
        ),
        (
            "noisy measurements",
            "M(0.125) 0 1 2 0 2 4\nMX(0.25) 0\nMY(0.375) 0\n",
            "MY(0.375) 0\nMX(0.25) 0\nM(0.125) 4 2 0 2 1 0\n",
        ),
        (
            "noisy measure resets",
            "MR(0.125) 0 1 2\nMRX(0.25) 0\nMRY(0.375) 0\n",
            "MRY 0\nZ_ERROR(0.375) 0\nMRX 0\nZ_ERROR(0.25) 0\nMR 2 1 0\nX_ERROR(0.125) 2 1 0\n",
        ),
        (
            "noisy pair-product detector flow",
            "MRY 0 1\nM 0\nTICK\nMZZ(0.125) 0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
            "MRY 1 0\nR 1\nTICK\nMZZ(0.125) 2 3 0 1\nTICK\nM 0\nDETECTOR rec[-2] rec[-1]\nMRY 1 0\n",
        ),
        (
            "MPP detector flow",
            "MPP !X0*X1 Y0*Y1 Z0*Z1\nDETECTOR rec[-1] rec[-2] rec[-3]\n",
            "MPP Z1*Z0 Y1*Y0 X1*!X0\nDETECTOR rec[-3] rec[-2] rec[-1]\n",
        ),
        (
            "MPAD detector and observable record tail",
            "MPAD 0 1\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
            "MPAD 1 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]\n",
        ),
        (
            "MPAD observable parity and metadata",
            "MPAD 0 1\nOBSERVABLE_INCLUDE[a](0) rec[-2]\nOBSERVABLE_INCLUDE[b](0) rec[-1]\n",
            "MPAD 1 0\nOBSERVABLE_INCLUDE[a](0) rec[-2] rec[-1]\n",
        ),
        (
            "observable Pauli packet",
            "RX 1\nOBSERVABLE_INCLUDE[test](1) X1\n",
            "OBSERVABLE_INCLUDE[test](1) X1\nMX 1\nOBSERVABLE_INCLUDE(1) rec[-1]\n",
        ),
        (
            "mixed tracker surface",
            "R 0\nH 0\nMX 0\nDETECTOR rec[-1]\n",
            "RX 0\nH 0\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "pair measurements",
            "MXX 0 1\nMYY 2 3\nMZZ 4 5\n",
            "MZZ 4 5\nMYY 2 3\nMXX 0 1\n",
        ),
        (
            "measurement-rich repeat",
            "REPEAT 2 {\n    M(0.125) 0\n}\n",
            "M(0.125) 0 0\n",
        ),
        (
            "tagged detector and tick ordering",
            "R 0 1 2\nTICK[a]\nM 0 1 2\nTICK[b]\nM 0 1 2\nDETECTOR[c](2) rec[-1]\nDETECTOR[d](1) rec[-2]\n",
            "R 2 1\nM 0\nTICK[b]\nM 2 1 0\nTICK[a]\nM 2 1 0\nDETECTOR[c](2) rec[-3]\nDETECTOR[d](1) rec[-2]\n",
        ),
        (
            "targetless products and ordinary noise",
            "X_ERROR(0.125)\nMPP\nMPAD\n",
            "MPAD\nMPP\nX_ERROR(0.125)\n",
        ),
    ];

    for (name, input, expected) in cases {
        assert_eq!(
            circuit_inverse_qec(&circuit(input)).expect(name),
            circuit(expected),
            "{name}"
        );
    }
}

#[test]
fn inverse_qec_keep_measurements_uses_the_general_reverse_flow_contract() {
    let input = circuit("R 0\nH 0\nMX 0\nDETECTOR rec[-1]\n");
    let expected = circuit("MX 0\nH 0\nM 0\nDETECTOR rec[-2] rec[-1]\n");

    assert_eq!(
        circuit_inverse_qec_with_options(
            &input,
            InverseQecOptions {
                keep_measurements: true,
            },
        )
        .expect("inverse mixed QEC circuit while retaining measurements"),
        expected
    );
}

#[test]
fn inverse_qec_rejects_anticommuting_measure_reset() {
    let input = circuit("R 0\nMX 0\nMR 0\nDETECTOR rec[-1]\n");
    let error = circuit_inverse_qec(&input)
        .expect_err("anticommuting reset, measurement, and measure-reset packet")
        .to_string();
    assert!(error.contains("anti-commuted"), "{error}");
}

#[test]
fn time_reversal_for_flows_common_semantic_matrix_matches_stim() {
    let empty_flow_cases = [
        ("empty circuit", "", ""),
        (
            "reset H MX detector",
            "R 0\nH 0\nMX 0\nDETECTOR rec[-1]\n",
            "RX 0\nH 0\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "unitary packet",
            "H 0\nISWAP 0 1 1 2 3 2\nS 0 3 4\n",
            "S_DAG 4 3 0\nISWAP_DAG 3 2 1 2 0 1\nH 0\n",
        ),
        (
            "ordinary measurement noise",
            "MR(0.125) 0\n",
            "MR 0\nX_ERROR(0.125) 0\n",
        ),
        (
            "product measurements",
            "MPP X0*Y1 Z2*X3\n",
            "MPP X3*Z2 Y1*X0\n",
        ),
        (
            "MPAD record routing",
            "MPAD 0 1\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
            "MPAD 1 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]\n",
        ),
        (
            "generated repetition code with detector coordinates",
            include_str!("consolidation_matrix/repetition_code_memory_d3_none.stim"),
            concat!(
                "R 4 2 0\n",
                "MR 3 1\n",
                "TICK\n",
                "CX 4 3 2 1\n",
                "TICK\n",
                "CX 2 3 0 1\n",
                "TICK\n",
                "MR 3 1\n",
                "DETECTOR(1, 1) rec[-1]\n",
                "DETECTOR(3, 1) rec[-2]\n",
                "TICK\n",
                "CX 4 3 2 1\n",
                "TICK\n",
                "CX 2 3 0 1\n",
                "TICK\n",
                "MR 3 1\n",
                "DETECTOR(1, 0) rec[-3] rec[-1]\n",
                "DETECTOR(3, 0) rec[-4] rec[-2]\n",
                "TICK\n",
                "CX 4 3 2 1\n",
                "TICK\n",
                "CX 2 3 0 1\n",
                "TICK\n",
                "M 4 3 2 1 0\n",
                "DETECTOR(1, 2) rec[-3] rec[-2] rec[-1]\n",
                "DETECTOR(3, 2) rec[-5] rec[-4] rec[-3]\n",
                "DETECTOR(1, 1) rec[-6] rec[-2]\n",
                "DETECTOR(3, 1) rec[-7] rec[-4]\n",
                "OBSERVABLE_INCLUDE(0) rec[-5]\n",
            ),
        ),
    ];
    for (name, input, expected) in empty_flow_cases {
        let (actual, flows) = circuit_time_reversed_for_flows(&circuit(input), &[]).expect(name);
        assert_eq!(actual, circuit(expected), "{name}");
        assert!(flows.is_empty(), "{name}");
    }

    type FlowCase = (
        &'static str,
        &'static str,
        &'static [&'static str],
        &'static str,
        &'static [&'static str],
    );
    let flow_cases: &[FlowCase] = &[
        (
            "folded self-inverse unitary flow",
            "REPEAT 1000001 {\n    H 0\n}\n",
            &["X -> Z"],
            "REPEAT 1000001 {\n    H 0\n}\n",
            &["Z -> X"],
        ),
        (
            "folded controlled unitary flow",
            "REPEAT 1000001 {\n    CY 0 1\n}\n",
            &["X0 -> X0*Y1"],
            "REPEAT 1000001 {\n    CY 0 1\n}\n",
            &["X0*Y1 -> X0"],
        ),
        (
            "folded non-self-inverse unitary flow",
            "REPEAT 1000001 {\n    SQRT_X 0\n}\n",
            &["Y0 -> Z0"],
            "REPEAT 1000001 {\n    SQRT_X_DAG 0\n}\n",
            &["Z0 -> Y0"],
        ),
        (
            "measurement becomes reset",
            "M 0\n",
            &["Z0 -> rec[-1]"],
            "R 0\n",
            &["1 -> Z0"],
        ),
        (
            "measurement retained by a future Pauli",
            "M 0\n",
            &["1 -> Z0 xor rec[-1]"],
            "M 0\n",
            &["Z0 -> rec[-1]"],
        ),
        (
            "pair measurement with unitary suffix",
            "MZZ 0 1\nH 0\nCX 0 1\nS 1\n",
            &["X0*X1 -> X0*Z1 xor rec[-1]", "Z0 -> Z0*Z1 xor rec[-1]"],
            "S_DAG 1\nCX 0 1\nH 0\nMZZ 0 1\n",
            &["X0*Z1 -> X0*X1 xor rec[-1]", "Z0*Z1 -> Z0 xor rec[-1]"],
        ),
        (
            "bounded measurement repeat",
            "REPEAT 2 {\n    M 0\n}\n",
            &[],
            "M 0 0\n",
            &[],
        ),
    ];
    for (name, input, input_flows, expected, expected_flows) in flow_cases {
        let input_flows = input_flows
            .iter()
            .map(|text| flow(text))
            .collect::<Vec<_>>();
        let (actual, actual_flows) =
            circuit_time_reversed_for_flows(&circuit(input), &input_flows).expect(name);
        assert_eq!(actual, circuit(expected), "{name}");
        assert_eq!(
            actual_flows,
            expected_flows
                .iter()
                .map(|text| flow(text))
                .collect::<Vec<_>>(),
            "{name}"
        );
    }
}

#[test]
fn circuit_inverse_qec_supports_measure_reset_pass_through_detector_flow() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec pass_through behavior.
    for (input_text, expected_text) in [
        (
            "
            R 0
            M 0
            MR 0
            DETECTOR rec[-1]
            ",
            "
            MR 0
            M 0 0
            DETECTOR rec[-1]
            ",
        ),
        (
            "
            RX 0
            MX 0
            MRX 0
            DETECTOR rec[-1]
            ",
            "
            MRX 0
            MX 0 0
            DETECTOR rec[-1]
            ",
        ),
        (
            "
            RY 0
            MY 0
            MRY 0
            DETECTOR rec[-1]
            ",
            "
            MRY 0
            MY 0 0
            DETECTOR rec[-1]
            ",
        ),
        (
            "
            R[r] 0
            M[m] 0
            MR[mr] 0
            DETECTOR[d](5) rec[-1]
            ",
            "
            MR[mr] 0
            M[m] 0
            M[r] 0
            DETECTOR[d](5) rec[-1]
            ",
        ),
        (
            "
            R 0 1
            M 0 1
            MR 0 1
            DETECTOR rec[-2] rec[-1]
            ",
            "
            MR 1 0
            M 1 0 1 0
            DETECTOR rec[-2] rec[-1]
            ",
        ),
        (
            "
            R 0 1
            M 0 1
            MR 0 1
            DETECTOR rec[-2]
            ",
            "
            MR 1 0
            M 1 0 1 0
            DETECTOR rec[-1]
            ",
        ),
        (
            "
            R 0 1
            M 0 1
            MR 0 1
            DETECTOR rec[-1]
            ",
            "
            MR 1 0
            M 1 0 1 0
            DETECTOR rec[-2]
            ",
        ),
    ] {
        let input = circuit(input_text);
        let expected = circuit(expected_text);

        assert_eq!(
            circuit_inverse_qec(&input).expect("inverse measure-reset pass-through"),
            expected,
            "{input_text}"
        );
    }
}

#[test]
fn circuit_inverse_qec_simplifies_measure_reset_pass_through_detector_parity() {
    for (input_text, expected_text) in [
        (
            "
            R 0
            M 0
            MR 0
            DETECTOR rec[-1] rec[-1]
            ",
            "
            MR 0
            M 0 0
            ",
        ),
        (
            "
            R 0
            M 0
            MR 0
            DETECTOR
            ",
            "
            MR 0
            M 0 0
            ",
        ),
    ] {
        let input = circuit(input_text);
        let expected = circuit(expected_text);

        assert_eq!(
            circuit_inverse_qec(&input).expect("inverse measure-reset detector parity"),
            expected,
            "{input_text}"
        );
    }
}

#[test]
fn time_reversed_for_flows_unitary_subset_supports_flow_past_end() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec flow-past-end coverage.
    let input = circuit("H 0\n");
    let flows = [flow("X300*Z0 -> X300*X0")];

    let (actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&input, &flows).expect("time reverse flows");

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
fn pfm_b1_python_measurement_ordering_m() {
    // Adapted from Stim v1.16.0 Python test_measurement_ordering.
    let input = circuit("M 0 1\n");
    let input_flows = [flow("1 -> Z0 xor rec[-2]"), flow("1 -> Z1 xor rec[-1]")];

    let (inverse, flows) = circuit_time_reversed_for_flows(&input, &input_flows)
        .expect("reverse multi-target measurement ordering");

    assert_eq!(flows.len(), input_flows.len());
    assert!(circuit_has_all_unsigned_stabilizer_flows(&inverse, &flows));
}

#[test]
fn pfm_b1_python_measurement_ordering_mzz() {
    // Adapted from Stim v1.16.0 Python test_measurement_ordering_2.
    let input = circuit("MZZ 0 1 2 3\n");
    let input_flows = [
        flow("1 -> Z0*Z1 xor rec[-2]"),
        flow("1 -> Z2*Z3 xor rec[-1]"),
    ];

    let (inverse, flows) = circuit_time_reversed_for_flows(&input, &input_flows)
        .expect("reverse pair-measurement ordering");

    assert_eq!(flows.len(), input_flows.len());
    assert!(circuit_has_all_unsigned_stabilizer_flows(&inverse, &flows));
}

#[test]
fn pfm_b1_python_measurement_ordering_mr() {
    // Adapted from Stim v1.16.0 Python test_measurement_ordering_3.
    let input = circuit("MR 0 1\n");
    let input_flows = [
        flow("Z0 -> rec[-2]"),
        flow("Z1 -> rec[-1]"),
        flow("1 -> Z0"),
        flow("1 -> Z1"),
    ];

    let (inverse, flows) = circuit_time_reversed_for_flows(&input, &input_flows)
        .expect("reverse multi-target measure-reset ordering");

    assert_eq!(flows.len(), input_flows.len());
    assert!(circuit_has_all_unsigned_stabilizer_flows(&inverse, &flows));
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
fn time_reversed_for_flows_measurement_rich_subset_can_keep_measurements() {
    // Adapted from Stim v1.16.0 Python time_reversed_for_flows
    // dont_turn_measurements_into_resets example.
    let options = TimeReversedForFlowsOptions {
        dont_turn_measurements_into_resets: true,
    };
    let input = circuit("M 0\n");
    let flows = [flow("Z0 -> rec[-1]")];

    let (actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows_with_options(&input, &flows, options)
            .expect("time reverse measurement without converting to reset");

    assert_eq!(actual_circuit, input);
    assert_eq!(actual_flows, vec![flow("1 -> Z0 xor rec[-1]")]);
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
fn time_reversed_for_flows_measurement_rich_subset_reverses_multi_target_measure_resets() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec measurement_ordering_3 coverage.
    let input = circuit("MR 0 1\n");
    let input_flows = [
        flow("Z0 -> rec[-2]"),
        flow("Z1 -> rec[-1]"),
        flow("1 -> Z0"),
        flow("1 -> Z1"),
        flow("1 -> Z0*Z1"),
    ];

    let (actual_circuit, actual_flows) = circuit_time_reversed_for_flows(&input, &input_flows)
        .expect("time reverse multi-target measure-reset");

    assert_eq!(actual_circuit, circuit("MR 1 0\n"));
    assert_eq!(
        actual_flows,
        vec![
            flow("1 -> Z0"),
            flow("1 -> Z1"),
            flow("Z0 -> rec[-1]"),
            flow("Z1 -> rec[-2]"),
            flow("Z0*Z1 -> rec[-2] xor rec[-1]"),
        ]
    );

    for (circuit_text, expected_circuit, input_flow, expected_flow) in [
        (
            "MRX 0 1\n",
            "MRX 1 0\n",
            "1 -> X0*X1",
            "X0*X1 -> rec[-2] xor rec[-1]",
        ),
        (
            "MRY 0 1\n",
            "MRY 1 0\n",
            "1 -> Y0*Y1",
            "Y0*Y1 -> rec[-2] xor rec[-1]",
        ),
    ] {
        let (actual_circuit, actual_flows) =
            circuit_time_reversed_for_flows(&circuit(circuit_text), &[flow(input_flow)])
                .expect("time reverse multi-target measure-reset basis");

        assert_eq!(actual_circuit, circuit(expected_circuit), "{circuit_text}");
        assert_eq!(actual_flows, vec![flow(expected_flow)], "{circuit_text}");
    }
}

#[test]
fn time_reversed_for_flows_measurement_rich_subset_reverses_inverted_measure_resets() {
    // Adapted from Stim v1.16.0 inverted measure-reset time_reversed_for_flows behavior.
    for (circuit_text, input_flows, expected_circuit, expected_flows) in [
        (
            "MR !0\n",
            vec!["1 -> Z0", "Z0 -> rec[-1]"],
            "MR !0\n",
            vec!["Z0 -> rec[-1]", "1 -> Z0"],
        ),
        (
            "MR !0 1\n",
            vec!["Z0*Z1 -> rec[-2] xor rec[-1]", "1 -> Z0", "1 -> Z1"],
            "MR 1 !0\n",
            vec!["1 -> Z0*Z1", "Z0 -> rec[-1]", "Z1 -> rec[-2]"],
        ),
        (
            "MR 0 !1\n",
            vec!["Z0*Z1 -> rec[-2] xor rec[-1]", "1 -> Z0", "1 -> Z1"],
            "MR !1 0\n",
            vec!["1 -> Z0*Z1", "Z0 -> rec[-1]", "Z1 -> rec[-2]"],
        ),
        (
            "MRX !0\n",
            vec!["1 -> X0", "X0 -> rec[-1]"],
            "MRX !0\n",
            vec!["X0 -> rec[-1]", "1 -> X0"],
        ),
        (
            "MRY 0 !1\n",
            vec!["Y0*Y1 -> rec[-2] xor rec[-1]", "1 -> Y0*Y1"],
            "MRY !1 0\n",
            vec!["1 -> Y0*Y1", "Y0*Y1 -> rec[-2] xor rec[-1]"],
        ),
    ] {
        let actual_input_flows = input_flows.into_iter().map(flow).collect::<Vec<_>>();

        let (actual_circuit, actual_flows) =
            circuit_time_reversed_for_flows(&circuit(circuit_text), &actual_input_flows)
                .expect("time reverse inverted measure-reset targets");

        assert_eq!(actual_circuit, circuit(expected_circuit), "{circuit_text}");
        assert_eq!(
            actual_flows,
            expected_flows.into_iter().map(flow).collect::<Vec<_>>(),
            "{circuit_text}"
        );
    }
}

#[test]
fn time_reversed_for_flows_measurement_rich_subset_reverses_resets() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec reset-to-measurement reversal behavior.
    for (circuit_text, input_flow, expected_circuit, expected_flow) in [
        ("R 0\n", "1 -> Z0", "M 0\n", "Z0 -> rec[-1]"),
        ("RX 0\n", "1 -> X0", "MX 0\n", "X0 -> rec[-1]"),
        ("RY 0\n", "1 -> Y0", "MY 0\n", "Y0 -> rec[-1]"),
    ] {
        let (actual_circuit, actual_flows) =
            circuit_time_reversed_for_flows(&circuit(circuit_text), &[flow(input_flow)])
                .expect("time reverse selected reset basis");

        assert_eq!(actual_circuit, circuit(expected_circuit), "{circuit_text}");
        assert_eq!(actual_flows, vec![flow(expected_flow)], "{circuit_text}");
    }
}

#[test]
fn time_reversed_for_flows_measurement_rich_subset_reverses_multi_target_resets() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec two_to_one reset ordering.
    let input = circuit("R 0 1\n");
    let input_flows = [flow("1 -> Z0"), flow("1 -> Z1"), flow("1 -> Z0*Z1")];

    let (actual_circuit, actual_flows) = circuit_time_reversed_for_flows(&input, &input_flows)
        .expect("time reverse multi-target reset");

    assert_eq!(actual_circuit, circuit("M 1 0\n"));
    assert_eq!(
        actual_flows,
        vec![
            flow("Z0 -> rec[-1]"),
            flow("Z1 -> rec[-2]"),
            flow("Z0*Z1 -> rec[-2] xor rec[-1]"),
        ]
    );

    for (circuit_text, expected_circuit, input_flow, expected_flow) in [
        (
            "RX 0 1\n",
            "MX 1 0\n",
            "1 -> X0*X1",
            "X0*X1 -> rec[-2] xor rec[-1]",
        ),
        (
            "RY 0 1\n",
            "MY 1 0\n",
            "1 -> Y0*Y1",
            "Y0*Y1 -> rec[-2] xor rec[-1]",
        ),
    ] {
        let (actual_circuit, actual_flows) =
            circuit_time_reversed_for_flows(&circuit(circuit_text), &[flow(input_flow)])
                .expect("time reverse multi-target reset basis");

        assert_eq!(actual_circuit, circuit(expected_circuit), "{circuit_text}");
        assert_eq!(actual_flows, vec![flow(expected_flow)], "{circuit_text}");
    }

    let (actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&circuit("R 0 1 2\n"), &[flow("1 -> Z0*Z1*Z2")])
            .expect("time reverse three-target reset");

    assert_eq!(actual_circuit, circuit("M 2 1 0\n"));
    assert_eq!(
        actual_flows,
        vec![flow("Z0*Z1*Z2 -> rec[-3] xor rec[-2] xor rec[-1]")]
    );
}

#[test]
fn time_reversed_for_flows_measurement_rich_subset_supports_flow_flip() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec flow_flip coverage.
    let input = circuit(
        "
        MY 0
        MRX 0
        MR 1
        R 0
    ",
    );
    let input_flows = [
        flow("Y0*Z1 -> rec[-3] xor rec[-1]"),
        flow("1 -> Z0*Z1"),
        flow("1 -> Z1"),
        flow("1 -> Z0"),
    ];

    let (actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&input, &input_flows).expect("time reverse flow_flip");

    assert_eq!(
        actual_circuit,
        circuit(
            "
            M 0
            MR 1
            MRX 0
            RY 0
            "
        )
    );
    assert_eq!(
        actual_flows,
        vec![
            flow("1 -> Y0*Z1"),
            flow("Z0*Z1 -> rec[-3] xor rec[-2]"),
            flow("Z1 -> rec[-2]"),
            flow("Z0 -> rec[-3]"),
        ]
    );
}

#[test]
fn time_reversed_for_flows_general_engine_handles_flow_flip_variants() {
    let input = circuit("MY 0\nMRX 0\nMR 1\nR 0\n");
    let (single_inverse, single_flows) =
        circuit_time_reversed_for_flows(&input, &[flow("1 -> Z0")])
            .expect("reverse one selected flow");
    assert_eq!(single_inverse, circuit("M 0\nMR 1\nMRX 0\nMY 0\n"));
    assert_eq!(single_flows, vec![flow("Z0 -> rec[-4]")]);

    let reordered = [
        flow("1 -> Z0"),
        flow("1 -> Z1"),
        flow("1 -> Z0*Z1"),
        flow("Y0*Z1 -> rec[-3] xor rec[-1]"),
    ];
    let (reordered_inverse, reordered_flows) = circuit_time_reversed_for_flows(&input, &reordered)
        .expect("reverse reordered selected flows");
    assert_eq!(reordered_inverse, circuit("M 0\nMR 1\nMRX 0\nRY 0\n"));
    assert_eq!(
        reordered_flows,
        vec![
            flow("Z0 -> rec[-3]"),
            flow("Z1 -> rec[-2]"),
            flow("Z0*Z1 -> rec[-3] xor rec[-2]"),
            flow("1 -> Y0*Z1"),
        ]
    );

    let invalid = circuit("MY 0\nMRX 0\nMR 1\nRY 0\n");
    let error = circuit_time_reversed_for_flows(
        &invalid,
        &[
            flow("Y0*Z1 -> rec[-3] xor rec[-1]"),
            flow("1 -> Z0*Z1"),
            flow("1 -> Z1"),
            flow("1 -> Z0"),
        ],
    )
    .expect_err("anticommuting variant is rejected")
    .to_string();
    assert!(error.contains("anti-commuted"), "{error}");
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
        error.contains("didn't satisfy one of the given flows"),
        "{error}"
    );
}

#[test]
fn time_reversed_for_flows_general_engine_handles_multiple_measurements() {
    let input = circuit("M 0\nTICK\nM 1\n");
    let (inverse, flows) = circuit_time_reversed_for_flows(&input, &[flow("Z0 -> rec[-2]")])
        .expect("reverse multi-instruction measurement flow");

    assert_eq!(inverse, circuit("M 1\nTICK\nR 0\n"));
    assert_eq!(flows, vec![flow("1 -> Z0")]);
}

#[test]
fn time_reversed_for_flows_matches_stim_duplicate_measurement_targets() {
    for (circuit_text, input_flow, expected_circuit, expected_flow) in [
        ("M 0 0\n", "1 -> Z0 xor rec[-1]", "M 0 0\n", "Z0 -> rec[-2]"),
        (
            "MX 0 0\n",
            "1 -> X0 xor rec[-1]",
            "MX 0 0\n",
            "X0 -> rec[-2]",
        ),
        (
            "MY 0 0\n",
            "1 -> Y0 xor rec[-1]",
            "MY 0 0\n",
            "Y0 -> rec[-2]",
        ),
        (
            "MZZ 0 1 1 2\n",
            "1 -> Z0*Z1 xor rec[-2]",
            "MZZ 1 2 0 1\n",
            "Z0*Z1 -> rec[-1]",
        ),
    ] {
        let (actual_circuit, actual_flows) =
            circuit_time_reversed_for_flows(&circuit(circuit_text), &[flow(input_flow)])
                .expect("reverse Stim-accepted duplicate measurement targets");

        assert_eq!(actual_circuit, circuit(expected_circuit), "{circuit_text}");
        assert_eq!(actual_flows, vec![flow(expected_flow)], "{circuit_text}");
    }
}

#[test]
fn inverse_qec_rejects_negative_zero_record_offsets() {
    for circuit_text in [
        "R 0\nM 0\nDETECTOR rec[-0]\n",
        "R 0\nM 0\nMR 0\nDETECTOR rec[-0]\n",
    ] {
        assert!(
            circuit_inverse_qec(&circuit(circuit_text)).is_err(),
            "{circuit_text}"
        );
    }
}

#[test]
fn time_reversed_for_flows_general_engine_preserves_noisy_measurements() {
    let input = circuit("M(0.125) 0\n");
    let (inverse, flows) = circuit_time_reversed_for_flows(&input, &[flow("Z0 -> rec[-1]")])
        .expect("reverse noisy measurement flow");

    assert_eq!(inverse, input);
    assert_eq!(flows, vec![flow("1 -> Z0 xor rec[-1]")]);
}

#[test]
fn time_reversed_for_flows_preserves_duplicate_reset_semantics() {
    for (circuit_text, input_flow, expected_flow) in [
        ("R 0 0\n", "1 -> Z0", "Z0 -> rec[-2]"),
        ("RX 0 0\n", "1 -> X0", "X0 -> rec[-2]"),
        ("RY 0 0\n", "1 -> Y0", "Y0 -> rec[-2]"),
        ("MR 0 0\n", "1 -> Z0", "Z0 -> rec[-2]"),
        ("MRX 0 0\n", "1 -> X0", "X0 -> rec[-2]"),
        ("MRY 0 0\n", "1 -> Y0", "Y0 -> rec[-2]"),
        ("MR 0 1 0\n", "1 -> Z0*Z1", "Z0*Z1 -> rec[-3] xor rec[-2]"),
    ] {
        let (inverse, reversed_flows) =
            circuit_time_reversed_for_flows(&circuit(circuit_text), &[flow(input_flow)])
                .expect("reverse duplicate reset semantics");
        assert_eq!(reversed_flows, vec![flow(expected_flow)], "{circuit_text}");
        assert!(
            circuit_has_all_unsigned_stabilizer_flows(&inverse, &reversed_flows),
            "{circuit_text}: {inverse}\n{reversed_flows:?}"
        );
    }
}

#[test]
fn time_reversed_for_flows_rejects_observable_dependencies_before_expansion() {
    let error = circuit_time_reversed_for_flows(
        &circuit("REPEAT 1000001 {\nR 0\n}\n"),
        &[flow("1 -> Z0 xor obs[7]")],
    )
    .expect_err("reject under-specified observable reversal")
    .to_string();
    assert!(error.contains("flow 0"), "{error}");
    assert!(error.contains("obs[7]"), "{error}");

    let cancelled = stab_algebra::Flow::new(
        stab_algebra::PauliString::identity(0).expect("identity input"),
        stab_algebra::PauliString::identity(0).expect("identity output"),
        [],
        [3, 3],
    )
    .expect("canonical cancelled observable terms");
    assert!(
        circuit_time_reversed_for_flows(&Circuit::new(), &[cancelled]).is_ok(),
        "cancelled observable terms do not survive the Flow boundary"
    );

    let error = circuit_time_reversed_for_flows(&circuit("R 0\n"), &[flow("1 -> Z0 xor rec[0]")])
        .expect_err("reset flow cannot reference a nonexistent measurement")
        .to_string();
    assert!(error.contains("out of range measurement"), "{error}");
}

#[test]
fn time_reversed_for_flows_accepts_and_normalizes_signed_inputs_like_stim() {
    for signed in ["-X -> X", "X -> -X"] {
        let (_, reversed) = circuit_time_reversed_for_flows(&circuit("I 0\n"), &[flow(signed)])
            .expect("signed flow is validated without its sign");
        assert_eq!(reversed, [flow("X -> X")], "{signed}");
    }
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn flow(text: &str) -> Flow {
    Flow::from_str(text).expect("parse flow")
}
