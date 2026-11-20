//! Periodic extension of a packed bit run, one word-sized step at a time.

use std::num::NonZeroUsize;

use crate::{BitError, BitResult, WORD_BITS, low_bits_mask, read_word_range_low};

/// Overwrites bits `filled..end` of the little-endian plane `words` so that every bit equals the
/// bit `period` places before it, given that bits `start..filled` already follow that rule.
///
/// Bit `i` lives in bit `i % 64` of word `i / 64`; bits outside `filled..end` are left untouched,
/// and `words` is never resized, so a caller sizes and admits the storage first. Because the
/// fill depends only on the bits before `filled`, a long fill can be split into consecutive
/// ranges, for example to poll cancellation between them, with the same result as one call.
///
/// Each step copies a run from a whole number of periods earlier, at most one word long and never
/// crossing a target word boundary. The copy distance doubles from `period` until it reaches the
/// first multiple of `period` that is at least a word, so all but the first few steps copy one
/// whole word from a source that is still in cache.
///
/// # Errors
///
/// [`BitError::BitRangeOutOfRange`] when fewer than `period` bits of the run are written before
/// `filled` (`start + period > filled`), or when `end` lies past the plane's word capacity. Both
/// are checked only when there is something to fill (`filled < end`).
pub fn fill_periodic_words(
    words: &mut [u64],
    start: usize,
    period: NonZeroUsize,
    filled: usize,
    end: usize,
) -> BitResult<()> {
    if filled >= end {
        return Ok(());
    }
    let period = period.get();
    let behind = filled
        .checked_sub(start)
        .filter(|written| *written >= period)
        .ok_or(BitError::BitRangeOutOfRange {
            start,
            end: start.saturating_add(period),
            len: filled,
        })?;
    let capacity = words.len().saturating_mul(WORD_BITS);
    if end > capacity {
        return Err(BitError::BitRangeOutOfRange {
            start: filled,
            end,
            len: capacity,
        });
    }
    // The shortest multiple of the period that spans a whole word; past it the distance is fixed.
    let longest = period.saturating_mul(WORD_BITS.div_ceil(period));
    let mut distance = (behind - behind % period).min(longest);
    let mut position = filled;
    while position < end {
        let offset = position % WORD_BITS;
        let run = distance.min(WORD_BITS - offset).min(end - position);
        let value = read_word_range_low(words, position - distance, run);
        let word = words
            .get_mut(position / WORD_BITS)
            .ok_or(BitError::BitIndexOutOfRange {
                index: position,
                len: capacity,
            })?;
        let mask = low_bits_mask(run) << offset;
        *word = (*word & !mask) | (value << offset);
        position += run;
        if distance < longest {
            let behind = position - start;
            distance = (behind - behind % period).min(longest);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        reason = "fixtures index planes they sized and use small nonzero periods"
    )]

    use super::*;

    fn bit(words: &[u64], index: usize) -> bool {
        (words[index / WORD_BITS] >> (index % WORD_BITS)) & 1 == 1
    }

    /// A plane whose first `start + period` bits are a fixed pseudo-random pattern and whose
    /// later bits are all ones, so the fill must overwrite rather than OR into its target.
    fn seeded(start: usize, period: usize, end: usize) -> Vec<u64> {
        let mut words = vec![u64::MAX; end.div_ceil(WORD_BITS) + 1];
        for index in 0..start + period {
            let mask = 1_u64 << (index % WORD_BITS);
            if (index * 7 + index / 3) % 5 < 2 {
                words[index / WORD_BITS] |= mask;
            } else {
                words[index / WORD_BITS] &= !mask;
            }
        }
        words
    }

    /// The word-wise fill must agree with a bit-by-bit periodic copy at every alignment and
    /// period, leave the bits past `end` untouched, and give the same plane when the range is
    /// split into consecutive pieces at arbitrary points.
    #[test]
    fn fill_matches_bitwise_copy_and_splits_freely() {
        for start in [0_usize, 1, 7, 63, 64, 65, 130] {
            for period in [1_usize, 2, 3, 5, 31, 63, 64, 65, 129, 130] {
                for end in [
                    start + period,
                    start + period + 1,
                    start + 7 * period + 3,
                    2_000,
                ] {
                    let end = end.max(start + period);
                    let filled = start + period;
                    let period_nz = NonZeroUsize::new(period).unwrap();
                    let mut whole = seeded(start, period, end);
                    let original = whole.clone();
                    fill_periodic_words(&mut whole, start, period_nz, filled, end).unwrap();
                    for index in 0..whole.len() * WORD_BITS {
                        let expected = if (filled..end).contains(&index) {
                            bit(&original, start + (index - start) % period)
                        } else {
                            bit(&original, index)
                        };
                        assert_eq!(
                            bit(&whole, index),
                            expected,
                            "{start} {period} {end} {index}"
                        );
                    }
                    let mut split = seeded(start, period, end);
                    let mut position = filled;
                    for step in [1_usize, 63, 2, 130, 7].into_iter().cycle() {
                        if position >= end {
                            break;
                        }
                        let stop = (position + step).min(end);
                        fill_periodic_words(&mut split, start, period_nz, position, stop).unwrap();
                        position = stop;
                    }
                    assert_eq!(split, whole, "{start} {period} {end}");
                }
            }
        }
    }

    /// An unwritten period or a range past the plane is refused without touching the words.
    #[test]
    fn fill_refuses_short_prefixes_and_out_of_range_ends() {
        let period = NonZeroUsize::new(5).unwrap();
        let mut words = vec![0x1234_u64; 2];
        assert_eq!(
            fill_periodic_words(&mut words, 3, period, 7, 20),
            Err(BitError::BitRangeOutOfRange {
                start: 3,
                end: 8,
                len: 7
            })
        );
        assert_eq!(
            fill_periodic_words(&mut words, 0, period, 5, 129),
            Err(BitError::BitRangeOutOfRange {
                start: 5,
                end: 129,
                len: 128
            })
        );
        assert_eq!(words, vec![0x1234_u64; 2]);
        assert_eq!(fill_periodic_words(&mut words, 0, period, 9, 9), Ok(()));
    }
}
