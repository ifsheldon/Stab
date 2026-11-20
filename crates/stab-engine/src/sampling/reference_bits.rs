//! The packed record a reference run writes: one bit per measurement, not one byte.

use std::num::NonZeroUsize;

use stab_bits::{BitSlice, BitVec, fill_periodic_words};
use stab_model::MeasureRecordOffset;

use super::api::SamplingExecutionError;
use super::reference_sample::ReferenceSample;
use crate::work_poll::{WORD_OP_UNITS, WorkPoll, chunk_units};

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

    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    /// The record's words; bits at and past [`Self::len`] are zero.
    #[cfg(test)]
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

    /// Appends `repetitions` copies of the bits from `start` to the current end.
    ///
    /// The storage is reserved fallibly up front and the copy is
    /// [`stab_bits::fill_periodic_words`], run in ranges of a bounded number of words with `poll`
    /// charged before each, so a fold of any length polls cancellation. On an error the record is
    /// left as it was before the call.
    pub(crate) fn append_repeated(
        &mut self,
        start: usize,
        repetitions: u64,
        poll: &mut WorkPoll<'_>,
    ) -> Result<(), SamplingExecutionError> {
        let Some(period) = NonZeroUsize::new(self.len.saturating_sub(start)) else {
            return Ok(());
        };
        if repetitions == 0 {
            return Ok(());
        }
        let overflow = || SamplingExecutionError::InternalInvariant {
            message: "reference folding output length overflowed".to_owned(),
        };
        let appended = usize::try_from(repetitions)
            .ok()
            .and_then(|repetitions| repetitions.checked_mul(period.get()))
            .ok_or_else(overflow)?;
        let total_bits = self.len.checked_add(appended).ok_or_else(overflow)?;
        let total_words = total_bits.div_ceil(WORD_BITS);
        self.words
            .try_reserve(total_words.saturating_sub(self.words.len()))
            .map_err(|error| SamplingExecutionError::SessionStorageAllocation {
                message: format!("reference folding output capacity {total_bits} bits: {error}"),
            })?;
        // Within the reserved capacity, so this never reallocates.
        self.words.resize(total_words, 0);
        let words_per_charge = (chunk_units() / WORD_OP_UNITS).max(1);
        let bits_per_charge = words_per_charge.saturating_mul(WORD_BITS);
        let mut position = self.len;
        while position < total_bits {
            let stop = position.saturating_add(bits_per_charge).min(total_bits);
            let step = poll
                .charge(words_per_charge.saturating_mul(WORD_OP_UNITS))
                .map_err(SamplingExecutionError::from)
                .and_then(|()| {
                    fill_periodic_words(&mut self.words, start, period, position, stop).map_err(
                        |error| SamplingExecutionError::InternalInvariant {
                            message: format!("reference folding copy failed: {error}"),
                        },
                    )
                });
            if let Err(error) = step {
                self.truncate_words();
                return Err(error);
            }
            position = stop;
        }
        self.len = total_bits;
        Ok(())
    }

    /// Drops the words past [`Self::len`] and zeroes the last word's unused bits, restoring the
    /// record's invariant after a partial write past its end.
    fn truncate_words(&mut self) {
        self.words.truncate(self.len.div_ceil(WORD_BITS));
        let tail = self.len % WORD_BITS;
        if tail != 0
            && let Some(last) = self.words.last_mut()
        {
            *last &= (1_u64 << tail) - 1;
        }
    }

    /// Appends every bit of `bits`, one word-aligned chunk at a time.
    pub(crate) fn extend_from_bitslice(&mut self, bits: BitSlice<'_>) {
        let mut remaining = bits.len();
        for &word in bits.words() {
            let count = remaining.min(WORD_BITS);
            let chunk = if count < WORD_BITS {
                word & ((1_u64 << count) - 1)
            } else {
                word
            };
            self.append_chunk(chunk, count);
            remaining -= count;
        }
    }

    /// Freezes the record into the public shared value, moving its words without a copy.
    pub(crate) fn into_sample(self) -> ReferenceSample {
        ReferenceSample::from(BitVec::from_words_truncated(self.len, self.words))
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "fold fixtures are bounded, so a failed append is a test failure"
)]
mod tests {
    use stab_model::Cancellation;

    use super::*;
    use crate::work_poll::test_hook;

    fn bits_of(record: &ReferenceBits) -> Vec<bool> {
        (0..record.len())
            .map(|index| record.get(index).expect("in-range bit"))
            .collect()
    }

    fn record_with(prefix: usize, pattern: usize) -> (ReferenceBits, Vec<bool>) {
        let mut record = ReferenceBits::default();
        let mut expected = Vec::new();
        for index in 0..prefix + pattern {
            let bit = (index * 7 + index / 3) % 5 < 2;
            record.push(bit);
            expected.push(bit);
        }
        (record, expected)
    }

    /// Folding copies a recurring period at arbitrary bit alignments; the word-wise copy must
    /// agree with a bit-by-bit copy and keep every bit past the end zero.
    #[test]
    fn repeated_appends_match_bitwise_copies_at_every_alignment() {
        let cancellation = Cancellation::new();
        for prefix in [0_usize, 1, 7, 63, 64, 65, 130] {
            for pattern in [1_usize, 2, 3, 5, 31, 63, 64, 65, 129, 130] {
                for repetitions in [1_u64, 2, 3, 5, 67, 200] {
                    let (mut record, mut expected) = record_with(prefix, pattern);
                    record
                        .append_repeated(prefix, repetitions, &mut WorkPoll::new(&cancellation))
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

    /// A long fold polls between chunks of words, so cancellation lands inside the copy instead
    /// of after it.
    #[test]
    fn long_repeated_appends_poll_and_honor_cancellation() {
        let cancellation = Cancellation::new();
        test_hook::with_units_per_poll(1 << 8, || {
            let (mut record, _) = record_with(3, 1);
            let (result, polls) = test_hook::count_polls(|| {
                record.append_repeated(3, 1 << 16, &mut WorkPoll::new(&cancellation))
            });
            result.expect("uncancelled fold");
            assert!(polls >= 8, "a 1024-word fold polled only {polls} times");

            let (mut record, _) = record_with(3, 1);
            let (result, fired) = test_hook::cancel_at_poll(2, || {
                record.append_repeated(3, 1 << 16, &mut WorkPoll::new(&cancellation))
            });
            assert!(fired);
            assert!(matches!(result, Err(SamplingExecutionError::Cancelled(_))));
        });
    }
}
