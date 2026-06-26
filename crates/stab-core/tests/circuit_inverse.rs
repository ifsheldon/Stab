#![allow(
    clippy::expect_used,
    reason = "M6 inverse-circuit parity tests mirror compact upstream examples"
)]

use stab_core::{Circuit, Tableau};

#[test]
fn circuit_inverse_unitary_matches_stim_example() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_inverse_unitary.test.cc.
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

    assert_eq!(input.inverse_unitary().expect("inverse"), expected);
}

#[test]
fn circuit_inverse_unitary_rejects_measurements_like_stim() {
    assert!(circuit("M 0").inverse_unitary().is_err());
}

#[test]
fn circuit_inverse_unitary_composes_to_identity_tableau() {
    let input = circuit(
        "
        SQRT_Y_DAG 1
        CZ 0 1
        SQRT_Y 1
        S 0
    ",
    );
    let inverse = input.inverse_unitary().expect("inverse");
    let input_tableau = input
        .to_tableau(false, false, false)
        .expect("input tableau");
    let inverse_tableau = inverse
        .to_tableau(false, false, false)
        .expect("inverse tableau");
    assert_eq!(
        input_tableau
            .then(&inverse_tableau)
            .expect("compose inverse"),
        Tableau::identity(2)
    );
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}
