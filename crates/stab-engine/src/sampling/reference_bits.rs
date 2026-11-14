//! The packed record a reference run writes: one bit per measurement, not one byte.

use stab_model::MeasureRecordOffset;

use super::api::SamplingExecutionError;

const WORD_BITS: usize = u64::BITS as usize;

/// A growable little-endian bit record whose bits past [`Self::len`] are always zero.
///
/// The reference simulator appends to it one measurement at a time and the repeat folder copies
/// a recurring period of it word by word, so a reference sample costs `ceil(n / 64)` words of
/// storage for `n` measurements.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ReferenceBits {
    words: Vec<u64>,
    len: usize,
}

impl ReferenceBits {
    /// An empty record with room for `capacity` bits, reserved fallibly.
    pub(crate) fn try_with_capacity(
        capacity: usize,
        label: &'static str,
    ) -> Result<Self, SamplingExecutionError> {
        let mut words = Vec::new();
        let word_count = capacity.div_ceil(WORD_BITS);
        words.try_reserve_exact(word_count).map_err(|error| {
            SamplingExecutionError::SessionStorageAllocation {
                message: format!("{label} capacity {capacity} bits: {error}"),
            }
        })?;
        Ok(Self { words, len: 0 })
    }

    /// Bytes `ReferenceBits` reserves for a record of `bits` bits.
    pub(crate) const fn storage_bytes(bits: usize) -> u128 {
        (bits.div_ceil(WORD_BITS) as u128) * (size_of::<u64>() as u128)
    }

    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    /// The record's words; bits at and past [`Self::len`] are zero.
    pub(crate) fn words(&self) -> &[u64] {
        &self.words
    }

    pub(crate) fn clear(&mut self) {
        self.words.clear();
        self.len = 0;
    }

    pub(crate) fn get(&self, index: usize) -> Option<bool> {
        if index >= self.len {
            return None;
        }
        self.words
            .get(index / WORD_BITS)
            .map(|word| (word >> (index % WORD_BITS)) & 1 == 1)
    }

    /// The bit `offset` steps back from the end, or false before the record's start.
    pub(crate) fn lookback(&self, offset: MeasureRecordOffset) -> bool {
        i64::try_from(self.len)
            .ok()
            .and_then(|len| len.checked_add(i64::from(offset.get())))
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| self.get(index))
            .unwrap_or(false)
    }

    pub(crate) fn push(&mut self, bit: bool) {
        let shift = self.len % WORD_BITS;
        if shift == 0 {
            self.words.push(u64::from(bit));
        } else if let Some(last) = self.words.last_mut() {
            *last |= u64::from(bit) << shift;
        }
        self.len += 1;
    }

    pub(crate) fn extend_from_bools(&mut self, bits: &[bool]) {
        for &bit in bits {
            self.push(bit);
        }
    }

    /// Appends `count` false bits.
    pub(crate) fn extend_zeros(&mut self, count: usize) {
        let new_len = self.len.saturating_add(count);
        self.words.resize(new_len.div_ceil(WORD_BITS), 0);
        self.len = new_len;
    }

    /// Appends the low `count` bits of `chunk`, which must be at most 64 and zero above `count`.
    fn append_chunk(&mut self, chunk: u64, count: usize) {
        let shift = self.len % WORD_BITS;
        if shift == 0 {
            self.words.push(chunk);
        } else {
            if let Some(last) = self.words.last_mut() {
                *last |= chunk << shift;
            }
            if shift + count > WORD_BITS {
                self.words.push(chunk >> (WORD_BITS - shift));
            }
        }
        self.len += count;
    }

    /// The `count <= 64` bits starting at `start`, which must lie inside the record.
    fn chunk_at(&self, start: usize, count: usize) -> u64 {
        let word = |index: usize| self.words.get(index).copied().unwrap_or(0);
        let shift = start % WORD_BITS;
        let first = start / WORD_BITS;
        let mut chunk = word(first) >> shift;
        if shift != 0 {
            chunk |= word(first + 1) << (WORD_BITS - shift);
        }
        if count < WORD_BITS {
            chunk &= (1_u64 << count) - 1;
        }
        chunk
    }

    /// Appends `repetitions` copies of the bits from `start` to the current end.
    pub(crate) fn append_repeated(
        &mut self,
        start: usize,
        repetitions: u64,
    ) -> Result<(), SamplingExecutionError> {
        let pattern_len = self.len.saturating_sub(start);
        if pattern_len == 0 || repetitions == 0 {
            return Ok(());
        }
        let appended = usize::try_from(repetitions)
            .ok()
            .and_then(|repetitions| repetitions.checked_mul(pattern_len))
            .ok_or_else(|| SamplingExecutionError::InternalInvariant {
                message: "reference folding output length overflowed".to_owned(),
            })?;
        let total_bits = self.len.checked_add(appended).ok_or_else(|| {
            SamplingExecutionError::InternalInvariant {
                message: "reference folding output length overflowed".to_owned(),
            }
        })?;
        let total_words = total_bits.div_ceil(WORD_BITS);
        self.words
            .try_reserve(total_words.saturating_sub(self.words.len()))
            .map_err(|error| SamplingExecutionError::SessionStorageAllocation {
                message: format!("reference folding output capacity {total_bits} bits: {error}"),
            })?;
        for _ in 0..repetitions {
            let mut copied = 0;
            while copied < pattern_len {
                let count = (pattern_len - copied).min(WORD_BITS);
                let chunk = self.chunk_at(start + copied, count);
                self.append_chunk(chunk, count);
                copied += count;
            }
        }
        Ok(())
    }

    /// Unpacks the record into one flag per bit, for callers whose contract is a flag list.
    pub(crate) fn to_bools(
        &self,
        label: &'static str,
    ) -> Result<Vec<bool>, SamplingExecutionError> {
        let mut bools = super::api::try_bool_buffer(self.len, label)?;
        self.extend_bools_into(&mut bools);
        Ok(bools)
    }

    /// Appends one flag per bit to `output`.
    pub(crate) fn extend_bools_into(&self, output: &mut Vec<bool>) {
        let mut remaining = self.len;
        for word in &self.words {
            let count = remaining.min(WORD_BITS);
            output.extend((0..count).map(|bit| (word >> bit) & 1 == 1));
            remaining -= count;
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "fold fixtures are bounded, so a failed append is a test failure"
)]
mod tests {
    use super::*;

    fn bits_of(record: &ReferenceBits) -> Vec<bool> {
        let mut bools = Vec::new();
        record.extend_bools_into(&mut bools);
        bools
    }

    /// Folding copies a recurring period at arbitrary bit alignments; the packed copy must agree
    /// with the flag-list copy it replaced and keep every bit past the end zero.
    #[test]
    fn repeated_appends_match_flag_list_copies_at_every_alignment() {
        for prefix in [0_usize, 1, 7, 63, 64, 65, 130] {
            for pattern in [1_usize, 3, 63, 64, 65, 129] {
                for repetitions in [1_u64, 2, 5] {
                    let mut record = ReferenceBits::default();
                    let mut expected = Vec::new();
                    for index in 0..prefix + pattern {
                        let bit = (index * 7 + index / 3) % 5 < 2;
                        record.push(bit);
                        expected.push(bit);
                    }
                    record
                        .append_repeated(prefix, repetitions)
                        .expect("bounded fold");
                    for _ in 0..repetitions {
                        expected.extend_from_within(prefix..prefix + pattern);
                    }
                    assert_eq!(bits_of(&record), expected);
                    let tail = record.len() % WORD_BITS;
                    if tail != 0 {
                        assert_eq!(record.words().last().copied().unwrap_or(0) >> tail, 0);
                    }
                    assert_eq!(record.words().len(), record.len().div_ceil(WORD_BITS));
                }
            }
        }
    }
}
