#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CliffordNonIdentityCounts {
    pub(crate) before: usize,
    pub(crate) after: usize,
}

pub(crate) struct CliffordPlanes<'a> {
    pub(crate) z_signs: &'a [u64],
    pub(crate) x_signs: &'a [u64],
    pub(crate) inv_x2x: &'a [u64],
    pub(crate) x2z: &'a [u64],
    pub(crate) z2x: &'a [u64],
    pub(crate) inv_z2z: &'a [u64],
}

pub(crate) struct CliffordPlanesMut<'a> {
    pub(crate) z_signs: &'a mut [u64],
    pub(crate) x_signs: &'a mut [u64],
    pub(crate) inv_x2x: &'a mut [u64],
    pub(crate) x2z: &'a mut [u64],
    pub(crate) z2x: &'a mut [u64],
    pub(crate) inv_z2z: &'a mut [u64],
}

pub(crate) fn clifford_right_multiply_words(
    left: CliffordPlanesMut<'_>,
    right: CliffordPlanes<'_>,
) -> CliffordNonIdentityCounts {
    #[cfg(feature = "portable-simd")]
    {
        portable_clifford_right_multiply_words(left, right)
    }
    #[cfg(not(feature = "portable-simd"))]
    {
        scalar_clifford_right_multiply_words(left, right)
    }
}

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
        let (((((left_z_signs, left_x_signs), left_inv_x2x), left_x2z), left_z2x), left_inv_z2z) =
            left;
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

#[cfg(feature = "portable-simd")]
fn portable_clifford_right_multiply_words(
    left: CliffordPlanesMut<'_>,
    right: CliffordPlanes<'_>,
) -> CliffordNonIdentityCounts {
    use stab_kernels_simd::WORD_LANES;

    debug_assert!(same_plane_lengths_mut(&left));
    debug_assert!(same_plane_lengths(&right));
    debug_assert_eq!(left.z_signs.len(), right.z_signs.len());

    let CliffordPlanesMut {
        z_signs: left_z_signs,
        x_signs: left_x_signs,
        inv_x2x: left_inv_x2x,
        x2z: left_x2z,
        z2x: left_z2x,
        inv_z2z: left_inv_z2z,
    } = left;
    let CliffordPlanes {
        z_signs: right_z_signs,
        x_signs: right_x_signs,
        inv_x2x: right_inv_x2x,
        x2z: right_x2z,
        z2x: right_z2x,
        inv_z2z: right_inv_z2z,
    } = right;

    let (left_z_sign_blocks, left_z_sign_tail) = left_z_signs.as_chunks_mut::<WORD_LANES>();
    let (left_x_sign_blocks, left_x_sign_tail) = left_x_signs.as_chunks_mut::<WORD_LANES>();
    let (left_inv_x2x_blocks, left_inv_x2x_tail) = left_inv_x2x.as_chunks_mut::<WORD_LANES>();
    let (left_x2z_blocks, left_x2z_tail) = left_x2z.as_chunks_mut::<WORD_LANES>();
    let (left_z2x_blocks, left_z2x_tail) = left_z2x.as_chunks_mut::<WORD_LANES>();
    let (left_inv_z2z_blocks, left_inv_z2z_tail) = left_inv_z2z.as_chunks_mut::<WORD_LANES>();
    let (right_z_sign_blocks, right_z_sign_tail) = right_z_signs.as_chunks::<WORD_LANES>();
    let (right_x_sign_blocks, right_x_sign_tail) = right_x_signs.as_chunks::<WORD_LANES>();
    let (right_inv_x2x_blocks, right_inv_x2x_tail) = right_inv_x2x.as_chunks::<WORD_LANES>();
    let (right_x2z_blocks, right_x2z_tail) = right_x2z.as_chunks::<WORD_LANES>();
    let (right_z2x_blocks, right_z2x_tail) = right_z2x.as_chunks::<WORD_LANES>();
    let (right_inv_z2z_blocks, right_inv_z2z_tail) = right_inv_z2z.as_chunks::<WORD_LANES>();

    let left_blocks = left_z_sign_blocks
        .iter_mut()
        .zip(left_x_sign_blocks)
        .zip(left_inv_x2x_blocks)
        .zip(left_x2z_blocks)
        .zip(left_z2x_blocks)
        .zip(left_inv_z2z_blocks);
    let right_blocks = right_z_sign_blocks
        .iter()
        .zip(right_x_sign_blocks)
        .zip(right_inv_x2x_blocks)
        .zip(right_x2z_blocks)
        .zip(right_z2x_blocks)
        .zip(right_inv_z2z_blocks);
    let mut counts = CliffordNonIdentityCounts::default();
    for (left, right) in left_blocks.zip(right_blocks) {
        let (((((left_z_signs, left_x_signs), left_inv_x2x), left_x2z), left_z2x), left_inv_z2z) =
            left;
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
        let (after, before_non_identity, after_non_identity) =
            stab_kernels_simd::clifford_right_multiply_block(
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
        counts.before += before_non_identity
            .into_iter()
            .map(|word| word.count_ones() as usize)
            .sum::<usize>();
        counts.after += after_non_identity
            .into_iter()
            .map(|word| word.count_ones() as usize)
            .sum::<usize>();
    }

    let tail_counts = scalar_clifford_right_multiply_words(
        CliffordPlanesMut {
            z_signs: left_z_sign_tail,
            x_signs: left_x_sign_tail,
            inv_x2x: left_inv_x2x_tail,
            x2z: left_x2z_tail,
            z2x: left_z2x_tail,
            inv_z2z: left_inv_z2z_tail,
        },
        CliffordPlanes {
            z_signs: right_z_sign_tail,
            x_signs: right_x_sign_tail,
            inv_x2x: right_inv_x2x_tail,
            x2z: right_x2z_tail,
            z2x: right_z2x_tail,
            inv_z2z: right_inv_z2z_tail,
        },
    );
    counts.before += tail_counts.before;
    counts.after += tail_counts.after;
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

pub(crate) struct PauliWordProduct {
    pub(crate) count_bit_1: u64,
    pub(crate) count_bit_2: u64,
    pub(crate) has_terms: bool,
}

pub(crate) fn pauli_right_multiply_words(
    left_x: &mut [u64],
    left_z: &mut [u64],
    right_x: &[u64],
    right_z: &[u64],
) -> PauliWordProduct {
    debug_assert_eq!(left_x.len(), left_z.len());
    debug_assert_eq!(left_x.len(), right_x.len());
    debug_assert_eq!(left_x.len(), right_z.len());

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
    PauliWordProduct {
        count_bit_1,
        count_bit_2,
        has_terms,
    }
}

#[cfg(all(test, feature = "portable-simd"))]
mod portable_tests {
    #![allow(
        clippy::cast_possible_truncation,
        reason = "bounded differential fixtures derive deterministic small word seeds"
    )]

    use super::*;

    #[test]
    fn portable_clifford_kernel_matches_scalar_across_blocks_and_tails() {
        for word_count in [0_usize, 1, 3, 4, 5, 8, 9] {
            let left = raw_planes(word_count, 3);
            let right = raw_planes(word_count, 41);
            let mut scalar = left.clone();
            let mut portable = left;

            let scalar_counts = scalar_clifford_right_multiply_words(
                mutable_planes(&mut scalar),
                immutable_planes(&right),
            );
            let portable_counts = portable_clifford_right_multiply_words(
                mutable_planes(&mut portable),
                immutable_planes(&right),
            );

            assert_eq!(portable, scalar, "word_count={word_count}");
            assert_eq!(portable_counts, scalar_counts, "word_count={word_count}");
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
