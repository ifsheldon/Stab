//! Thin wrappers over the `stab-kernels-simd` word kernels that report their length checks as
//! stabilizer errors.

pub(crate) use stab_kernels_simd::{
    CliffordNonIdentityCounts, CliffordPlanes, CliffordPlanesMut, PauliPlanes, PauliPlanesMut,
    PauliWordProduct,
};

use crate::{StabilizerError, StabilizerResult};

/// Right-multiplies `right` into `left` and returns the non-identity counts before and after.
///
/// # Errors
///
/// A plane-length disagreement is reported as
/// [`StabilizerError::InconsistentCliffordStringMetadata`]: callers build every plane from the same
/// Clifford string word count, so a mismatch means the string's planes are out of sync.
#[inline]
pub(crate) fn clifford_right_multiply_words(
    left: CliffordPlanesMut<'_>,
    right: CliffordPlanes<'_>,
) -> StabilizerResult<CliffordNonIdentityCounts> {
    stab_kernels_simd::clifford_right_multiply_words(left, right)
        .map_err(|_mismatch| StabilizerError::InconsistentCliffordStringMetadata)
}

/// Right-multiplies `right` into `left` and returns the product's phase exponent and whether any
/// of its terms is not the identity.
///
/// # Errors
///
/// Callers pass planes of one word count, so a disagreement is the internal
/// [`StabilizerError::PlaneWordCountMismatch`].
#[inline]
pub(crate) fn pauli_right_multiply_words(
    left: PauliPlanesMut<'_>,
    right: PauliPlanes<'_>,
) -> StabilizerResult<PauliWordProduct> {
    stab_kernels_simd::pauli_right_multiply_words(left, right).map_err(word_count_mismatch)
}

/// Right-multiplies `right` into `left` and returns only the base-`i` exponent of the product's
/// scalar, skipping the non-identity tracking of [`pauli_right_multiply_words`].
#[inline]
pub(crate) fn pauli_right_multiply_words_log_i(
    left: PauliPlanesMut<'_>,
    right: PauliPlanes<'_>,
) -> StabilizerResult<u8> {
    stab_kernels_simd::pauli_right_multiply_words_log_i(left, right).map_err(word_count_mismatch)
}

const fn word_count_mismatch(mismatch: stab_kernels_simd::WordLengthMismatch) -> StabilizerError {
    StabilizerError::PlaneWordCountMismatch {
        expected_words: mismatch.expected,
        actual_words: mismatch.actual,
    }
}

/// XORs one generator row `(x, z)` into the accumulated product `(acc_x, acc_z)` and returns the
/// word-wise `acc_z & x` taken before the update, whose total parity over every accumulated row is
/// the commutation sign `sum_{j<l} z_j . x_l` of the ordered product.
pub(crate) fn pauli_accumulate_words(
    acc_x: &mut [u64],
    acc_z: &mut [u64],
    x: &[u64],
    z: &[u64],
) -> u64 {
    debug_assert_eq!(acc_x.len(), acc_z.len());
    debug_assert_eq!(acc_x.len(), x.len());
    debug_assert_eq!(acc_x.len(), z.len());
    let mut parity = 0_u64;
    for (((acc_x, acc_z), x), z) in acc_x.iter_mut().zip(acc_z.iter_mut()).zip(x).zip(z) {
        parity ^= *acc_z & *x;
        *acc_x ^= *x;
        *acc_z ^= *z;
    }
    parity
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::cast_possible_truncation,
        clippy::unwrap_used,
        reason = "bounded differential fixtures derive deterministic small word seeds"
    )]

    use super::*;

    /// Independent scalar reference for the SIMD Clifford kernel.
    fn scalar_clifford_right_multiply_words(
        left: CliffordPlanesMut<'_>,
        right: CliffordPlanes<'_>,
    ) -> CliffordNonIdentityCounts {
        debug_assert!(same_plane_lengths_mut(&left));
        debug_assert!(same_plane_lengths(&right));
        debug_assert_eq!(left.z_signs.len(), right.z_signs.len());

        let left_words = left
            .z_signs
            .iter_mut()
            .zip(left.x_signs)
            .zip(left.inv_x2x)
            .zip(left.x2z)
            .zip(left.z2x)
            .zip(left.inv_z2z);
        let right_words = right
            .z_signs
            .iter()
            .zip(right.x_signs)
            .zip(right.inv_x2x)
            .zip(right.x2z)
            .zip(right.z2x)
            .zip(right.inv_z2z);
        let mut counts = CliffordNonIdentityCounts::default();
        for (left, right) in left_words.zip(right_words) {
            let (
                ((((left_z_signs, left_x_signs), left_inv_x2x), left_x2z), left_z2x),
                left_inv_z2z,
            ) = left;
            let (
                ((((right_z_signs, right_x_signs), right_inv_x2x), right_x2z), right_z2x),
                right_inv_z2z,
            ) = right;
            let before = [
                *left_z_signs,
                *left_x_signs,
                *left_inv_x2x,
                *left_x2z,
                *left_z2x,
                *left_inv_z2z,
            ];
            let after = clifford_product(
                before,
                [
                    *right_z_signs,
                    *right_x_signs,
                    *right_inv_x2x,
                    *right_x2z,
                    *right_z2x,
                    *right_inv_z2z,
                ],
            );
            [
                *left_z_signs,
                *left_x_signs,
                *left_inv_x2x,
                *left_x2z,
                *left_z2x,
                *left_inv_z2z,
            ] = after;
            counts.before += non_identity_count(before);
            counts.after += non_identity_count(after);
        }
        counts
    }

    fn clifford_product(left: [u64; 6], right: [u64; 6]) -> [u64; 6] {
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

    fn non_identity_count(words: [u64; 6]) -> usize {
        words
            .into_iter()
            .fold(0, |combined, word| combined | word)
            .count_ones() as usize
    }

    fn same_plane_lengths(planes: &CliffordPlanes<'_>) -> bool {
        let len = planes.z_signs.len();
        [
            planes.x_signs,
            planes.inv_x2x,
            planes.x2z,
            planes.z2x,
            planes.inv_z2z,
        ]
        .into_iter()
        .all(|plane| plane.len() == len)
    }

    fn same_plane_lengths_mut(planes: &CliffordPlanesMut<'_>) -> bool {
        let len = planes.z_signs.len();
        [
            &*planes.x_signs,
            &*planes.inv_x2x,
            &*planes.x2z,
            &*planes.z2x,
            &*planes.inv_z2z,
        ]
        .into_iter()
        .all(|plane| plane.len() == len)
    }

    #[test]
    fn simd_clifford_kernel_matches_scalar_across_blocks_and_tails() {
        for word_count in [0_usize, 1, 3, 4, 5, 8, 9] {
            let left = raw_planes(word_count, 3);
            let right = raw_planes(word_count, 41);
            let mut scalar = left.clone();
            let mut simd = left;

            let scalar_counts = scalar_clifford_right_multiply_words(
                mutable_planes(&mut scalar),
                immutable_planes(&right),
            );
            let simd_counts =
                clifford_right_multiply_words(mutable_planes(&mut simd), immutable_planes(&right))
                    .unwrap();

            assert_eq!(simd, scalar, "word_count={word_count}");
            assert_eq!(simd_counts, scalar_counts, "word_count={word_count}");
        }
    }

    fn scalar_pauli_right_multiply_words(
        left_x: &mut [u64],
        left_z: &mut [u64],
        right_x: &[u64],
        right_z: &[u64],
    ) -> PauliWordProduct {
        let mut count_bit_1 = 0_u64;
        let mut count_bit_2 = 0_u64;
        let mut has_terms = false;
        for (((left_x, left_z), right_x), right_z) in
            left_x.iter_mut().zip(left_z).zip(right_x).zip(right_z)
        {
            let old_left_x = *left_x;
            let old_left_z = *left_z;
            *left_x ^= *right_x;
            *left_z ^= *right_z;

            let old_x_new_z = old_left_x & *right_z;
            let anti_commutes = (*right_x & old_left_z) ^ old_x_new_z;
            count_bit_2 ^= (count_bit_1 ^ *left_x ^ *left_z ^ old_x_new_z) & anti_commutes;
            count_bit_1 ^= anti_commutes;
            has_terms |= (*left_x | *left_z) != 0;
        }
        let log_i = ((count_bit_1.count_ones() ^ (count_bit_2.count_ones() << 1)) & 3) as u8;
        PauliWordProduct { log_i, has_terms }
    }

    #[test]
    fn simd_pauli_kernel_matches_scalar_across_blocks_and_tails() {
        for word_count in [0_usize, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 17] {
            let [lx, lz, rx, rz, _, _] = raw_planes(word_count, 11);
            let mut scalar_x = lx.clone();
            let mut scalar_z = lz.clone();
            let mut simd_x = lx.clone();
            let mut simd_z = lz.clone();
            let scalar_prod =
                scalar_pauli_right_multiply_words(&mut scalar_x, &mut scalar_z, &rx, &rz);
            let simd_prod = pauli_right_multiply_words(
                PauliPlanesMut {
                    x: &mut simd_x,
                    z: &mut simd_z,
                },
                PauliPlanes { x: &rx, z: &rz },
            )
            .unwrap();
            assert_eq!(simd_x, scalar_x, "word_count={word_count}");
            assert_eq!(simd_z, scalar_z, "word_count={word_count}");
            assert_eq!(simd_prod, scalar_prod, "word_count={word_count}");

            let mut phase_x = lx;
            let mut phase_z = lz;
            let log_i = pauli_right_multiply_words_log_i(
                PauliPlanesMut {
                    x: &mut phase_x,
                    z: &mut phase_z,
                },
                PauliPlanes { x: &rx, z: &rz },
            )
            .unwrap();
            assert_eq!(phase_x, scalar_x, "phase only, word_count={word_count}");
            assert_eq!(phase_z, scalar_z, "phase only, word_count={word_count}");
            assert_eq!(
                log_i, scalar_prod.log_i,
                "phase only, word_count={word_count}"
            );
        }
    }

    fn raw_planes(word_count: usize, offset: u64) -> [Vec<u64>; 6] {
        std::array::from_fn(|plane| {
            (0..word_count)
                .map(|word| splitmix64(offset + (plane * 17 + word) as u64))
                .collect()
        })
    }

    fn mutable_planes(planes: &mut [Vec<u64>; 6]) -> CliffordPlanesMut<'_> {
        let [z_signs, x_signs, inv_x2x, x2z, z2x, inv_z2z] = planes;
        CliffordPlanesMut {
            z_signs,
            x_signs,
            inv_x2x,
            x2z,
            z2x,
            inv_z2z,
        }
    }

    fn immutable_planes(planes: &[Vec<u64>; 6]) -> CliffordPlanes<'_> {
        let [z_signs, x_signs, inv_x2x, x2z, z2x, inv_z2z] = planes;
        CliffordPlanes {
            z_signs,
            x_signs,
            inv_x2x,
            x2z,
            z2x,
            inv_z2z,
        }
    }

    fn splitmix64(index: u64) -> u64 {
        let mut value = index.wrapping_add(0x9e37_79b9_7f4a_7c15);
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
}
