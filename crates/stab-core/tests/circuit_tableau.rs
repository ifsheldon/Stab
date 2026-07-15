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
        C_XYZ 0
        C_ZYX 0
        H 0
        H_XY 0
        H_XZ 0
        H_YZ 0
        S 0
        SQRT_X 0
        SQRT_X_DAG 0
        SQRT_Y 0
        SQRT_Y_DAG 0
        SQRT_Z 0
        SQRT_Z_DAG 0
        S_DAG 0
        SPP X0
        SPP_DAG Z0
        CNOT 0 1
        CX 0 1
        CY 0 1
        CZ 0 1
        ISWAP 0 1
        ISWAP_DAG 0 1
        SQRT_XX 0 1
        SQRT_XX_DAG 0 1
        SQRT_YY 0 1
        SQRT_YY_DAG 0 1
        SQRT_ZZ 0 1
        SQRT_ZZ_DAG 0 1
        SWAP 0 1
        XCX 0 1
        XCY 0 1
        XCZ 0 1
        YCX 0 1
        YCY 0 1
        YCZ 0 1
        ZCX 0 1
        ZCY 0 1
        ZCZ 0 1
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
        CORRELATED_ERROR(0.1) X0
        DEPOLARIZE1(0.1) 0
        DEPOLARIZE2(0.1) 0 1
        E(0.1) X0
        ELSE_CORRELATED_ERROR(0.1) Y1
        PAULI_CHANNEL_1(0.1,0.2,0.3) 0
        PAULI_CHANNEL_2(0.01,0.01,0.01,0.01,0.01,0.01,0.01,0.01,0.01,0.01,0.01,0.01,0.01,0.01,0.01) 0 1
    ",
    );
    assert!(noise.to_tableau(false, false, false).is_err());
    assert!(noise.to_tableau(false, true, true).is_err());
    assert_eq!(
        noise.to_tableau(true, false, false).expect("ignored noise"),
        Tableau::identity(2).expect("Tableau identity")
    );

    let measure = circuit("M 0\nMPP X0\nMX 0\nMY 0\nMZ 0\n");
    assert!(measure.to_tableau(false, false, false).is_err());
    assert!(measure.to_tableau(true, false, true).is_err());
    assert_eq!(
        measure
            .to_tableau(false, true, false)
            .expect("ignored measurements"),
        Tableau::identity(1).expect("Tableau identity")
    );

    let reset = circuit("R 0\nRX 0\nRY 0\nRZ 0\n");
    assert!(reset.to_tableau(false, false, false).is_err());
    assert!(reset.to_tableau(true, true, false).is_err());
    assert_eq!(
        reset
            .to_tableau(false, false, true)
            .expect("ignored resets"),
        Tableau::identity(1).expect("Tableau identity")
    );

    let measure_reset = circuit("MR 0\nMRX 0\nMRY 0\nMRZ 0\n");
    assert!(measure_reset.to_tableau(false, false, false).is_err());
    assert!(measure_reset.to_tableau(true, false, true).is_err());
    assert!(measure_reset.to_tableau(true, true, false).is_err());
    assert_eq!(
        measure_reset
            .to_tableau(false, true, true)
            .expect("ignored measure-resets"),
        Tableau::identity(1).expect("Tableau identity")
    );

    let noisy_measure = circuit("M(0.1) 0\nMPP(0.1) X0\nMXX(0.1) 0 1\n");
    assert!(noisy_measure.to_tableau(true, false, false).is_err());
    assert!(noisy_measure.to_tableau(false, true, false).is_err());
    assert_eq!(
        noisy_measure
            .to_tableau(true, true, false)
            .expect("ignored noisy measurements"),
        Tableau::identity(2).expect("Tableau identity")
    );

    let noisy_measure_reset = circuit("MR(0.1) 0\n");
    assert!(noisy_measure_reset.to_tableau(true, false, true).is_err());
    assert!(noisy_measure_reset.to_tableau(true, true, false).is_err());
    assert!(noisy_measure_reset.to_tableau(false, true, true).is_err());
    assert_eq!(
        noisy_measure_reset
            .to_tableau(true, true, true)
            .expect("ignored noisy measure-reset"),
        Tableau::identity(1).expect("Tableau identity")
    );

    let heralded = circuit("HERALDED_ERASE(0.1) 0\n");
    assert!(heralded.to_tableau(true, false, false).is_err());
    assert!(heralded.to_tableau(false, true, false).is_err());
    assert_eq!(
        heralded
            .to_tableau(true, true, false)
            .expect("ignored heralded result and noise"),
        Tableau::identity(1).expect("Tableau identity")
    );

    let zero_probability = circuit("M(0) 0\nMR(0) 0\n");
    assert_eq!(
        zero_probability
            .to_tableau(false, true, true)
            .expect("zero-probability measurement noise"),
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

    let mut combined = annotations.clone();
    combined.append_circuit(&measure_reset);
    combined.append_circuit(&measure);
    combined.append_circuit(&reset);
    combined.append_circuit(&unitary);
    combined.append_circuit(&noise);
    assert_eq!(
        combined
            .to_tableau(true, true, true)
            .expect("ignore all non-unitary gates")
            .len(),
        2
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
        circuit("SPP Z0")
            .to_tableau(false, false, false)
            .expect("SPP Z"),
        circuit("S 0").to_tableau(false, false, false).expect("S")
    );
    assert_eq!(
        circuit("SPP X0")
            .to_tableau(false, false, false)
            .expect("SPP X"),
        circuit("SQRT_X 0")
            .to_tableau(false, false, false)
            .expect("SQRT_X")
    );
    assert_eq!(
        circuit("SPP_DAG Y0*Y1")
            .to_tableau(false, false, false)
            .expect("SPP_DAG YY"),
        circuit("SQRT_YY_DAG 0 1")
            .to_tableau(false, false, false)
            .expect("SQRT_YY_DAG")
    );
    assert_eq!(
        circuit("SPP !Z0")
            .to_tableau(false, false, false)
            .expect("negative SPP Z"),
        circuit("S_DAG 0")
            .to_tableau(false, false, false)
            .expect("S_DAG")
    );
    assert_eq!(
        circuit("SPP X0*Y0*Y1*Z1")
            .to_tableau(false, false, false)
            .expect("repeated-qubit Hermitian SPP"),
        circuit("H 1\nCX 1 0\nS_DAG 0\nCX 1 0\nH 1\n")
            .to_tableau(false, false, false)
            .expect("primitive negative Z0*X1 phasing")
    );
    assert!(
        circuit("SPP X0*Z0")
            .to_tableau(false, false, false)
            .is_err()
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

#[test]
fn circuit_to_tableau_folds_huge_and_nested_repeats_exactly() {
    let identity = Tableau::identity(1).expect("Tableau identity");
    let hadamard = circuit("H 0")
        .to_tableau(false, false, false)
        .expect("H tableau");

    let folded = circuit(
        "
        H 0
        REPEAT 37 {
            S 0
            H 0
        }
        SQRT_X 0
    ",
    )
    .to_tableau(false, false, false)
    .expect("folded noncommuting repeat");
    let unrolled_text = format!("H 0\n{}SQRT_X 0\n", "S 0\nH 0\n".repeat(37));
    let unrolled = circuit(&unrolled_text)
        .to_tableau(false, false, false)
        .expect("unrolled noncommuting repeat");
    assert_eq!(folded, unrolled);

    assert_eq!(
        circuit(
            "
            REPEAT 1000000000000 {
                H 0
            }
        "
        )
        .to_tableau(false, false, false)
        .expect("huge even repeat"),
        identity
    );
    assert_eq!(
        circuit(
            "
            REPEAT 1000000000001 {
                H 0
            }
        "
        )
        .to_tableau(false, false, false)
        .expect("huge odd repeat"),
        hadamard
    );
    assert_eq!(
        circuit(
            "
            REPEAT 1000000000001 {
                REPEAT 1000000000001 {
                    H 0
                }
            }
        "
        )
        .to_tableau(false, false, false)
        .expect("nested huge odd repeats"),
        circuit("H 0")
            .to_tableau(false, false, false)
            .expect("H tableau")
    );
    assert_eq!(
        circuit(
            "
            REPEAT 1000000000000 {
                TICK
                M(0) 0
            }
        "
        )
        .to_tableau(false, true, false)
        .expect("huge ignored repeat"),
        Tableau::identity(1).expect("Tableau identity")
    );
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn cnot_tableau() -> Tableau {
    Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").expect("CNOT tableau")
}
