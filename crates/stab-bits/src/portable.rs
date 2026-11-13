use super::{BitError, BitResult, WORD_BITS};

/// XORs equal-length word slices through the slice-level SIMD kernel.
///
/// Callers have already checked equal bit lengths, so a word-length disagreement here is an
/// internal invariant violation; it is reported as the mismatched bit capacities.
pub(super) fn xor_assign_words(lhs: &mut [u64], rhs: &[u64]) -> BitResult<()> {
    stab_kernels_simd::xor_assign_words(lhs, rhs).map_err(|mismatch| BitError::LengthMismatch {
        left: mismatch.expected.saturating_mul(WORD_BITS),
        right: mismatch.actual.saturating_mul(WORD_BITS),
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::cast_possible_truncation,
        clippy::unwrap_used,
        reason = "bounded differential fixtures rotate by small word indexes"
    )]

    use super::super::{BIT_BLOCK_WORDS, scalar};
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

            let mut portable = left.clone();
            let mut reference = left;
            xor_assign_words(&mut portable, &right).unwrap();
            scalar::xor_assign_words(&mut reference, &right);
            assert_eq!(portable, reference);
        }
    }
}
