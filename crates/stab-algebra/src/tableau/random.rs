//! Uniform sampling over the Clifford group, ported from Stim's
//! `random_stabilizer_tableau_raw` and `Tableau::random` (`src/stim/stabilizers/tableau.inl`).
//!
//! Reference: Sergey Bravyi and Dmitri Maslov, "Hadamard-free circuits expose
//! the structure of the Clifford group", <https://arxiv.org/abs/2003.09412>.
//!
//! The `n x n` GF(2) matrices are packed [`BitMatrix`] rows and the `2n x 2n` symplectic
//! matrices are [`PauliRows`], whose row `r < n` is the image of `X_r` and row `n + r` the image
//! of `Z_r`. Matrix products XOR whole rows, so sampling costs `O(n^3 / 64)` word operations.
//! For `n <= 64` the random stream is consumed exactly as by the earlier single-word sampler, so
//! seeded tableaux at those widths are unchanged.

use rand::{Rng, RngExt as _};
use stab_bits::BitMatrix;

use super::{PauliRows, Tableau};
use crate::budget::{StabilizerResource, pauli_rows_bytes, tableau_bytes, try_with_capacity};
use crate::{StabilizerError, StabilizerResult};

const WORD_BITS: usize = u64::BITS as usize;

/// Square matrices [`random_tableau`] allocates: the four triangular inputs, the two transposes
/// that mirror the symmetric ones, and for each fused matrix a product, an inverse, and the
/// inverse's transpose.
const SQUARE_MATRICES: u128 = 12;
/// Symplectic `2n x 2n` matrices besides the output: both fused matrices and the permuted one.
const SYMPLECTIC_MATRICES: u128 = 3;

/// Conservative bytes allocated by [`random_tableau`] for `num_qubits`, counting every matrix
/// and vector it creates as if all were alive together.
pub(super) fn random_tableau_bytes(num_qubits: usize) -> u128 {
    let n = num_qubits as u128;
    let square = n * (n.div_ceil(64) * 8);
    let vectors = n * (size_of::<bool>() as u128 + 2 * size_of::<usize>() as u128);
    tableau_bytes(num_qubits)
        + SYMPLECTIC_MATRICES * pauli_rows_bytes(2 * n, num_qubits)
        + SQUARE_MATRICES * square
        + vectors
}

pub(super) fn random_tableau<R>(n: usize, rng: &mut R) -> StabilizerResult<Tableau>
where
    R: Rng + ?Sized,
{
    let (hadamard, permutation) = sample_quantum_mallows(n, rng)?;

    let mut symmetric = BitMatrix::zeros(n, n)?;
    let mut symmetric_m = BitMatrix::zeros(n, n)?;
    for row in 0..n {
        randomize_prefix(&mut symmetric, row, row + 1, rng)?;
        randomize_prefix(&mut symmetric_m, row, row + 1, rng)?;
        let keep = |col: usize| -> StabilizerResult<bool> {
            if col == row {
                return flag(&hadamard, row);
            }
            let (h_row, h_col) = (flag(&hadamard, row)?, flag(&hadamard, col)?);
            let (p_row, p_col) = (index(&permutation, row)?, index(&permutation, col)?);
            Ok((h_row && h_col)
                || (h_row && !h_col && p_row < p_col)
                || (!h_row && h_col && p_row > p_col))
        };
        mask_row(&mut symmetric_m, row, row + 1, keep)?;
    }
    mirror_lower_triangle(&mut symmetric)?;
    mirror_lower_triangle(&mut symmetric_m)?;

    let mut lower = BitMatrix::identity(n)?;
    let mut lower_m = BitMatrix::identity(n)?;
    for row in 0..n {
        randomize_prefix(&mut lower, row, row, rng)?;
        lower.set(row, row, true)?;
        randomize_prefix(&mut lower_m, row, row, rng)?;
        let keep = |col: usize| -> StabilizerResult<bool> {
            let (h_row, h_col) = (flag(&hadamard, row)?, flag(&hadamard, col)?);
            let (p_row, p_col) = (index(&permutation, row)?, index(&permutation, col)?);
            Ok((!h_row && h_col)
                || (h_row && h_col && p_row > p_col)
                || (!h_row && !h_col && p_row < p_col))
        };
        mask_row(&mut lower_m, row, row, keep)?;
        lower_m.set(row, row, true)?;
    }

    let fused = fuse(&lower, &symmetric)?;
    drop((lower, symmetric));
    let fused_m = fuse(&lower_m, &symmetric_m)?;
    drop((lower_m, symmetric_m));

    let mut permuted = PauliRows::try_zeroed(2 * n, n, StabilizerResource::RandomTableau)?;
    for row in 0..n {
        let source = index(&permutation, row)?;
        let (x_target, z_target) = if flag(&hadamard, row)? {
            (n + row, row)
        } else {
            (row, n + row)
        };
        permuted
            .row_mut(x_target)?
            .copy_from_slice(fused.row(source)?);
        permuted
            .row_mut(z_target)?
            .copy_from_slice(fused.row(n + source)?);
    }
    drop(fused);

    let mut result = PauliRows::try_zeroed(2 * n, n, StabilizerResource::RandomTableau)?;
    // `permuted` is a symplectic matrix, so conjugating `fused_m`'s rows by it as an unsigned
    // Tableau yields exactly the GF(2) product; the signs it computes are replaced below.
    super::action::apply_rows(&permuted, &fused_m, &mut result)?;
    for row in 0..2 * n {
        result.set_sign(row, rng.random_bool(0.5))?;
    }
    Ok(Tableau::from_rows(result))
}

/// Samples the Hadamard layer and qubit permutation from the quantum Mallows distribution.
fn sample_quantum_mallows<R>(n: usize, rng: &mut R) -> StabilizerResult<(Vec<bool>, Vec<usize>)>
where
    R: Rng + ?Sized,
{
    let mut hadamard = try_with_capacity(StabilizerResource::RandomTableau, n)?;
    let mut permutation = try_with_capacity(StabilizerResource::RandomTableau, n)?;
    let mut remaining = try_with_capacity(StabilizerResource::RandomTableau, n)?;
    remaining.extend(0..n);
    for _ in 0..n {
        let m = remaining.len();
        let u = rng.random::<f64>();
        let eps = 4_f64.powi(-i32::try_from(m).unwrap_or(i32::MAX));
        // `-ceil(log2(u + (1 - u) eps))` lies in `0..2m` because `u + (1 - u) eps` lies in
        // `[4^-m, 1)`; for `m` beyond the exponent range `eps` underflows to zero and the
        // sample is clamped below instead.
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the sampled value is a nonnegative integer that the clamp below bounds by 2m"
        )]
        let sampled = (-(u + (1.0 - u) * eps).log2().ceil()) as usize;
        // `u == 0` would give at least `2m`; that measure-zero edge joins its neighbouring
        // bucket instead of indexing past `remaining`.
        let mut k = sampled.min(2 * m - 1);
        hadamard.push(k < m);
        if k >= m {
            k = 2 * m - k - 1;
        }
        if k >= remaining.len() {
            return Err(StabilizerError::TableauIndexOutOfRange {
                index: k,
                len: remaining.len(),
            });
        }
        permutation.push(remaining.remove(k));
    }
    Ok((hadamard, permutation))
}

fn flag(values: &[bool], position: usize) -> StabilizerResult<bool> {
    values
        .get(position)
        .copied()
        .ok_or(StabilizerError::TableauIndexOutOfRange {
            index: position,
            len: values.len(),
        })
}

fn index(values: &[usize], position: usize) -> StabilizerResult<usize> {
    values
        .get(position)
        .copied()
        .ok_or(StabilizerError::TableauIndexOutOfRange {
            index: position,
            len: values.len(),
        })
}

/// Replaces bits `0..count` of `row` with random bits, drawing one `u64` per started word and
/// nothing when `count` is zero.
fn randomize_prefix<R>(
    matrix: &mut BitMatrix,
    row: usize,
    count: usize,
    rng: &mut R,
) -> StabilizerResult<()>
where
    R: Rng + ?Sized,
{
    let mut words = matrix.row_words_mut(row)?;
    let mut remaining = count;
    for word in words.iter_mut() {
        if remaining == 0 {
            break;
        }
        let drawn = rng.random::<u64>();
        if remaining >= WORD_BITS {
            *word = drawn;
            remaining -= WORD_BITS;
        } else {
            let mask = (1_u64 << remaining) - 1;
            *word = (*word & !mask) | (drawn & mask);
            remaining = 0;
        }
    }
    Ok(())
}

/// Clears bits `col < count` of `row` for which `keep(col)` is false.
fn mask_row(
    matrix: &mut BitMatrix,
    row: usize,
    count: usize,
    keep: impl Fn(usize) -> StabilizerResult<bool>,
) -> StabilizerResult<()> {
    for col in 0..count {
        if matrix.get(row, col) == Some(true) && !keep(col)? {
            matrix.set(row, col, false)?;
        }
    }
    Ok(())
}

/// Copies the lower triangle onto the upper triangle.
fn mirror_lower_triangle(matrix: &mut BitMatrix) -> StabilizerResult<()> {
    let n = matrix.rows();
    let mut transposed = transpose(matrix)?;
    for row in 0..n {
        // Keep the transpose's strictly upper part, which is the original strictly lower part.
        let mut words = transposed.row_words_mut(row)?;
        clear_prefix(&mut words, row + 1);
    }
    for row in 0..n {
        let upper = transposed.row(row)?;
        let mut target = matrix.row_words_mut(row)?;
        for (target, source) in target.iter_mut().zip(upper.words()) {
            *target |= *source;
        }
    }
    Ok(())
}

/// Below this width a bit-by-bit transpose is cheaper than one 64 x 64 tile transpose, which
/// would mostly move padding.
const BITWISE_TRANSPOSE_WIDTH: usize = 16;

/// Transposes a square matrix, bit by bit when it is narrow and by tiles otherwise.
fn transpose(matrix: &BitMatrix) -> StabilizerResult<BitMatrix> {
    let n = matrix.rows();
    if n > BITWISE_TRANSPOSE_WIDTH {
        return Ok(matrix.transpose()?);
    }
    let mut result = BitMatrix::zeros(n, n)?;
    for row in 0..n {
        for col in 0..n {
            if matrix.get(row, col) == Some(true) {
                result.set(col, row, true)?;
            }
        }
    }
    Ok(result)
}

fn clear_prefix(words: &mut [u64], count: usize) {
    let mut remaining = count;
    for word in words.iter_mut() {
        if remaining == 0 {
            break;
        }
        if remaining >= WORD_BITS {
            *word = 0;
            remaining -= WORD_BITS;
        } else {
            *word &= !((1_u64 << remaining) - 1);
            remaining = 0;
        }
    }
}

/// Builds `[[lower, 0], [symmetric * lower, (lower^-1)^T]]` as symplectic rows.
fn fuse(lower: &BitMatrix, symmetric: &BitMatrix) -> StabilizerResult<PauliRows> {
    let n = lower.rows();
    let product = multiply_square(symmetric, lower)?;
    let inverse_transpose = transpose(&inverse_lower_triangular(lower)?)?;
    let mut fused = PauliRows::try_zeroed(2 * n, n, StabilizerResource::RandomTableau)?;
    let half = fused.half_words();
    for row in 0..n {
        copy_half(fused.row_mut(row)?, 0, half, lower.row(row)?.words())?;
        let target = fused.row_mut(n + row)?;
        copy_half(target, 0, half, product.row(row)?.words())?;
        copy_half(target, 1, half, inverse_transpose.row(row)?.words())?;
    }
    Ok(fused)
}

fn copy_half(
    row: &mut [u64],
    half: usize,
    half_words: usize,
    source: &[u64],
) -> StabilizerResult<()> {
    let row_len = row.len();
    let target = row
        .get_mut(half * half_words..(half + 1) * half_words)
        .ok_or(StabilizerError::LengthMismatch {
            left: source.len(),
            right: row_len,
        })?;
    if target.len() != source.len() {
        return Err(StabilizerError::LengthMismatch {
            left: source.len(),
            right: target.len(),
        });
    }
    target.copy_from_slice(source);
    Ok(())
}

/// Returns `left * right` over GF(2).
fn multiply_square(left: &BitMatrix, right: &BitMatrix) -> StabilizerResult<BitMatrix> {
    let n = left.rows();
    let mut result = BitMatrix::zeros(n, n)?;
    let words = n.div_ceil(WORD_BITS);
    for_each_selected_blocked(
        n,
        words,
        |row, word| word_at(left.row(row)?.words(), word),
        |row, position| {
            xor_words(
                &mut result.row_words_mut(row)?,
                right.row(position)?.words(),
            )
        },
    )?;
    Ok(result)
}

/// Inverts a unit lower-triangular matrix: row `t` of the inverse is `e_t` XOR the inverse rows
/// selected by the strictly lower bits of row `t`. Selections are visited in ascending position,
/// so every selected inverse row is complete before it is used.
fn inverse_lower_triangular(matrix: &BitMatrix) -> StabilizerResult<BitMatrix> {
    let n = matrix.rows();
    let mut result = BitMatrix::identity(n)?;
    let words = n.div_ceil(WORD_BITS);
    for_each_selected_blocked(
        n,
        words,
        |row, word| word_at(matrix.row(row)?.words(), word),
        |row, position| {
            if position < row {
                result.xor_row_into(position, row)?;
            }
            Ok(())
        },
    )?;
    Ok(result)
}

/// Output rows handled together by [`for_each_selected_blocked`].
const BLOCK_ROWS: usize = 64;
/// Below this many rows the operands stay in cache, so rows are visited one at a time instead of
/// paying for the block transposition.
const BLOCKED_MIN_ROWS: usize = 4 * BLOCK_ROWS;

/// Calls `visit(row, position)` for every set bit of every row's selection words, in ascending
/// position for each row and, across rows, never visiting a position before every smaller one.
/// Large inputs take rows 64 at a time and transpose their selection words, so each selected
/// source row is loaded once per block while it is in cache.
fn for_each_selected_blocked(
    rows: usize,
    words_per_row: usize,
    mut selection_word: impl FnMut(usize, usize) -> StabilizerResult<u64>,
    mut visit: impl FnMut(usize, usize) -> StabilizerResult<()>,
) -> StabilizerResult<()> {
    if rows < BLOCKED_MIN_ROWS {
        for row in 0..rows {
            for word_index in 0..words_per_row {
                let mut selected = selection_word(row, word_index)?;
                while let Some(bit) = selected.lowest_one() {
                    let bit = bit as usize;
                    selected &= selected - 1;
                    visit(row, word_index * WORD_BITS + bit)?;
                }
            }
        }
        return Ok(());
    }
    let mut tile = BitMatrix::zeros(WORD_BITS, BLOCK_ROWS)?;
    let mut words = [0_u64; BLOCK_ROWS];
    for first in (0..rows).step_by(BLOCK_ROWS) {
        let count = BLOCK_ROWS.min(rows - first);
        for word_index in 0..words_per_row {
            for (offset, word) in words.iter_mut().enumerate() {
                *word = if offset < count {
                    selection_word(first + offset, word_index)?
                } else {
                    0
                };
            }
            if words.iter().all(|word| *word == 0) {
                continue;
            }
            tile.copy_transpose_from_word_rows(&words)?;
            for bit in 0..WORD_BITS {
                let mut selected = word_at(tile.row(bit)?.words(), 0)?;
                while let Some(offset) = selected.lowest_one() {
                    let offset = offset as usize;
                    selected &= selected - 1;
                    visit(first + offset, word_index * WORD_BITS + bit)?;
                }
            }
        }
    }
    Ok(())
}

fn word_at(words: &[u64], index: usize) -> StabilizerResult<u64> {
    words
        .get(index)
        .copied()
        .ok_or(StabilizerError::TableauIndexOutOfRange {
            index,
            len: words.len(),
        })
}

fn xor_words(target: &mut [u64], source: &[u64]) -> StabilizerResult<()> {
    if target.len() != source.len() {
        return Err(StabilizerError::LengthMismatch {
            left: source.len(),
            right: target.len(),
        });
    }
    for (target, source) in target.iter_mut().zip(source) {
        *target ^= *source;
    }
    Ok(())
}
