use std::simd::Simd;
use std::simd::num::SimdUint as _;

use super::{BIT_BLOCK_WORDS, scalar};

type WordBlock = Simd<u64, BIT_BLOCK_WORDS>;

pub(super) fn xor_block(
    left: [u64; BIT_BLOCK_WORDS],
    right: [u64; BIT_BLOCK_WORDS],
) -> [u64; BIT_BLOCK_WORDS] {
    (WordBlock::from_array(left) ^ WordBlock::from_array(right)).to_array()
}

pub(super) fn and_block(
    left: [u64; BIT_BLOCK_WORDS],
    right: [u64; BIT_BLOCK_WORDS],
) -> [u64; BIT_BLOCK_WORDS] {
    (WordBlock::from_array(left) & WordBlock::from_array(right)).to_array()
}

pub(super) fn or_block(
    left: [u64; BIT_BLOCK_WORDS],
    right: [u64; BIT_BLOCK_WORDS],
) -> [u64; BIT_BLOCK_WORDS] {
    (WordBlock::from_array(left) | WordBlock::from_array(right)).to_array()
}

pub(super) fn xor_assign_words(lhs: &mut [u64], rhs: &[u64]) {
    let (lhs_blocks, lhs_tail) = lhs.as_chunks_mut::<BIT_BLOCK_WORDS>();
    let (rhs_blocks, rhs_tail) = rhs.as_chunks::<BIT_BLOCK_WORDS>();
    for (left, right) in lhs_blocks.iter_mut().zip(rhs_blocks) {
        *left = xor_block(*left, *right);
    }
    scalar::xor_assign_words(lhs_tail, rhs_tail);
}

pub(super) fn and_assign_words(lhs: &mut [u64], rhs: &[u64]) {
    let (lhs_blocks, lhs_tail) = lhs.as_chunks_mut::<BIT_BLOCK_WORDS>();
    let (rhs_blocks, rhs_tail) = rhs.as_chunks::<BIT_BLOCK_WORDS>();
    for (left, right) in lhs_blocks.iter_mut().zip(rhs_blocks) {
        *left = and_block(*left, *right);
    }
    scalar::and_assign_words(lhs_tail, rhs_tail);
}

pub(super) fn or_assign_words(lhs: &mut [u64], rhs: &[u64]) {
    let (lhs_blocks, lhs_tail) = lhs.as_chunks_mut::<BIT_BLOCK_WORDS>();
    let (rhs_blocks, rhs_tail) = rhs.as_chunks::<BIT_BLOCK_WORDS>();
    for (left, right) in lhs_blocks.iter_mut().zip(rhs_blocks) {
        *left = or_block(*left, *right);
    }
    scalar::or_assign_words(lhs_tail, rhs_tail);
}

pub(super) fn masked_xor_assign_words(lhs: &mut [u64], rhs: &[u64], mask: &[u64]) {
    let (lhs_blocks, lhs_tail) = lhs.as_chunks_mut::<BIT_BLOCK_WORDS>();
    let (rhs_blocks, rhs_tail) = rhs.as_chunks::<BIT_BLOCK_WORDS>();
    let (mask_blocks, mask_tail) = mask.as_chunks::<BIT_BLOCK_WORDS>();
    for ((left, right), mask) in lhs_blocks.iter_mut().zip(rhs_blocks).zip(mask_blocks) {
        let left_block = WordBlock::from_array(*left);
        let right_block = WordBlock::from_array(*right);
        let mask_block = WordBlock::from_array(*mask);
        *left = (left_block ^ (right_block & mask_block)).to_array();
    }
    scalar::masked_xor_assign_words(lhs_tail, rhs_tail, mask_tail);
}

pub(super) fn and_not_assign_words(lhs: &mut [u64], rhs: &[u64]) {
    let (lhs_blocks, lhs_tail) = lhs.as_chunks_mut::<BIT_BLOCK_WORDS>();
    let (rhs_blocks, rhs_tail) = rhs.as_chunks::<BIT_BLOCK_WORDS>();
    for (left, right) in lhs_blocks.iter_mut().zip(rhs_blocks) {
        *left = (WordBlock::from_array(*left) & !WordBlock::from_array(*right)).to_array();
    }
    scalar::and_not_assign_words(lhs_tail, rhs_tail);
}

pub(super) fn not_zero_words(words: &[u64]) -> bool {
    let (blocks, tail) = words.as_chunks::<BIT_BLOCK_WORDS>();
    blocks
        .iter()
        .any(|block| WordBlock::from_array(*block).reduce_or() != 0)
        || scalar::not_zero_words(tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simd_not_zero_matches_scalar_for_zero_early_and_late_words() {
        for word_count in 0..=2 * BIT_BLOCK_WORDS + 1 {
            let zero = vec![0; word_count];
            assert_eq!(not_zero_words(&zero), scalar::not_zero_words(&zero));
            for nonzero_index in 0..word_count {
                let mut words = zero.clone();
                let updated = words.get_mut(nonzero_index).is_some_and(|word| {
                    *word = 1_u64 << (nonzero_index % u64::BITS as usize);
                    true
                });
                assert!(updated, "generated word index must be in range");
                assert_eq!(not_zero_words(&words), scalar::not_zero_words(&words));
            }
        }
    }
}
