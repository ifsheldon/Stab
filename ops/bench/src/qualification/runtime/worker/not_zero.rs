use std::hint::black_box;
use std::sync::atomic::{Ordering, compiler_fence};

use super::{WorkerError, byte_digest_words};

pub(super) const NOT_ZERO_MIN_BITS: u64 = 64;
pub(super) const NOT_ZERO_MAX_BITS: u64 = 268_435_456;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NotZeroPattern {
    Early,
    Zero,
    Late,
}

impl NotZeroPattern {
    pub(super) fn hit_index(self, bit_count: u64) -> Option<u64> {
        match self {
            Self::Early => Some(bit_count * 3 / 50),
            Self::Zero => None,
            Self::Late => bit_count.checked_sub(1),
        }
    }

    pub(super) const fn marker(self, bit_count: u64) -> u64 {
        match self {
            Self::Early => bit_count * 3 / 50,
            Self::Zero => u64::MAX,
            Self::Late => bit_count - 1,
        }
    }
}

#[derive(Clone)]
pub(super) struct NotZeroFixture {
    pub(super) bits: stab_core::BitVec,
    pub(super) input_bytes: u64,
    pub(super) input_digest: [u64; 4],
    pub(super) pattern: NotZeroPattern,
}

pub(super) fn not_zero_fixture(
    bit_count: u64,
    pattern: NotZeroPattern,
) -> Result<NotZeroFixture, WorkerError> {
    let word_count = validate_not_zero_width(bit_count)?;
    let bit_count_usize =
        usize::try_from(bit_count).map_err(|_| WorkerError::NotZeroWidthRange(bit_count))?;
    let mut words = Vec::new();
    words
        .try_reserve_exact(word_count)
        .map_err(WorkerError::NotZeroFixtureAllocation)?;
    words.resize(word_count, 0);
    if let Some(hit_index) = pattern.hit_index(bit_count) {
        let hit_index =
            usize::try_from(hit_index).map_err(|_| WorkerError::NotZeroWidthRange(bit_count))?;
        let word =
            words
                .get_mut(hit_index / u64::BITS as usize)
                .ok_or(WorkerError::NotZeroHitIndex {
                    index: hit_index,
                    bit_count: bit_count_usize,
                })?;
        *word |= 1_u64 << (hit_index % u64::BITS as usize);
    }
    let input_bytes = u64::try_from(word_count)
        .ok()
        .and_then(|count| count.checked_mul(u64::BITS as u64 / 8))
        .ok_or(WorkerError::InputSizeRange)?;
    let input_digest = byte_digest_words(&words);
    Ok(NotZeroFixture {
        bits: stab_core::BitVec::from_words_truncated(bit_count_usize, words),
        input_bytes,
        input_digest,
        pattern,
    })
}

pub(super) fn validate_not_zero_width(bit_count: u64) -> Result<usize, WorkerError> {
    if bit_count < NOT_ZERO_MIN_BITS {
        return Err(WorkerError::NotZeroWidthMinimum {
            actual: bit_count,
            minimum: NOT_ZERO_MIN_BITS,
        });
    }
    if bit_count > NOT_ZERO_MAX_BITS {
        return Err(WorkerError::NotZeroWidthLimit {
            actual: bit_count,
            maximum: NOT_ZERO_MAX_BITS,
        });
    }
    usize::try_from(bit_count.div_ceil(u64::BITS as u64))
        .map_err(|_| WorkerError::NotZeroWidthRange(bit_count))
}

pub(super) fn simd_bits_not_zero(iterations: u64, fixture: &NotZeroFixture) -> u64 {
    let mut checksum = 0_u64;
    for _ in 0..iterations {
        compiler_fence(Ordering::SeqCst);
        let is_not_zero = black_box(&fixture.bits).not_zero();
        checksum = checksum.wrapping_add(u64::from(is_not_zero));
    }
    checksum
}

pub(super) fn not_zero_output_digest(
    checksum: u64,
    iterations: u64,
    work_items: u64,
    fixture: &NotZeroFixture,
) -> [u64; 4] {
    byte_digest_words(&[
        checksum,
        iterations,
        work_items,
        fixture.pattern.marker(work_items),
        fixture.input_digest[0],
        fixture.input_digest[1],
        fixture.input_digest[2],
        fixture.input_digest[3],
    ])
}
