//! Stable packed bit storage, checked views, scalar kernels, and layout primitives.

mod iter;
mod scalar;
mod transpose;

use std::num::NonZeroUsize;
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, DerefMut, Not, Range,
};

use thiserror::Error;

pub use iter::BitIter;

pub const BIT_BLOCK_WORDS: usize = 4;
const WORD_BITS: usize = u64::BITS as usize;

pub type BitResult<T> = Result<T, BitError>;

/// Transposes a 64 by 64 bit tile in place, so bit `j` of word `i` becomes bit `i` of word `j`.
///
/// This is the block kernel behind [`BitMatrix::transpose`], for callers that store packed rows
/// in their own layout and move one aligned 64 by 64 block at a time.
pub fn transpose_bit_tile(tile: &mut [u64; 64]) {
    transpose::transpose_tile(tile);
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BitError {
    #[error("bit length mismatch: left={left} right={right}")]
    LengthMismatch { left: usize, right: usize },

    #[error("packed byte length mismatch: expected {expected}, got {actual}")]
    ByteLengthMismatch { actual: usize, expected: usize },

    #[error("bit index {index} is outside length {len}")]
    BitIndexOutOfRange { index: usize, len: usize },

    #[error("row index {row} is outside row count {rows}")]
    RowIndexOutOfRange { row: usize, rows: usize },

    #[error("matrix operation requires a square matrix, got {rows}x{cols}")]
    NotSquareMatrix { rows: usize, cols: usize },

    #[error("matrix size {rows}x{cols} overflows addressable storage")]
    MatrixSizeOverflow { rows: usize, cols: usize },

    #[error("word-row transpose supports at most {maximum} rows, got {actual}")]
    WordRowTransposeTooWide { actual: usize, maximum: usize },

    #[error("matrix-to-word-row transpose supports at most {maximum} rows, got {actual}")]
    MatrixToWordRowsTooTall { actual: usize, maximum: usize },

    #[error("matrix row prefix {requested} exceeds the available {available} rows")]
    MatrixRowPrefixOutOfRange { requested: usize, available: usize },

    #[error("could not allocate packed storage for {words} words")]
    StorageAllocationFailed { words: usize },

    #[error("bit range {start}..{end} is outside length {len}")]
    BitRangeOutOfRange {
        start: usize,
        end: usize,
        len: usize,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct BitLen(usize);

impl BitLen {
    pub fn new(bits: usize) -> Self {
        Self(bits)
    }

    pub const fn get(self) -> usize {
        self.0
    }

    pub fn word_count(self) -> usize {
        self.0.div_ceil(WORD_BITS)
    }

    fn last_word_mask(self) -> u64 {
        let tail = self.0 % WORD_BITS;
        if tail == 0 {
            u64::MAX
        } else {
            (1_u64 << tail) - 1
        }
    }

    fn popcount_words(self, words: &[u64]) -> usize {
        let Some((last, prefix)) = words.split_last() else {
            return 0;
        };
        scalar::popcount_words(prefix) + scalar::popcount_word(*last & self.last_word_mask())
    }

    fn not_zero_words(self, words: &[u64]) -> bool {
        let Some((last, prefix)) = words.split_last() else {
            return false;
        };
        scalar::not_zero_words(prefix) || (*last & self.last_word_mask()) != 0
    }
}

impl From<usize> for BitLen {
    fn from(value: usize) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BitBlock {
    words: [u64; BIT_BLOCK_WORDS],
}

impl BitBlock {
    pub const ZERO: Self = Self {
        words: [0; BIT_BLOCK_WORDS],
    };

    pub const ONES: Self = Self {
        words: [u64::MAX; BIT_BLOCK_WORDS],
    };

    pub const fn zero() -> Self {
        Self::ZERO
    }

    pub const fn from_words(words: [u64; BIT_BLOCK_WORDS]) -> Self {
        Self { words }
    }

    pub const fn words(self) -> [u64; BIT_BLOCK_WORDS] {
        self.words
    }

    /// Borrows the block's words, lowest lanes first, without copying them.
    pub const fn as_words(&self) -> &[u64; BIT_BLOCK_WORDS] {
        &self.words
    }

    pub fn xor(self, rhs: Self) -> Self {
        Self::from_words(scalar::xor_block(self.words, rhs.words))
    }

    pub fn and(self, rhs: Self) -> Self {
        Self::from_words(scalar::and_block(self.words, rhs.words))
    }

    pub fn or(self, rhs: Self) -> Self {
        Self::from_words(scalar::or_block(self.words, rhs.words))
    }

    pub fn popcount(self) -> usize {
        scalar::popcount_words(&self.words)
    }
}

impl BitXor for BitBlock {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        self.xor(rhs)
    }
}

impl BitXorAssign for BitBlock {
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = self.xor(rhs);
    }
}

impl BitAnd for BitBlock {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.and(rhs)
    }
}

impl BitAndAssign for BitBlock {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = self.and(rhs);
    }
}

impl BitOr for BitBlock {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.or(rhs)
    }
}

impl BitOrAssign for BitBlock {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.or(rhs);
    }
}

impl Not for BitBlock {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::from_words(self.words.map(Not::not))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BitSlice<'a> {
    words: &'a [u64],
    bit_len: BitLen,
}

impl<'a> BitSlice<'a> {
    #[inline]
    pub fn new(words: &'a [u64], bit_len: impl Into<BitLen>) -> BitResult<Self> {
        let bit_len = bit_len.into();
        if words.len() != bit_len.word_count() {
            return Err(BitError::LengthMismatch {
                left: words.len() * WORD_BITS,
                right: bit_len.get(),
            });
        }
        Ok(Self { words, bit_len })
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.bit_len.get()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the borrowed storage words.
    ///
    /// Bits at or beyond [`Self::len`] in the final word are outside the logical slice and may be
    /// nonzero. Use [`Self::get`] or [`Self::popcount`] when inactive tail bits must be ignored.
    #[inline]
    pub fn words(&self) -> &'a [u64] {
        self.words
    }

    pub fn get(&self, index: usize) -> Option<bool> {
        get_bit(self.words, self.bit_len, index)
    }

    pub fn popcount(&self) -> usize {
        self.bit_len.popcount_words(self.words)
    }
}

#[derive(Debug)]
pub struct BitWordsMut<'a> {
    words: &'a mut [u64],
    bit_len: BitLen,
}

impl Deref for BitWordsMut<'_> {
    type Target = [u64];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.words
    }
}

impl DerefMut for BitWordsMut<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.words
    }
}

impl Drop for BitWordsMut<'_> {
    #[inline]
    fn drop(&mut self) {
        mask_unused_tail_bits(self.words, self.bit_len);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitVec {
    words: Vec<u64>,
    bit_len: BitLen,
}

impl BitVec {
    pub fn zeros(bit_len: impl Into<BitLen>) -> Self {
        let bit_len = bit_len.into();
        Self {
            words: vec![0; bit_len.word_count()],
            bit_len,
        }
    }

    pub fn from_words_truncated(bit_len: impl Into<BitLen>, mut words: Vec<u64>) -> Self {
        let bit_len = bit_len.into();
        words.resize(bit_len.word_count(), 0);
        mask_unused_tail_bits(&mut words, bit_len);
        Self { words, bit_len }
    }

    pub fn from_bits(bits: impl IntoIterator<Item = bool>) -> Self {
        let bits = bits.into_iter().collect::<Vec<_>>();
        let bit_len = BitLen::new(bits.len());
        let words = bits
            .chunks(WORD_BITS)
            .map(|chunk| {
                chunk.iter().enumerate().fold(0_u64, |word, (index, bit)| {
                    word | (u64::from(*bit) << index)
                })
            })
            .collect();
        Self { words, bit_len }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.bit_len.get()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn word_count(&self) -> usize {
        self.words.len()
    }

    #[inline]
    pub fn words(&self) -> &[u64] {
        &self.words
    }

    #[inline]
    pub fn words_mut(&mut self) -> BitWordsMut<'_> {
        BitWordsMut {
            words: &mut self.words,
            bit_len: self.bit_len,
        }
    }

    pub fn resize_zeros(&mut self, bit_len: impl Into<BitLen>) {
        let bit_len = bit_len.into();
        self.words.resize(bit_len.word_count(), 0);
        self.bit_len = bit_len;
        self.mask_unused_tail_bits();
    }

    pub fn as_bitslice(&self) -> BitSlice<'_> {
        BitSlice {
            words: &self.words,
            bit_len: self.bit_len,
        }
    }

    pub fn get(&self, index: usize) -> Option<bool> {
        get_bit(&self.words, self.bit_len, index)
    }

    pub fn set(&mut self, index: usize, value: bool) -> BitResult<()> {
        set_bit(&mut self.words, self.bit_len, index, value)
    }

    pub fn clear(&mut self) {
        self.words.fill(0);
    }

    pub fn xor_assign(&mut self, rhs: &BitSlice<'_>) -> BitResult<()> {
        ensure_same_bit_len(self.len(), rhs.len())?;
        xor_assign_word_slices(&mut self.words, rhs.words())?;
        self.mask_unused_tail_bits();
        Ok(())
    }

    pub fn and_assign(&mut self, rhs: &BitSlice<'_>) -> BitResult<()> {
        ensure_same_bit_len(self.len(), rhs.len())?;
        scalar::and_assign_words(&mut self.words, rhs.words());
        self.mask_unused_tail_bits();
        Ok(())
    }

    pub fn or_assign(&mut self, rhs: &BitSlice<'_>) -> BitResult<()> {
        ensure_same_bit_len(self.len(), rhs.len())?;
        scalar::or_assign_words(&mut self.words, rhs.words());
        self.mask_unused_tail_bits();
        Ok(())
    }

    pub fn masked_xor_assign(&mut self, rhs: &BitSlice<'_>, mask: &BitSlice<'_>) -> BitResult<()> {
        ensure_same_bit_len(self.len(), rhs.len())?;
        ensure_same_bit_len(self.len(), mask.len())?;
        scalar::masked_xor_assign_words(&mut self.words, rhs.words(), mask.words());
        self.mask_unused_tail_bits();
        Ok(())
    }

    pub fn copy_from_bitslice(&mut self, rhs: &BitSlice<'_>) -> BitResult<()> {
        ensure_same_bit_len(self.len(), rhs.len())?;
        self.words.copy_from_slice(rhs.words());
        self.mask_unused_tail_bits();
        Ok(())
    }

    pub fn xor_range_from(
        &mut self,
        target_start: usize,
        rhs: &BitSlice<'_>,
        source_start: usize,
        bit_count: usize,
    ) -> BitResult<()> {
        checked_range(target_start, bit_count, self.len())?;
        checked_range(source_start, bit_count, rhs.len())?;
        let mut remaining = bit_count;
        let mut target_index = target_start;
        let mut source_index = source_start;
        let target_len = self.len();
        while remaining > 0 {
            let target_word_index = target_index / WORD_BITS;
            let target_bit_offset = target_index % WORD_BITS;
            let chunk_bits = remaining.min(WORD_BITS - target_bit_offset);
            let source_word = read_word_range_low(rhs.words(), source_index, chunk_bits);
            let chunk_mask = low_bits_mask(chunk_bits) << target_bit_offset;
            let source_aligned = (source_word << target_bit_offset) & chunk_mask;
            let target_word =
                self.words
                    .get_mut(target_word_index)
                    .ok_or(BitError::BitIndexOutOfRange {
                        index: target_index,
                        len: target_len,
                    })?;
            *target_word ^= source_aligned;
            remaining -= chunk_bits;
            target_index += chunk_bits;
            source_index += chunk_bits;
        }
        self.mask_unused_tail_bits();
        Ok(())
    }

    pub fn popcount(&self) -> usize {
        self.bit_len.popcount_words(&self.words)
    }

    pub fn not_zero(&self) -> bool {
        self.bit_len.not_zero_words(&self.words)
    }

    fn mask_unused_tail_bits(&mut self) {
        mask_unused_tail_bits(&mut self.words, self.bit_len);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitMatrix {
    rows: usize,
    cols: BitLen,
    row_words: usize,
    words: Vec<u64>,
}

impl BitMatrix {
    pub fn zeros(rows: usize, cols: impl Into<BitLen>) -> BitResult<Self> {
        Self::zeros_chunked(rows, cols, NonZeroUsize::MAX, |_| Ok::<(), BitError>(()))
    }

    /// Builds the same matrix as [`Self::zeros`], zeroing its storage in chunks of at most
    /// `chunk_words` words and calling `before_chunk` with each chunk's word count before that
    /// chunk is written.
    ///
    /// The whole storage is still reserved fallibly, once, before the first chunk, so a caller can
    /// interleave bounded work, such as polling a cancellation signal, with the zeroing of a large
    /// matrix without reallocating. The first error `before_chunk` returns stops the zeroing and
    /// is returned after the partial storage is released; a matrix with no words never calls it.
    pub fn zeros_chunked<E: From<BitError>>(
        rows: usize,
        cols: impl Into<BitLen>,
        chunk_words: NonZeroUsize,
        mut before_chunk: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        let cols = cols.into();
        let row_words = cols.word_count();
        let word_count = rows
            .checked_mul(row_words)
            .ok_or(BitError::MatrixSizeOverflow {
                rows,
                cols: cols.get(),
            })?;
        let mut words = Vec::new();
        words
            .try_reserve_exact(word_count)
            .map_err(|_| BitError::StorageAllocationFailed { words: word_count })?;
        while words.len() < word_count {
            let chunk = chunk_words.get().min(word_count - words.len());
            before_chunk(chunk)?;
            // Within the reserved capacity, so this never reallocates.
            words.resize(words.len() + chunk, 0);
        }
        Ok(Self {
            rows,
            cols,
            row_words,
            words,
        })
    }

    pub fn identity(size: usize) -> BitResult<Self> {
        let mut matrix = Self::zeros(size, size)?;
        for index in 0..size {
            matrix.set(index, index, true)?;
        }
        Ok(matrix)
    }

    pub const fn rows(&self) -> usize {
        self.rows
    }

    pub const fn cols(&self) -> usize {
        self.cols.get()
    }

    pub fn get(&self, row: usize, col: usize) -> Option<bool> {
        if row >= self.rows {
            return None;
        }
        let range = self.row_range(row)?;
        get_bit(self.words.get(range)?, self.cols, col)
    }

    pub fn set(&mut self, row: usize, col: usize, value: bool) -> BitResult<()> {
        self.ensure_row(row)?;
        let range = self.row_range(row).ok_or(BitError::RowIndexOutOfRange {
            row,
            rows: self.rows,
        })?;
        let row_words = self
            .words
            .get_mut(range)
            .ok_or(BitError::RowIndexOutOfRange {
                row,
                rows: self.rows,
            })?;
        set_bit(row_words, self.cols, col, value)
    }

    pub fn copy_row_from_bools(&mut self, row: usize, bits: &[bool]) -> BitResult<()> {
        if bits.len() != self.cols() {
            return Err(BitError::LengthMismatch {
                left: bits.len(),
                right: self.cols(),
            });
        }
        let row_words = self.raw_row_words_mut(row)?;
        row_words.fill(0);
        for (index, bit) in bits.iter().copied().enumerate() {
            if bit {
                let word =
                    row_words
                        .get_mut(index / WORD_BITS)
                        .ok_or(BitError::BitIndexOutOfRange {
                            index,
                            len: bits.len(),
                        })?;
                *word |= 1_u64 << (index % WORD_BITS);
            }
        }
        Ok(())
    }

    /// Replaces one row from little-endian packed bytes.
    pub fn copy_row_from_le_bytes(&mut self, row: usize, bytes: &[u8]) -> BitResult<()> {
        let expected = self.cols().div_ceil(u8::BITS as usize);
        if bytes.len() != expected {
            return Err(BitError::ByteLengthMismatch {
                actual: bytes.len(),
                expected,
            });
        }
        let bit_len = self.cols.get();
        let tail_mask = self.cols.last_word_mask();
        let row_words = self.raw_row_words_mut(row)?;
        row_words.fill(0);
        let (chunks, remainder) = bytes.as_chunks::<{ size_of::<u64>() }>();
        let mut words = row_words.iter_mut();
        for chunk in chunks {
            let word = words.next().ok_or(BitError::BitIndexOutOfRange {
                index: bytes.len().saturating_mul(u8::BITS as usize),
                len: bit_len,
            })?;
            *word = u64::from_le_bytes(*chunk);
        }
        if !remainder.is_empty() {
            let word = words.next().ok_or(BitError::BitIndexOutOfRange {
                index: bytes.len().saturating_mul(u8::BITS as usize),
                len: bit_len,
            })?;
            let mut packed = [0_u8; size_of::<u64>()];
            for (target, source) in packed.iter_mut().zip(remainder) {
                *target = *source;
            }
            *word = u64::from_le_bytes(packed);
        }
        if let Some(last) = row_words.last_mut() {
            *last &= tail_mask;
        }
        Ok(())
    }

    /// Borrows one row's storage words for direct word-level writes.
    ///
    /// Bits written beyond [`Self::cols`] in the final word are cleared when the guard drops, so
    /// the row keeps the matrix's zero-tail invariant.
    pub fn row_words_mut(&mut self, row: usize) -> BitResult<BitWordsMut<'_>> {
        let bit_len = self.cols;
        Ok(BitWordsMut {
            words: self.raw_row_words_mut(row)?,
            bit_len,
        })
    }

    pub fn row(&self, row: usize) -> BitResult<BitSlice<'_>> {
        self.ensure_row(row)?;
        let range = self.row_range(row).ok_or(BitError::RowIndexOutOfRange {
            row,
            rows: self.rows,
        })?;
        let words = self.words.get(range).ok_or(BitError::RowIndexOutOfRange {
            row,
            rows: self.rows,
        })?;
        Ok(BitSlice {
            words,
            bit_len: self.cols,
        })
    }

    pub fn xor_row_into(&mut self, source: usize, target: usize) -> BitResult<()> {
        self.ensure_row(source)?;
        self.ensure_row(target)?;
        if source == target {
            self.raw_row_words_mut(target)?.fill(0);
            return Ok(());
        }
        let (source_words, target_words) = self.row_pair_words_mut(source, target)?;
        xor_assign_word_slices(target_words, source_words)
    }

    pub fn masked_xor_row_into(
        &mut self,
        source: usize,
        target: usize,
        mask: &BitSlice<'_>,
    ) -> BitResult<()> {
        self.ensure_row(source)?;
        self.ensure_row(target)?;
        ensure_same_bit_len(self.cols(), mask.len())?;
        if source == target {
            let target_words = self.raw_row_words_mut(target)?;
            scalar::and_not_assign_words(target_words, mask.words());
        } else {
            let (source_words, target_words) = self.row_pair_words_mut(source, target)?;
            scalar::masked_xor_assign_words(target_words, source_words, mask.words());
        }
        Ok(())
    }

    pub fn swap_rows(&mut self, left: usize, right: usize) -> BitResult<()> {
        self.ensure_row(left)?;
        self.ensure_row(right)?;
        if left == right {
            return Ok(());
        }
        let (left_words, right_words) = self.row_pair_words_mut(left, right)?;
        left_words.swap_with_slice(right_words);
        Ok(())
    }

    pub fn transpose(&self) -> BitResult<Self> {
        let mut transposed = Self::zeros(self.cols(), self.rows)?;
        transpose::out_of_place(self, &mut transposed);
        Ok(transposed)
    }

    /// Replaces this matrix with the transpose of one packed word per source row.
    ///
    /// `source_rows.len()` must equal this matrix's column count and this matrix may contain at
    /// most 64 rows. Bits beyond the target row count are ignored.
    pub fn copy_transpose_from_word_rows(&mut self, source_rows: &[u64]) -> BitResult<()> {
        if source_rows.len() != self.cols() {
            return Err(BitError::LengthMismatch {
                left: source_rows.len(),
                right: self.cols(),
            });
        }
        if self.rows > WORD_BITS {
            return Err(BitError::WordRowTransposeTooWide {
                actual: self.rows,
                maximum: WORD_BITS,
            });
        }
        transpose::word_rows_into(source_rows, self, 0);
        Ok(())
    }

    /// Replaces the rows of one 64-row block with the transpose of one packed word per column.
    ///
    /// Bit `i` of `source_rows[c]` becomes column `c` of row `row_start + i`. The block covers
    /// rows `row_start..row_start + 64` clipped to this matrix, and source bits beyond that
    /// clipped block are ignored. `row_start` must name an existing row, and
    /// `source_rows.len()` must equal this matrix's column count.
    pub fn copy_transpose_from_word_rows_at(
        &mut self,
        row_start: usize,
        source_rows: &[u64],
    ) -> BitResult<()> {
        if source_rows.len() != self.cols() {
            return Err(BitError::LengthMismatch {
                left: source_rows.len(),
                right: self.cols(),
            });
        }
        self.ensure_row(row_start)?;
        transpose::word_rows_into(source_rows, self, row_start);
        Ok(())
    }

    /// Copies the transpose of a row prefix into one packed word per matrix column.
    ///
    /// At most 64 source rows may be selected. Bits beyond `row_count` in each target word are
    /// cleared, so callers can safely reuse the target storage for shorter batches.
    pub fn copy_transpose_prefix_into_word_rows(
        &self,
        row_count: usize,
        target_rows: &mut [u64],
    ) -> BitResult<()> {
        if target_rows.len() != self.cols() {
            return Err(BitError::LengthMismatch {
                left: target_rows.len(),
                right: self.cols(),
            });
        }
        if row_count > self.rows {
            return Err(BitError::MatrixRowPrefixOutOfRange {
                requested: row_count,
                available: self.rows,
            });
        }
        if row_count > WORD_BITS {
            return Err(BitError::MatrixToWordRowsTooTall {
                actual: row_count,
                maximum: WORD_BITS,
            });
        }
        transpose::matrix_rows_into_word_rows(self, 0, row_count, target_rows);
        Ok(())
    }

    /// Copies the transpose of `row_count` rows starting at `row_start` into one packed word per
    /// matrix column, so bit `i` of `target_rows[c]` is column `c` of row `row_start + i`.
    ///
    /// At most 64 rows may be selected. Bits beyond `row_count` in each target word are cleared.
    pub fn copy_transpose_rows_into_word_rows(
        &self,
        row_start: usize,
        row_count: usize,
        target_rows: &mut [u64],
    ) -> BitResult<()> {
        if target_rows.len() != self.cols() {
            return Err(BitError::LengthMismatch {
                left: target_rows.len(),
                right: self.cols(),
            });
        }
        if row_count > WORD_BITS {
            return Err(BitError::MatrixToWordRowsTooTall {
                actual: row_count,
                maximum: WORD_BITS,
            });
        }
        if row_start
            .checked_add(row_count)
            .is_none_or(|end| end > self.rows)
        {
            return Err(BitError::MatrixRowPrefixOutOfRange {
                requested: row_start.saturating_add(row_count),
                available: self.rows,
            });
        }
        transpose::matrix_rows_into_word_rows(self, row_start, row_count, target_rows);
        Ok(())
    }

    pub fn transpose_square_in_place(&mut self) -> BitResult<()> {
        if self.rows != self.cols() {
            return Err(BitError::NotSquareMatrix {
                rows: self.rows,
                cols: self.cols(),
            });
        }
        transpose::square_in_place(self);
        Ok(())
    }

    fn ensure_row(&self, row: usize) -> BitResult<()> {
        if row >= self.rows {
            return Err(BitError::RowIndexOutOfRange {
                row,
                rows: self.rows,
            });
        }
        Ok(())
    }

    fn row_range(&self, row: usize) -> Option<Range<usize>> {
        let start = row.checked_mul(self.row_words)?;
        let end = start.checked_add(self.row_words)?;
        Some(start..end)
    }

    fn raw_row_words_mut(&mut self, row: usize) -> BitResult<&mut [u64]> {
        self.ensure_row(row)?;
        let range = self.row_range(row).ok_or(BitError::RowIndexOutOfRange {
            row,
            rows: self.rows,
        })?;
        self.words
            .get_mut(range)
            .ok_or(BitError::RowIndexOutOfRange {
                row,
                rows: self.rows,
            })
    }

    fn row_pair_words_mut(
        &mut self,
        first: usize,
        second: usize,
    ) -> BitResult<(&mut [u64], &mut [u64])> {
        self.ensure_row(first)?;
        self.ensure_row(second)?;
        let first_range = self.row_range(first).ok_or(BitError::RowIndexOutOfRange {
            row: first,
            rows: self.rows,
        })?;
        let second_range = self.row_range(second).ok_or(BitError::RowIndexOutOfRange {
            row: second,
            rows: self.rows,
        })?;
        if first < second {
            let (before_second, from_second) = self.words.split_at_mut(second_range.start);
            let first_words =
                before_second
                    .get_mut(first_range)
                    .ok_or(BitError::RowIndexOutOfRange {
                        row: first,
                        rows: self.rows,
                    })?;
            let second_words =
                from_second
                    .get_mut(..self.row_words)
                    .ok_or(BitError::RowIndexOutOfRange {
                        row: second,
                        rows: self.rows,
                    })?;
            Ok((first_words, second_words))
        } else {
            let (before_first, from_first) = self.words.split_at_mut(first_range.start);
            let first_words =
                from_first
                    .get_mut(..self.row_words)
                    .ok_or(BitError::RowIndexOutOfRange {
                        row: first,
                        rows: self.rows,
                    })?;
            let second_words =
                before_first
                    .get_mut(second_range)
                    .ok_or(BitError::RowIndexOutOfRange {
                        row: second,
                        rows: self.rows,
                    })?;
            Ok((first_words, second_words))
        }
    }
}

fn get_bit(words: &[u64], bit_len: BitLen, index: usize) -> Option<bool> {
    if index >= bit_len.get() {
        return None;
    }
    let word = words.get(index / WORD_BITS)?;
    Some((word & bit_mask(index)) != 0)
}

fn set_bit(words: &mut [u64], bit_len: BitLen, index: usize, value: bool) -> BitResult<()> {
    if index >= bit_len.get() {
        return Err(BitError::BitIndexOutOfRange {
            index,
            len: bit_len.get(),
        });
    }
    let word = words
        .get_mut(index / WORD_BITS)
        .ok_or(BitError::BitIndexOutOfRange {
            index,
            len: bit_len.get(),
        })?;
    let mask = bit_mask(index);
    if value {
        *word |= mask;
    } else {
        *word &= !mask;
    }
    Ok(())
}

fn bit_mask(index: usize) -> u64 {
    1_u64 << (index % WORD_BITS)
}

fn checked_range(start: usize, bit_count: usize, len: usize) -> BitResult<()> {
    let end = start
        .checked_add(bit_count)
        .ok_or(BitError::BitRangeOutOfRange {
            start,
            end: usize::MAX,
            len,
        })?;
    if end > len {
        return Err(BitError::BitRangeOutOfRange { start, end, len });
    }
    Ok(())
}

/// XORs equal-length word slices through the slice-level SIMD kernel.
///
/// Callers have already checked equal bit lengths, so a word-length disagreement here is an
/// internal invariant violation; it is reported as the mismatched bit capacities.
fn xor_assign_word_slices(lhs: &mut [u64], rhs: &[u64]) -> BitResult<()> {
    stab_kernels_simd::xor_assign_words(lhs, rhs).map_err(|mismatch| BitError::LengthMismatch {
        left: mismatch.expected.saturating_mul(WORD_BITS),
        right: mismatch.actual.saturating_mul(WORD_BITS),
    })
}

fn ensure_same_bit_len(left: usize, right: usize) -> BitResult<()> {
    if left != right {
        return Err(BitError::LengthMismatch { left, right });
    }
    Ok(())
}

fn read_word_range_low(words: &[u64], bit_start: usize, bit_count: usize) -> u64 {
    debug_assert!(bit_count <= WORD_BITS);
    if bit_count == 0 {
        return 0;
    }
    let word_index = bit_start / WORD_BITS;
    let bit_offset = bit_start % WORD_BITS;
    let low = words.get(word_index).copied().unwrap_or(0) >> bit_offset;
    let value = if bit_offset == 0 || bit_offset + bit_count <= WORD_BITS {
        low
    } else {
        let high = words.get(word_index + 1).copied().unwrap_or(0) << (WORD_BITS - bit_offset);
        low | high
    };
    value & low_bits_mask(bit_count)
}

fn low_bits_mask(bit_count: usize) -> u64 {
    if bit_count >= WORD_BITS {
        u64::MAX
    } else {
        (1_u64 << bit_count) - 1
    }
}

#[inline]
fn mask_unused_tail_bits(words: &mut [u64], bit_len: BitLen) {
    let tail = bit_len.get() % WORD_BITS;
    if tail != 0
        && let Some(last) = words.last_mut()
    {
        *last &= (1_u64 << tail) - 1;
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::cast_possible_truncation,
        clippy::unwrap_used,
        reason = "bounded differential fixtures rotate by small word indexes"
    )]

    use super::*;

    /// The chunked constructor builds exactly the matrix `zeros` builds, in chunks that cover
    /// every word once and never exceed the requested size.
    #[test]
    fn chunked_zeros_equal_zeros_in_bounded_chunks() {
        for (rows, cols, chunk) in [(0, 100, 3), (5, 0, 1), (1, 1, 1), (7, 130, 4), (3, 64, 100)] {
            let chunk = NonZeroUsize::new(chunk).unwrap();
            let mut chunks = Vec::new();
            let chunked = BitMatrix::zeros_chunked(rows, cols, chunk, |words| {
                chunks.push(words);
                Ok::<(), BitError>(())
            })
            .unwrap();
            let zeros = BitMatrix::zeros(rows, cols).unwrap();
            assert_eq!(chunked, zeros, "{rows}x{cols}");
            assert_eq!(
                chunks.iter().sum::<usize>(),
                zeros.words.len(),
                "{rows}x{cols}"
            );
            assert!(chunks.iter().all(|words| (1..=chunk.get()).contains(words)));
        }
    }

    /// The first failing callback stops the zeroing and its error is returned unchanged, while
    /// shape errors are still reported before any callback.
    #[test]
    fn chunked_zeros_stop_at_the_first_failing_chunk() {
        #[derive(Debug, PartialEq)]
        enum Stop {
            Bits(BitError),
            Callback(usize),
        }
        impl From<BitError> for Stop {
            fn from(error: BitError) -> Self {
                Self::Bits(error)
            }
        }
        let mut calls = 0;
        let result = BitMatrix::zeros_chunked(4, 640, NonZeroUsize::new(7).unwrap(), |_| {
            calls += 1;
            if calls == 3 {
                Err(Stop::Callback(calls))
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err(Stop::Callback(3)));
        assert_eq!(calls, 3);

        let mut called = false;
        let overflow = BitMatrix::zeros_chunked(usize::MAX, 128, NonZeroUsize::MIN, |_| {
            called = true;
            Ok::<(), Stop>(())
        });
        assert_eq!(
            overflow,
            Err(Stop::Bits(BitError::MatrixSizeOverflow {
                rows: usize::MAX,
                cols: 128
            }))
        );
        assert!(!called);
    }

    #[test]
    fn simd_xor_matches_scalar_across_blocks_and_tails() {
        for word_count in 0..=2 * BIT_BLOCK_WORDS + 1 {
            let left = (0..word_count)
                .map(|index| 0x5555_aaaa_1234_5678_u64.rotate_left(index as u32))
                .collect::<Vec<_>>();
            let right = (0..word_count)
                .map(|index| 0xf0f0_0f0f_9876_5432_u64.rotate_right(index as u32))
                .collect::<Vec<_>>();

            let mut simd = left.clone();
            let mut reference = left;
            xor_assign_word_slices(&mut simd, &right).unwrap();
            scalar::xor_assign_words(&mut reference, &right);
            assert_eq!(simd, reference);
        }
    }
}
