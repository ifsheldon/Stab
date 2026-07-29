use super::BIT_BLOCK_WORDS;

#[cfg(not(feature = "portable-simd"))]
pub(super) fn xor_block(
    mut left: [u64; BIT_BLOCK_WORDS],
    right: [u64; BIT_BLOCK_WORDS],
) -> [u64; BIT_BLOCK_WORDS] {
    xor_assign_words(&mut left, &right);
    left
}

pub(super) fn and_block(
    mut left: [u64; BIT_BLOCK_WORDS],
    right: [u64; BIT_BLOCK_WORDS],
) -> [u64; BIT_BLOCK_WORDS] {
    and_assign_words(&mut left, &right);
    left
}

pub(super) fn or_block(
    mut left: [u64; BIT_BLOCK_WORDS],
    right: [u64; BIT_BLOCK_WORDS],
) -> [u64; BIT_BLOCK_WORDS] {
    or_assign_words(&mut left, &right);
    left
}

pub(super) fn xor_assign_words(lhs: &mut [u64], rhs: &[u64]) {
    for (left, right) in lhs.iter_mut().zip(rhs) {
        *left ^= *right;
    }
}

pub(super) fn and_assign_words(lhs: &mut [u64], rhs: &[u64]) {
    for (left, right) in lhs.iter_mut().zip(rhs) {
        *left &= *right;
    }
}

pub(super) fn or_assign_words(lhs: &mut [u64], rhs: &[u64]) {
    for (left, right) in lhs.iter_mut().zip(rhs) {
        *left |= *right;
    }
}

pub(super) fn masked_xor_assign_words(lhs: &mut [u64], rhs: &[u64], mask: &[u64]) {
    for ((left, right), mask) in lhs.iter_mut().zip(rhs).zip(mask) {
        *left ^= *right & *mask;
    }
}

pub(super) fn and_not_assign_words(lhs: &mut [u64], rhs: &[u64]) {
    for (left, right) in lhs.iter_mut().zip(rhs) {
        *left &= !*right;
    }
}

pub(super) fn popcount_words(words: &[u64]) -> usize {
    words.iter().map(|word| popcount_word(*word)).sum()
}

pub(super) fn not_zero_words(words: &[u64]) -> bool {
    words.iter().any(|word| *word != 0)
}

pub(super) fn popcount_word(word: u64) -> usize {
    word.count_ones() as usize
}
