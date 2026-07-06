#![allow(
    clippy::expect_used,
    reason = "PFM2 QEC inverse noisy measurement tests mirror compact upstream examples"
)]

use stab_core::{Circuit, circuit_inverse_qec};

#[test]
fn circuit_inverse_qec_supports_selected_noisy_measurement_only_reverse() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec noisy_m behavior.
    let input = circuit(
        "
        M(0.125) 0 1 2 0 2 4
        MX(0.25) 0
        MY(0.375) 0
    ",
    );
    let expected = circuit(
        "
        MY(0.375) 0
        MX(0.25) 0
        M(0.125) 4 2 0 2 1 0
    ",
    );

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected noisy measurements"),
        expected
    );
    assert_eq!(
        input
            .inverse_qec()
            .expect("method inverse selected noisy measurements"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_preserves_selected_measurement_metadata_and_inversions() {
    let input = circuit(
        "
        M[m](0.125) !0 1
        MX[x](0.25) 2
        MY[y](0.375) 3 !4
    ",
    );
    let expected = circuit(
        "
        MY[y](0.375) !4 3
        MX[x](0.25) 2
        M[m](0.125) 1 !0
    ",
    );

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected tagged noisy measurements"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_drops_empty_selected_measurement_instructions() {
    let input = circuit(
        "
        M
        MX 1
        MY
    ",
    );
    let expected = circuit("MX 1\n");

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected empty noisy measurements"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_rejects_unpromoted_noisy_measurement_shapes() {
    for circuit_text in [
        "MR(0.125) 0\n",
        "MRX(0.125) 0\n",
        "MRY(0.125) 0\n",
        "MXX(0.125) 0 1\n",
        "MYY(0.125) 0 1\n",
        "MZZ(0.125) 0 1\n",
        "MPP(0.125) X0*X1\n",
        "M 0\nDETECTOR rec[-1]\n",
        "M 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        "REPEAT 2 {\n    M(0.125) 0\n}\n",
    ] {
        let error = circuit_inverse_qec(&circuit(circuit_text))
            .expect_err("unpromoted noisy measurement inverse-QEC shape is rejected")
            .to_string();

        assert!(
            error.contains("operation MR is not unitary")
                || error.contains("operation MRX is not unitary")
                || error.contains("operation MRY is not unitary")
                || error.contains("operation MXX is not unitary")
                || error.contains("operation MYY is not unitary")
                || error.contains("operation MZZ is not unitary")
                || error.contains("operation MPP is not unitary")
                || error.contains("operation DETECTOR is not unitary")
                || error.contains("operation OBSERVABLE_INCLUDE is not unitary")
                || error.contains("operation M is not unitary")
                || error.contains("repeat body"),
            "{circuit_text}: {error}"
        );
    }
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect(text)
}
