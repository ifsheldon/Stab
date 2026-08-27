#![allow(
    clippy::expect_used,
    reason = "M6 Flow parity tests use direct assertions to mirror compact upstream examples"
)]

use std::str::FromStr;

use stab_algebra::{Flow, PauliString, StabilizerError};

#[test]
fn flows_preserve_values_text_multiplication_and_typed_failures() {
    flow_from_str_canonicalizes_duplicate_terms();
    flow_from_str_matches_stim_examples();
    flow_observable_terms_match_stim();
    flow_display_and_sparse_round_trip_match_stim();
    flow_ordering_matches_stim_examples();
    flow_multiplication_matches_stim_examples();
}

fn flow_from_str_canonicalizes_duplicate_terms() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/flow.test.cc.
    assert_eq!(
        flow(
            "X -> Y xor rec[-1] xor rec[-1] xor rec[-1] xor rec[-2] xor rec[-2] xor rec[-3] xor obs[1] xor obs[1] xor obs[3] xor obs[3] xor obs[3]"
        ),
        new_flow(pauli("X"), pauli("Y"), [-3, -1], [3])
    );
}

fn flow_from_str_matches_stim_examples() {
    for text in [
        "",
        "X",
        "X>X",
        "X-X",
        "X > X",
        "X - X",
        "->X",
        "X->",
        "rec[0] -> X",
        "X -> rec[ -1]",
        "X -> X rec[-1]",
        "X -> X xor",
        "X -> rec[-1] xor X",
        "X -> obs[-1]",
        "X -> obs[A]",
        "X -> obs[]",
        "X -> obs[ 5]",
        "X -> rec[]",
    ] {
        assert_eq!(
            Flow::from_str(text),
            Err(StabilizerError::InvalidFlowText {
                text: text.to_owned(),
            }),
            "{text:?}"
        );
    }

    assert_eq!(
        Flow::from_str("iX -> X"),
        Err(StabilizerError::AntiHermitianFlow)
    );

    assert_eq!(flow("1 -> 1"), new_flow(pauli(""), pauli(""), [], []));
    assert_eq!(
        flow("1 -> -rec[0]"),
        new_flow(pauli(""), pauli("-"), [0], [])
    );
    assert_eq!(flow("i -> -i"), new_flow(pauli(""), pauli("-"), [], []));
    assert_eq!(flow("iX -> -iY"), new_flow(pauli("X"), pauli("-Y"), [], []));
    assert_eq!(flow("X->-Y"), new_flow(pauli("X"), pauli("-Y"), [], []));
    assert_eq!(flow("X -> -Y"), new_flow(pauli("X"), pauli("-Y"), [], []));
    assert_eq!(flow("-X -> Y"), new_flow(pauli("-X"), pauli("Y"), [], []));
    assert_eq!(
        flow("XYZ -> -Z_Z"),
        new_flow(pauli("XYZ"), pauli("-Z_Z"), [], [])
    );
    assert_eq!(
        flow("XYZ -> Z_Y xor rec[-1]"),
        new_flow(pauli("XYZ"), pauli("Z_Y"), [-1], [])
    );
    assert_eq!(
        flow("XYZ -> Z_Y xor rec[5]"),
        new_flow(pauli("XYZ"), pauli("Z_Y"), [5], [])
    );
    assert_eq!(
        flow("XYZ -> rec[-1]"),
        new_flow(pauli("XYZ"), pauli(""), [-1], [])
    );
    assert_eq!(
        flow("XYZ -> Z_Y xor rec[-1] xor rec[-3]"),
        new_flow(pauli("XYZ"), pauli("Z_Y"), [-3, -1], [])
    );
    assert_eq!(
        flow("XYZ -> ZIY xor rec[55] xor rec[-3]"),
        new_flow(pauli("XYZ"), pauli("Z_Y"), [-3, 55], [])
    );
    assert_eq!(
        flow("XYZ -> ZIY xor rec[-3] xor rec[55]"),
        new_flow(pauli("XYZ"), pauli("Z_Y"), [-3, 55], [])
    );
    assert_eq!(
        flow("X9 -> -Z5*Y3 xor rec[55] xor rec[-3]"),
        new_flow(pauli("_________X"), pauli("-___Y_Z"), [-3, 55], [])
    );
}

fn flow_observable_terms_match_stim() {
    assert_eq!(
        flow("X9 -> obs[5]"),
        new_flow(pauli("_________X"), pauli(""), [], [5])
    );
    assert_eq!(
        flow("X9 -> X xor obs[5] xor obs[3] xor rec[-1]"),
        new_flow(pauli("_________X"), pauli("X"), [-1], [3, 5])
    );
    assert_eq!(
        flow("X9 -> X xor obs[5] xor rec[-1] xor obs[3]"),
        new_flow(pauli("_________X"), pauli("X"), [-1], [3, 5])
    );
}

fn flow_display_and_sparse_round_trip_match_stim() {
    let value = new_flow(pauli("XY"), pauli("_Z"), [-3], []);
    assert_eq!(value.to_string(), "XY -> _Z xor rec[-3]");
    assert_eq!(flow("X0*Y1 -> Z1 xor rec[-3]"), value);
    assert_eq!(flow("XY -> _Z xor rec[-3]"), value);

    assert_eq!(
        flow("1 -> rec[-1]"),
        new_flow(pauli(""), pauli(""), [-1], [])
    );
    assert_eq!(
        flow("1 -> 1 xor rec[-1]"),
        new_flow(pauli(""), pauli(""), [-1], [])
    );
    assert_eq!(
        flow("1 -> Z9 xor rec[55]"),
        new_flow(pauli(""), pauli("_________Z"), [55], [])
    );
    assert_eq!(
        flow("-1 -> -X xor rec[-1] xor rec[-3]"),
        new_flow(pauli("-"), pauli("-X"), [-3, -1], [])
    );
    assert_eq!(
        flow("X20 -> Y xor rec[-1]").to_string(),
        "X20 -> Y0 xor rec[-1]"
    );
    assert_eq!(
        flow("X20*I21 -> Y xor rec[-1]").to_string(),
        "____________________X_ -> Y xor rec[-1]"
    );
}

fn flow_ordering_matches_stim_examples() {
    assert!(!(flow("1 -> 1") < flow("1 -> 1")));
    assert!(!(flow("X -> 1") < flow("1 -> 1")));
    assert!(!(flow("1 -> X") < flow("1 -> 1")));
    assert!(!(flow("1 -> rec[-1]") < flow("1 -> 1")));
    assert!(flow("1 -> 1") < flow("X -> 1"));
    assert!(flow("1 -> 1") < flow("1 -> X"));
    assert!(flow("1 -> 1") < flow("1 -> rec[-1]"));
    assert!(flow("1 -> -rec[0] xor rec[1]") < flow("1 -> Z"));
    assert!(flow("1 -> Z") < flow("1 -> -Z"));
}

fn flow_multiplication_matches_stim_examples() {
    assert_eq!(
        flow("XYZ -> 1")
            .multiply(&flow("1 -> XYZ"))
            .expect("multiply"),
        flow("XYZ -> XYZ")
    );
    assert_eq!(
        flow("XX_ -> 1")
            .multiply(&flow("_XX -> 1"))
            .expect("multiply"),
        flow("X_X -> 1")
    );
    assert_eq!(
        flow("1 -> XX_")
            .multiply(&flow("1 -> _XX"))
            .expect("multiply"),
        flow("1 -> X_X")
    );
    assert_eq!(
        flow("1 -> rec[-1] xor rec[-3]")
            .multiply(&flow("1 -> rec[-1] xor rec[-2]"))
            .expect("multiply"),
        flow("1 -> rec[-2] xor rec[-3]")
    );
    assert_eq!(
        flow("1 -> obs[1] xor obs[3]")
            .multiply(&flow("1 -> obs[1] xor obs[2]"))
            .expect("multiply"),
        flow("1 -> obs[2] xor obs[3]")
    );
    assert_eq!(
        flow("X -> X").multiply(&flow("Z -> Z")).expect("multiply"),
        flow("Y -> Y")
    );
    assert_eq!(
        flow("1 -> XX")
            .multiply(&flow("1 -> ZZ"))
            .expect("multiply"),
        flow("1 -> -YY")
    );
    assert_eq!(
        flow("1 -> obs[1]")
            .multiply(&flow("1 -> obs[1]"))
            .expect("multiply"),
        flow("1 -> 1")
    );
    assert_eq!(
        flow("1 -> rec[1]")
            .multiply(&flow("1 -> rec[1]"))
            .expect("multiply"),
        flow("1 -> 1")
    );
    for (left, right) in [("1 -> X", "1 -> Y"), ("1 -> Y", "1 -> X")] {
        assert_eq!(
            flow(left).multiply(&flow(right)),
            Err(StabilizerError::InvalidFlowProduct {
                left: left.to_owned(),
                right: right.to_owned(),
            })
        );
    }
    assert_eq!(
        flow("-1 -> 1")
            .multiply(&flow("1 -> 1"))
            .expect("left input sign"),
        flow("-1 -> 1")
    );
    assert_eq!(
        flow("1 -> 1")
            .multiply(&flow("-1 -> 1"))
            .expect("right input sign"),
        flow("1 -> -1")
    );
    assert_eq!(
        flow("-1 -> 1")
            .multiply(&flow("-1 -> 1"))
            .expect("two input signs"),
        flow("-1 -> -1")
    );
    assert_eq!(
        flow("1 -> -1")
            .multiply(&flow("1 -> 1"))
            .expect("left output sign"),
        flow("1 -> -1")
    );
    assert_eq!(
        flow("1 -> 1")
            .multiply(&flow("1 -> -1"))
            .expect("right output sign"),
        flow("1 -> -1")
    );
}

fn pauli(text: &str) -> PauliString {
    PauliString::from_str(text).expect("parse PauliString")
}

fn new_flow(
    input: PauliString,
    output: PauliString,
    measurements: impl IntoIterator<Item = i32>,
    observables: impl IntoIterator<Item = u32>,
) -> Flow {
    Flow::new(input, output, measurements, observables).expect("test Flow stays within its limit")
}

fn flow(text: &str) -> Flow {
    Flow::from_str(text).expect("parse Flow")
}
