#![allow(
    clippy::expect_used,
    reason = "PFM2 QEC inverse observable include tests mirror compact upstream examples"
)]

use stab_core::{Circuit, circuit_inverse_qec};

#[test]
fn circuit_inverse_qec_supports_selected_observable_pauli_include() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec obs_include_pauli behavior.
    let input = circuit(
        "
        RX 1
        OBSERVABLE_INCLUDE[test](1) X1
    ",
    );
    let expected = circuit(
        "
        OBSERVABLE_INCLUDE[test](1) X1
        MX 1
        OBSERVABLE_INCLUDE(1) rec[-1]
    ",
    );

    assert_eq!(
        circuit_inverse_qec(&input).expect("inverse selected observable Pauli include"),
        expected
    );
    assert_eq!(
        input
            .inverse_qec()
            .expect("method inverse selected observable Pauli include"),
        expected
    );
}

#[test]
fn circuit_inverse_qec_rejects_unpromoted_observable_pauli_include_shapes() {
    for circuit_text in [
        "R 1\nOBSERVABLE_INCLUDE[test](1) X1\n",
        "RY 1\nOBSERVABLE_INCLUDE[test](1) X1\n",
        "RX\nOBSERVABLE_INCLUDE[test](1) X1\n",
        "RX 0\nOBSERVABLE_INCLUDE[test](1) X1\n",
        "RX 1 1\nOBSERVABLE_INCLUDE[test](1) X1\n",
        "RX 1 2\nOBSERVABLE_INCLUDE[test](1) X1\n",
        "RX 1\nOBSERVABLE_INCLUDE[test](0) X1\n",
        "RX 1\nOBSERVABLE_INCLUDE[test](1)\n",
        "RX 1\nOBSERVABLE_INCLUDE[test](1) Z1\n",
        "RX 1\nOBSERVABLE_INCLUDE[test](1) !X1\n",
        "RX 1\nOBSERVABLE_INCLUDE[test](1) X2\n",
        "RX 1\nOBSERVABLE_INCLUDE[test](1) X1 X1\n",
        "RX 1\nOBSERVABLE_INCLUDE[test](1) X1 rec[-1]\n",
        "RX 1\nOBSERVABLE_INCLUDE(1) X1\n",
        "RX[tag] 1\nOBSERVABLE_INCLUDE[test](1) X1\n",
        "RX 1\nTICK\nOBSERVABLE_INCLUDE[test](1) X1\n",
        "REPEAT 2 {\n    RX 1\n}\nOBSERVABLE_INCLUDE[test](1) X1\n",
    ] {
        let error = circuit_inverse_qec(&circuit(circuit_text))
            .expect_err("unpromoted observable Pauli include is rejected")
            .to_string();

        assert!(
            error.contains("inverse_qec selected observable Pauli include subset")
                || error.contains("operation OBSERVABLE_INCLUDE is not unitary")
                || error.contains("operation R is not unitary")
                || error.contains("operation RY is not unitary")
                || error.contains("operation RX is not unitary")
                || error.contains("repeat body"),
            "{circuit_text}: {error}"
        );
    }
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect(text)
}
