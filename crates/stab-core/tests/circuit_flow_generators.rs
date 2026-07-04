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
fn circuit_flow_generators_rejects_unpromoted_measurement_rich_shapes() {
    for text in [
        "MR 0 0\n",
        "MXX 0 1\nH 0\n",
        "M 0\nH 0\n",
        "M 0\nCX sweep[0] 0\n",
        "REPEAT 2 {\n    M 0\n}\n",
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
fn circuit_flow_generators_measurement_subset_rejects_anti_hermitian_mpp_products() {
    let error = circuit_flow_generators(&circuit("MPP X0*Y0\n"))
        .expect_err("anti-Hermitian MPP product")
        .to_string();
    assert!(
        error.contains("not Hermitian"),
        "unexpected anti-Hermitian MPP error: {error}"
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
