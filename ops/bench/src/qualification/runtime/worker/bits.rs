use std::sync::atomic::{Ordering, compiler_fence};

use super::{WorkerError, byte_digest_word_pair, byte_digest_words};

pub(super) const POPCOUNT_ALIGNMENT_BITS: u64 = 256;
pub(super) const POPCOUNT_MIN_BITS: u64 = 512;
pub(super) const POPCOUNT_MAX_BITS: u64 = 268_435_456;
pub(super) const POPCOUNT_TOGGLE_BIT: usize = 300;
pub(super) const DENSE_XOR_ALIGNMENT_BITS: u64 = 256;
pub(super) const DENSE_XOR_MIN_BITS: u64 = 256;
pub(super) const DENSE_XOR_MAX_BITS: u64 = 268_435_456;

#[derive(Clone)]
pub(super) struct PopcountFixture {
    pub(super) bits: stab_core::BitVec,
    pub(super) input_bytes: u64,
    pub(super) input_digest: [u64; 4],
}

pub(super) fn popcount_fixture(bit_count: u64) -> Result<PopcountFixture, WorkerError> {
    let word_count = validate_popcount_width(bit_count)?;
    let bit_count_usize =
        usize::try_from(bit_count).map_err(|_| WorkerError::PopcountWidthRange(bit_count))?;
    let mut words = Vec::new();
    words
        .try_reserve_exact(word_count)
        .map_err(WorkerError::PopcountFixtureAllocation)?;
    for index in 0..word_count {
        let index = u64::try_from(index).map_err(|_| WorkerError::PopcountWordIndexRange)?;
        words.push(splitmix64_word(index));
    }
    let input_bytes = u64::try_from(word_count)
        .ok()
        .and_then(|count| count.checked_mul(u64::BITS as u64 / 8))
        .ok_or(WorkerError::InputSizeRange)?;
    let input_digest = byte_digest_words(&words);
    Ok(PopcountFixture {
        bits: stab_core::BitVec::from_words_truncated(bit_count_usize, words),
        input_bytes,
        input_digest,
    })
}

pub(super) fn validate_popcount_width(bit_count: u64) -> Result<usize, WorkerError> {
    if bit_count < POPCOUNT_MIN_BITS {
        return Err(WorkerError::PopcountWidthMinimum {
            actual: bit_count,
            minimum: POPCOUNT_MIN_BITS,
        });
    }
    if bit_count > POPCOUNT_MAX_BITS {
        return Err(WorkerError::PopcountWidthLimit {
            actual: bit_count,
            maximum: POPCOUNT_MAX_BITS,
        });
    }
    if !bit_count.is_multiple_of(POPCOUNT_ALIGNMENT_BITS) {
        return Err(WorkerError::PopcountWidthAlignment {
            actual: bit_count,
            alignment: POPCOUNT_ALIGNMENT_BITS,
        });
    }
    usize::try_from(bit_count / u64::BITS as u64)
        .map_err(|_| WorkerError::PopcountWidthRange(bit_count))
}

fn splitmix64_word(index: u64) -> u64 {
    let mut value = index.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

pub(super) fn simd_word_popcount(
    iterations: u64,
    fixture: &mut PopcountFixture,
    toggle_state: &mut bool,
) -> Result<u64, WorkerError> {
    let mut checksum = 0_u64;
    for _ in 0..iterations {
        compiler_fence(Ordering::SeqCst);
        *toggle_state = !*toggle_state;
        fixture.bits.set(POPCOUNT_TOGGLE_BIT, *toggle_state)?;
        let count =
            u64::try_from(fixture.bits.popcount()).map_err(|_| WorkerError::PopcountResultRange)?;
        checksum = checksum.wrapping_add(count);
    }
    Ok(checksum)
}

pub(super) fn popcount_output_digest(
    checksum: u64,
    iterations: u64,
    work_items: u64,
    input_digest: [u64; 4],
    final_bit: bool,
) -> [u64; 4] {
    byte_digest_words(&[
        checksum,
        iterations,
        work_items,
        input_digest[0],
        input_digest[1],
        input_digest[2],
        input_digest[3],
        u64::from(final_bit),
    ])
}

#[derive(Clone)]
pub(super) struct DenseXorFixture {
    pub(super) destination: stab_core::BitVec,
    pub(super) source: stab_core::BitVec,
    pub(super) input_bytes: u64,
    pub(super) input_digest: [u64; 4],
}

pub(super) fn dense_xor_fixture(bit_count: u64) -> Result<DenseXorFixture, WorkerError> {
    let word_count = validate_dense_xor_width(bit_count)?;
    let bit_count_usize =
        usize::try_from(bit_count).map_err(|_| WorkerError::DenseXorWidthRange(bit_count))?;
    let mut destination_words = Vec::new();
    destination_words
        .try_reserve_exact(word_count)
        .map_err(WorkerError::DenseXorFixtureAllocation)?;
    let mut source_words = Vec::new();
    source_words
        .try_reserve_exact(word_count)
        .map_err(WorkerError::DenseXorFixtureAllocation)?;
    for index in 0..word_count {
        let index = u64::try_from(index).map_err(|_| WorkerError::DenseXorWordIndexRange)?;
        let destination_index = index
            .checked_mul(2)
            .ok_or(WorkerError::DenseXorWordIndexRange)?;
        let source_index = destination_index
            .checked_add(1)
            .ok_or(WorkerError::DenseXorWordIndexRange)?;
        destination_words.push(splitmix64_word(destination_index));
        source_words.push(splitmix64_word(source_index));
    }
    let input_bytes = u64::try_from(word_count)
        .ok()
        .and_then(|count| count.checked_mul(2 * (u64::BITS as u64 / 8)))
        .ok_or(WorkerError::InputSizeRange)?;
    let input_digest = byte_digest_word_pair(&destination_words, &source_words);
    Ok(DenseXorFixture {
        destination: stab_core::BitVec::from_words_truncated(bit_count_usize, destination_words),
        source: stab_core::BitVec::from_words_truncated(bit_count_usize, source_words),
        input_bytes,
        input_digest,
    })
}

pub(super) fn validate_dense_xor_width(bit_count: u64) -> Result<usize, WorkerError> {
    if bit_count < DENSE_XOR_MIN_BITS {
        return Err(WorkerError::DenseXorWidthMinimum {
            actual: bit_count,
            minimum: DENSE_XOR_MIN_BITS,
        });
    }
    if bit_count > DENSE_XOR_MAX_BITS {
        return Err(WorkerError::DenseXorWidthLimit {
            actual: bit_count,
            maximum: DENSE_XOR_MAX_BITS,
        });
    }
    if !bit_count.is_multiple_of(DENSE_XOR_ALIGNMENT_BITS) {
        return Err(WorkerError::DenseXorWidthAlignment {
            actual: bit_count,
            alignment: DENSE_XOR_ALIGNMENT_BITS,
        });
    }
    usize::try_from(bit_count / u64::BITS as u64)
        .map_err(|_| WorkerError::DenseXorWidthRange(bit_count))
}

pub(super) fn dense_xor(iterations: u64, fixture: &mut DenseXorFixture) -> Result<(), WorkerError> {
    for _ in 0..iterations {
        compiler_fence(Ordering::SeqCst);
        fixture
            .destination
            .xor_assign(&fixture.source.as_bitslice())?;
    }
    Ok(())
}

pub(super) fn dense_xor_output_digest(
    fixture: &DenseXorFixture,
    iterations: u64,
    work_items: u64,
) -> [u64; 4] {
    let destination_digest = byte_digest_words(fixture.destination.words());
    let source_digest = byte_digest_words(fixture.source.words());
    byte_digest_words(&[
        iterations,
        work_items,
        fixture.input_digest[0],
        fixture.input_digest[1],
        fixture.input_digest[2],
        fixture.input_digest[3],
        destination_digest[0],
        destination_digest[1],
        destination_digest[2],
        destination_digest[3],
        source_digest[0],
        source_digest[1],
        source_digest[2],
        source_digest[3],
    ])
}
