#![allow(
    clippy::expect_used,
    reason = "PFM2 QEC inverse MZZ tests mirror compact upstream examples"
)]

use stab_core::{Circuit, circuit_inverse_qec};

#[test]
fn circuit_inverse_qec_supports_selected_mzz_detector_flow() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec mzz behavior.
    let input = circuit(
        "
        MRY 0 1
        M 0
        TICK
        MZZ(0.125) 0 1 2 3
        TICK
        M 1
        MRY 0 1
        DETECTOR rec[-3] rec[-5] rec[-6]
    ",
    );
    let expected = circuit(
        "
        MRY 1 0
        R 1
        TICK
        MZZ(0.125) 2 3 0 1
        TICK
        M 0
        DETECTOR rec[-2] rec[-1]
        MRY 1 0
    ",
    );

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected MZZ detector flow"),
        expected
    );
    assert_eq!(
        input
            .inverse_qec()
            .expect("method inverse selected MZZ detector flow"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_supports_selected_mzz_detector_metadata() {
    let input = circuit(
        "
        MRY 0 1
        M 0
        TICK[a]
        MZZ(0.125) 0 1 2 3
        TICK[b]
        M 1
        MRY 0 1
        DETECTOR[c](5, 6) rec[-3] rec[-5] rec[-6]
    ",
    );
    let expected = circuit(
        "
        MRY 1 0
        R 1
        TICK[a]
        MZZ(0.125) 2 3 0 1
        TICK[b]
        M 0
        DETECTOR[c](5, 6) rec[-2] rec[-1]
        MRY 1 0
    ",
    );

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected MZZ detector metadata"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_rejects_unpromoted_mzz_shapes() {
    for circuit_text in [
        "MRY 0 1\nM 0\nMZZ(0.125) 0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
        "MRY 0 1\nM 0\nTICK\nMZZ 0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
        "MRY[a] 0 1\nM 0\nTICK\nMZZ(0.125) 0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
        "MRY 0 1\nM[a] 0\nTICK\nMZZ(0.125) 0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
        "MRY 0 1\nM 0\nTICK\nMZZ[a](0.125) 0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
        "MRY 0 1\nM 0\nTICK\nMZZ(0.125) 0 1 2 3\nTICK\nM[a] 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
        "MRY 0 1\nM 0\nTICK\nMZZ(0.125) 0 1 2 3\nTICK\nM 1\nMRY[a] 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
        "MRY 1 0\nM 0\nTICK\nMZZ(0.125) 0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
        "MRY 0 1\nM 1\nTICK\nMZZ(0.125) 0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
        "MRY 0 1\nM 0\nTICK\nMZZ(0.125) 0 1 3 2\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
        "MRY 0 1\nM 0\nTICK\nMXX(0.125) 0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
        "MRY 0 1\nM 0\nTICK\nMZZ(0.125) !0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
        "MRY 0 1\nM 0\nTICK\nMZZ(0.125) 0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5]\n",
        "MRY 0 1\nM 0\nTICK\nMZZ(0.125) 0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6] rec[-6]\n",
        "MRY 0 1\nM 0\nTICK\nMZZ(0.125) 0 1 2 3\nTICK\nM 1\nMRY 0 1\nOBSERVABLE_INCLUDE(0) rec[-3]\n",
        "REPEAT 2 {\n    MRY 0 1\n}\nM 0\nTICK\nMZZ(0.125) 0 1 2 3\nTICK\nM 1\nMRY 0 1\nDETECTOR rec[-3] rec[-5] rec[-6]\n",
    ] {
        let error = circuit_inverse_qec(&circuit(circuit_text))
            .expect_err("unpromoted MZZ detector flow is rejected")
            .to_string();

        assert!(
            error.contains("inverse_qec selected MZZ detector subset")
                || error.contains("operation DETECTOR is not unitary")
                || error.contains("operation MRY is not unitary")
                || error.contains("operation MZZ is not unitary")
                || error.contains("operation MXX is not unitary")
                || error.contains("operation OBSERVABLE_INCLUDE is not unitary")
                || error.contains("repeat body"),
            "{circuit_text}: {error}"
        );
    }
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect(text)
}
