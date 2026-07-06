#![allow(
    clippy::expect_used,
    reason = "PFM2 QEC inverse MPP parity tests mirror compact upstream examples"
)]

use stab_core::{Circuit, circuit_inverse_qec};

#[test]
fn circuit_inverse_qec_supports_selected_mpp_detector_flow() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec mpp behavior.
    for (input_text, expected_text) in [
        (
            "
            MPP !X0*X1 Y0*Y1 Z0*Z1
            DETECTOR rec[-1] rec[-2] rec[-3]
            ",
            "
            MPP Z1*Z0 Y1*Y0 X1*!X0
            DETECTOR rec[-3] rec[-2] rec[-1]
            ",
        ),
        (
            "
            MPP[m] !X0*X1 Y0*Y1 Z0*Z1
            DETECTOR[d](7) rec[-1] rec[-2] rec[-3]
            ",
            "
            MPP[m] Z1*Z0 Y1*Y0 X1*!X0
            DETECTOR[d](7) rec[-3] rec[-2] rec[-1]
            ",
        ),
        (
            "
            MPP !X0*X0
            DETECTOR rec[-1]
            ",
            "
            MPP X0*!X0
            DETECTOR rec[-1]
            ",
        ),
    ] {
        let input = circuit(input_text);
        let expected = circuit(expected_text);

        assert_eq!(
            circuit_inverse_qec(&input).expect("inverse selected MPP detector flow"),
            expected,
            "{input_text}"
        );
        assert_eq!(
            input
                .inverse_qec()
                .expect("method inverse selected MPP detector flow"),
            expected,
            "{input_text}"
        );
    }
}

#[test]
fn circuit_inverse_qec_rejects_unpromoted_mpp_shapes() {
    for circuit_text in [
        "MPP X0*X1 Z0*Z1\nDETECTOR rec[-2]\n",
        "MPP X0*X1 Z0*Z1\nDETECTOR rec[-1]\n",
        "MPP X0*Y1*Z2\nDETECTOR rec[-1]\n",
        "MPP X0*X1 Z0*Z1\nDETECTOR\n",
        "MPP\nDETECTOR rec[-1]\n",
        "MPP X0*X1 Z0*Z1\nDETECTOR rec[-1] rec[-1] rec[-2]\n",
        "MPP(0.125) X0*X1 Z0*Z1\nDETECTOR rec[-1] rec[-2]\n",
        "MPP X0*Z0\nDETECTOR rec[-1]\n",
        "MPP X0*X1 Z0*Z1\nDETECTOR rec[-1] rec[-2]\nDETECTOR rec[-1]\n",
    ] {
        let error = circuit_inverse_qec(&circuit(circuit_text))
            .expect_err("unpromoted MPP inverse-QEC shape is rejected")
            .to_string();

        assert!(
            error.contains("inverse_qec selected MPP detector subset")
                || error.contains("anti-Hermitian")
                || error.contains("operation DETECTOR is not unitary")
                || error.contains("operation MPP is not unitary"),
            "{circuit_text}: {error}"
        );
    }
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect(text)
}
