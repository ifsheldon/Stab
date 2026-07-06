#![allow(
    clippy::expect_used,
    reason = "PFM2 QEC inverse m_det tests mirror compact upstream examples"
)]

use stab_core::{Circuit, circuit_inverse_qec};

#[test]
fn circuit_inverse_qec_supports_selected_m_det_detector_flow() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec m_det behavior.
    let input = circuit(
        "
        R 0 1 2
        TICK
        M 0 1 2
        TICK
        M 0 1 2
        DETECTOR(2) rec[-1]
        DETECTOR(1) rec[-2]
    ",
    );
    let expected = circuit(
        "
        R 2 1
        M 0
        TICK
        M 2 1 0
        TICK
        M 2 1 0
        DETECTOR(2) rec[-3]
        DETECTOR(1) rec[-2]
    ",
    );

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected m_det detector flow"),
        expected
    );
    assert_eq!(
        input
            .inverse_qec()
            .expect("method inverse selected m_det detector flow"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_supports_selected_m_det_detector_metadata() {
    let input = circuit(
        "
        R 0 1 2
        TICK[a]
        M 0 1 2
        TICK[b]
        M 0 1 2
        DETECTOR[c](2) rec[-1]
        DETECTOR[d](1) rec[-2]
    ",
    );
    let expected = circuit(
        "
        R 2 1
        M 0
        TICK[a]
        M 2 1 0
        TICK[b]
        M 2 1 0
        DETECTOR[c](2) rec[-3]
        DETECTOR[d](1) rec[-2]
    ",
    );

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected m_det detector metadata"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_rejects_unpromoted_m_det_shapes() {
    for circuit_text in [
        "R 0 1 2\nM 0 1 2\nTICK\nM 0 1 2\nDETECTOR(2) rec[-1]\nDETECTOR(1) rec[-2]\n",
        "R 0 1 2\nTICK\nM 0 1 2\nTICK\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\n",
        "R 0 1 2\nTICK\nM 0 1 2\nTICK\nM 0 1 2\nDETECTOR rec[-1]\nDETECTOR rec[-1]\n",
        "R 0 1 2\nTICK\nM 0 1 2\nTICK\nM 0 1 2\nDETECTOR rec[-1] rec[-1]\nDETECTOR rec[-2]\n",
        "R 0 1 2\nTICK\nM 0 1 2\nTICK\nM 0 1 2\nDETECTOR rec[-1] rec[-2]\nDETECTOR rec[-2]\n",
        "R 0 1 2\nTICK\nM 0 1 2\nTICK\nM 0 1 2\nDETECTOR\nDETECTOR rec[-2]\n",
        "R 0 1 2\nTICK\nM 0 1 2\nTICK\nM 0 1 2\nDETECTOR rec[-1]\n",
        "R 0 1 2\nTICK\nM 0 1 2\nTICK\nM 0 1 2\nDETECTOR rec[-1]\nDETECTOR rec[-2]\nDETECTOR rec[-3]\n",
        "R[a] 0 1 2\nTICK\nM 0 1 2\nTICK\nM 0 1 2\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "R 0 1 2\nTICK\nM[a] 0 1 2\nTICK\nM 0 1 2\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "R 0 1 2\nTICK\nM 0 1 2\nTICK\nM[a] 0 1 2\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "RX 0 1 2\nTICK\nMX 0 1 2\nTICK\nMX 0 1 2\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "R 0 1 1\nTICK\nM 0 1 1\nTICK\nM 0 1 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "R 0 1 2\nTICK\nM 0 1 3\nTICK\nM 0 1 2\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "R 0 1 2 3\nTICK\nM 0 1 2 3\nTICK\nM 0 1 2 3\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "R 0 1 2\nTICK\nM(0.125) 0 1 2\nTICK\nM 0 1 2\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "R 0 1 2\nTICK\nM !0 1 2\nTICK\nM 0 1 2\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "R 0 1 2\nTICK\nM 0 1 2\nTICK\nM 0 1 2\nOBSERVABLE_INCLUDE(0) rec[-1]\nDETECTOR rec[-2]\n",
        "REPEAT 2 {\n    R 0 1 2\n}\nTICK\nM 0 1 2\nTICK\nM 0 1 2\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
    ] {
        let error = circuit_inverse_qec(&circuit(circuit_text))
            .expect_err("unpromoted m_det detector flow is rejected")
            .to_string();

        assert!(
            error.contains("inverse_qec selected m_det subset")
                || error.contains("inverse_qec selected reset-measure-detector subset")
                || error.contains("operation R is not unitary")
                || error.contains("operation TICK is not unitary")
                || error.contains("operation DETECTOR is not unitary")
                || error.contains("operation OBSERVABLE_INCLUDE is not unitary")
                || error.contains("repeat body"),
            "{circuit_text}: {error}"
        );
    }
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect(text)
}
