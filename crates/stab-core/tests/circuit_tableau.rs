#![allow(
    clippy::expect_used,
    reason = "M6 circuit/tableau parity tests mirror compact upstream examples"
)]

use stab_core::{Circuit, Tableau};

#[test]
fn circuit_to_tableau_ignores_or_rejects_non_unitary_gate_classes_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_vs_tableau.test.cc.
    let unitary = circuit(
        "
        I 0
        X 0
        Y 0
        Z 0
        H 0
        S 0
        S_DAG 0
        SQRT_X 0
        SQRT_X_DAG 0
        SQRT_Y 0
        SQRT_Y_DAG 0
        CX 0 1
        CY 0 1
        CZ 0 1
        SWAP 0 1
    ",
    );
    assert_eq!(
        unitary
            .to_tableau(false, false, false)
            .expect("unitary tableau")
            .len(),
        2
    );

    let noise = circuit(
        "
        X_ERROR(0.1) 0
        Y_ERROR(0.1) 0
        Z_ERROR(0.1) 0
    ",
    );
    assert!(noise.to_tableau(false, false, false).is_err());
    assert_eq!(
        noise.to_tableau(true, false, false).expect("ignored noise"),
        Tableau::identity(1).expect("Tableau identity")
    );

    let measure = circuit("M 0\nMX 0\nMY 0\n");
    assert!(measure.to_tableau(false, false, false).is_err());
    assert_eq!(
        measure
            .to_tableau(false, true, false)
            .expect("ignored measurements"),
        Tableau::identity(1).expect("Tableau identity")
    );

    let reset = circuit("R 0\nRX 0\nRY 0\n");
    assert!(reset.to_tableau(false, false, false).is_err());
    assert_eq!(
        reset
            .to_tableau(false, false, true)
            .expect("ignored resets"),
        Tableau::identity(1).expect("Tableau identity")
    );

    let annotations = circuit(
        "
        REPEAT 10 {
            I 0
        }
        DETECTOR(1, 2)
        OBSERVABLE_INCLUDE(1)
        QUBIT_COORDS(0,1,2) 0
        SHIFT_COORDS(2, 3, 4)
        TICK
    ",
    );
    assert_eq!(
        annotations
            .to_tableau(false, false, false)
            .expect("annotations ignored"),
        Tableau::identity(1).expect("Tableau identity")
    );
}

#[test]
fn circuit_to_tableau_matches_stim_basic_examples() {
    assert_eq!(
        circuit("").to_tableau(false, false, false).expect("empty"),
        Tableau::identity(0).expect("Tableau identity")
    );

    assert_eq!(
        circuit(
            "
            REPEAT 10 {
                X 0
                TICK
            }
        "
        )
        .to_tableau(false, false, false)
        .expect("even X repeat"),
        Tableau::identity(1).expect("Tableau identity")
    );

    assert_eq!(
        circuit(
            "
            REPEAT 11 {
                X 0
                TICK
            }
        "
        )
        .to_tableau(false, false, false)
        .expect("odd X repeat"),
        Tableau::gate1("+X", "-Z").expect("X tableau")
    );

    assert_eq!(
        circuit("S 0").to_tableau(false, false, false).expect("S"),
        Tableau::gate1("+Y", "+Z").expect("S tableau")
    );

    assert_eq!(
        circuit(
            "
            SQRT_Y_DAG 1
            CZ 0 1
            SQRT_Y 1
        "
        )
        .to_tableau(false, false, false)
        .expect("conjugated CZ"),
        cnot_tableau()
    );

    assert_eq!(
        circuit(
            "
            R 0
            X_ERROR(0.1) 0
            SQRT_Y_DAG 1
            CZ 0 1
            SQRT_Y 1
            M 0
        "
        )
        .to_tableau(true, true, true)
        .expect("ignored non-unitary gates"),
        cnot_tableau()
    );
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn cnot_tableau() -> Tableau {
    Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").expect("CNOT tableau")
}
