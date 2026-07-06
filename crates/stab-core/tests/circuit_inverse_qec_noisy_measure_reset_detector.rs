#![allow(
    clippy::expect_used,
    reason = "PFM2 QEC inverse noisy measure-reset detector tests mirror compact upstream examples"
)]

use stab_core::{Circuit, circuit_inverse_qec};

#[test]
fn circuit_inverse_qec_supports_selected_noisy_measure_reset_detector_flow() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec noisy_mr_det behavior.
    let input = circuit(
        "
        MR(0.125) 0
        TICK
        MR(0.25) 0
        MR(0.375) 0
        DETECTOR rec[-1]
    ",
    );
    let expected = circuit(
        "
        MR 0
        X_ERROR(0.375) 0
        MR 0
        X_ERROR(0.25) 0
        DETECTOR rec[-1]
        TICK
        MR 0
        X_ERROR(0.125) 0
    ",
    );

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected noisy measure-reset detector flow"),
        expected
    );
    assert_eq!(
        input
            .inverse_qec()
            .expect("method inverse selected noisy measure-reset detector flow"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_supports_selected_noisy_measure_reset_detector_metadata_and_bases() {
    for (input_text, expected_text) in [
        (
            "
            MR[a](0.125) 0
            TICK[t]
            MR[b](0.25) 0
            MR[c](0.375) 0
            DETECTOR[d](2, 3) rec[-1]
            ",
            "
            MR[c] 0
            X_ERROR[c](0.375) 0
            MR[b] 0
            X_ERROR[b](0.25) 0
            DETECTOR[d](2, 3) rec[-1]
            TICK[t]
            MR[a] 0
            X_ERROR[a](0.125) 0
            ",
        ),
        (
            "
            MRX(0.125) 1
            TICK
            MRX(0.25) 1
            MRX(0.375) 1
            DETECTOR rec[-1]
            ",
            "
            MRX 1
            Z_ERROR(0.375) 1
            MRX 1
            Z_ERROR(0.25) 1
            DETECTOR rec[-1]
            TICK
            MRX 1
            Z_ERROR(0.125) 1
            ",
        ),
        (
            "
            MRY(0.125) 2
            TICK
            MRY(0.25) 2
            MRY(0.375) 2
            DETECTOR rec[-1]
            ",
            "
            MRY 2
            Z_ERROR(0.375) 2
            MRY 2
            Z_ERROR(0.25) 2
            DETECTOR rec[-1]
            TICK
            MRY 2
            Z_ERROR(0.125) 2
            ",
        ),
    ] {
        let input = circuit(input_text);
        let expected = circuit(expected_text);

        assert_eq!(
            circuit_inverse_qec(&input)
                .expect("inverse selected noisy measure-reset detector metadata"),
            expected,
            "{input_text}"
        );
    }
}

#[test]
fn circuit_inverse_qec_rejects_unpromoted_noisy_measure_reset_detector_shapes() {
    for circuit_text in [
        "MR(0.125) 0\nTICK\nMR(0.25) 0\nMR(0.375) 0\nDETECTOR rec[-2]\n",
        "MR(0.125) 0\nTICK\nMR(0.25) 0\nMR(0.375) 0\nDETECTOR rec[-1] rec[-1]\n",
        "MR(0.125) 0\nTICK\nMR(0.25) 0\nMR(0.375) 0\nDETECTOR\n",
        "MR(0.125) 0\nTICK\nMRX(0.25) 0\nMR(0.375) 0\nDETECTOR rec[-1]\n",
        "MR(0.125) 0\nTICK\nMR(0.25) 1\nMR(0.375) 0\nDETECTOR rec[-1]\n",
        "MR(0.125) 0 1\nTICK\nMR(0.25) 0 1\nMR(0.375) 0 1\nDETECTOR rec[-1]\n",
        "MR(0.125) !0\nTICK\nMR(0.25) !0\nMR(0.375) !0\nDETECTOR rec[-1]\n",
        "MR(0.125) 0\nMR(0.25) 0\nMR(0.375) 0\nDETECTOR rec[-1]\n",
        "MR(0.125) 0\nTICK\nMR(0.25) 0\nMR(0.375) 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        "REPEAT 2 {\n    MR(0.125) 0\n}\nTICK\nMR(0.25) 0\nMR(0.375) 0\nDETECTOR rec[-1]\n",
    ] {
        let error = circuit_inverse_qec(&circuit(circuit_text))
            .expect_err("unpromoted noisy measure-reset detector flow is rejected")
            .to_string();

        assert!(
            error.contains("inverse_qec selected noisy measure-reset detector subset")
                || error.contains("inverse_qec selected noisy measure-reset subset")
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
