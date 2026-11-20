#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "unitary conversion tests index compact dense fixtures"
)]

use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_4, TAU};

use num_complex::Complex32;
use rand::rngs::SmallRng;
use rand::{RngExt as _, SeedableRng as _};

use super::{Endianness, PauliImage, amplitude_bit};
use crate::{PauliSign, PauliString, StabilizerError, Tableau};

type Matrix = Vec<Vec<Complex32>>;

fn c(re: f32, im: f32) -> Complex32 {
    Complex32::new(re, im)
}

fn image(pauli: &PauliString, little_endian: bool) -> PauliImage {
    let num_qubits = pauli.len();
    let (mut x, mut z) = (0, 0);
    for qubit in 0..num_qubits {
        let basis = pauli.get(qubit).expect("pauli basis");
        let mask = 1 << amplitude_bit(qubit, num_qubits, little_endian);
        if basis.x_bit() {
            x |= mask;
        }
        if basis.z_bit() {
            z |= mask;
        }
    }
    PauliImage {
        negative: pauli.sign().is_negative(),
        x,
        z,
    }
}

/// Applies a Hermitian Pauli: basis state `c` goes to `i^exponent(c)` times basis state `c ^ x`.
fn apply(pauli: PauliImage, vector: &[Complex32]) -> Vec<Complex32> {
    let mut output = vec![c(0.0, 0.0); vector.len()];
    for (column, value) in vector.iter().enumerate() {
        let phase = c(0.0, 1.0).powu(pauli.exponent(column));
        output[column ^ pauli.x] = phase * value;
    }
    output
}

/// The unitary of `tableau`, up to global phase: column 0 is the state stabilized by every Z
/// image, and column `j` is the product of the X images of `j`'s bits applied to it.
fn tableau_unitary(tableau: &Tableau, little_endian: bool) -> Matrix {
    let num_qubits = tableau.len();
    let dimension = 1 << num_qubits;
    let mut x_by_bit = vec![None; num_qubits];
    let mut z_images = Vec::new();
    for qubit in 0..num_qubits {
        let bit = amplitude_bit(qubit, num_qubits, little_endian);
        x_by_bit[bit] = Some(image(&tableau.x_output(qubit).expect("x"), little_endian));
        z_images.push(image(&tableau.z_output(qubit).expect("z"), little_endian));
    }
    let column0 = (0..dimension)
        .find_map(|start| {
            let mut state = vec![c(0.0, 0.0); dimension];
            state[start] = c(1.0, 0.0);
            for z in &z_images {
                let flipped = apply(*z, &state);
                for (amplitude, other) in state.iter_mut().zip(flipped) {
                    *amplitude = (*amplitude + other) * 0.5;
                }
            }
            let norm = state.iter().map(|value| value.norm_sqr()).sum::<f32>();
            (norm > 1e-6).then(|| state.iter().map(|value| value / norm.sqrt()).collect())
        })
        .expect("stabilizer state");
    let mut columns: Vec<Vec<Complex32>> = vec![column0];
    for column in 1..dimension {
        let bit = column.trailing_zeros() as usize;
        let x = x_by_bit[bit].expect("x image");
        columns.push(apply(x, &columns[column ^ (1 << bit)]));
    }
    (0..dimension)
        .map(|row| columns.iter().map(|column| column[row]).collect())
        .collect()
}

fn scaled(matrix: &Matrix, factor: Complex32) -> Matrix {
    matrix
        .iter()
        .map(|row| row.iter().map(|value| value * factor).collect())
        .collect()
}

fn kron(left: &Matrix, right: &Matrix) -> Matrix {
    let mut output = Vec::new();
    for left_row in left {
        for right_row in right {
            output.push(
                left_row
                    .iter()
                    .flat_map(|a| right_row.iter().map(move |b| a * b))
                    .collect(),
            );
        }
    }
    output
}

fn identity(dimension: usize) -> Matrix {
    (0..dimension)
        .map(|row| {
            (0..dimension)
                .map(|column| c(if row == column { 1.0 } else { 0.0 }, 0.0))
                .collect()
        })
        .collect()
}

fn hadamard() -> Matrix {
    vec![
        vec![c(FRAC_1_SQRT_2, 0.0), c(FRAC_1_SQRT_2, 0.0)],
        vec![c(FRAC_1_SQRT_2, 0.0), c(-FRAC_1_SQRT_2, 0.0)],
    ]
}

fn assert_equal_up_to_phase(actual: &Matrix, expected: &Matrix) {
    let row = expected
        .iter()
        .position(|row| row[0].norm_sqr() > 1e-3)
        .expect("nonzero column");
    let phase = expected[row][0] / actual[row][0];
    for (actual_row, expected_row) in actual.iter().zip(expected) {
        for (a, e) in actual_row.iter().zip(expected_row) {
            assert!((a * phase - e).norm() < 1e-5, "{actual:?} != {expected:?}");
        }
    }
}

#[test]
fn reference_unitary_matches_literal_gate_matrices() {
    let s = vec![
        vec![c(1.0, 0.0), c(0.0, 0.0)],
        vec![c(0.0, 0.0), c(0.0, 1.0)],
    ];
    let cnot = vec![
        vec![c(1.0, 0.0), c(0.0, 0.0), c(0.0, 0.0), c(0.0, 0.0)],
        vec![c(0.0, 0.0), c(1.0, 0.0), c(0.0, 0.0), c(0.0, 0.0)],
        vec![c(0.0, 0.0), c(0.0, 0.0), c(0.0, 0.0), c(1.0, 0.0)],
        vec![c(0.0, 0.0), c(0.0, 0.0), c(1.0, 0.0), c(0.0, 0.0)],
    ];
    let h = Tableau::gate1("+Z", "+X").expect("H");
    let s_gate = Tableau::gate1("+Y", "+Z").expect("S");
    let cx = Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").expect("CX");
    assert_equal_up_to_phase(&tableau_unitary(&h, true), &hadamard());
    assert_equal_up_to_phase(&tableau_unitary(&s_gate, true), &s);
    assert_equal_up_to_phase(&tableau_unitary(&cx, false), &cnot);
}

#[test]
fn random_clifford_unitaries_round_trip_in_both_endiannesses() {
    let mut rng = SmallRng::seed_from_u64(0x554e_4954_4152_5931);
    for num_qubits in 1..=6 {
        for _ in 0..6 {
            let tableau = Tableau::random(num_qubits, &mut rng).expect("random tableau");
            for little_endian in [true, false] {
                let phase = Complex32::from_polar(1.0, rng.random_range(0.0..TAU));
                let matrix = scaled(&tableau_unitary(&tableau, little_endian), phase);
                assert_eq!(
                    Tableau::from_unitary(
                        &matrix,
                        if little_endian {
                            Endianness::Little
                        } else {
                            Endianness::Big
                        }
                    )
                    .expect("Clifford unitary"),
                    tableau,
                    "{num_qubits} qubits, little endian {little_endian}"
                );
            }
        }
    }
}

fn negated(sign: PauliSign) -> PauliSign {
    if sign.is_negative() {
        PauliSign::Plus
    } else {
        PauliSign::Minus
    }
}

#[test]
fn composing_with_a_pauli_flips_exactly_the_anticommuting_output_signs() {
    let mut rng = SmallRng::seed_from_u64(0x5349_474e_5331);
    let num_qubits = 3;
    let tableau = Tableau::random(num_qubits, &mut rng).expect("random tableau");
    let unitary = tableau_unitary(&tableau, true);
    for qubit in 0..num_qubits {
        let mask = 1 << qubit;
        // U X_q permutes columns; U Z_q negates the columns with bit q set.
        let times_x: Matrix = unitary
            .iter()
            .map(|row| (0..row.len()).map(|column| row[column ^ mask]).collect())
            .collect();
        let times_z: Matrix = unitary
            .iter()
            .map(|row| {
                row.iter()
                    .enumerate()
                    .map(|(column, value)| if column & mask != 0 { -value } else { *value })
                    .collect()
            })
            .collect();
        let after_x = Tableau::from_unitary(&times_x, Endianness::Little).expect("U X");
        let after_z = Tableau::from_unitary(&times_z, Endianness::Little).expect("U Z");
        for other in 0..num_qubits {
            let x = tableau.x_output(other).expect("x");
            let z = tableau.z_output(other).expect("z");
            let flipped_z = if other == qubit {
                z.with_sign(negated(z.sign()))
            } else {
                z.clone()
            };
            let flipped_x = if other == qubit {
                x.with_sign(negated(x.sign()))
            } else {
                x.clone()
            };
            assert_eq!(after_x.x_output(other).expect("x"), x);
            assert_eq!(after_x.z_output(other).expect("z"), flipped_z);
            assert_eq!(after_z.x_output(other).expect("x"), flipped_x);
            assert_eq!(after_z.z_output(other).expect("z"), z);
        }
    }
}

#[test]
fn perturbations_inside_the_snap_tolerance_are_accepted() {
    let mut rng = SmallRng::seed_from_u64(0x4e45_4152_3331);
    let tableau = Tableau::random(3, &mut rng).expect("random tableau");
    let mut matrix = tableau_unitary(&tableau, true);
    let (row, column) = (0..8)
        .flat_map(|row| (0..8).map(move |column| (row, column)))
        .find(|(row, column)| matrix[*row][*column].norm_sqr() > 1e-3)
        .expect("nonzero entry");
    // A 0.05 radian phase error is a squared ratio distance of about 0.0025 and leaves the column
    // norm unchanged; a relative magnitude error of 1e-6 stays inside the norm tolerance.
    matrix[row][column] *= Complex32::from_polar(1.0 + 1e-6, 0.05);
    assert_eq!(
        Tableau::from_unitary(&matrix, Endianness::Little).expect("near Clifford"),
        tableau
    );
}

#[test]
fn perturbations_outside_the_tolerance_are_rejected_by_class() {
    let dense = kron(&hadamard(), &hadamard());
    let mut phase_error = dense.clone();
    // Squared ratio distance 2 - 2 cos(0.6) is about 0.35, past the 0.125 threshold.
    phase_error[0][1] *= Complex32::from_polar(1.0, 0.6);
    assert_eq!(
        Tableau::from_unitary(&phase_error, Endianness::Little),
        Err(StabilizerError::MatrixNotUnitary)
    );

    let mut magnitude_error = dense;
    // The ratio still snaps, but column 1's squared norm moves by about 0.005.
    magnitude_error[0][1] *= 1.01;
    assert_eq!(
        Tableau::from_unitary(&magnitude_error, Endianness::Little),
        Err(StabilizerError::MatrixNotUnitary)
    );

    let mut diagonal_phase = identity(4);
    diagonal_phase[3][3] = Complex32::from_polar(1.0, 0.6);
    assert_eq!(
        Tableau::from_unitary(&diagonal_phase, Endianness::Little),
        Err(StabilizerError::UnitaryMatrixNotClifford)
    );
}

fn x_rotation(angle: f32) -> Matrix {
    let (cos, sin) = (angle.cos(), angle.sin());
    vec![
        vec![c(cos, 0.0), c(0.0, -sin)],
        vec![c(0.0, -sin), c(cos, 0.0)],
    ]
}

#[test]
fn small_rotations_snap_to_the_identity_like_stim() {
    // Pinned Stim v1.16.0 accepts RX(0.3) as the identity and rejects RX(0.35): the ratio
    // sin(0.3) / cos(0.3) is within the snapping threshold of zero.
    let matrix = kron(&x_rotation(0.3), &identity(2));
    assert_eq!(
        Tableau::from_unitary(&matrix, Endianness::Little).expect("snapped rotation"),
        Tableau::identity(2).expect("identity")
    );
    assert_eq!(
        Tableau::from_unitary(&x_rotation(0.35), Endianness::Little),
        Err(StabilizerError::UnitaryMatrixNotClifford)
    );
}

#[test]
fn non_clifford_unitaries_are_rejected_as_not_clifford() {
    let t_gate = vec![
        vec![c(1.0, 0.0), c(0.0, 0.0)],
        vec![c(0.0, 0.0), Complex32::from_polar(1.0, FRAC_PI_4)],
    ];
    for (name, gate) in [("T", t_gate), ("RX(0.5)", x_rotation(0.5))] {
        let matrix = kron(&kron(&gate, &identity(2)), &identity(2));
        assert_eq!(
            Tableau::from_unitary(&matrix, Endianness::Little),
            Err(StabilizerError::UnitaryMatrixNotClifford),
            "{name}"
        );
    }
}

#[test]
fn every_column_is_verified_beyond_the_probed_ones() {
    // Identifying the generators of an identity-like matrix on three qubits reads columns 0
    // through 6 outside row 0; column 7 is reached only by the full column check.
    let mut duplicated = identity(8);
    for row in &mut duplicated {
        row[7] = row[3];
    }
    assert_eq!(
        Tableau::from_unitary(&duplicated, Endianness::Little),
        Err(StabilizerError::MatrixNotUnitary)
    );

    let mut controlled_controlled_s = identity(8);
    controlled_controlled_s[7][7] = c(0.0, 1.0);
    assert_eq!(
        Tableau::from_unitary(&controlled_controlled_s, Endianness::Little),
        Err(StabilizerError::UnitaryMatrixNotClifford)
    );
}

#[test]
fn hostile_entries_are_reported_as_not_unitary() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut matrix = identity(2);
        matrix[1][0] = c(0.0, value);
        assert_eq!(
            Tableau::from_unitary(&matrix, Endianness::Little),
            Err(StabilizerError::MatrixNotUnitary)
        );
    }
    let zero_first_column = vec![
        vec![c(0.0, 0.0), c(1.0, 0.0)],
        vec![c(0.0, 0.0), c(0.0, 0.0)],
    ];
    assert_eq!(
        Tableau::from_unitary(&zero_first_column, Endianness::Little),
        Err(StabilizerError::MatrixNotUnitary)
    );
    assert_eq!(
        Tableau::from_unitary(&scaled(&identity(4), c(2.0, 0.0)), Endianness::Little),
        Err(StabilizerError::MatrixNotUnitary)
    );
    assert_eq!(
        Tableau::from_unitary(&[vec![c(0.5, 0.0)]], Endianness::Little),
        Err(StabilizerError::MatrixNotUnitary)
    );
    assert_eq!(
        Tableau::from_unitary(&[vec![c(0.0, 1.0)]], Endianness::Little).expect("scalar phase"),
        Tableau::identity(0).expect("empty tableau")
    );
}
