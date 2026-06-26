use std::simd::Simd;

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
