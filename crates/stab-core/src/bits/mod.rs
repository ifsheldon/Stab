mod scalar;
mod simd;

use std::fmt::{Display, Formatter};
use std::ops::Range;

use thiserror::Error;

pub const BIT_BLOCK_WORDS: usize = 4;
const WORD_BITS: usize = u64::BITS as usize;
const SPARSE_XOR_STACK_ITEMS: usize = 64;

pub type BitResult<T> = Result<T, BitError>;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BitError {
    #[error("bit length mismatch: left={left} right={right}")]
    LengthMismatch { left: usize, right: usize },

    #[error("bit index {index} is outside length {len}")]
    BitIndexOutOfRange { index: usize, len: usize },

    #[error("row index {row} is outside row count {rows}")]
    RowIndexOutOfRange { row: usize, rows: usize },

    #[error("matrix shape mismatch: left={left_rows}x{left_cols} right={right_rows}x{right_cols}")]
    MatrixShapeMismatch {
        left_rows: usize,
        left_cols: usize,
        right_rows: usize,
        right_cols: usize,
    },

    #[error("matrix operation requires a square matrix, got {rows}x{cols}")]
    NotSquareMatrix { rows: usize, cols: usize },

    #[error("matrix size {rows}x{cols} overflows addressable storage")]
    MatrixSizeOverflow { rows: usize, cols: usize },

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

    pub fn get(self) -> usize {
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
        prefix.iter().any(|word| *word != 0) || (*last & self.last_word_mask()) != 0
    }
}

impl From<usize> for BitLen {
    fn from(value: usize) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BitBlock {
    words: [u64; BIT_BLOCK_WORDS],
}

impl BitBlock {
    pub fn zero() -> Self {
        Self {
            words: [0; BIT_BLOCK_WORDS],
        }
    }

    pub fn from_words(words: [u64; BIT_BLOCK_WORDS]) -> Self {
        Self { words }
    }

    pub fn words(self) -> [u64; BIT_BLOCK_WORDS] {
        self.words
    }

    pub fn xor(self, rhs: Self) -> Self {
        Self::from_words(simd::xor_block(self.words, rhs.words))
    }

    pub fn and(self, rhs: Self) -> Self {
        Self::from_words(simd::and_block(self.words, rhs.words))
    }

    pub fn or(self, rhs: Self) -> Self {
        Self::from_words(simd::or_block(self.words, rhs.words))
    }

    pub fn popcount(self) -> usize {
        scalar::popcount_words(&self.words)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BitSlice<'a> {
    words: &'a [u64],
    bit_len: BitLen,
}

impl<'a> BitSlice<'a> {
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

    pub fn len(&self) -> usize {
        self.bit_len.get()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

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
        if let Some(last) = words.last_mut() {
            *last &= bit_len.last_word_mask();
        }
        Self { words, bit_len }
    }

    pub fn len(&self) -> usize {
        self.bit_len.get()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn word_count(&self) -> usize {
        self.words.len()
    }

    pub fn words(&self) -> &[u64] {
        &self.words
    }

    pub(crate) fn words_mut(&mut self) -> &mut [u64] {
        &mut self.words
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
        simd::xor_assign_words(&mut self.words, rhs.words());
        self.mask_unused_tail_bits();
        Ok(())
    }

    pub fn and_assign(&mut self, rhs: &BitSlice<'_>) -> BitResult<()> {
        ensure_same_bit_len(self.len(), rhs.len())?;
        simd::and_assign_words(&mut self.words, rhs.words());
        self.mask_unused_tail_bits();
        Ok(())
    }

    pub fn or_assign(&mut self, rhs: &BitSlice<'_>) -> BitResult<()> {
        ensure_same_bit_len(self.len(), rhs.len())?;
        simd::or_assign_words(&mut self.words, rhs.words());
        self.mask_unused_tail_bits();
        Ok(())
    }

    pub fn masked_xor_assign(&mut self, rhs: &BitSlice<'_>, mask: &BitSlice<'_>) -> BitResult<()> {
        ensure_same_bit_len(self.len(), rhs.len())?;
        ensure_same_bit_len(self.len(), mask.len())?;
        simd::masked_xor_assign_words(&mut self.words, rhs.words(), mask.words());
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
        if let Some(last) = self.words.last_mut() {
            *last &= self.bit_len.last_word_mask();
        }
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
        let cols = cols.into();
        let row_words = cols.word_count();
        let word_count = rows
            .checked_mul(row_words)
            .ok_or(BitError::MatrixSizeOverflow {
                rows,
                cols: cols.get(),
            })?;
        Ok(Self {
            rows,
            cols,
            row_words,
            words: vec![0; word_count],
        })
    }

    pub fn identity(size: usize) -> BitResult<Self> {
        let mut matrix = Self::zeros(size, size)?;
        for index in 0..size {
            matrix.set(index, index, true)?;
        }
        Ok(matrix)
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
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
            self.row_words_mut(target)?.fill(0);
            return Ok(());
        }
        let (source_words, target_words) = self.row_pair_words_mut(source, target)?;
        simd::xor_assign_words(target_words, source_words);
        Ok(())
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
            let source_words = self.row(source)?.words().to_vec();
            let target_words = self.row_words_mut(target)?;
            simd::masked_xor_assign_words(target_words, &source_words, mask.words());
        } else {
            let (source_words, target_words) = self.row_pair_words_mut(source, target)?;
            simd::masked_xor_assign_words(target_words, source_words, mask.words());
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
        for row in 0..self.rows {
            for col in 0..self.cols() {
                if self.get(row, col).unwrap_or(false) {
                    transposed.set(col, row, true)?;
                }
            }
        }
        Ok(transposed)
    }

    pub fn transpose_square_in_place(&mut self) -> BitResult<()> {
        if self.rows != self.cols() {
            return Err(BitError::NotSquareMatrix {
                rows: self.rows,
                cols: self.cols(),
            });
        }
        *self = self.transpose()?;
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

    fn row_words_mut(&mut self, row: usize) -> BitResult<&mut [u64]> {
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

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct SparseXorVec {
    items: Vec<u32>,
}

impl SparseXorVec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_sorted_items(items: Vec<u32>) -> Self {
        let mut out = Self { items };
        out.items = inplace_xor_sort(std::mem::take(&mut out.items));
        out
    }

    pub fn items(&self) -> &[u32] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn contains(&self, item: u32) -> bool {
        self.items.binary_search(&item).is_ok()
    }

    pub fn xor_item(&mut self, item: u32) {
        let Some(last) = self.items.last().copied() else {
            self.items.push(item);
            return;
        };
        match last.cmp(&item) {
            std::cmp::Ordering::Less => {
                self.items.push(item);
                return;
            }
            std::cmp::Ordering::Equal => {
                self.items.pop();
                return;
            }
            std::cmp::Ordering::Greater => {}
        }
        match self.items.first().copied() {
            Some(first) if first > item => {
                self.items.insert(0, item);
                return;
            }
            Some(first) if first == item => {
                self.items.remove(0);
                return;
            }
            _ => {}
        }
        self.xor_item_middle(item);
    }

    #[allow(
        clippy::indexing_slicing,
        reason = "middle scan indexes stay within the vector length checked by the while condition"
    )]
    fn xor_item_middle(&mut self, item: u32) {
        let mut index = 1usize;
        while index + 1 < self.items.len() {
            let existing = self.items[index];
            if existing < item {
                index += 1;
            } else if existing == item {
                self.items.remove(index);
                return;
            } else {
                self.items.insert(index, item);
                return;
            }
        }
        self.items.insert(index, item);
    }

    pub fn xor_assign(&mut self, rhs: &Self) {
        symmetric_difference_sorted_in_place(&mut self.items, &rhs.items);
    }

    pub fn is_superset_of(&self, rhs: &[u32]) -> bool {
        is_subset_of_sorted(rhs, &self.items)
    }
}

impl Display for SparseXorVec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("SparseXorVec{")?;
        for (index, item) in self.items.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{item}")?;
        }
        f.write_str("}")
    }
}

pub fn is_subset_of_sorted(needle: &[u32], haystack: &[u32]) -> bool {
    let mut haystack_iter = haystack.iter();
    for item in needle {
        let mut found = false;
        for candidate in haystack_iter.by_ref() {
            if candidate == item {
                found = true;
                break;
            }
            if candidate > item {
                return false;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

pub fn inplace_xor_sort(mut items: Vec<u32>) -> Vec<u32> {
    items.sort_unstable();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if out.last().is_some_and(|last| *last == item) {
            out.pop();
        } else {
            out.push(item);
        }
    }
    out
}

pub fn is_power_of_2(value: u64) -> bool {
    value.is_power_of_two()
}

pub fn floor_lg2(value: u64) -> Option<u32> {
    (value != 0).then(|| u64::BITS - 1 - value.leading_zeros())
}

pub fn first_set_bit(value: u64, start: u32) -> Option<u32> {
    if start >= u64::BITS {
        return None;
    }
    let shifted = value >> start;
    (shifted != 0).then(|| start + shifted.trailing_zeros())
}

#[allow(
    clippy::indexing_slicing,
    reason = "loop bounds guard sparse vector indexing while merging in place from the back"
)]
fn symmetric_difference_sorted_in_place(left: &mut Vec<u32>, right: &[u32]) {
    if right.is_empty() {
        return;
    }
    if left.is_empty() {
        left.extend_from_slice(right);
        return;
    }
    if symmetric_difference_sorted_with_stack(left, right) {
        return;
    }

    let left_len = left.len();
    let total_len = left_len + right.len();
    left.resize(total_len, 0);
    let mut left_index = left_len;
    let mut right_index = right.len();
    let mut write_index = total_len;

    while left_index > 0 && right_index > 0 {
        let left_item = left[left_index - 1];
        let right_item = right[right_index - 1];
        match left_item.cmp(&right_item) {
            std::cmp::Ordering::Less => {
                write_index -= 1;
                right_index -= 1;
                left[write_index] = right_item;
            }
            std::cmp::Ordering::Equal => {
                left_index -= 1;
                right_index -= 1;
            }
            std::cmp::Ordering::Greater => {
                write_index -= 1;
                left_index -= 1;
                left[write_index] = left_item;
            }
        }
    }

    while right_index > 0 {
        write_index -= 1;
        right_index -= 1;
        left[write_index] = right[right_index];
    }
    while left_index > 0 {
        write_index -= 1;
        left_index -= 1;
        left[write_index] = left[left_index];
    }

    let output_len = total_len - write_index;
    if write_index > 0 {
        left.copy_within(write_index..total_len, 0);
    }
    left.truncate(output_len);
}

#[allow(
    clippy::indexing_slicing,
    reason = "stack output bounds are guarded by the combined sparse-vector length check"
)]
fn symmetric_difference_sorted_with_stack(left: &mut Vec<u32>, right: &[u32]) -> bool {
    let total_len = left.len() + right.len();
    if total_len > SPARSE_XOR_STACK_ITEMS {
        return false;
    }
    let mut out = [0_u32; SPARSE_XOR_STACK_ITEMS];
    let output_len = symmetric_difference_sorted_into(left, right, &mut out);
    left.clear();
    left.extend_from_slice(&out[..output_len]);
    true
}

#[allow(
    clippy::indexing_slicing,
    reason = "merge indexes are bounded by input lengths and the pre-sized output buffer"
)]
fn symmetric_difference_sorted_into(left: &[u32], right: &[u32], out: &mut [u32]) -> usize {
    let mut left_index = 0usize;
    let mut right_index = 0usize;
    let mut write_index = 0usize;
    while left_index < left.len() {
        if right_index == right.len() || left[left_index] < right[right_index] {
            out[write_index] = left[left_index];
            write_index += 1;
            left_index += 1;
        } else if right[right_index] < left[left_index] {
            out[write_index] = right[right_index];
            write_index += 1;
            right_index += 1;
        } else {
            left_index += 1;
            right_index += 1;
        }
    }
    while right_index < right.len() {
        out[write_index] = right[right_index];
        write_index += 1;
        right_index += 1;
    }
    write_index
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
