#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "M6 unitary/tableau parity tests mirror compact upstream examples"
)]

use num_complex::Complex32;
use stab_core::{Tableau, unitary_to_tableau};

#[test]
fn unitary_to_tableau_handles_single_qubit_gate_data_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/util_top/stabilizers_vs_amplitudes.test.cc
    // and src/stim/gates/gate_data_{pauli,hada,period_4}.cc.
    let inv_sqrt2 = f32::sqrt(0.5);
    for (matrix, expected) in [
        (
            matrix(&[&[(1.0, 0.0), (0.0, 0.0)], &[(0.0, 0.0), (1.0, 0.0)]]),
            Tableau::gate1("+X", "+Z"),
        ),
        (
            matrix(&[&[(0.0, 0.0), (1.0, 0.0)], &[(1.0, 0.0), (0.0, 0.0)]]),
            Tableau::gate1("+X", "-Z"),
        ),
        (
            matrix(&[&[(0.0, 0.0), (0.0, -1.0)], &[(0.0, 1.0), (0.0, 0.0)]]),
            Tableau::gate1("-X", "-Z"),
        ),
        (
            matrix(&[&[(1.0, 0.0), (0.0, 0.0)], &[(0.0, 0.0), (-1.0, 0.0)]]),
            Tableau::gate1("-X", "+Z"),
        ),
        (
            matrix(&[
                &[(inv_sqrt2, 0.0), (inv_sqrt2, 0.0)],
                &[(inv_sqrt2, 0.0), (-inv_sqrt2, 0.0)],
            ]),
            Tableau::gate1("+Z", "+X"),
        ),
        (
            matrix(&[&[(1.0, 0.0), (0.0, 0.0)], &[(0.0, 0.0), (0.0, 1.0)]]),
            Tableau::gate1("+Y", "+Z"),
        ),
        (
            matrix(&[&[(1.0, 0.0), (0.0, 0.0)], &[(0.0, 0.0), (0.0, -1.0)]]),
            Tableau::gate1("-Y", "+Z"),
        ),
    ] {
        let expected = expected.expect("expected tableau");
        assert_eq!(
            unitary_to_tableau(&matrix, true).expect("little endian"),
            expected
        );
        assert_eq!(
            unitary_to_tableau(&matrix, false).expect("big endian"),
            expected
        );
    }
}

#[test]
fn unitary_to_tableau_handles_controlled_gate_endianness_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/util_top/stabilizers_vs_amplitudes.test.cc.
    let xcz = xcz_unitary();
    assert_eq!(
        unitary_to_tableau(&xcz, true).expect("XCZ little endian"),
        xcz_tableau()
    );
    assert_eq!(
        unitary_to_tableau(&xcz, false).expect("XCZ big endian"),
        zcx_tableau()
    );

    let zcx = zcx_unitary();
    assert_eq!(
        unitary_to_tableau(&zcx, true).expect("ZCX little endian"),
        zcx_tableau()
    );
    assert_eq!(
        unitary_to_tableau(&zcx, false).expect("ZCX big endian"),
        xcz_tableau()
    );

    let xcy = xcy_unitary();
    assert_eq!(
        unitary_to_tableau(&xcy, true).expect("XCY little endian"),
        xcy_tableau()
    );
    assert_eq!(
        unitary_to_tableau(&xcy, false).expect("XCY big endian"),
        ycx_tableau()
    );

    let ycx = ycx_unitary();
    assert_eq!(
        unitary_to_tableau(&ycx, true).expect("YCX little endian"),
        ycx_tableau()
    );
    assert_eq!(
        unitary_to_tableau(&ycx, false).expect("YCX big endian"),
        xcy_tableau()
    );
}

#[test]
fn unitary_to_tableau_rejects_non_clifford_or_malformed_matrices_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/util_top/stabilizers_vs_amplitudes.test.cc.
    let eighth_turn_phase = f32::sqrt(0.5);
    assert!(
        unitary_to_tableau(
            &[
                vec![c(1.0, 0.0), c(0.0, 0.0)],
                vec![c(0.0, 0.0), c(eighth_turn_phase, eighth_turn_phase)],
            ],
            false,
        )
        .is_err()
    );

    assert!(
        unitary_to_tableau(
            &[
                vec![c(1.0, 0.0), c(0.0, 0.0), c(0.0, 0.0), c(0.0, 0.0)],
                vec![c(0.0, 0.0), c(1.0, 0.0), c(0.0, 0.0), c(0.0, 0.0)],
                vec![c(0.0, 0.0), c(0.0, 0.0), c(1.0, 0.0), c(0.0, 0.0)],
                vec![c(0.0, 0.0), c(0.0, 0.0), c(0.0, 0.0), c(0.0, 1.0)],
            ],
            false,
        )
        .is_err()
    );

    assert!(
        unitary_to_tableau(
            &[
                vec![
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                ],
            ],
            false,
        )
        .is_err()
    );

    assert!(unitary_to_tableau(&[vec![c(1.0, 0.0)], vec![c(0.0, 0.0)]], false).is_err());
}

#[test]
fn unitary_to_tableau_snaps_near_clifford_phases_like_stim() {
    // Stim v1.16.0 smooths stabilizer-state ratios within squared distance 0.125 of 0, +/-1,
    // and +/-i. This near-identity phase is therefore accepted as the identity tableau.
    let theta = 0.01_f32;
    let near_identity_phase = matrix(&[
        &[(1.0, 0.0), (0.0, 0.0)],
        &[(0.0, 0.0), (theta.cos(), theta.sin())],
    ]);
    assert_eq!(
        unitary_to_tableau(&near_identity_phase, true).expect("near identity phase"),
        Tableau::identity(1)
    );

    let theta = 0.5_f32;
    let non_clifford_phase = matrix(&[
        &[(1.0, 0.0), (0.0, 0.0)],
        &[(0.0, 0.0), (theta.cos(), theta.sin())],
    ]);
    assert!(unitary_to_tableau(&non_clifford_phase, true).is_err());
}

fn xcz_unitary() -> Vec<Vec<Complex32>> {
    matrix(&[
        &[(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        &[(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        &[(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
        &[(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
    ])
}

fn zcx_unitary() -> Vec<Vec<Complex32>> {
    matrix(&[
        &[(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        &[(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
        &[(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
        &[(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
    ])
}

fn xcy_unitary() -> Vec<Vec<Complex32>> {
    matrix(&[
        &[(0.5, 0.0), (0.5, 0.0), (0.0, -0.5), (0.0, 0.5)],
        &[(0.5, 0.0), (0.5, 0.0), (0.0, 0.5), (0.0, -0.5)],
        &[(0.0, 0.5), (0.0, -0.5), (0.5, 0.0), (0.5, 0.0)],
        &[(0.0, -0.5), (0.0, 0.5), (0.5, 0.0), (0.5, 0.0)],
    ])
}

fn ycx_unitary() -> Vec<Vec<Complex32>> {
    matrix(&[
        &[(0.5, 0.0), (0.0, -0.5), (0.5, 0.0), (0.0, 0.5)],
        &[(0.0, 0.5), (0.5, 0.0), (0.0, -0.5), (0.5, 0.0)],
        &[(0.5, 0.0), (0.0, 0.5), (0.5, 0.0), (0.0, -0.5)],
        &[(0.0, -0.5), (0.5, 0.0), (0.0, 0.5), (0.5, 0.0)],
    ])
}

fn matrix(rows: &[&[(f32, f32)]]) -> Vec<Vec<Complex32>> {
    rows.iter()
        .map(|row| row.iter().map(|(real, imag)| c(*real, *imag)).collect())
        .collect()
}

fn c(real: f32, imag: f32) -> Complex32 {
    Complex32::new(real, imag)
}

fn xcz_tableau() -> Tableau {
    Tableau::gate2("+X_", "+ZZ", "+XX", "+_Z").expect("XCZ tableau")
}

fn zcx_tableau() -> Tableau {
    Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").expect("ZCX tableau")
}

fn xcy_tableau() -> Tableau {
    Tableau::gate2("+X_", "+ZY", "+XX", "+XZ").expect("XCY tableau")
}

fn ycx_tableau() -> Tableau {
    Tableau::gate2("+XX", "+ZX", "+_X", "+YZ").expect("YCX tableau")
}
