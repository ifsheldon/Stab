#![allow(
    clippy::expect_used,
    reason = "M6 circuit-flow-generator parity tests mirror compact upstream examples"
)]

use stab_core::{Circuit, Flow, circuit_flow_generators};

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
fn circuit_flow_generators_rejects_measurement_rich_flows_for_later_slices() {
    assert!(circuit_flow_generators(&circuit("M 0\n")).is_err());
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
