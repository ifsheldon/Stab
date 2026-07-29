#![feature(portable_simd)]
//! Nightly-only portable-SIMD kernels over raw fixed-width word blocks.

use std::simd::Simd;

pub const WORD_LANES: usize = 4;
pub const CLIFFORD_PLANES: usize = 6;

pub type WordBlock = [u64; WORD_LANES];
pub type CliffordWordBlock = [WordBlock; CLIFFORD_PLANES];

type SimdWordBlock = Simd<u64, WORD_LANES>;

/// XORs one fixed-width word block into another.
#[inline]
pub fn xor_assign_block(left: &mut WordBlock, right: &WordBlock) {
    *left = (SimdWordBlock::from_array(*left) ^ SimdWordBlock::from_array(*right)).to_array();
}

/// Right-multiplies four packed Clifford words.
///
/// The returned tuple contains the transformed planes, followed by one bit mask for the
/// non-identity lanes before and after multiplication.
#[inline]
pub fn clifford_right_multiply_block(
    left: CliffordWordBlock,
    right: CliffordWordBlock,
) -> (CliffordWordBlock, WordBlock, WordBlock) {
    let [
        left_z_signs,
        left_x_signs,
        left_inv_x2x,
        left_x2z,
        left_z2x,
        left_inv_z2z,
    ] = left.map(SimdWordBlock::from_array);
    let [
        right_z_signs,
        right_x_signs,
        right_inv_x2x,
        right_x2z,
        right_z2x,
        right_inv_z2z,
    ] = right.map(SimdWordBlock::from_array);

    let inv_x2x = (left_inv_x2x | right_inv_x2x) ^ (left_z2x & right_x2z);
    let x2z = (!right_inv_x2x & left_x2z) ^ (!left_inv_z2z & right_x2z);
    let z2x = (!left_inv_x2x & right_z2x) ^ (!right_inv_z2z & left_z2x);
    let inv_z2z = (left_x2z & right_z2x) ^ (left_inv_z2z | right_inv_z2z);
    let right_x2y = !right_inv_x2x & right_x2z;
    let right_z2y = !right_inv_z2z & right_z2x;
    let dy = (left_x2z & left_z2x) ^ left_inv_x2x ^ left_z2x ^ left_x2z ^ left_inv_z2z;
    let x_signs = right_x_signs
        ^ (!right_inv_x2x & left_x_signs)
        ^ (right_x2y & dy)
        ^ (right_x2z & left_z_signs);
    let z_signs = right_z_signs
        ^ (right_z2x & left_x_signs)
        ^ (right_z2y & dy)
        ^ (!right_inv_z2z & left_z_signs);

    let before_non_identity =
        left_z_signs | left_x_signs | left_inv_x2x | left_x2z | left_z2x | left_inv_z2z;
    let after_non_identity = z_signs | x_signs | inv_x2x | x2z | z2x | inv_z2z;
    (
        [z_signs, x_signs, inv_x2x, x2z, z2x, inv_z2z].map(SimdWordBlock::to_array),
        before_non_identity.to_array(),
        after_non_identity.to_array(),
    )
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::cast_possible_truncation,
        clippy::indexing_slicing,
        reason = "fixed-size kernel fixtures use bounded array indexes and deterministic small seeds"
    )]

    use super::*;

    #[test]
    fn raw_xor_block_matches_scalar_formula() {
        let left = [0, u64::MAX, 0x55aa, 0x1234_5678_9abc_def0];
        let right = [u64::MAX, 0, 0xaa55, 0xfedc_ba98_7654_3210];
        let mut actual = left;

        xor_assign_block(&mut actual, &right);
        assert_eq!(
            actual,
            std::array::from_fn(|index| left[index] ^ right[index])
        );
    }

    #[test]
    fn clifford_block_matches_scalar_lane_products() {
        let left = std::array::from_fn(|plane| {
            std::array::from_fn(|lane| splitmix64((plane * WORD_LANES + lane) as u64))
        });
        let right = std::array::from_fn(|plane| {
            std::array::from_fn(|lane| splitmix64((41 + plane * WORD_LANES + lane) as u64))
        });
        let (result, before_non_identity, after_non_identity) =
            clifford_right_multiply_block(left, right);
        let expected = std::array::from_fn(|plane| {
            std::array::from_fn(|lane| {
                let left_lane = std::array::from_fn(|index| left[index][lane]);
                let right_lane = std::array::from_fn(|index| right[index][lane]);
                scalar_clifford_product(left_lane, right_lane)[plane]
            })
        });

        assert_eq!(result, expected);
        assert_eq!(before_non_identity, non_identity_mask(left),);
        assert_eq!(after_non_identity, non_identity_mask(expected),);
    }

    fn non_identity_mask(block: CliffordWordBlock) -> WordBlock {
        block
            .into_iter()
            .reduce(|left, right| std::array::from_fn(|index| left[index] | right[index]))
            .unwrap_or([0; WORD_LANES])
    }

    fn scalar_clifford_product(left: [u64; 6], right: [u64; 6]) -> [u64; 6] {
        let [
            left_z_signs,
            left_x_signs,
            left_inv_x2x,
            left_x2z,
            left_z2x,
            left_inv_z2z,
        ] = left;
        let [
            right_z_signs,
            right_x_signs,
            right_inv_x2x,
            right_x2z,
            right_z2x,
            right_inv_z2z,
        ] = right;
        let inv_x2x = (left_inv_x2x | right_inv_x2x) ^ (left_z2x & right_x2z);
        let x2z = (!right_inv_x2x & left_x2z) ^ (!left_inv_z2z & right_x2z);
        let z2x = (!left_inv_x2x & right_z2x) ^ (!right_inv_z2z & left_z2x);
        let inv_z2z = (left_x2z & right_z2x) ^ (left_inv_z2z | right_inv_z2z);
        let right_x2y = !right_inv_x2x & right_x2z;
        let right_z2y = !right_inv_z2z & right_z2x;
        let dy = (left_x2z & left_z2x) ^ left_inv_x2x ^ left_z2x ^ left_x2z ^ left_inv_z2z;
        let x_signs = right_x_signs
            ^ (!right_inv_x2x & left_x_signs)
            ^ (right_x2y & dy)
            ^ (right_x2z & left_z_signs);
        let z_signs = right_z_signs
            ^ (right_z2x & left_x_signs)
            ^ (right_z2y & dy)
            ^ (!right_inv_z2z & left_z_signs);
        [z_signs, x_signs, inv_x2x, x2z, z2x, inv_z2z]
    }

    fn splitmix64(index: u64) -> u64 {
        let mut value = index.wrapping_add(0x9e37_79b9_7f4a_7c15);
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
}
