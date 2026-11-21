#![allow(
    clippy::expect_used,
    reason = "M6 circuit-flow-generator parity tests mirror compact upstream examples"
)]

use std::str::FromStr;

use stab_algebra::{Flow, PauliBasis};
use stab_analysis::{circuit_flow_generators, decomposed_circuit};
use stab_model::Circuit;

#[path = "support/flow_verdicts.rs"]
mod flow_verdicts;

use flow_verdicts::flow_holds;

/// Exact generator strings recorded from pinned Stim v1.16.0 (C++ and Python
/// `circuit_flow_generators` tests plus direct probes). Rows already owned by the pinned
/// `pfm_b4_flow_evidence` generator matrix are not repeated here.
const GENERATOR_GOLDENS: &[(&str, &[&str])] = &[
    // Negative-zero record lookbacks resolve like Stim.
    (
        "M 0\nDETECTOR rec[-0]\n",
        &["1 -> Z xor rec[0]", "Z -> rec[0]"],
    ),
    (
        "M 0\nCX rec[-0] 1\nM 1\n",
        &[
            "1 -> _Z xor rec[1]",
            "1 -> Z_ xor rec[0]",
            "_Z -> 1",
            "Z_ -> rec[0]",
        ],
    ),
    // Reset fast paths keep idle lower qubits.
    ("R 1\n", &["1 -> _Z", "X_ -> X_", "Z_ -> Z_"]),
    (
        "MR 1\n",
        &["1 -> _Z", "_Z -> rec[0]", "X_ -> X_", "Z_ -> Z_"],
    ),
    // Resets undone on entangled rows: several rows anticommute with the reset, so one is
    // sacrificed (and swap-removed) and the reset qubit is cleared from the rest.
    (
        "R 0\nH 0\nCX 0 1 0 2\nM 1 2\n",
        &[
            "1 -> __Z xor rec[1]",
            "1 -> _Z_ xor rec[0]",
            "__Z -> Z__ xor rec[1]",
            "_XX -> X__",
            "_Z_ -> Z__ xor rec[0]",
        ],
    ),
    (
        "H 0 1\nCX 0 2 1 2\nMR 2\nRX 0 1\nCX 0 1\nM 0 1\n",
        &[
            "1 -> __Z",
            "1 -> _Z_ xor rec[2]",
            "1 -> Z__ xor rec[1]",
            "XXZ -> rec[0]",
        ],
    ),
    (
        "H 0\nCX 0 1 0 2 0 3\nR 3 0 2\nMX 1\n",
        &[
            "1 -> ___Z",
            "1 -> __Z_",
            "1 -> _X__ xor rec[0]",
            "1 -> Z___",
            "_X__ -> rec[0]",
        ],
    ),
    // Single-instruction measurements and resets in every basis, with inversion.
    ("MX 0\n", &["1 -> X xor rec[0]", "X -> rec[0]"]),
    ("MY 0\n", &["1 -> Y xor rec[0]", "Y -> rec[0]"]),
    ("RX 0\n", &["1 -> X"]),
    ("RY 0\n", &["1 -> Y"]),
    ("MRX 0\n", &["1 -> X", "X -> rec[0]"]),
    ("MRY 0\n", &["1 -> Y", "Y -> rec[0]"]),
    ("MR !0\n", &["1 -> Z", "Z -> -rec[0]"]),
    ("MRX !0\n", &["1 -> X", "X -> -rec[0]"]),
    ("MRY !0\n", &["1 -> Y", "Y -> -rec[0]"]),
    (
        "MR !0 1\n",
        &["1 -> _Z", "1 -> Z_", "_Z -> rec[1]", "Z_ -> -rec[0]"],
    ),
    // MPAD values are records, not simulated qubits.
    ("MPAD 1 0\n", &["1 -> rec[0]", "1 -> -rec[1]"]),
    ("MPAD 1 0\nTICK\n", &["1 -> rec[0]", "1 -> -rec[1]"]),
    ("MPAD 0\nTICK\n", &["1 -> rec[0]"]),
    ("H 0\nMPAD 1\n", &["1 -> -rec[0]", "X -> Z", "Z -> X"]),
    (
        "M 0\nMPAD 1 0\n",
        &[
            "1 -> rec[1]",
            "1 -> -rec[2]",
            "1 -> Z xor rec[0]",
            "Z -> rec[0]",
        ],
    ),
    // Sweep-controlled Paulis are no-ops on either side, with or without measurements.
    ("CX sweep[0] 0\n", &["X -> X", "Z -> Z"]),
    ("CX 0 sweep[0]\n", &["X -> X", "Z -> Z"]),
    ("CY sweep[0] 0\n", &["X -> X", "Z -> Z"]),
    ("CY 0 sweep[0]\n", &["X -> X", "Z -> Z"]),
    ("CZ sweep[0] 0\n", &["X -> X", "Z -> Z"]),
    ("CZ 0 sweep[0]\n", &["X -> X", "Z -> Z"]),
    ("XCZ sweep[0] 0\n", &["X -> X", "Z -> Z"]),
    ("XCZ 0 sweep[0]\n", &["X -> X", "Z -> Z"]),
    ("YCZ sweep[0] 0\n", &["X -> X", "Z -> Z"]),
    ("YCZ 0 sweep[0]\n", &["X -> X", "Z -> Z"]),
    (
        "XCZ 1 sweep[0]\n",
        &["_X -> _X", "_Z -> _Z", "X_ -> X_", "Z_ -> Z_"],
    ),
    (
        "YCZ 1 sweep[0]\n",
        &["_X -> _X", "_Z -> _Z", "X_ -> X_", "Z_ -> Z_"],
    ),
    (
        "CX sweep[0] 1\n",
        &["_X -> _X", "_Z -> _Z", "X_ -> X_", "Z_ -> Z_"],
    ),
    (
        "CY sweep[0] 1\n",
        &["_X -> _X", "_Z -> _Z", "X_ -> X_", "Z_ -> Z_"],
    ),
    (
        "CZ sweep[0] 1\n",
        &["_X -> _X", "_Z -> _Z", "X_ -> X_", "Z_ -> Z_"],
    ),
    (
        "CZ 1 sweep[0]\n",
        &["_X -> _X", "_Z -> _Z", "X_ -> X_", "Z_ -> Z_"],
    ),
    (
        "M 0\nCY sweep[0] 1\n",
        &["1 -> Z_ xor rec[0]", "_X -> _X", "_Z -> _Z", "Z_ -> rec[0]"],
    ),
    (
        "M 0\nCX sweep[0] 0\n",
        &["1 -> Z xor rec[0]", "Z -> rec[0]"],
    ),
    (
        "M 0\nCX 0 sweep[0]\n",
        &["1 -> Z xor rec[0]", "Z -> rec[0]"],
    ),
    (
        "M 0 1\nCX sweep[0] 0 sweep[1] 1\n",
        &[
            "1 -> _Z xor rec[1]",
            "1 -> Z_ xor rec[0]",
            "_Z -> rec[1]",
            "Z_ -> rec[0]",
        ],
    ),
    (
        "M 0\nCZ 0 sweep[0]\n",
        &["1 -> Z xor rec[0]", "Z -> rec[0]"],
    ),
    (
        "M 0\nXCZ 0 sweep[0]\n",
        &["1 -> Z xor rec[0]", "Z -> rec[0]"],
    ),
    (
        "M 0\nYCZ 0 sweep[0]\n",
        &["1 -> Z xor rec[0]", "Z -> rec[0]"],
    ),
    // Product measurements: feedback, self-cancelling products, and inversion.
    (
        "MPP X0*X1\nCX rec[-1] 0\n",
        &[
            "1 -> XX xor rec[0]",
            "_X -> _X",
            "X_ -> _X xor rec[0]",
            "ZZ -> ZZ xor rec[0]",
        ],
    ),
    ("MPP X0*X0\n", &["1 -> rec[0]", "X -> X", "Z -> Z"]),
    ("MPP !X0*X0\n", &["1 -> -rec[0]", "X -> X", "Z -> Z"]),
    (
        "MPP X0 X1*X1\n",
        &[
            "1 -> rec[1]",
            "1 -> X_ xor rec[0]",
            "_X -> _X",
            "_Z -> _Z",
            "X_ -> rec[0]",
        ],
    ),
    // Multi-target heralded noise before a product measurement.
    (
        "HERALDED_ERASE(0.04) 0 2\nMPP X0*X1*Z2\n",
        &[
            "1 -> rec[0]",
            "1 -> rec[1]",
            "1 -> XXZ xor rec[2]",
            "__Z -> __Z",
            "_X_ -> _X_",
            "_ZX -> _ZX",
            "X__ -> _XZ xor rec[2]",
            "Z_X -> Z_X",
        ],
    ),
    (
        "HERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 0 2\nMPP X0*Y1*Z2\n",
        &[
            "1 -> rec[0]",
            "1 -> rec[1]",
            "1 -> XYZ xor rec[2]",
            "__Z -> __Z",
            "_XX -> _XX",
            "_ZX -> _ZX",
            "X__ -> _YZ xor rec[2]",
            "Z_X -> Z_X",
        ],
    ),
    (
        "HERALDED_ERASE(0.04) 0 2\nHERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 1 2\nMPP X0*Y1*Z2\n",
        &[
            "1 -> rec[0]",
            "1 -> rec[1]",
            "1 -> rec[2]",
            "1 -> rec[3]",
            "1 -> XYZ xor rec[4]",
            "__Z -> __Z",
            "_XX -> _XX",
            "_ZX -> _ZX",
            "X__ -> _YZ xor rec[4]",
            "Z_X -> Z_X",
        ],
    ),
    // Python multi-target examples on offset qubits.
    (
        "M 1 2\n",
        &[
            "1 -> __Z xor rec[1]",
            "1 -> _Z_ xor rec[0]",
            "__Z -> rec[1]",
            "_Z_ -> rec[0]",
            "X__ -> X__",
            "Z__ -> Z__",
        ],
    ),
    (
        "MX 1 2\n",
        &[
            "1 -> __X xor rec[1]",
            "1 -> _X_ xor rec[0]",
            "__X -> rec[1]",
            "_X_ -> rec[0]",
            "X__ -> X__",
            "Z__ -> Z__",
        ],
    ),
    ("MYY 1 2 3 4\n", PAIR_Y_GENERATORS),
    ("MPP Y1*Y2 Y3*Y4\n", PAIR_Y_GENERATORS),
];

const PAIR_Y_GENERATORS: &[&str] = &[
    "1 -> ___YY xor rec[1]",
    "1 -> _YY__ xor rec[0]",
    "____Y -> ____Y",
    "___XZ -> ___ZX xor rec[1]",
    "___ZZ -> ___ZZ",
    "__Y__ -> __Y__",
    "_XZ__ -> _ZX__ xor rec[0]",
    "_ZZ__ -> _ZZ__",
    "X____ -> X____",
    "Z____ -> Z____",
];

#[test]
fn circuit_flow_generators_match_stim_goldens() {
    for &(text, expected) in GENERATOR_GOLDENS {
        assert_eq!(generator_strings(text), expected, "{text}");
    }
}

#[test]
fn circuit_flow_generators_measurement_subset_flows_satisfy_checker() {
    for text in [
        "M 0\n",
        "M 0 0\n",
        "MX 0\n",
        "MY 0\n",
        "R 0\n",
        "RX 0\n",
        "RY 0\n",
        "MR 0\n",
        "MRX 0\n",
        "MRY 0\n",
        "MR !0\n",
        "MRX !0\n",
        "MRY !0\n",
        "MR !0 1\n",
        "MXX 2 0\n",
        "MXX !0 1\n",
        "MYY 3 1 2 3\n",
        "MZZ 3 1 2 3\n",
        "MPP Y0*Y1 Y2*Y3\n",
        "MPP X0*X0\n",
        "MPP !X0*X0\n",
        "MPP X0 X1*X1\n",
        "SPP Z0\n",
        "SPP X0 Z0\n",
        "SPP X0*X1\n",
        "SPP_DAG Z0\n",
        "M 0\nCX rec[-1] 0\n",
        "MPP X0*X1\nCX rec[-1] 0\n",
        "M 0\nCY rec[-1] 1\n",
        "M 0\nCY sweep[0] 1\n",
        "M 0\nYCZ 0 sweep[0]\n",
        "MPAD 0 1 1 0\n",
        "
        HERALDED_ERASE(0.04) 1
        HERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 1
        TICK
        MPP X0*Y1*Z2 Z0*Z1
        ",
        "
        HERALDED_ERASE(0.04) 0 2
        MPP X0*X1*Z2
        ",
        "
        HERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 0 2
        MPP X0*Y1*Z2
        ",
        "
        HERALDED_ERASE(0.04) 0 2
        HERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 1 2
        MPP X0*Y1*Z2
        ",
    ] {
        let circuit = circuit(text);
        let flows = circuit_flow_generators(&circuit).expect(text);
        assert_eq!(
            flow_holds(&circuit, &flows),
            vec![true; flows.len()],
            "{text}"
        );
    }
}

#[test]
fn circuit_flow_generators_promotes_composed_measurement_subset() {
    assert_eq!(
        generator_strings("M 0\nTICK\nM 0\n"),
        vec!["1 -> rec[0] xor rec[1]", "1 -> Z xor rec[1]", "Z -> rec[1]",]
    );
    assert_eq!(
        generator_strings("R 0\nM 0\n"),
        vec!["1 -> rec[0]", "1 -> Z"]
    );
    assert_eq!(
        generator_strings("M 0\nR 0\n"),
        vec!["1 -> Z", "Z -> rec[0]"]
    );

    for text in [
        "M 0\nMX 1\nMY 2\n",
        "MXX 0 1\nMZZ 0 1\n",
        "MPP X0*Y1\nMPAD 0 1\n",
        "
        R 0
        M 0
        MR 1
        MPP Z0*X1
        MPAD 0 1
        ",
    ] {
        let circuit = circuit(text);
        let flows = circuit_flow_generators(&circuit).expect(text);
        assert_eq!(
            flow_holds(&circuit, &flows),
            vec![true; flows.len()],
            "{text}"
        );
    }
}

#[test]
fn circuit_flow_generators_promotes_unitary_mixed_measurement_subset() {
    assert_eq!(
        generator_strings("H 0\nM 0\n"),
        vec!["1 -> Z xor rec[0]", "X -> rec[0]"]
    );
    assert_eq!(
        generator_strings("S 0\nMX 0\n"),
        vec!["1 -> X xor rec[0]", "Y -> -rec[0]"]
    );
    assert_eq!(
        generator_strings("S_DAG 0\nMX 0\n"),
        vec!["1 -> X xor rec[0]", "Y -> rec[0]"]
    );

    for text in [
        "M 0\nH 0\n",
        "H 0\nM 0\nS 0\n",
        "MXX 0 1\nH 0\nCX 0 1\n",
        "R 0\nH 0\nM 0\n",
        "
        REPEAT 2 {
            M 0
            H 0
        }
        ",
    ] {
        let circuit = circuit(text);
        let flows = circuit_flow_generators(&circuit).expect(text);
        assert_eq!(
            flow_holds(&circuit, &flows),
            vec![true; flows.len()],
            "{text}"
        );
    }

    assert_eq!(
        circuit_flow_generators(&circuit(
            "
            REPEAT 2 {
                M 0
                H 0
            }
            "
        ))
        .expect("repeat unitary-mixed generators"),
        circuit_flow_generators(&circuit("M 0\nH 0\nM 0\nH 0\n"))
            .expect("expanded unitary-mixed generators")
    );
}

#[test]
fn circuit_flow_generators_measurement_subset_ignores_annotations_and_noise() {
    let base = circuit(
        "
        H 0
        M 0
        CX rec[-1] 1
        MZZ 0 1
        MPAD 0 1
        ",
    );
    let decorated = circuit(
        "
        QUBIT_COORDS(1, 2) 0
        H 0
        X_ERROR(0.125) 0
        Y_ERROR(0.125) 1
        Z_ERROR(0.125) 0
        DEPOLARIZE1(0.125) 0
        DEPOLARIZE2(0.125) 0 1
        PAULI_CHANNEL_1(0.01, 0.02, 0.03) 0
        PAULI_CHANNEL_2(0.001, 0.002, 0.003, 0.004, 0.005, 0.006, 0.007, 0.008, 0.009, 0.010, 0.011, 0.012, 0.013, 0.014, 0.015) 0 1
        CORRELATED_ERROR(0.125) X0
        ELSE_CORRELATED_ERROR(0.125) Z1
        I_ERROR(0.125) 0
        II_ERROR(0.125) 0 1
        SHIFT_COORDS(1, 2)
        M 0
        DETECTOR(5) rec[-1]
        OBSERVABLE_INCLUDE(3) rec[-1]
        CX rec[-1] 1
        MZZ 0 1
        DETECTOR rec[-1]
        OBSERVABLE_INCLUDE(4) X0 Z1
        MPAD 0 1
        TICK
        ",
    );

    let expected = circuit_flow_generators(&base).expect("base generators");
    let actual = circuit_flow_generators(&decorated).expect("decorated generators");
    assert_eq!(actual, expected);
}

#[test]
fn circuit_flow_generators_measurement_subset_composes_spp_unitaries() {
    for text in [
        "SPP Z0\nM 0\n",
        "SPP_DAG Z0\nMX 0\n",
        "SPP X0*X1\nMXX 0 1\n",
        "
        REPEAT 2 {
            SPP X0*X1
            MZZ 0 1
        }
        ",
    ] {
        let original = circuit(text);
        let flows = circuit_flow_generators(&original).expect(text);
        let decomposed = decomposed_circuit(&original).expect("decompose SPP circuit");
        assert_eq!(
            flows,
            circuit_flow_generators(&decomposed).expect("decomposed generators"),
            "{text}"
        );
        assert_eq!(
            flow_holds(&original, &flows),
            vec![true; flows.len()],
            "{text}"
        );
    }
}

#[test]
fn circuit_flow_generators_measurement_subset_promotes_generated_all_operations_fixture() {
    // Adapted from Stim v1.16.0 generate_test_circuit_with_all_operations plus the
    // circuit_flow_generators all_operations property check.
    let sections = [
        "
        QUBIT_COORDS(1, 2, 3) 0

        I 0
        X 1
        Y 2
        Z 3
        TICK

        ",
        "
        C_XYZ 0
        C_NXYZ 1
        C_XNYZ 2
        C_XYNZ 3
        C_ZYX 4
        C_NZYX 5
        C_ZNYX 6
        C_ZYNX 7
        H_XY 0
        H_XZ 1
        H_YZ 2
        H_NXY 3
        H_NXZ 4
        H_NYZ 5
        SQRT_X 0
        SQRT_X_DAG 1
        SQRT_Y 2
        SQRT_Y_DAG 3
        SQRT_Z 4
        SQRT_Z_DAG 5
        TICK

        ",
        "
        CXSWAP 0 1
        ISWAP 2 3
        ISWAP_DAG 4 5
        SWAP 6 7
        SWAPCX 8 9
        CZSWAP 10 11
        SQRT_XX 0 1
        SQRT_XX_DAG 2 3
        SQRT_YY 4 5
        SQRT_YY_DAG 6 7
        SQRT_ZZ 8 9
        SQRT_ZZ_DAG 10 11
        II 12 13
        XCX 0 1
        XCY 2 3
        XCZ 4 5
        YCX 6 7
        YCY 8 9
        YCZ 10 11
        ZCX 12 13
        ZCY 14 15
        ZCZ 16 17
        TICK

        ",
        "
        CORRELATED_ERROR(0.01) X1 Y2 Z3
        ELSE_CORRELATED_ERROR(0.02) X4 Y7 Z6
        DEPOLARIZE1(0.02) 0
        DEPOLARIZE2(0.03) 1 2
        PAULI_CHANNEL_1(0.01, 0.02, 0.03) 3
        PAULI_CHANNEL_2(0.001, 0.002, 0.003, 0.004, 0.005, 0.006, 0.007, 0.008, 0.009, 0.010, 0.011, 0.012, 0.013, 0.014, 0.015) 4 5
        X_ERROR(0.01) 0
        Y_ERROR(0.02) 1
        Z_ERROR(0.03) 2
        HERALDED_ERASE(0.04) 3
        HERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 6
        I_ERROR(0.06) 7
        II_ERROR(0.07) 8 9
        TICK

        ",
        "
        MPP X0*Y1*Z2 Z0*Z1
        SPP X0*Y1*Z2 X3
        SPP_DAG X0*Y1*Z2 X2
        TICK

        ",
        "
        MRX 0
        MRY 1
        MRZ 2
        MX 3
        MY 4
        MZ 5 6
        RX 7
        RY 8
        RZ 9
        TICK

        ",
        "
        MXX 0 1 2 3
        MYY 4 5
        MZZ 6 7
        TICK

        ",
        "
        REPEAT 3 {
            H 0
            CX 0 1
            S 1
            TICK
        }
        TICK

        ",
        "
        MR 0
        X_ERROR(0.1) 0
        MR(0.01) 0
        SHIFT_COORDS(1, 2, 3)
        DETECTOR(1, 2, 3) rec[-1]
        OBSERVABLE_INCLUDE(0) rec[-1]
        MPAD 0 1 0
        OBSERVABLE_INCLUDE(1) Z2 Z3
        TICK

        ",
        "
        MRX !0
        MY !1
        MZZ !2 3
        OBSERVABLE_INCLUDE(1) rec[-1]
        MYY !4 !5
        MPP X6*!Y7*Z8
        TICK

        ",
        "
        CX rec[-1] 0
        CY sweep[0] 1
        CZ 2 rec[-1]
        ",
    ];
    let mut text = String::new();
    for (index, section) in sections.iter().enumerate() {
        text.push_str(section);
        let circuit = circuit(&text);
        let result = circuit_flow_generators(&circuit);
        assert!(
            result.is_ok(),
            "all-operations prefix section {index} failed: {:?}",
            result.err()
        );
    }
    let all_operations_circuit = circuit(&text);
    let flows =
        circuit_flow_generators(&all_operations_circuit).expect("all-operations generators");
    assert!(!flows.is_empty());
    let expected = [
        "1 -> rec[0]",
        "1 -> rec[1]",
        "1 -> rec[13] xor rec[23]",
        "1 -> rec[16]",
        "1 -> rec[17]",
        "1 -> rec[19]",
        "1 -> -rec[18]",
        "1 -> _________Z________",
        "1 -> _______ZY_________ xor rec[10] xor rec[14]",
        "1 -> -______XXX_________ xor rec[10] xor rec[14] xor rec[24]",
        "1 -> ______Z_Y_________ xor rec[10]",
        "1 -> _____Y____________ xor rec[8] xor rec[23]",
        "1 -> ____Y_____________ xor rec[8]",
        "1 -> __XX______________ xor rec[12] xor rec[24]",
        "1 -> -__ZZ______________ xor rec[22]",
        "1 -> -_Y________________ xor rec[21]",
        "1 -> X_________________",
        "X17 -> Z16*X17",
        "Z17 -> Z17",
        "________________X_ -> ________________XZ",
        "________________Z_ -> ________________Z_",
        "_______________X__ -> ______________ZX__",
        "_______________Z__ -> ______________ZZ__",
        "______________X___ -> ______________XY__",
        "______________Z___ -> ______________Z___",
        "_____________X____ -> _____________X____",
        "_____________Z____ -> ____________ZZ____",
        "____________X_____ -> ____________XX____",
        "____________Z_____ -> ____________Z_____",
        "___________X______ -> -__________Y_______",
        "___________Z______ -> __________ZZ______",
        "__________X_______ -> -__________YY______",
        "__________Z_______ -> ___________Z______",
        "______YX__________ -> rec[10]",
        "____XZ____________ -> rec[8]",
        "____ZY____________ -> -rec[9]",
        "___Y______________ -> rec[2] xor rec[4] xor rec[5] xor rec[7]",
        "__X_______________ -> rec[2] xor rec[4] xor rec[5]",
        "XX________________ -> rec[3]",
        "ZY________________ -> rec[4] xor rec[5]",
    ];
    let mut expected = expected
        .into_iter()
        .map(|text| Flow::from_str(text).expect("parse pinned all-operations flow"))
        .collect::<Vec<_>>();
    expected.sort_unstable();
    let mut signed_actual = flows.clone();
    signed_actual.sort_unstable();
    assert_eq!(signed_actual, expected);
    assert_eq!(
        flow_holds(&all_operations_circuit, &flows),
        vec![true; flows.len()]
    );
}

#[test]
fn circuit_flow_generators_promotes_bounded_repeat_measurement_subset() {
    for (repeat_text, expanded_text) in [
        ("REPEAT 2 {\n    M 0\n}\n", "M 0\nM 0\n"),
        (
            "
            M 0
            REPEAT 2 {
                TICK
                MX 1
            }
            ",
            "
            M 0
            TICK
            MX 1
            TICK
            MX 1
            ",
        ),
        (
            "
            REPEAT 2 {
                REPEAT 2 {
                    M 0
                    TICK
                }
            }
            ",
            "
            M 0
            TICK
            M 0
            TICK
            M 0
            TICK
            M 0
            TICK
            ",
        ),
        (
            "
            REPEAT 2 {
                MPP X0*Y1
                MPAD 0
            }
            ",
            "
            MPP X0*Y1
            MPAD 0
            MPP X0*Y1
            MPAD 0
            ",
        ),
    ] {
        let repeat_circuit = circuit(repeat_text);
        let repeat_flows = circuit_flow_generators(&repeat_circuit).expect(repeat_text);
        assert_eq!(
            repeat_flows,
            circuit_flow_generators(&circuit(expanded_text)).expect(expanded_text)
        );
        assert_eq!(
            flow_holds(&repeat_circuit, &repeat_flows),
            vec![true; repeat_flows.len()],
            "{repeat_text}"
        );
    }
}

#[test]
fn circuit_flow_generators_measurement_subset_supports_duplicate_reset_targets() {
    for suffix in ["", "TICK\n"] {
        assert_eq!(generator_strings(&format!("R 0 0\n{suffix}")), ["1 -> Z"]);
        for (instruction, expected) in [
            (
                "MR 0 0",
                vec!["1 -> rec[0] xor rec[1]", "1 -> Z", "Z -> rec[1]"],
            ),
            (
                "MR !0 0",
                vec!["1 -> -rec[0] xor rec[1]", "1 -> Z", "Z -> rec[1]"],
            ),
            (
                "MR 0 !0",
                vec!["1 -> -rec[0] xor rec[1]", "1 -> Z", "Z -> -rec[1]"],
            ),
            (
                "MR 0 0 0",
                vec![
                    "1 -> rec[0] xor rec[2]",
                    "1 -> rec[1] xor rec[2]",
                    "1 -> Z",
                    "Z -> rec[2]",
                ],
            ),
            (
                "MR 0 1 0",
                vec![
                    "1 -> rec[0] xor rec[2]",
                    "1 -> _Z",
                    "1 -> Z_",
                    "_Z -> rec[1]",
                    "Z_ -> rec[2]",
                ],
            ),
        ] {
            assert_eq!(
                generator_strings(&format!("{instruction}\n{suffix}")),
                expected,
                "{instruction} with suffix {suffix:?}"
            );
        }
    }
}

#[test]
fn circuit_flow_generators_measurement_subset_supports_mixed_feedback_capable_groups() {
    for (text, expected) in [
        (
            "M 0\nCX rec[-1] sweep[0]\n",
            vec!["1 -> Z xor rec[0]", "Z -> rec[0]"],
        ),
        (
            "M 0\nCX sweep[0] 0 1 2\n",
            vec![
                "1 -> Z__ xor rec[0]",
                "__X -> __X",
                "__Z -> _ZZ",
                "_X_ -> _XX",
                "_Z_ -> _Z_",
                "Z__ -> rec[0]",
            ],
        ),
        (
            "M 0\nCX rec[-1] 0 1 2\n",
            vec![
                "1 -> Z__",
                "__X -> __X",
                "__Z -> _ZZ",
                "_X_ -> _XX",
                "_Z_ -> _Z_",
                "Z__ -> rec[0]",
            ],
        ),
        (
            "M 0\nCX 1 2 rec[-1] 0\n",
            vec![
                "1 -> Z__",
                "__X -> __X",
                "__Z -> _ZZ",
                "_X_ -> _XX",
                "_Z_ -> _Z_",
                "Z__ -> rec[0]",
            ],
        ),
    ] {
        assert_eq!(generator_strings(text), expected, "{text}");
    }
}

#[test]
fn circuit_flow_generators_measurement_subset_supports_measurement_free_mixed_sweep_groups() {
    let circuit = circuit("CX sweep[0] 0 1 2\n");
    let flows = circuit_flow_generators(&circuit).expect("mixed sweep generators");
    assert_eq!(
        flows.iter().map(ToString::to_string).collect::<Vec<_>>(),
        [
            "__X -> __X",
            "__Z -> _ZZ",
            "_X_ -> _XX",
            "_Z_ -> _Z_",
            "X__ -> X__",
            "Z__ -> Z__",
        ]
    );
    assert_eq!(flow_holds(&circuit, &flows), vec![true; flows.len()]);
}

#[test]
fn circuit_flow_generators_measurement_subset_folds_ignored_only_circuits_without_caps() {
    let high = circuit_flow_generators(&circuit("QUBIT_COORDS(0) 2048\n"))
        .expect("high-index ignored annotation generators");
    assert_eq!(high.len(), 4098);
    let first = high.first().expect("high-index first identity flow");
    assert_eq!(first.input().get(2048), Some(PauliBasis::X));
    assert_eq!(first.output().get(2048), Some(PauliBasis::X));
    let last = high.last().expect("high-index last identity flow");
    assert_eq!(last.input().get(0), Some(PauliBasis::Z));
    assert_eq!(last.output().get(0), Some(PauliBasis::Z));

    assert_eq!(
        generator_strings("REPEAT 1000001 {\n    X_ERROR(0.1) 0\n}\n"),
        ["X -> X", "Z -> Z"]
    );
    assert_eq!(
        generator_strings("H 0\nX_ERROR(0.1) 0\n"),
        ["X -> Z", "Z -> X"]
    );
}

#[test]
fn circuit_flow_generators_measurement_subset_rejects_anti_hermitian_mpp_products() {
    let error = circuit_flow_generators(&circuit("MPP X0*Y0\n"))
        .expect_err("anti-Hermitian MPP product")
        .to_string();
    assert!(
        error.contains("not Hermitian"),
        "unexpected anti-Hermitian MPP error: {error}"
    );

    let error = circuit_flow_generators(&circuit("SPP X0*Z0\n"))
        .expect_err("anti-Hermitian SPP product")
        .to_string();
    assert!(
        error.contains("anti-Hermitian"),
        "unexpected anti-Hermitian SPP error: {error}"
    );

    let error = circuit_flow_generators(&circuit("SPP X0*Z0\nM 0\n"))
        .expect_err("composed anti-Hermitian SPP product")
        .to_string();
    assert!(
        error.contains("anti-Hermitian"),
        "unexpected composed anti-Hermitian SPP error: {error}"
    );
}

fn generator_strings(text: &str) -> Vec<String> {
    circuit_flow_generators(&circuit(text))
        .expect("flow generators")
        .into_iter()
        .map(|flow| flow.to_string())
        .collect()
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}
