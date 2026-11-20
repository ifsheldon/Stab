use num_complex::Complex32;

use crate::budget::{allocation_failed, try_with_capacity};

use super::{
    PauliBasis, PauliSign, PauliString, StabilizerError, StabilizerResource, StabilizerResult,
    Tableau,
};

#[cfg(test)]
mod tests;

/// Per-entry tolerance of the Gram check that classifies a rejected matrix.
const UNITARITY_EPSILON: f32 = 1e-4;
/// Stim v1.16.0's `smooth_stabilizer_state` snapping threshold on squared ratio distance.
const STIM_SNAP_DISTANCE_SQUARED: f32 = 0.125;
/// Tolerance on each column's squared norm.
const COLUMN_NORM_EPSILON: f64 = 1e-4;
/// Snapped cell code for a zero ratio; codes `0..4` are the exponent `k` of `i^k`.
const ZERO_CELL: u8 = 4;

/// Converts a Clifford unitary matrix into the corresponding tableau, up to global phase.
///
/// `matrix` is a list of rows. When `little_endian` is true, qubit 0 corresponds to the least
/// significant amplitude bit. When it is false, qubit 0 corresponds to the most significant
/// amplitude bit, matching Stim's `unitary_to_tableau` convention.
///
/// # Acceptance contract
///
/// Let `d = 2^n` be the dimension and `β` the largest-magnitude entry of column 0 (the first one
/// on ties). The matrix is accepted, and the tableau `T` returned, exactly when:
///
/// 1. every entry is finite;
/// 2. every column's squared norm is within `1e-4` of 1;
/// 3. every ratio `M[r][c] / β` lies within squared distance `0.125` of a value `s[r][c]` in
///    `{0, 1, -1, i, -i}`, which is Stim v1.16.0's stabilizer-state smoothing threshold; and
/// 4. the snapped matrix `s` is exactly a scalar multiple of the unitary of the Clifford tableau
///    `T`, checked on every entry.
///
/// Condition 4 is decided in exact arithmetic on the snapped matrix: the image of each generator
/// is read from column 0 of `s P s†` and from the `n` columns `2^p`, and every column `j` of `s`
/// is then checked to equal the image of `X^j` applied to column 0, which must itself be
/// stabilized by every image of `Z`. Like Stim, a matrix within the snapping tolerance of a
/// Clifford is accepted without a separate per-entry unitarity check, so a near-Clifford matrix
/// whose columns have unit norm but are not exactly orthogonal is accepted.
///
/// # Errors
///
/// - A height that is zero or not a power of two, or a row whose width differs from the height,
///   is a shape error, checked before any entry is read.
/// - A non-finite entry, or a column 0 whose entries are all zero, is
///   [`StabilizerError::MatrixNotUnitary`].
/// - Any other rejected matrix is classified by the full Gram check: it is
///   [`StabilizerError::MatrixNotUnitary`] when some entry of `M M†` differs from the identity
///   by more than `1e-4`, and [`StabilizerError::UnitaryMatrixNotClifford`] otherwise.
///
/// # Cost
///
/// Accepting costs `O(n K d + d^2)`, where `K <= d` is the number of nonzero entries in a column
/// of the Clifford, and allocates one byte per entry, reserved fallibly, besides `O(d)` scratch.
/// Only a rejected matrix pays the `O(d^3)` Gram check. Any power-of-two dimension is accepted,
/// like Stim; the CPU time is the caller's.
pub fn unitary_to_tableau(
    matrix: &[Vec<Complex32>],
    little_endian: bool,
) -> StabilizerResult<Tableau> {
    let num_qubits = validate_matrix_shape(matrix)?;
    if let Some(tableau) = clifford_tableau(matrix, num_qubits, little_endian)? {
        return Ok(tableau);
    }
    ensure_unitary(matrix)?;
    Err(StabilizerError::UnitaryMatrixNotClifford)
}

/// Returns the tableau when the matrix satisfies the acceptance contract, `None` when it must
/// be classified by the Gram check, or an error that needs no classification.
fn clifford_tableau(
    matrix: &[Vec<Complex32>],
    num_qubits: usize,
    little_endian: bool,
) -> StabilizerResult<Option<Tableau>> {
    let Some(snapped) = SnappedMatrix::new(matrix)? else {
        return Ok(None);
    };
    let Some(row0_support) = snapped.row_support(0)? else {
        return Ok(None);
    };

    let mut x_images = Vec::with_capacity(num_qubits);
    let mut z_images = Vec::with_capacity(num_qubits);
    let mut xs = Vec::with_capacity(num_qubits);
    let mut zs = Vec::with_capacity(num_qubits);
    for qubit in 0..num_qubits {
        let bit = amplitude_bit(qubit, num_qubits, little_endian);
        let (Some(x_image), Some(z_image)) = (
            snapped.generator_image(num_qubits, bit, PauliBasis::X, &row0_support),
            snapped.generator_image(num_qubits, bit, PauliBasis::Z, &row0_support),
        ) else {
            return Ok(None);
        };
        xs.push(x_image.to_pauli(num_qubits, little_endian));
        zs.push(z_image.to_pauli(num_qubits, little_endian));
        x_images.push((bit, x_image));
        z_images.push(z_image);
    }

    let tableau = match Tableau::from_conjugated_generators(xs, zs) {
        Ok(tableau) => tableau,
        Err(
            StabilizerError::ConjugatedGeneratorPairCommutes { .. }
            | StabilizerError::ConjugatedGeneratorsAnticommute { .. },
        ) => return Ok(None),
        Err(other) => return Err(other),
    };
    let verified = z_images
        .iter()
        .all(|image| snapped.column0_is_stabilized_by(*image))
        && x_images
            .iter()
            .all(|(bit, image)| snapped.columns_follow_x_image(*bit, *image));
    Ok(verified.then_some(tableau))
}

/// The ratios `M[r][c] / β` snapped to `{0, ±1, ±i}`, stored row-major.
struct SnappedMatrix {
    dimension: usize,
    cells: Vec<u8>,
}

impl SnappedMatrix {
    /// Snaps every entry, or returns `None` when a column norm or a ratio is out of tolerance.
    fn new(matrix: &[Vec<Complex32>]) -> StabilizerResult<Option<Self>> {
        let dimension = matrix.len();
        if matrix
            .iter()
            .flatten()
            .any(|value| !value.re.is_finite() || !value.im.is_finite())
        {
            return Err(StabilizerError::MatrixNotUnitary);
        }
        let mut base = Complex32::new(0.0, 0.0);
        for value in matrix.iter().filter_map(|row| row.first()) {
            if value.norm_sqr() > base.norm_sqr() {
                base = *value;
            }
        }
        if base.norm_sqr() == 0.0 {
            return Err(StabilizerError::MatrixNotUnitary);
        }

        let mut column_norms = try_with_capacity(StabilizerResource::Tableau, dimension)?;
        column_norms.resize(dimension, 0.0_f64);
        for row in matrix {
            for (norm, value) in column_norms.iter_mut().zip(row) {
                *norm += f64::from(value.norm_sqr());
            }
        }
        if column_norms
            .iter()
            .any(|norm| (norm - 1.0).abs() > COLUMN_NORM_EPSILON)
        {
            return Ok(None);
        }

        let cell_count = dimension
            .checked_mul(dimension)
            .ok_or_else(|| allocation_failed(StabilizerResource::Tableau, usize::MAX))?;
        let mut cells = try_with_capacity(StabilizerResource::Tableau, cell_count)?;
        let inverse_base = base.inv();
        for value in matrix.iter().flatten() {
            let Some(cell) = snap_ratio(*value * inverse_base) else {
                return Ok(None);
            };
            cells.push(cell);
        }
        Ok(Some(Self { dimension, cells }))
    }

    fn row(&self, row: usize) -> Option<&[u8]> {
        let start = row.checked_mul(self.dimension)?;
        self.cells.get(start..start.checked_add(self.dimension)?)
    }

    /// The nonzero cells of `row` as `(column, exponent of the conjugate)`.
    fn row_support(&self, row: usize) -> StabilizerResult<Option<Vec<(usize, u32)>>> {
        let Some(cells) = self.row(row) else {
            return Ok(None);
        };
        let count = cells.iter().filter(|cell| **cell != ZERO_CELL).count();
        let mut support = try_with_capacity(StabilizerResource::Tableau, count)?;
        support.extend(
            cells
                .iter()
                .enumerate()
                .filter(|(_, cell)| **cell != ZERO_CELL)
                .map(|(column, cell)| (column, conjugate_exponent(*cell))),
        );
        Ok(Some(support))
    }

    /// Identifies `s P s† / K` for the generator `P` on amplitude bit `bit`, where `K` is the
    /// number of nonzero cells in row 0, or returns `None` when it is not a Hermitian Pauli.
    fn generator_image(
        &self,
        num_qubits: usize,
        bit: usize,
        basis: PauliBasis,
        row0_support: &[(usize, u32)],
    ) -> Option<PauliImage> {
        let mask = 1_usize.checked_shl(u32::try_from(bit).ok()?)?;
        let generator = PauliImage {
            negative: false,
            x: if basis.x_bit() { mask } else { 0 },
            z: if basis.z_bit() { mask } else { 0 },
        };
        let weight = row0_support.len();

        // Column 0 of `s P s†` has one nonzero entry, at row `x`, worth `K` times its phase.
        let mut found = None;
        for row_index in 0..self.dimension {
            let row = self.row(row_index)?;
            let terms = row0_support.iter().copied();
            if let Some(exponent) = conjugation_entry(row, terms, generator, weight)? {
                if found.is_some() {
                    return None;
                }
                found = Some((row_index, exponent));
            }
        }
        let (x, column0_exponent) = found?;

        // Entry `(2^p ^ x, 2^p)` carries the extra sign `(-1)^(z_p)`.
        let mut z = 0_usize;
        for p in 0..num_qubits {
            let column = 1_usize.checked_shl(u32::try_from(p).ok()?)?;
            let target = self.row(column ^ x)?;
            let terms = self
                .row(column)?
                .iter()
                .enumerate()
                .filter(|(_, cell)| **cell != ZERO_CELL)
                .map(|(index, cell)| (index, conjugate_exponent(*cell)));
            let exponent = conjugation_entry(target, terms, generator, weight)??;
            if exponent == (column0_exponent + 2) % 4 {
                z |= column;
            } else if exponent != column0_exponent {
                return None;
            }
        }

        let negative = match (column0_exponent + 4 - (x & z).count_ones() % 4) % 4 {
            0 => false,
            2 => true,
            _ => return None,
        };
        Some(PauliImage { negative, x, z })
    }

    /// Checks `Z' s[:, 0] = s[:, 0]` for one image `Z'`.
    fn column0_is_stabilized_by(&self, image: PauliImage) -> bool {
        (0..self.dimension).all(|column| {
            let (Some(target), Some(source)) = (
                self.row(column ^ image.x).and_then(|row| row.first()),
                self.row(column).and_then(|row| row.first()),
            ) else {
                return false;
            };
            u32::from(*target) == times(*source, image.exponent(column))
        })
    }

    /// Checks `s[:, j] = X' s[:, j ^ 2^bit]` for every column `j` whose lowest set bit is `bit`.
    fn columns_follow_x_image(&self, bit: usize, image: PauliImage) -> bool {
        let Some(step) = u32::try_from(bit)
            .ok()
            .and_then(|bit| 1_usize.checked_shl(bit))
        else {
            return false;
        };
        let Some(stride) = step.checked_mul(2) else {
            return false;
        };
        (0..self.dimension).all(|column| {
            let (Some(target), Some(source)) = (self.row(column ^ image.x), self.row(column))
            else {
                return false;
            };
            let exponent = image.exponent(column);
            (step..self.dimension).step_by(stride).all(|j| {
                match (target.get(j), source.get(j ^ step)) {
                    (Some(target), Some(source)) => u32::from(*target) == times(*source, exponent),
                    _ => false,
                }
            })
        })
    }
}

/// A Hermitian Pauli `(-1)^negative i^{|x & z|} X^x Z^z` over amplitude bits, so that it maps
/// basis state `c` to `(-1)^negative i^{|x & z|} (-1)^{|z & c|}` times basis state `c ^ x`.
#[derive(Clone, Copy, Debug)]
struct PauliImage {
    negative: bool,
    x: usize,
    z: usize,
}

impl PauliImage {
    /// Exponent `k` of the phase `i^k` this Pauli puts on basis state `column`.
    fn exponent(self, column: usize) -> u32 {
        let mut exponent = (self.x & self.z).count_ones() % 4;
        if (self.z & column).count_ones() % 2 == 1 {
            exponent += 2;
        }
        if self.negative {
            exponent += 2;
        }
        exponent % 4
    }

    fn to_pauli(self, num_qubits: usize, little_endian: bool) -> PauliString {
        let sign = if self.negative {
            PauliSign::Minus
        } else {
            PauliSign::Plus
        };
        PauliString::from_bases_unchecked(
            sign,
            (0..num_qubits).map(|qubit| {
                let mask = 1_usize << amplitude_bit(qubit, num_qubits, little_endian);
                PauliBasis::from_xz(self.x & mask != 0, self.z & mask != 0)
            }),
        )
    }
}

/// One entry of `s P s†`: `Σ target[k ^ x_P] i^{p(k)} conj(s[c][k])` over the nonzero terms
/// `(k, exponent of conj(s[c][k]))` of row `c`.
///
/// Returns `Some(None)` for zero, `Some(Some(k))` for `weight * i^k`, and `None` otherwise.
fn conjugation_entry(
    target: &[u8],
    terms: impl Iterator<Item = (usize, u32)>,
    generator: PauliImage,
    weight: usize,
) -> Option<Option<u32>> {
    let mut counts = [0_usize; 4];
    for (column, conjugate) in terms {
        let cell = *target.get(column ^ generator.x)?;
        let exponent = times(cell, generator.exponent(column) + conjugate);
        if let Some(count) = usize::try_from(exponent)
            .ok()
            .and_then(|exponent| counts.get_mut(exponent))
        {
            *count = count.checked_add(1)?;
        }
    }
    let [one, i, minus_one, minus_i] = counts.map(i128::try_from);
    let real = one.ok()? - minus_one.ok()?;
    let imaginary = i.ok()? - minus_i.ok()?;
    let weight = i128::try_from(weight).ok()?;
    match (real, imaginary) {
        (0, 0) => Some(None),
        (real, 0) if real == weight => Some(Some(0)),
        (0, imaginary) if imaginary == weight => Some(Some(1)),
        (real, 0) if real == -weight => Some(Some(2)),
        (0, imaginary) if imaginary == -weight => Some(Some(3)),
        _ => None,
    }
}

/// Exponent of the conjugate of a nonzero snapped cell.
fn conjugate_exponent(cell: u8) -> u32 {
    (4 - u32::from(cell) % 4) % 4
}

/// Multiplies a snapped cell by `i^exponent`, returning the product's cell code.
fn times(cell: u8, exponent: u32) -> u32 {
    if cell == ZERO_CELL {
        u32::from(ZERO_CELL)
    } else {
        (u32::from(cell) + exponent) % 4
    }
}

/// Snaps a ratio to `{0, 1, i, -1, -i}` under Stim's smoothing threshold.
fn snap_ratio(ratio: Complex32) -> Option<u8> {
    [
        (ZERO_CELL, Complex32::new(0.0, 0.0)),
        (0, Complex32::new(1.0, 0.0)),
        (1, Complex32::new(0.0, 1.0)),
        (2, Complex32::new(-1.0, 0.0)),
        (3, Complex32::new(0.0, -1.0)),
    ]
    .into_iter()
    .find(|(_, candidate)| (ratio - candidate).norm_sqr() < STIM_SNAP_DISTANCE_SQUARED)
    .map(|(cell, _)| cell)
}

fn validate_matrix_shape(matrix: &[Vec<Complex32>]) -> StabilizerResult<usize> {
    let height = matrix.len();
    if height == 0 || !height.is_power_of_two() {
        return Err(StabilizerError::UnitaryMatrixHeightNotPowerOfTwo { height });
    }
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

/// The `O(d^3)` Gram check that classifies a rejected matrix.
fn ensure_unitary(matrix: &[Vec<Complex32>]) -> StabilizerResult<()> {
    for (left_index, left) in matrix.iter().enumerate() {
        for (right_index, right) in matrix.iter().enumerate() {
            let mut dot = Complex32::new(0.0, 0.0);
            for (left, right) in left.iter().zip(right) {
                dot += *left * right.conj();
            }
            let expected = if left_index == right_index {
                Complex32::new(1.0, 0.0)
            } else {
                Complex32::new(0.0, 0.0)
            };
            if (dot - expected).norm_sqr() > UNITARITY_EPSILON * UNITARITY_EPSILON {
                return Err(StabilizerError::MatrixNotUnitary);
            }
        }
    }
    Ok(())
}

fn amplitude_bit(qubit: usize, num_qubits: usize, little_endian: bool) -> usize {
    if little_endian {
        qubit
    } else {
        num_qubits - qubit - 1
    }
}
