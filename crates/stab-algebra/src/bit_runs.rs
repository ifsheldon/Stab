//! Word-level copies of bit runs between packed little-endian bit planes.
//!
//! Bit `i` of a plane lives in bit `i % 64` of word `i / 64`. [`or_bits_at`] ORs bits into its
//! destination, so the destination range must be zero before the call, as it is in storage
//! reserved with `try_zeroed_words`; [`extend_periodic`] overwrites its range.

use std::num::NonZeroUsize;

use stab_bits::{BitError, fill_periodic_words};

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
/// be written; see [`stab_bits::fill_periodic_words`].
pub(crate) fn extend_periodic(
    words: &mut [u64],
    period: usize,
    len: usize,
) -> Result<(), BitError> {
    match NonZeroUsize::new(period) {
        Some(period) => fill_periodic_words(words, 0, period, period.get(), len),
        None if len == 0 => Ok(()),
        None => Err(outside(words, len)),
    }
}
