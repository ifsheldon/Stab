use stab_core::advanced::storage::BitVec;

use super::{WORD_BITS, semantic_preflight, stab_runner_error};
use crate::error::BenchError;

pub(super) fn validate_bitvec_preflight(
    row_id: &str,
    left: &BitVec,
    right: &BitVec,
    mask: &BitVec,
    not_zero: &BitVec,
) -> Result<(), BenchError> {
    let expected_xor = left
        .words()
        .iter()
        .zip(right.words())
        .map(|(left, right)| left ^ right)
        .collect::<Vec<_>>();
    let mut actual_xor = right.clone();
    actual_xor
        .xor_assign(&left.as_bitslice())
        .map_err(|error| stab_runner_error(row_id, error))?;
    require_exact_words(row_id, "bit-vector XOR", actual_xor.words(), &expected_xor)?;
    actual_xor
        .xor_assign(&left.as_bitslice())
        .map_err(|error| stab_runner_error(row_id, error))?;
    require_exact_words(
        row_id,
        "bit-vector XOR two-step cycle",
        actual_xor.words(),
        right.words(),
    )?;

    let expected_masked = left
        .words()
        .iter()
        .zip(right.words())
        .zip(mask.words())
        .map(|((left, right), mask)| left ^ (right & mask))
        .collect::<Vec<_>>();
    let mut actual_masked = left.clone();
    actual_masked
        .masked_xor_assign(&right.as_bitslice(), &mask.as_bitslice())
        .map_err(|error| stab_runner_error(row_id, error))?;
    require_exact_words(
        row_id,
        "masked bit-vector XOR",
        actual_masked.words(),
        &expected_masked,
    )?;

    let expected_range = reference_range_xor(left.words(), right.words(), 31, 17, 4096);
    let mut actual_range = left.clone();
    actual_range
        .xor_range_from(31, &right.as_bitslice(), 17, 4096)
        .map_err(|error| stab_runner_error(row_id, error))?;
    require_exact_words(
        row_id,
        "bit-vector range XOR",
        actual_range.words(),
        &expected_range,
    )?;

    let mut actual_copy = BitVec::zeros(left.len());
    actual_copy
        .copy_from_bitslice(&left.as_bitslice())
        .map_err(|error| stab_runner_error(row_id, error))?;
    require_exact_words(row_id, "bit-vector copy", actual_copy.words(), left.words())?;
    semantic_preflight::require_exact(
        row_id,
        "bit-vector nonzero",
        &not_zero.not_zero(),
        &not_zero.words().iter().any(|word| *word != 0),
    )
}

pub(super) fn validate_popcount_preflight(
    row_id: &str,
    bits: &BitVec,
    toggle_index: usize,
) -> Result<(), BenchError> {
    let mut expected_words = bits.words().to_vec();
    let word = expected_words
        .get_mut(toggle_index / WORD_BITS)
        .ok_or_else(|| stab_runner_error(row_id, "popcount toggle index is out of range"))?;
    *word ^= 1_u64 << (toggle_index % WORD_BITS);
    let expected_popcount = expected_words
        .iter()
        .map(|word| word.count_ones() as usize)
        .sum::<usize>();
    let mut actual = bits.clone();
    let bit = actual
        .get(toggle_index)
        .ok_or_else(|| stab_runner_error(row_id, "popcount toggle index is out of range"))?;
    actual
        .set(toggle_index, !bit)
        .map_err(|error| stab_runner_error(row_id, error))?;
    require_exact_words(
        row_id,
        "bit-vector popcount mutation",
        actual.words(),
        &expected_words,
    )?;
    semantic_preflight::require_exact(
        row_id,
        "bit-vector popcount",
        &actual.popcount(),
        &expected_popcount,
    )?;
    actual
        .set(toggle_index, bit)
        .map_err(|error| stab_runner_error(row_id, error))?;
    require_exact_words(
        row_id,
        "bit-vector popcount two-step cycle",
        actual.words(),
        bits.words(),
    )
}

fn require_exact_words(
    row_id: &str,
    contract: &str,
    actual: &[u64],
    expected: &[u64],
) -> Result<(), BenchError> {
    semantic_preflight::require_exact(row_id, contract, actual, expected)
}

fn reference_range_xor(
    left: &[u64],
    right: &[u64],
    left_start: usize,
    right_start: usize,
    bit_len: usize,
) -> Vec<u64> {
    let mut expected = left.to_vec();
    for offset in 0..bit_len {
        let source_index = right_start + offset;
        let source = right
            .get(source_index / WORD_BITS)
            .is_some_and(|word| word & (1_u64 << (source_index % WORD_BITS)) != 0);
        if source {
            let target_index = left_start + offset;
            if let Some(word) = expected.get_mut(target_index / WORD_BITS) {
                *word ^= 1_u64 << (target_index % WORD_BITS);
            }
        }
    }
    expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_word_witness_rejects_a_same_length_mutation() {
        let expected = [1_u64, 2, 3];
        let actual = [1_u64, 2, 4];
        let error = require_exact_words("m5-simd-bits", "bit-vector XOR", &actual, &expected)
            .expect_err("same-length mutation must fail");
        assert!(error.to_string().contains("wrong content"));
    }

    #[test]
    fn scalar_range_reference_moves_only_selected_source_bits() {
        let actual = reference_range_xor(&[0], &[0b1010], 1, 1, 3);
        assert_eq!(actual, [0b1010]);
    }
}
