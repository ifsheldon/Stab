#![allow(
    clippy::expect_used,
    reason = "M6 circuit-flow-generator parity tests mirror compact upstream examples"
)]

use stab_core::{
    Circuit, Flow, check_if_circuit_has_unsigned_stabilizer_flows, circuit_flow_generators,
};

#[test]
fn circuit_flow_generators_empty_and_single_qubit_unitaries_match_stim() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_flow_generators.test.cc.
    assert_eq!(
        circuit_flow_generators(&circuit("")).expect("empty generators"),
        Vec::<Flow>::new()
    );
    assert_eq!(generator_strings("X 0\n"), vec!["X -> X", "Z -> -Z"]);
    assert_eq!(generator_strings("H 0\n"), vec!["X -> Z", "Z -> X"]);
    assert_eq!(generator_strings("S 0\n"), vec!["X -> Y", "Z -> Z"]);
    assert_eq!(generator_strings("S_DAG 0\n"), vec!["X -> -Y", "Z -> Z"]);
}

#[test]
fn circuit_flow_generators_composed_unitary_matches_stim() {
    assert_eq!(
        generator_strings(
            "
            SQRT_X 0
            S 0
        ",
        ),
        vec!["X -> Y", "Z -> X"]
    );
}

#[test]
fn circuit_flow_generators_promotes_spp_measurement_row_unitary_examples() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_flow_generators.test.cc.
    assert_eq!(generator_strings("SPP Z0\n"), vec!["X -> Y", "Z -> Z"]);
    assert_eq!(generator_strings("SPP X0 Z0\n"), vec!["X -> Y", "Z -> X"]);
    assert_eq!(
        generator_strings("SPP X0*X1\n"),
        vec!["_X -> _X", "_Z -> -XY", "X_ -> X_", "Z_ -> -YX"]
    );
    assert_eq!(generator_strings("SPP_DAG Z0\n"), vec!["X -> -Y", "Z -> Z"]);
}

#[test]
fn circuit_flow_generators_two_qubit_unitary_order_matches_stim() {
    assert_eq!(
        generator_strings("ISWAP 3 1 2 3\n"),
        vec![
            "___X -> _YZ_",
            "___Z -> _Z__",
            "__X_ -> __ZY",
            "__Z_ -> ___Z",
            "_X__ -> -_ZXZ",
            "_Z__ -> __Z_",
            "X___ -> X___",
            "Z___ -> Z___",
        ]
    );
}

#[test]
fn circuit_flow_generators_promotes_single_instruction_measurement_subset() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_flow_generators.test.cc.
    assert_eq!(
        generator_strings("M 0\n"),
        vec!["1 -> Z xor rec[0]", "Z -> rec[0]"]
    );
    assert_eq!(
        generator_strings("M 0 0\n"),
        vec!["1 -> rec[0] xor rec[1]", "1 -> Z xor rec[1]", "Z -> rec[1]",]
    );
    assert_eq!(
        generator_strings("MX 0\n"),
        vec!["1 -> X xor rec[0]", "X -> rec[0]"]
    );
    assert_eq!(
        generator_strings("MY 0\n"),
        vec!["1 -> Y xor rec[0]", "Y -> rec[0]"]
    );
    assert_eq!(generator_strings("R 0\n"), vec!["1 -> Z"]);
    assert_eq!(generator_strings("RX 0\n"), vec!["1 -> X"]);
    assert_eq!(generator_strings("RY 0\n"), vec!["1 -> Y"]);
    assert_eq!(generator_strings("MR 0\n"), vec!["1 -> Z", "Z -> rec[0]"]);
    assert_eq!(generator_strings("MRX 0\n"), vec!["1 -> X", "X -> rec[0]"]);
    assert_eq!(generator_strings("MRY 0\n"), vec!["1 -> Y", "Y -> rec[0]"]);
    assert_eq!(
        generator_strings("MPAD 0 1 1 0\n"),
        vec!["1 -> rec[0]", "1 -> rec[3]", "1 -> -rec[1]", "1 -> -rec[2]"]
    );
    assert_eq!(
        generator_strings("M 0\nCX rec[-1] 0\n"),
        vec!["1 -> Z", "Z -> rec[0]"]
    );
    assert_eq!(
        generator_strings("M 0\nXCZ 0 rec[-1]\n"),
        vec!["1 -> Z", "Z -> rec[0]"]
    );
    assert_eq!(
        generator_strings("M 0\nCY rec[-1] 1\n"),
        vec![
            "1 -> Z_ xor rec[0]",
            "_X -> _X xor rec[0]",
            "_Z -> _Z xor rec[0]",
            "Z_ -> rec[0]",
        ]
    );
    assert_eq!(
        generator_strings("MPP X0*X1\nCX rec[-1] 0\n"),
        vec![
            "1 -> XX xor rec[0]",
            "_X -> _X",
            "X_ -> _X xor rec[0]",
            "ZZ -> ZZ xor rec[0]",
        ]
    );
    assert_eq!(
        generator_strings("MXX 2 0\n"),
        vec![
            "1 -> X_X xor rec[0]",
            "__X -> __X",
            "_X_ -> _X_",
            "_Z_ -> _Z_",
            "X__ -> __X xor rec[0]",
            "Z_Z -> Z_Z",
        ]
    );
    assert_eq!(
        generator_strings("MYY 3 1 2 3\n"),
        vec![
            "1 -> __YY xor rec[1]",
            "1 -> _Y_Y xor rec[0]",
            "___Y -> ___Y",
            "__Y_ -> ___Y xor rec[1]",
            "_XZZ -> _ZZX xor rec[0]",
            "_ZZZ -> _ZZZ",
            "X___ -> X___",
            "Z___ -> Z___",
        ]
    );
    assert_eq!(
        generator_strings("MZZ 3 1 2 3\n"),
        vec![
            "1 -> __ZZ xor rec[1]",
            "1 -> _Z_Z xor rec[0]",
            "___Z -> ___Z",
            "__Z_ -> ___Z xor rec[1]",
            "_XXX -> _XXX",
            "_Z__ -> ___Z xor rec[0]",
            "X___ -> X___",
            "Z___ -> Z___",
        ]
    );
    assert_eq!(
        generator_strings("MPP Y0*Y1 Y2*Y3\n"),
        vec![
            "1 -> __YY xor rec[1]",
            "1 -> YY__ xor rec[0]",
            "___Y -> ___Y",
            "__XZ -> __ZX xor rec[1]",
            "__ZZ -> __ZZ",
            "_Y__ -> _Y__",
            "XZ__ -> ZX__ xor rec[0]",
            "ZZ__ -> ZZ__",
        ]
    );
    assert_eq!(
        generator_strings("MPP X0*X0\n"),
        vec!["1 -> rec[0]", "X -> X", "Z -> Z"]
    );
    assert_eq!(
        generator_strings("MPP !X0*X0\n"),
        vec!["1 -> -rec[0]", "X -> X", "Z -> Z"]
    );
    assert_eq!(
        generator_strings("MPP X0 X1*X1\n"),
        vec![
            "1 -> rec[1]",
            "1 -> X_ xor rec[0]",
            "_X -> _X",
            "_Z -> _Z",
            "X_ -> rec[0]",
        ]
    );
    assert_eq!(
        generator_strings(
            "
            HERALDED_ERASE(0.04) 1
            HERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 1
            TICK
            MPP X0*Y1*Z2 Z0*Z1
        ",
        ),
        vec![
            "1 -> rec[0]",
            "1 -> rec[1]",
            "1 -> XYZ xor rec[2]",
            "1 -> ZZ_ xor rec[3]",
            "__Z -> __Z",
            "_ZX -> _ZX",
            "XXX -> _ZY xor rec[2]",
            "Z_X -> _ZX xor rec[3]",
        ]
    );
}

#[test]
fn circuit_flow_generators_promotes_python_multi_target_examples() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_flow_generators_test.py.
    assert_eq!(
        generator_strings("M 1 2\n"),
        vec![
            "1 -> __Z xor rec[1]",
            "1 -> _Z_ xor rec[0]",
            "__Z -> rec[1]",
            "_Z_ -> rec[0]",
            "X__ -> X__",
            "Z__ -> Z__",
        ]
    );
    assert_eq!(
        generator_strings("MX 1 2\n"),
        vec![
            "1 -> __X xor rec[1]",
            "1 -> _X_ xor rec[0]",
            "__X -> rec[1]",
            "_X_ -> rec[0]",
            "X__ -> X__",
            "Z__ -> Z__",
        ]
    );
    let expected_pair_y_generators = vec![
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
    assert_eq!(
        generator_strings("MYY 1 2 3 4\n"),
        expected_pair_y_generators
    );
    assert_eq!(
        generator_strings("MPP Y1*Y2 Y3*Y4\n"),
        expected_pair_y_generators
    );
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
        "MPAD 0 1 1 0\n",
        "
        HERALDED_ERASE(0.04) 1
        HERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 1
        TICK
        MPP X0*Y1*Z2 Z0*Z1
        ",
    ] {
        let circuit = circuit(text);
        let flows = circuit_flow_generators(&circuit).expect(text);
        assert_eq!(
            check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows),
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
            check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows),
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
            check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows),
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
        assert_eq!(
            flows,
            circuit_flow_generators(&original.decomposed().expect("decompose SPP circuit"))
                .expect("decomposed generators"),
            "{text}"
        );
        assert_eq!(
            check_if_circuit_has_unsigned_stabilizer_flows(&original, &flows),
            vec![true; flows.len()],
            "{text}"
        );
    }
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
            check_if_circuit_has_unsigned_stabilizer_flows(&repeat_circuit, &repeat_flows),
            vec![true; repeat_flows.len()],
            "{repeat_text}"
        );
    }
}

#[test]
fn circuit_flow_generators_rejects_unpromoted_measurement_rich_shapes() {
    for text in [
        "MR 0 0\n",
        "M 0\nCX sweep[0] 0\n",
        "M 0\nCX rec[-1] 0 1 2\n",
        "M 0\nCX 1 2 rec[-1] 0\n",
    ] {
        let error = circuit_flow_generators(&circuit(text))
            .expect_err("unpromoted measurement-rich flow generator shape")
            .to_string();
        assert!(
            error.contains("circuit_flow_generators only supports"),
            "{text}: {error}"
        );
    }
}

#[test]
fn circuit_flow_generators_rejects_excessive_repeat_measurement_expansion() {
    let error = circuit_flow_generators(&circuit("REPEAT 1000000 {\n    M 0\n}\n"))
        .expect_err("excessive repeat-contained measurement flow generator")
        .to_string();
    assert!(
        error.contains("measurement-rich flow-generator rows")
            && error.contains("current limit 4096"),
        "unexpected excessive repeat error: {error}"
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
