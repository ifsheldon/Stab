//! Uniform sampling over the Clifford group, ported from Stim's
//! `random_stabilizer_tableau_raw` (`src/stim/stabilizers/tableau.inl`).
//!
//! Reference: Sergey Bravyi and Dmitri Maslov, "Hadamard-free circuits expose
//! the structure of the Clifford group", <https://arxiv.org/abs/2003.09412>.
//!
//! The qubit count is bounded by
//! [`crate::StabilizerResource::RandomTableauQubits`] (at most 64), so each
//! `n x n` GF(2) matrix row fits one `u64` and each `2n x 2n` row one `u128`.
#![allow(
    clippy::indexing_slicing,
    reason = "every matrix has exactly n or 2n rows and every index is below that bound"
)]

use rand::{Rng, RngExt as _};

/// Samples the `2n x 2n` symplectic matrix of a uniformly random Clifford
/// operation. Row `r < n` is the image of `X_r` and row `n + r` the image of
/// `Z_r`; bit `c < n` is an X component and bit `n + c` a Z component.
pub(super) fn random_symplectic<R>(n: usize, rng: &mut R) -> Vec<u128>
where
    R: Rng + ?Sized,
{
    debug_assert!(n <= 64);
    let (hadamard, permutation) = sample_quantum_mallows(n, rng);

    let mut symmetric = vec![0_u64; n];
    let mut symmetric_m = vec![0_u64; n];
    for row in 0..n {
        symmetric[row] = random_low_bits(rng, row + 1);
        let mut bits = random_low_bits(rng, row + 1);
        if !hadamard[row] {
            bits &= !(1_u64 << row);
        }
        for col in 0..row {
            let keep = (hadamard[row] && hadamard[col])
                || (hadamard[row] && !hadamard[col] && permutation[row] < permutation[col])
                || (!hadamard[row] && hadamard[col] && permutation[row] > permutation[col]);
            if !keep {
                bits &= !(1_u64 << col);
            }
        }
        symmetric_m[row] = bits;
    }
    mirror_lower_triangle(&mut symmetric);
    mirror_lower_triangle(&mut symmetric_m);

    let mut lower = vec![0_u64; n];
    let mut lower_m = vec![0_u64; n];
    for row in 0..n {
        lower[row] = random_low_bits(rng, row) | (1_u64 << row);
        let mut bits = random_low_bits(rng, row);
        for col in 0..row {
            let keep = (!hadamard[row] && hadamard[col])
                || (hadamard[row] && hadamard[col] && permutation[row] > permutation[col])
                || (!hadamard[row] && !hadamard[col] && permutation[row] < permutation[col]);
            if !keep {
                bits &= !(1_u64 << col);
            }
        }
        lower_m[row] = bits | (1_u64 << row);
    }

    let fused = from_quadrants(
        &lower,
        &multiply(&symmetric, &lower),
        &transpose(&inverse_lower_triangular(&lower)),
    );
    let fused_m = from_quadrants(
        &lower_m,
        &multiply(&symmetric_m, &lower_m),
        &transpose(&inverse_lower_triangular(&lower_m)),
    );

    let mut permuted = vec![0_u128; 2 * n];
    for row in 0..n {
        let source = permutation[row];
        let (x_row, z_row) = (fused[source], fused[source + n]);
        if hadamard[row] {
            (permuted[row], permuted[row + n]) = (z_row, x_row);
        } else {
            (permuted[row], permuted[row + n]) = (x_row, z_row);
        }
    }
    fused_m
        .iter()
        .map(|&row| combine_rows(row, &permuted))
        .collect()
}

/// Samples the Hadamard layer and qubit permutation from the quantum Mallows
/// distribution.
fn sample_quantum_mallows<R>(n: usize, rng: &mut R) -> (Vec<bool>, Vec<usize>)
where
    R: Rng + ?Sized,
{
    let mut hadamard = Vec::with_capacity(n);
    let mut permutation = Vec::with_capacity(n);
    let mut remaining = (0..n).collect::<Vec<_>>();
    for _ in 0..n {
        let m = remaining.len();
        let u = rng.random::<f64>();
        let eps = 4_f64.powi(-i32::try_from(m).unwrap_or(i32::MAX));
        // `-ceil(log2(u + (1 - u) eps))` lies in `0..2m` because
        // `u + (1 - u) eps` lies in `[4^-m, 1)`.
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the sampled value is a small nonnegative integer below 2m <= 128"
        )]
        let sampled = (-(u + (1.0 - u) * eps).log2().ceil()) as usize;
        // `u == 0` would give exactly `2m`; that measure-zero edge joins its
        // neighbouring bucket instead of indexing past `remaining`.
        let mut k = sampled.min(2 * m - 1);
        hadamard.push(k < m);
        if k >= m {
            k = 2 * m - k - 1;
        }
        permutation.push(remaining.remove(k));
    }
    (hadamard, permutation)
}

fn random_low_bits<R>(rng: &mut R, count: usize) -> u64
where
    R: Rng + ?Sized,
{
    match count {
        0 => 0,
        64.. => rng.random::<u64>(),
        _ => rng.random::<u64>() & ((1_u64 << count) - 1),
    }
}

fn mirror_lower_triangle(matrix: &mut [u64]) {
    for row in 0..matrix.len() {
        for col in 0..row {
            if (matrix[row] >> col) & 1 == 1 {
                matrix[col] |= 1_u64 << row;
            }
        }
    }
}

/// Returns `left * right` over GF(2).
fn multiply(left: &[u64], right: &[u64]) -> Vec<u64> {
    left.iter()
        .map(|&row| {
            right
                .iter()
                .enumerate()
                .filter(|(k, _)| (row >> k) & 1 == 1)
                .fold(0, |acc, (_, &right_row)| acc ^ right_row)
        })
        .collect()
}

fn combine_rows(selector: u128, rows: &[u128]) -> u128 {
    rows.iter()
        .enumerate()
        .filter(|(k, _)| (selector >> k) & 1 == 1)
        .fold(0, |acc, (_, &row)| acc ^ row)
}

/// Inverts a unit lower-triangular matrix by forward elimination.
fn inverse_lower_triangular(matrix: &[u64]) -> Vec<u64> {
    let n = matrix.len();
    let mut result = (0..n).map(|row| 1_u64 << row).collect::<Vec<_>>();
    for target in 0..n {
        let mut row = matrix[target];
        for pivot in 0..target {
            if (row >> pivot) & 1 == 1 {
                row ^= matrix[pivot];
                result[target] ^= result[pivot];
            }
        }
    }
    result
}

fn transpose(matrix: &[u64]) -> Vec<u64> {
    let n = matrix.len();
    (0..n)
        .map(|col| {
            (0..n)
                .filter(|&row| (matrix[row] >> col) & 1 == 1)
                .fold(0, |acc, row| acc | (1_u64 << row))
        })
        .collect()
}

/// Builds `[[upper_left, 0], [lower_left, lower_right]]`.
fn from_quadrants(upper_left: &[u64], lower_left: &[u64], lower_right: &[u64]) -> Vec<u128> {
    let n = upper_left.len();
    upper_left
        .iter()
        .map(|&row| u128::from(row))
        .chain(
            lower_left
                .iter()
                .zip(lower_right)
                .map(|(&left, &right)| u128::from(left) | (u128::from(right) << n)),
        )
        .collect()
}
