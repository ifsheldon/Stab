use super::{BIT_BLOCK_WORDS, scalar};

pub(super) fn xor_block(
    mut left: [u64; BIT_BLOCK_WORDS],
    right: [u64; BIT_BLOCK_WORDS],
) -> [u64; BIT_BLOCK_WORDS] {
    stab_kernels_simd::xor_assign_block(&mut left, &right);
    left
}

pub(super) fn xor_assign_words(lhs: &mut [u64], rhs: &[u64]) {
    let (lhs_blocks, lhs_tail) = lhs.as_chunks_mut::<BIT_BLOCK_WORDS>();
    let (rhs_blocks, rhs_tail) = rhs.as_chunks::<BIT_BLOCK_WORDS>();
    for (left, right) in lhs_blocks.iter_mut().zip(rhs_blocks) {
        stab_kernels_simd::xor_assign_block(left, right);
    }
    scalar::xor_assign_words(lhs_tail, rhs_tail);
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::cast_possible_truncation,
        reason = "bounded differential fixtures rotate by small word indexes"
    )]

    use super::*;

    #[test]
    fn portable_xor_matches_scalar_across_blocks_and_tails() {
        for word_count in 0..=2 * BIT_BLOCK_WORDS + 1 {
            let left = (0..word_count)
                .map(|index| 0x5555_aaaa_1234_5678_u64.rotate_left(index as u32))
                .collect::<Vec<_>>();
            let right = (0..word_count)
                .map(|index| 0xf0f0_0f0f_9876_5432_u64.rotate_right(index as u32))
                .collect::<Vec<_>>();

            assert_binary(
                left.clone(),
                &right,
                xor_assign_words,
                scalar::xor_assign_words,
            );
        }
    }

    fn assert_binary(
        mut portable: Vec<u64>,
        right: &[u64],
        portable_op: fn(&mut [u64], &[u64]),
        scalar_op: fn(&mut [u64], &[u64]),
    ) {
        let mut reference = portable.clone();
        portable_op(&mut portable, right);
        scalar_op(&mut reference, right);
        assert_eq!(portable, reference);
    }
}
