//! Word-level copies of bit runs between packed little-endian bit planes.
//!
//! Bit `i` of a plane lives in bit `i % 64` of word `i / 64`. Every helper ORs bits into its
//! destination, so the destination range must be zero before the call, as it is in storage
//! reserved with `try_zeroed_words`.

use stab_bits::BitError;

const WORD_BITS: usize = u64::BITS as usize;

/// Keeps the low `len` bits of `value`, for `len` in `1..=64`.
const fn low_bits(value: u64, len: usize) -> u64 {
    if len >= WORD_BITS {
        value
    } else {
        value & ((1_u64 << len) - 1)
    }
}

fn outside(words: &[u64], end_bit: usize) -> BitError {
    BitError::LengthMismatch {
        left: words.len().saturating_mul(WORD_BITS),
        right: end_bit,
    }
}

/// Reads the `len` bits starting at bit `offset`, for `len` in `1..=64`.
fn read_run(words: &[u64], offset: usize, len: usize) -> Result<u64, BitError> {
    let end = offset.saturating_add(len);
    let word = offset / WORD_BITS;
    let shift = offset % WORD_BITS;
    let mut value = *words.get(word).ok_or_else(|| outside(words, end))? >> shift;
    if shift != 0 && shift + len > WORD_BITS {
        value |= *words.get(word + 1).ok_or_else(|| outside(words, end))? << (WORD_BITS - shift);
    }
    Ok(low_bits(value, len))
}

/// ORs the low `len` bits of `value` into the bits starting at `offset`, for `len` in `1..=64`;
/// the bits of `value` above `len` must be zero.
fn or_run(words: &mut [u64], offset: usize, value: u64, len: usize) -> Result<(), BitError> {
    let end = offset.saturating_add(len);
    let word = offset / WORD_BITS;
    let shift = offset % WORD_BITS;
    let word_count = words.len();
    let error = || BitError::LengthMismatch {
        left: word_count.saturating_mul(WORD_BITS),
        right: end,
    };
    *words.get_mut(word).ok_or_else(error)? |= value << shift;
    if shift != 0 && shift + len > WORD_BITS {
        *words.get_mut(word + 1).ok_or_else(error)? |= value >> (WORD_BITS - shift);
    }
    Ok(())
}

/// ORs the first `len` bits of `source` into `target` starting at bit `offset`.
pub(crate) fn or_bits_at(
    target: &mut [u64],
    offset: usize,
    source: &[u64],
    len: usize,
) -> Result<(), BitError> {
    let mut copied = 0_usize;
    for &word in source.iter().take(len.div_ceil(WORD_BITS)) {
        let run = (len - copied).min(WORD_BITS);
        or_run(target, offset + copied, low_bits(word, run), run)?;
        copied += run;
    }
    if copied == len {
        Ok(())
    } else {
        Err(outside(source, len))
    }
}

/// Fills bits `period..len` of `words` by repeating its first `period` bits, which must already
/// be written, while every later bit is still zero.
///
/// Each step copies a run from a whole number of periods earlier, at most one word long and never
/// crossing a target word boundary. The distance doubles while it is shorter than a word, so all
/// but the first few steps copy one whole word.
pub(crate) fn extend_periodic(
    words: &mut [u64],
    period: usize,
    len: usize,
) -> Result<(), BitError> {
    if period == 0 {
        return if len == 0 {
            Ok(())
        } else {
            Err(outside(words, len))
        };
    }
    let mut position = period;
    while position < len {
        let distance = position - position % period;
        let run = distance
            .min(WORD_BITS - position % WORD_BITS)
            .min(len - position);
        let value = read_run(words, position - distance, run)?;
        or_run(words, position, value, run)?;
        position += run;
    }
    Ok(())
}
