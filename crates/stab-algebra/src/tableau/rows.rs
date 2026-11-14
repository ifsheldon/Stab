//! Packed signed Pauli rows, the storage of [`super::Tableau`] and of the stabilizer solver.
//!
//! Each row stores `half_words` X words followed by `half_words` Z words, with the bits past
//! `num_qubits` in each half kept zero, and the row signs are one packed plane. Every multi-row
//! operation works on whole words, so its cost is the number of rows touched times
//! `num_qubits / 64`.

use crate::budget::{StabilizerResource, try_zeroed_words, words_for_bits};
use crate::{PauliSign, PauliString, StabilizerError, StabilizerResult};
use stab_bits::BitVec;

const WORD_BITS: usize = u64::BITS as usize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PauliRows {
    num_qubits: usize,
    half_words: usize,
    row_count: usize,
    words: Vec<u64>,
    signs: Vec<u64>,
}

impl PauliRows {
    /// Allocates `row_count` identity rows over `num_qubits` qubits with checked sizes and
    /// fallible reservation. Callers admit the bytes against a budget first when the request is
    /// not already bounded by storage they hold.
    pub(crate) fn try_zeroed(
        row_count: usize,
        num_qubits: usize,
        resource: StabilizerResource,
    ) -> StabilizerResult<Self> {
        let half_words = words_for_bits(num_qubits);
        let word_count = row_count
            .checked_mul(half_words)
            .and_then(|words| words.checked_mul(2))
            .ok_or(StabilizerError::ResourceSizeOverflow {
                resource,
                item_count: num_qubits,
                repetitions: row_count,
            })?;
        Ok(Self {
            num_qubits,
            half_words,
            row_count,
            words: try_zeroed_words(resource, word_count)?,
            signs: try_zeroed_words(resource, words_for_bits(row_count))?,
        })
    }

    /// Builds the two rows of a one-qubit Tableau from `(negative, x, z)` images of `X` and `Z`.
    pub(crate) fn one_qubit(x_image: (bool, bool, bool), z_image: (bool, bool, bool)) -> Self {
        let (x_negative, x_x, x_z) = x_image;
        let (z_negative, z_x, z_z) = z_image;
        Self {
            num_qubits: 1,
            half_words: 1,
            row_count: 2,
            words: vec![
                u64::from(x_x),
                u64::from(x_z),
                u64::from(z_x),
                u64::from(z_z),
            ],
            signs: vec![u64::from(x_negative) | (u64::from(z_negative) << 1)],
        }
    }

    pub(crate) const fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    pub(crate) const fn half_words(&self) -> usize {
        self.half_words
    }

    pub(crate) const fn row_count(&self) -> usize {
        self.row_count
    }

    pub(crate) const fn row_words(&self) -> usize {
        2 * self.half_words
    }

    fn range(&self, row: usize) -> StabilizerResult<std::ops::Range<usize>> {
        if row >= self.row_count {
            return Err(StabilizerError::TableauIndexOutOfRange {
                index: row,
                len: self.row_count,
            });
        }
        let start = row * self.row_words();
        Ok(start..start + self.row_words())
    }

    /// The packed `[x | z]` words of `row`.
    pub(crate) fn row(&self, row: usize) -> StabilizerResult<&[u64]> {
        let range = self.range(row)?;
        self.words
            .get(range)
            .ok_or(StabilizerError::TableauIndexOutOfRange {
                index: row,
                len: self.row_count,
            })
    }

    pub(crate) fn row_mut(&mut self, row: usize) -> StabilizerResult<&mut [u64]> {
        let range = self.range(row)?;
        let len = self.row_count;
        self.words
            .get_mut(range)
            .ok_or(StabilizerError::TableauIndexOutOfRange { index: row, len })
    }

    /// Borrows the packed words of rows `first..first + count`.
    pub(crate) fn rows(&self, first: usize, count: usize) -> Option<&[u64]> {
        let width = self.row_words();
        let end = first
            .checked_add(count)
            .filter(|end| *end <= self.row_count)?;
        self.words.get(first * width..end * width)
    }

    /// Borrows the packed words of rows `first..first + count` mutably.
    pub(crate) fn rows_mut(&mut self, first: usize, count: usize) -> StabilizerResult<&mut [u64]> {
        let width = self.row_words();
        let len = self.row_count;
        let end = first.checked_add(count).filter(|end| *end <= len).ok_or(
            StabilizerError::TableauIndexOutOfRange {
                index: first.saturating_add(count),
                len,
            },
        )?;
        self.words
            .get_mut(first * width..end * width)
            .ok_or(StabilizerError::TableauIndexOutOfRange { index: end, len })
    }

    /// Borrows two distinct rows mutably, in argument order.
    pub(crate) fn two_rows_mut(
        &mut self,
        first: usize,
        second: usize,
    ) -> StabilizerResult<(&mut [u64], &mut [u64])> {
        let first_range = self.range(first)?;
        let second_range = self.range(second)?;
        let width = self.row_words();
        if first == second {
            return Err(StabilizerError::TableauIndexOutOfRange {
                index: second,
                len: self.row_count,
            });
        }
        if first < second {
            let (head, tail) = self.words.split_at_mut(second_range.start);
            let first_words = head
                .get_mut(first_range)
                .ok_or_else(|| out_of_range(first))?;
            let second_words = tail.get_mut(..width).ok_or_else(|| out_of_range(second))?;
            Ok((first_words, second_words))
        } else {
            let (head, tail) = self.words.split_at_mut(first_range.start);
            let second_words = head
                .get_mut(second_range)
                .ok_or_else(|| out_of_range(second))?;
            let first_words = tail.get_mut(..width).ok_or_else(|| out_of_range(first))?;
            Ok((first_words, second_words))
        }
    }

    /// Borrows `target` mutably and `source` shared; the rows must differ.
    pub(crate) fn row_pair_mut(
        &mut self,
        target: usize,
        source: usize,
    ) -> StabilizerResult<(&mut [u64], &[u64])> {
        let (target_words, source_words) = self.two_rows_mut(target, source)?;
        Ok((target_words, source_words))
    }

    /// Swaps two rows and their signs.
    pub(crate) fn swap_rows(&mut self, left: usize, right: usize) -> StabilizerResult<()> {
        if left == right {
            self.range(left)?;
            return Ok(());
        }
        let (left_words, right_words) = self.two_rows_mut(left, right)?;
        left_words.swap_with_slice(right_words);
        let left_sign = self.sign(left)?;
        let right_sign = self.sign(right)?;
        self.set_sign(left, right_sign)?;
        self.set_sign(right, left_sign)
    }

    pub(crate) fn sign(&self, row: usize) -> StabilizerResult<bool> {
        let word = self
            .signs
            .get(row / WORD_BITS)
            .filter(|_| row < self.row_count)
            .ok_or_else(|| out_of_range(row))?;
        Ok(word >> (row % WORD_BITS) & 1 != 0)
    }

    pub(crate) fn set_sign(&mut self, row: usize, value: bool) -> StabilizerResult<()> {
        let len = self.row_count;
        let word = self
            .signs
            .get_mut(row / WORD_BITS)
            .filter(|_| row < len)
            .ok_or_else(|| out_of_range(row))?;
        let mask = 1_u64 << (row % WORD_BITS);
        if value {
            *word |= mask;
        } else {
            *word &= !mask;
        }
        Ok(())
    }

    pub(crate) fn flip_sign(&mut self, row: usize) -> StabilizerResult<()> {
        let value = self.sign(row)?;
        self.set_sign(row, !value)
    }

    /// The packed sign plane, one bit per row.
    pub(crate) fn sign_words(&self) -> &[u64] {
        &self.signs
    }

    /// Every packed row word in row order, then the packed sign plane.
    pub(crate) fn words_and_signs(&self) -> (&[u64], &[u64]) {
        (&self.words, &self.signs)
    }

    /// Mutably borrows every packed row word in row order and the packed sign plane.
    ///
    /// Callers must keep the bits past `num_qubits` in each half and past `row_count` in the
    /// sign plane zero.
    pub(crate) fn words_and_signs_mut(&mut self) -> (&mut [u64], &mut [u64]) {
        (&mut self.words, &mut self.signs)
    }

    /// Sets one qubit's Pauli bits in `row`.
    pub(crate) fn set_bits(
        &mut self,
        row: usize,
        qubit: usize,
        x: bool,
        z: bool,
    ) -> StabilizerResult<()> {
        if qubit >= self.num_qubits {
            return Err(StabilizerError::TableauIndexOutOfRange {
                index: qubit,
                len: self.num_qubits,
            });
        }
        let half_words = self.half_words;
        let words = self.row_mut(row)?;
        let (xs, zs) = words.split_at_mut(half_words);
        write_bit(xs, qubit, x)?;
        write_bit(zs, qubit, z)
    }

    /// Sets bit `index` of plane `plane` (0 for X, 1 for Z) in `row`.
    pub(crate) fn set_plane_bit(
        &mut self,
        row: usize,
        plane: usize,
        index: usize,
    ) -> StabilizerResult<()> {
        if index >= self.num_qubits || plane > 1 {
            return Err(StabilizerError::TableauIndexOutOfRange {
                index,
                len: self.num_qubits,
            });
        }
        let half_words = self.half_words;
        let words = self.row_mut(row)?;
        let half = words
            .get_mut(plane * half_words..(plane + 1) * half_words)
            .ok_or_else(|| out_of_range(row))?;
        write_bit(half, index, true)
    }

    /// Reads one qubit's `(x, z)` bits in `row`.
    pub(crate) fn bits(&self, row: usize, qubit: usize) -> StabilizerResult<(bool, bool)> {
        if qubit >= self.num_qubits {
            return Err(StabilizerError::TableauIndexOutOfRange {
                index: qubit,
                len: self.num_qubits,
            });
        }
        let (xs, zs) = self.row(row)?.split_at(self.half_words);
        Ok((read_bit(xs, qubit)?, read_bit(zs, qubit)?))
    }

    /// Copies `row` into an owned Pauli string.
    pub(crate) fn to_pauli(&self, row: usize) -> StabilizerResult<PauliString> {
        let (xs, zs) = self.row(row)?.split_at(self.half_words);
        let sign = if self.sign(row)? {
            PauliSign::Minus
        } else {
            PauliSign::Plus
        };
        PauliString::from_parts(
            sign,
            BitVec::from_words_truncated(self.num_qubits, try_copy(xs)?),
            BitVec::from_words_truncated(self.num_qubits, try_copy(zs)?),
        )
    }

    /// Overwrites `row` with `pauli`, which must have this storage's width.
    pub(crate) fn set_from_pauli(
        &mut self,
        row: usize,
        pauli: &PauliString,
    ) -> StabilizerResult<()> {
        if pauli.len() != self.num_qubits {
            return Err(StabilizerError::LengthMismatch {
                left: pauli.len(),
                right: self.num_qubits,
            });
        }
        let half_words = self.half_words;
        let words = self.row_mut(row)?;
        let (xs, zs) = words.split_at_mut(half_words);
        copy_words(xs, pauli.x_bits())?;
        copy_words(zs, pauli.z_bits())?;
        self.set_sign(row, pauli.sign().is_negative())
    }
}

/// Right-multiplies the packed row `source` into `target` and returns the base-`i` exponent of
/// the scalar the product needs on top of the two rows' real signs.
pub(crate) fn multiply_row_into(target: &mut [u64], source: &[u64], half_words: usize) -> u8 {
    let (target_x, target_z) = target.split_at_mut(half_words);
    let (source_x, source_z) = source.split_at(half_words);
    let product =
        crate::kernels::pauli_right_multiply_words(target_x, target_z, source_x, source_z);
    let mut log_i = popcount_mod_4(product.count_bit_1);
    log_i ^= popcount_mod_4(product.count_bit_2) << 1;
    log_i & 3
}

/// The parity of the symplectic inner product of two packed rows.
pub(crate) fn rows_anticommute(left: &[u64], right: &[u64], half_words: usize) -> bool {
    let (left_x, left_z) = left.split_at(half_words);
    let (right_x, right_z) = right.split_at(half_words);
    let mut parity = 0_u64;
    for (((lx, lz), rx), rz) in left_x.iter().zip(left_z).zip(right_x).zip(right_z) {
        parity ^= (lx & rz) ^ (lz & rx);
    }
    parity.count_ones() & 1 == 1
}

/// The number of `Y` terms of a packed row modulo four.
pub(crate) fn y_count_mod_4(row: &[u64], half_words: usize) -> u8 {
    let (xs, zs) = row.split_at(half_words);
    let count = xs.iter().zip(zs).fold(0_u32, |count, (x, z)| {
        count.wrapping_add((x & z).count_ones())
    });
    (count & 3) as u8
}

fn popcount_mod_4(word: u64) -> u8 {
    (word.count_ones() & 3) as u8
}

fn out_of_range(index: usize) -> StabilizerError {
    StabilizerError::TableauIndexOutOfRange { index, len: index }
}

fn read_bit(words: &[u64], index: usize) -> StabilizerResult<bool> {
    let word = words
        .get(index / WORD_BITS)
        .ok_or_else(|| out_of_range(index))?;
    Ok(word >> (index % WORD_BITS) & 1 != 0)
}

fn write_bit(words: &mut [u64], index: usize, value: bool) -> StabilizerResult<()> {
    let word = words
        .get_mut(index / WORD_BITS)
        .ok_or_else(|| out_of_range(index))?;
    let mask = 1_u64 << (index % WORD_BITS);
    if value {
        *word |= mask;
    } else {
        *word &= !mask;
    }
    Ok(())
}

fn copy_words(target: &mut [u64], source: &[u64]) -> StabilizerResult<()> {
    if target.len() != source.len() {
        return Err(StabilizerError::LengthMismatch {
            left: source.len(),
            right: target.len(),
        });
    }
    target.copy_from_slice(source);
    Ok(())
}

fn try_copy(words: &[u64]) -> StabilizerResult<Vec<u64>> {
    let mut copy = try_zeroed_words(StabilizerResource::PauliString, words.len())?;
    copy.copy_from_slice(words);
    Ok(copy)
}
