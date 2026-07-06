#![allow(
    clippy::expect_used,
    reason = "PFM2 QEC inverse noisy measure-reset tests mirror compact upstream examples"
)]

use stab_core::{Circuit, circuit_inverse_qec};

#[test]
fn circuit_inverse_qec_supports_selected_noisy_measure_reset_only_reverse() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec noisy_mr behavior.
    let input = circuit(
        "
        MR(0.125) 0 1 2 0 2 4
        MRX(0.25) 0
        MRY(0.375) 0
    ",
    );
    let expected = circuit(
        "
        MRY 0
        Z_ERROR(0.375) 0
        MRX 0
        Z_ERROR(0.25) 0
        MR 4 2 0
        X_ERROR(0.125) 4 2 0
        MR 2 1 0
        X_ERROR(0.125) 2 1 0
    ",
    );

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected noisy measure-resets"),
        expected
    );
    assert_eq!(
        input
            .inverse_qec()
            .expect("method inverse selected noisy measure-resets"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_preserves_selected_measure_reset_metadata_and_chunking() {
    let input = circuit(
        "
        MR[m](0.125) 0 1 0 1
        MRX[x](0.25) 2
        MRY[y](0.375) 3 4
    ",
    );
    let expected = circuit(
        "
        MRY[y] 4 3
        Z_ERROR[y](0.375) 4 3
        MRX[x] 2
        Z_ERROR[x](0.25) 2
        MR[m] 1 0
        X_ERROR[m](0.125) 1 0
        MR[m] 1 0
        X_ERROR[m](0.125) 1 0
    ",
    );

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected tagged noisy measure-resets"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_supports_selected_noiseless_measure_reset_only_reverse() {
    let input = circuit(
        "
        MR !0 1
        MRX[x] 2
        MRY[y] 3 !4
    ",
    );
    let expected = circuit(
        "
        MRY[y] !4 3
        MRX[x] 2
        MR 1 !0
    ",
    );

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected noiseless measure-resets"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_drops_empty_selected_measure_reset_instructions() {
    let input = circuit(
        "
        MR(0.125)
        MRX 1
        MRY(0.25)
    ",
    );
    let expected = circuit("MRX 1\n");

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected empty measure-resets"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_rejects_unpromoted_noisy_measure_reset_shapes() {
    for circuit_text in [
        "MR(0.125) !0\n",
        "MRX(0.25) !0\n",
        "MRY(0.375) !0\n",
        "MR(0.125) 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        "REPEAT 2 {\n    MR(0.125) 0\n}\n",
    ] {
        let error = circuit_inverse_qec(&circuit(circuit_text))
            .expect_err("unpromoted noisy measure-reset inverse-QEC shape is rejected")
            .to_string();

        assert!(
            error.contains("inverse_qec selected noisy measure-reset subset")
                || error.contains("operation TICK is not unitary")
                || error.contains("operation DETECTOR is not unitary")
                || error.contains("operation OBSERVABLE_INCLUDE is not unitary")
                || error.contains("operation MR is not unitary")
                || error.contains("repeat body"),
            "{circuit_text}: {error}"
        );
    }
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect(text)
}
