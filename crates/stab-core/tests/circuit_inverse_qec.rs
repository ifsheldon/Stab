#![allow(
    clippy::expect_used,
    reason = "M6 QEC inverse parity tests mirror compact upstream examples"
)]

use stab_core::{Circuit, circuit_inverse_qec};

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

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}
