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

/// One time-reversal row: `(name, circuit, flows, keep measurements, expected circuit and
/// flows)`. `None` expectations come from Python examples that only require the reversed
/// circuit to satisfy the reversed flows.
type ReversalCase = (
    &'static str,
    &'static str,
    &'static [&'static str],
    bool,
    Option<(&'static str, &'static [&'static str])>,
);

/// Exact Stim v1.16.0 `time_reversed_for_flows` examples (C++ `circuit_inverse_qec` tests, Python
/// examples, and direct probes) beyond the pinned common matrix. Every row additionally checks
/// that the reversed circuit satisfies every reversed flow.
const REVERSAL_CASES: &[ReversalCase] = &[
    (
        "flow past the end of the circuit",
        "H 0\n",
        &["X300*Z0 -> X300*X0"],
        false,
        Some(("H 0\n", &["X300*X0 -> X300*Z0"])),
    ),
    (
        "extra idle flow qubits",
        "H 2\n",
        &["X300 -> X300", "X2*Z301 -> Z2*Z301"],
        false,
        Some(("H 2\n", &["X300 -> X300", "Z2*Z301 -> X2*Z301"])),
    ),
    (
        "SWAP",
        "SWAP 0 1\n",
        &["X0 -> X1", "Z1 -> Z0"],
        false,
        Some(("SWAP 0 1\n", &["X1 -> X0", "Z0 -> Z1"])),
    ),
    (
        "SQRT_X",
        "SQRT_X 0\n",
        &["X0 -> X0", "Z0 -> -Y0"],
        false,
        Some(("SQRT_X_DAG 0\n", &["X0 -> X0", "Y0 -> Z0"])),
    ),
    (
        "ISWAP",
        "ISWAP 0 1\n",
        &["X0 -> Z0*Y1", "X1 -> Y0*Z1"],
        false,
        Some(("ISWAP_DAG 0 1\n", &["Z0*Y1 -> X0", "Y0*Z1 -> X1"])),
    ),
    (
        "flow through MZZ",
        "MZZ 0 1\n",
        &[
            "X0*X1 -> Y0*Y1 xor rec[-1]",
            "X0*X1 -> X0*X1",
            "Z0 -> Z1 xor rec[-1]",
            "Z0 -> Z0",
        ],
        false,
        Some((
            "MZZ 0 1\n",
            &[
                "Y0*Y1 -> X0*X1 xor rec[-1]",
                "X0*X1 -> X0*X1",
                "Z1 -> Z0 xor rec[-1]",
                "Z0 -> Z0",
            ],
        )),
    ),
    (
        "retained MX",
        "MX 0\n",
        &["1 -> X0 xor rec[-1]"],
        false,
        Some(("MX 0\n", &["X0 -> rec[-1]"])),
    ),
    (
        "retained MY",
        "MY 0\n",
        &["1 -> Y0 xor rec[-1]"],
        false,
        Some(("MY 0\n", &["Y0 -> rec[-1]"])),
    ),
    (
        "retained MXX",
        "MXX 0 1\n",
        &["1 -> X0*X1 xor rec[-1]"],
        false,
        Some(("MXX 0 1\n", &["X0*X1 -> rec[-1]"])),
    ),
    (
        "retained MYY",
        "MYY 0 1\n",
        &["1 -> Y0*Y1 xor rec[-1]"],
        false,
        Some(("MYY 0 1\n", &["Y0*Y1 -> rec[-1]"])),
    ),
    (
        "Python measurement ordering M",
        "M 0 1\n",
        &["1 -> Z0 xor rec[-2]", "1 -> Z1 xor rec[-1]"],
        false,
        None,
    ),
    (
        "Python measurement ordering MZZ",
        "MZZ 0 1 2 3\n",
        &["1 -> Z0*Z1 xor rec[-2]", "1 -> Z2*Z3 xor rec[-1]"],
        false,
        None,
    ),
    (
        "Python measurement ordering MR",
        "MR 0 1\n",
        &["Z0 -> rec[-2]", "Z1 -> rec[-1]", "1 -> Z0", "1 -> Z1"],
        false,
        None,
    ),
    (
        "M with explicit empty output becomes R",
        "M 0\n",
        &["Z0 -> _ xor rec[-1]"],
        false,
        Some(("R 0\n", &["_ -> Z0"])),
    ),
    (
        "MX becomes RX",
        "MX 0\n",
        &["X0 -> rec[-1]"],
        false,
        Some(("RX 0\n", &["1 -> X0"])),
    ),
    (
        "MY becomes RY",
        "MY 0\n",
        &["Y0 -> rec[-1]"],
        false,
        Some(("RY 0\n", &["1 -> Y0"])),
    ),
    (
        "dont_turn_measurements_into_resets keeps M",
        "M 0\n",
        &["Z0 -> rec[-1]"],
        true,
        Some(("M 0\n", &["1 -> Z0 xor rec[-1]"])),
    ),
    (
        "MR",
        "MR 0\n",
        &["1 -> Z0", "Z0 -> rec[-1]"],
        false,
        Some(("MR 0\n", &["Z0 -> rec[-1]", "1 -> Z0"])),
    ),
    (
        "MRX",
        "MRX 0\n",
        &["1 -> X0", "X0 -> rec[-1]"],
        false,
        Some(("MRX 0\n", &["X0 -> rec[-1]", "1 -> X0"])),
    ),
    (
        "MRY",
        "MRY 0\n",
        &["1 -> Y0", "Y0 -> rec[-1]"],
        false,
        Some(("MRY 0\n", &["Y0 -> rec[-1]", "1 -> Y0"])),
    ),
    (
        "multi-target MR",
        "MR 0 1\n",
        &[
            "Z0 -> rec[-2]",
            "Z1 -> rec[-1]",
            "1 -> Z0",
            "1 -> Z1",
            "1 -> Z0*Z1",
        ],
        false,
        Some((
            "MR 1 0\n",
            &[
                "1 -> Z0",
                "1 -> Z1",
                "Z0 -> rec[-1]",
                "Z1 -> rec[-2]",
                "Z0*Z1 -> rec[-2] xor rec[-1]",
            ],
        )),
    ),
    (
        "multi-target MRX",
        "MRX 0 1\n",
        &["1 -> X0*X1"],
        false,
        Some(("MRX 1 0\n", &["X0*X1 -> rec[-2] xor rec[-1]"])),
    ),
    (
        "multi-target MRY",
        "MRY 0 1\n",
        &["1 -> Y0*Y1"],
        false,
        Some(("MRY 1 0\n", &["Y0*Y1 -> rec[-2] xor rec[-1]"])),
    ),
    (
        "inverted MR",
        "MR !0\n",
        &["1 -> Z0", "Z0 -> rec[-1]"],
        false,
        Some(("MR !0\n", &["Z0 -> rec[-1]", "1 -> Z0"])),
    ),
    (
        "inverted first MR target",
        "MR !0 1\n",
        &["Z0*Z1 -> rec[-2] xor rec[-1]", "1 -> Z0", "1 -> Z1"],
        false,
        Some((
            "MR 1 !0\n",
            &["1 -> Z0*Z1", "Z0 -> rec[-1]", "Z1 -> rec[-2]"],
        )),
    ),
    (
        "inverted second MR target",
        "MR 0 !1\n",
        &["Z0*Z1 -> rec[-2] xor rec[-1]", "1 -> Z0", "1 -> Z1"],
        false,
        Some((
            "MR !1 0\n",
            &["1 -> Z0*Z1", "Z0 -> rec[-1]", "Z1 -> rec[-2]"],
        )),
    ),
    (
        "inverted MRX",
        "MRX !0\n",
        &["1 -> X0", "X0 -> rec[-1]"],
        false,
        Some(("MRX !0\n", &["X0 -> rec[-1]", "1 -> X0"])),
    ),
    (
        "inverted second MRY target",
        "MRY 0 !1\n",
        &["Y0*Y1 -> rec[-2] xor rec[-1]", "1 -> Y0*Y1"],
        false,
        Some((
            "MRY !1 0\n",
            &["1 -> Y0*Y1", "Y0*Y1 -> rec[-2] xor rec[-1]"],
        )),
    ),
    (
        "R becomes M",
        "R 0\n",
        &["1 -> Z0"],
        false,
        Some(("M 0\n", &["Z0 -> rec[-1]"])),
    ),
    (
        "RX becomes MX",
        "RX 0\n",
        &["1 -> X0"],
        false,
        Some(("MX 0\n", &["X0 -> rec[-1]"])),
    ),
    (
        "RY becomes MY",
        "RY 0\n",
        &["1 -> Y0"],
        false,
        Some(("MY 0\n", &["Y0 -> rec[-1]"])),
    ),
    (
        "multi-target R",
        "R 0 1\n",
        &["1 -> Z0", "1 -> Z1", "1 -> Z0*Z1"],
        false,
        Some((
            "M 1 0\n",
            &[
                "Z0 -> rec[-1]",
                "Z1 -> rec[-2]",
                "Z0*Z1 -> rec[-2] xor rec[-1]",
            ],
        )),
    ),
    (
        "multi-target RX",
        "RX 0 1\n",
        &["1 -> X0*X1"],
        false,
        Some(("MX 1 0\n", &["X0*X1 -> rec[-2] xor rec[-1]"])),
    ),
    (
        "multi-target RY",
        "RY 0 1\n",
        &["1 -> Y0*Y1"],
        false,
        Some(("MY 1 0\n", &["Y0*Y1 -> rec[-2] xor rec[-1]"])),
    ),
    (
        "three-target R",
        "R 0 1 2\n",
        &["1 -> Z0*Z1*Z2"],
        false,
        Some((
            "M 2 1 0\n",
            &["Z0*Z1*Z2 -> rec[-3] xor rec[-2] xor rec[-1]"],
        )),
    ),
    (
        "flow flip",
        "MY 0\nMRX 0\nMR 1\nR 0\n",
        &[
            "Y0*Z1 -> rec[-3] xor rec[-1]",
            "1 -> Z0*Z1",
            "1 -> Z1",
            "1 -> Z0",
        ],
        false,
        Some((
            "M 0\nMR 1\nMRX 0\nRY 0\n",
            &[
                "1 -> Y0*Z1",
                "Z0*Z1 -> rec[-3] xor rec[-2]",
                "Z1 -> rec[-2]",
                "Z0 -> rec[-3]",
            ],
        )),
    ),
    (
        "flow flip with one selected flow",
        "MY 0\nMRX 0\nMR 1\nR 0\n",
        &["1 -> Z0"],
        false,
        Some(("M 0\nMR 1\nMRX 0\nMY 0\n", &["Z0 -> rec[-4]"])),
    ),
    (
        "flow flip with reordered flows",
        "MY 0\nMRX 0\nMR 1\nR 0\n",
        &[
            "1 -> Z0",
            "1 -> Z1",
            "1 -> Z0*Z1",
            "Y0*Z1 -> rec[-3] xor rec[-1]",
        ],
        false,
        Some((
            "M 0\nMR 1\nMRX 0\nRY 0\n",
            &[
                "Z0 -> rec[-3]",
                "Z1 -> rec[-2]",
                "Z0*Z1 -> rec[-3] xor rec[-2]",
                "1 -> Y0*Z1",
            ],
        )),
    ),
    (
        "measurements across a TICK",
        "M 0\nTICK\nM 1\n",
        &["Z0 -> rec[-2]"],
        false,
        Some(("M 1\nTICK\nR 0\n", &["1 -> Z0"])),
    ),
    (
        "duplicate M targets",
        "M 0 0\n",
        &["1 -> Z0 xor rec[-1]"],
        false,
        Some(("M 0 0\n", &["Z0 -> rec[-2]"])),
    ),
    (
        "duplicate MX targets",
        "MX 0 0\n",
        &["1 -> X0 xor rec[-1]"],
        false,
        Some(("MX 0 0\n", &["X0 -> rec[-2]"])),
    ),
    (
        "duplicate MY targets",
        "MY 0 0\n",
        &["1 -> Y0 xor rec[-1]"],
        false,
        Some(("MY 0 0\n", &["Y0 -> rec[-2]"])),
    ),
    (
        "overlapping MZZ targets",
        "MZZ 0 1 1 2\n",
        &["1 -> Z0*Z1 xor rec[-2]"],
        false,
        Some(("MZZ 1 2 0 1\n", &["Z0*Z1 -> rec[-1]"])),
    ),
];

#[test]
fn time_reversed_for_flows_matches_stim_and_reversed_flows_hold() {
    for &(name, input, input_flows, keep_measurements, expected) in REVERSAL_CASES {
        let input_flows = input_flows
            .iter()
            .map(|text| flow(text))
            .collect::<Vec<_>>();
        let options = TimeReversedForFlowsOptions {
            dont_turn_measurements_into_resets: keep_measurements,
        };
        let (actual, actual_flows) =
            circuit_time_reversed_for_flows_with_options(&circuit(input), &input_flows, options)
                .expect(name);
        assert_eq!(actual_flows.len(), input_flows.len(), "{name}");
        assert!(
            circuit_has_all_unsigned_stabilizer_flows(&actual, &actual_flows),
            "{name}: reversed circuit must satisfy the reversed flows"
        );
        if let Some((expected_circuit, expected_flows)) = expected {
            assert_eq!(actual, circuit(expected_circuit), "{name}");
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
}

#[test]
fn time_reversed_for_flows_rejects_flows_the_input_does_not_satisfy() {
    // A large repeated unitary outside the folded subset is validated on the dense tableau path
    // now that its expanded-instruction cap is retired, so it is accepted when it holds.
    assert!(
        circuit_time_reversed_for_flows(
            &circuit("REPEAT 1001 {\n    SWAP 0 1\n}\n"),
            &[flow("X0 -> X1")]
        )
        .is_ok()
    );
    for (name, input, input_flows, expected_error) in [
        (
            "unitary flow",
            "H 0\n",
            &["Z0 -> Z0"][..],
            "requires input circuit to satisfy flow 0",
        ),
        (
            "general unitary flow",
            "SWAP 0 1\n",
            &["X0 -> X0"][..],
            "requires input circuit to satisfy flow 0",
        ),
        (
            "measurement flow",
            "M 0\n",
            &["X0 -> rec[-1]"][..],
            "didn't satisfy one of the given flows",
        ),
        (
            "anticommuting flow-flip variant",
            "MY 0\nMRX 0\nMR 1\nRY 0\n",
            &[
                "Y0*Z1 -> rec[-3] xor rec[-1]",
                "1 -> Z0*Z1",
                "1 -> Z1",
                "1 -> Z0",
            ][..],
            "anti-commuted",
        ),
    ] {
        let input_flows = input_flows
            .iter()
            .map(|text| flow(text))
            .collect::<Vec<_>>();
        let error = circuit_time_reversed_for_flows(&circuit(input), &input_flows)
            .expect_err(name)
            .to_string();
        assert!(error.contains(expected_error), "{name}: {error}");
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
