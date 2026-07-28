use num_complex::Complex32;

use super::{
    PauliBasis, PauliSign, PauliString, StabilizerError, StabilizerResource, StabilizerResult,
    Tableau,
};

const UNITARITY_EPSILON: f32 = 1e-4;
const STIM_SNAP_DISTANCE_SQUARED: f32 = 0.125;

/// Converts a Clifford unitary matrix into the corresponding tableau, up to global phase.
///
/// The matrix must be square, unitary, and have power-of-two dimensions. When `little_endian` is
/// true, qubit 0 corresponds to the least significant amplitude bit. When it is false, qubit 0
/// corresponds to the most significant amplitude bit, matching Stim's `unitary_to_tableau`
/// convention. Matrix entries that should be Pauli phases are snapped using Stim's v1.16.0
/// stabilizer-state smoothing threshold. The matrix dimension is checked against
/// [`StabilizerResource::UnitaryMatrixDimension`] before row-shape or numerical work.
pub fn unitary_to_tableau(
    matrix: &[Vec<Complex32>],
    little_endian: bool,
) -> StabilizerResult<Tableau> {
    let num_qubits = validate_matrix_shape(matrix)?;
    ensure_unitary(matrix)?;

    let mut xs = Vec::with_capacity(num_qubits);
    let mut zs = Vec::with_capacity(num_qubits);
    for input_index in 0..num_qubits {
        xs.push(conjugated_generator_to_pauli(
            matrix,
            num_qubits,
            input_index,
            PauliBasis::X,
            little_endian,
        )?);
        zs.push(conjugated_generator_to_pauli(
            matrix,
            num_qubits,
            input_index,
            PauliBasis::Z,
            little_endian,
        )?);
    }

    let tableau = Tableau::from_output_columns_unchecked(xs, zs);
    if tableau.satisfies_invariants()? {
        Ok(tableau)
    } else {
        Err(StabilizerError::UnitaryMatrixNotClifford)
    }
}

fn validate_matrix_shape(matrix: &[Vec<Complex32>]) -> StabilizerResult<usize> {
    let height = matrix.len();
    if height == 0 || !height.is_power_of_two() {
        return Err(StabilizerError::UnitaryMatrixHeightNotPowerOfTwo { height });
    }
    StabilizerResource::UnitaryMatrixDimension.ensure(height)?;
    for (row, values) in matrix.iter().enumerate() {
        if values.len() != height {
            return Err(StabilizerError::UnitaryMatrixRowWidthMismatch {
                row,
                width: values.len(),
                height,
            });
        }
    }
    Ok(log2_power_of_two(height))
}

fn log2_power_of_two(value: usize) -> usize {
    let mut result = 0;
    let mut remaining = value;
    while remaining > 1 {
        remaining >>= 1;
        result += 1;
    }
    result
}

fn ensure_unitary(matrix: &[Vec<Complex32>]) -> StabilizerResult<()> {
    let dimension = matrix.len();
    for left in 0..dimension {
        for right in 0..dimension {
            let mut dot = zero();
            for column in 0..dimension {
                dot +=
                    matrix_cell(matrix, left, column)? * matrix_cell(matrix, right, column)?.conj();
            }
            let expected = if left == right { one() } else { zero() };
            if !close(dot, expected) {
                return Err(StabilizerError::MatrixNotUnitary);
            }
        }
    }
    Ok(())
}

fn conjugated_generator_to_pauli(
    matrix: &[Vec<Complex32>],
    num_qubits: usize,
    input_index: usize,
    basis: PauliBasis,
    little_endian: bool,
) -> StabilizerResult<PauliString> {
    let dimension = matrix.len();
    let bit = amplitude_bit(input_index, num_qubits, little_endian);
    let generator_x_mask = if basis.x_bit() { 1_usize << bit } else { 0 };
    let generator_z_mask = if basis.z_bit() { 1_usize << bit } else { 0 };
    let mut conjugated = Vec::with_capacity(dimension);

    for output_row in 0..dimension {
        let mut row = Vec::with_capacity(dimension);
        for output_column in 0..dimension {
            let mut value = zero();
            for input_column in 0..dimension {
                let input_row = input_column ^ generator_x_mask;
                let phase = pauli_column_phase(
                    PauliSign::Plus,
                    generator_x_mask,
                    generator_z_mask,
                    input_column,
                );
                value += matrix_cell(matrix, output_row, input_row)?
                    * phase
                    * matrix_cell(matrix, output_column, input_column)?.conj();
            }
            row.push(value);
        }
        conjugated.push(row);
    }

    signed_pauli_from_matrix(&conjugated, num_qubits, little_endian)
}

fn signed_pauli_from_matrix(
    matrix: &[Vec<Complex32>],
    num_qubits: usize,
    little_endian: bool,
) -> StabilizerResult<PauliString> {
    let dimension = matrix.len();
    let mut output_rows = Vec::with_capacity(dimension);
    let mut output_phases = Vec::with_capacity(dimension);

    for column in 0..dimension {
        let mut found = None;
        for row in 0..dimension {
            let value = matrix_cell(matrix, row, column)?;
            if near_zero(value) {
                continue;
            }
            if found.is_some() {
                return Err(StabilizerError::UnitaryMatrixNotClifford);
            }
            found = Some((row, snap_pauli_phase(value)?));
        }
        let Some((row, phase)) = found else {
            return Err(StabilizerError::UnitaryMatrixNotClifford);
        };
        output_rows.push(row);
        output_phases.push(phase);
    }

    let Some(first_row) = output_rows.first().copied() else {
        return Err(StabilizerError::UnitaryMatrixNotClifford);
    };
    let x_mask = first_row;
    for (column, row) in output_rows.iter().copied().enumerate() {
        if row ^ column != x_mask {
            return Err(StabilizerError::UnitaryMatrixNotClifford);
        }
    }

    for z_mask in 0..dimension {
        for sign in [PauliSign::Plus, PauliSign::Minus] {
            if phases_match(sign, x_mask, z_mask, &output_phases) {
                return Ok(pauli_from_masks(
                    sign,
                    x_mask,
                    z_mask,
                    num_qubits,
                    little_endian,
                ));
            }
        }
    }

    Err(StabilizerError::UnitaryMatrixNotClifford)
}

fn phases_match(
    sign: PauliSign,
    x_mask: usize,
    z_mask: usize,
    output_phases: &[Complex32],
) -> bool {
    output_phases
        .iter()
        .copied()
        .enumerate()
        .all(|(column, actual)| {
            let expected = pauli_column_phase(sign, x_mask, z_mask, column);
            close(actual, expected)
        })
}

fn pauli_from_masks(
    sign: PauliSign,
    x_mask: usize,
    z_mask: usize,
    num_qubits: usize,
    little_endian: bool,
) -> PauliString {
    PauliString::from_bases_unchecked(
        sign,
        (0..num_qubits).map(|qubit| {
            let bit = amplitude_bit(qubit, num_qubits, little_endian);
            let mask = 1_usize << bit;
            PauliBasis::from_xz(x_mask & mask != 0, z_mask & mask != 0)
        }),
    )
}

fn pauli_column_phase(sign: PauliSign, x_mask: usize, z_mask: usize, column: usize) -> Complex32 {
    let mut phase = i_power((x_mask & z_mask).count_ones());
    if (z_mask & column).count_ones() & 1 != 0 {
        phase = -phase;
    }
    if sign.is_negative() { -phase } else { phase }
}

fn snap_pauli_phase(value: Complex32) -> StabilizerResult<Complex32> {
    [one(), -one(), i(), -i()]
        .into_iter()
        .find(|candidate| snap_close(value, *candidate))
        .ok_or(StabilizerError::UnitaryMatrixNotClifford)
}

fn i_power(exponent: u32) -> Complex32 {
    match exponent & 3 {
        0 => one(),
        1 => i(),
        2 => -one(),
        _ => -i(),
    }
}

fn amplitude_bit(qubit: usize, num_qubits: usize, little_endian: bool) -> usize {
    if little_endian {
        qubit
    } else {
        num_qubits - qubit - 1
    }
}

fn matrix_cell(
    matrix: &[Vec<Complex32>],
    row: usize,
    column: usize,
) -> StabilizerResult<Complex32> {
    matrix
        .get(row)
        .and_then(|values| values.get(column))
        .copied()
        .ok_or(StabilizerError::UnitaryMatrixNotClifford)
}

fn close(left: Complex32, right: Complex32) -> bool {
    (left - right).norm_sqr() <= UNITARITY_EPSILON * UNITARITY_EPSILON
}

fn snap_close(left: Complex32, right: Complex32) -> bool {
    (left - right).norm_sqr() < STIM_SNAP_DISTANCE_SQUARED
}

fn near_zero(value: Complex32) -> bool {
    value.norm_sqr() < STIM_SNAP_DISTANCE_SQUARED
}

fn zero() -> Complex32 {
    Complex32::new(0.0, 0.0)
}

fn one() -> Complex32 {
    Complex32::new(1.0, 0.0)
}

fn i() -> Complex32 {
    Complex32::new(0.0, 1.0)
}
