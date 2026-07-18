use std::simd::Simd;

use super::BIT_BLOCK_WORDS;

type WordBlock = Simd<u64, BIT_BLOCK_WORDS>;

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

#[derive(Clone, Copy)]
struct CliffordBlock {
    z_signs: WordBlock,
    x_signs: WordBlock,
    inv_x2x: WordBlock,
    x2z: WordBlock,
    z2x: WordBlock,
    inv_z2z: WordBlock,
}

impl CliffordBlock {
    fn right_multiply(self, rhs: Self) -> Self {
        let inv_x2x = (self.inv_x2x | rhs.inv_x2x) ^ (self.z2x & rhs.x2z);
        let x2z = (!rhs.inv_x2x & self.x2z) ^ (!self.inv_z2z & rhs.x2z);
        let z2x = (!self.inv_x2x & rhs.z2x) ^ (!rhs.inv_z2z & self.z2x);
        let inv_z2z = (self.x2z & rhs.z2x) ^ (self.inv_z2z | rhs.inv_z2z);

        let rhs_x2y = !rhs.inv_x2x & rhs.x2z;
        let rhs_z2y = !rhs.inv_z2z & rhs.z2x;
        let dy = (self.x2z & self.z2x) ^ self.inv_x2x ^ self.z2x ^ self.x2z ^ self.inv_z2z;
        let x_signs =
            rhs.x_signs ^ (!rhs.inv_x2x & self.x_signs) ^ (rhs_x2y & dy) ^ (rhs.x2z & self.z_signs);
        let z_signs =
            rhs.z_signs ^ (rhs.z2x & self.x_signs) ^ (rhs_z2y & dy) ^ (!rhs.inv_z2z & self.z_signs);

        Self {
            z_signs,
            x_signs,
            inv_x2x,
            x2z,
            z2x,
            inv_z2z,
        }
    }

    fn any(self) -> WordBlock {
        self.z_signs | self.x_signs | self.inv_x2x | self.x2z | self.z2x | self.inv_z2z
    }
}

pub(crate) fn clifford_right_multiply_words(
    left: CliffordPlanesMut<'_>,
    right: CliffordPlanes<'_>,
) -> usize {
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

    let (left_z_sign_blocks, left_z_sign_tail) = left_z_signs.as_chunks_mut::<BIT_BLOCK_WORDS>();
    let (left_x_sign_blocks, left_x_sign_tail) = left_x_signs.as_chunks_mut::<BIT_BLOCK_WORDS>();
    let (left_inv_x2x_blocks, left_inv_x2x_tail) = left_inv_x2x.as_chunks_mut::<BIT_BLOCK_WORDS>();
    let (left_x2z_blocks, left_x2z_tail) = left_x2z.as_chunks_mut::<BIT_BLOCK_WORDS>();
    let (left_z2x_blocks, left_z2x_tail) = left_z2x.as_chunks_mut::<BIT_BLOCK_WORDS>();
    let (left_inv_z2z_blocks, left_inv_z2z_tail) = left_inv_z2z.as_chunks_mut::<BIT_BLOCK_WORDS>();
    let (right_z_sign_blocks, right_z_sign_tail) = right_z_signs.as_chunks::<BIT_BLOCK_WORDS>();
    let (right_x_sign_blocks, right_x_sign_tail) = right_x_signs.as_chunks::<BIT_BLOCK_WORDS>();
    let (right_inv_x2x_blocks, right_inv_x2x_tail) = right_inv_x2x.as_chunks::<BIT_BLOCK_WORDS>();
    let (right_x2z_blocks, right_x2z_tail) = right_x2z.as_chunks::<BIT_BLOCK_WORDS>();
    let (right_z2x_blocks, right_z2x_tail) = right_z2x.as_chunks::<BIT_BLOCK_WORDS>();
    let (right_inv_z2z_blocks, right_inv_z2z_tail) = right_inv_z2z.as_chunks::<BIT_BLOCK_WORDS>();

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
    let mut non_identity_count = 0_usize;
    for (left, right) in left_blocks.zip(right_blocks) {
        let (((((left_z_signs, left_x_signs), left_inv_x2x), left_x2z), left_z2x), left_inv_z2z) =
            left;
        let (
            ((((right_z_signs, right_x_signs), right_inv_x2x), right_x2z), right_z2x),
            right_inv_z2z,
        ) = right;
        let result = CliffordBlock {
            z_signs: WordBlock::from_array(*left_z_signs),
            x_signs: WordBlock::from_array(*left_x_signs),
            inv_x2x: WordBlock::from_array(*left_inv_x2x),
            x2z: WordBlock::from_array(*left_x2z),
            z2x: WordBlock::from_array(*left_z2x),
            inv_z2z: WordBlock::from_array(*left_inv_z2z),
        }
        .right_multiply(CliffordBlock {
            z_signs: WordBlock::from_array(*right_z_signs),
            x_signs: WordBlock::from_array(*right_x_signs),
            inv_x2x: WordBlock::from_array(*right_inv_x2x),
            x2z: WordBlock::from_array(*right_x2z),
            z2x: WordBlock::from_array(*right_z2x),
            inv_z2z: WordBlock::from_array(*right_inv_z2z),
        });
        *left_z_signs = result.z_signs.to_array();
        *left_x_signs = result.x_signs.to_array();
        *left_inv_x2x = result.inv_x2x.to_array();
        *left_x2z = result.x2z.to_array();
        *left_z2x = result.z2x.to_array();
        *left_inv_z2z = result.inv_z2z.to_array();
        non_identity_count += result
            .any()
            .to_array()
            .into_iter()
            .map(|word| word.count_ones() as usize)
            .sum::<usize>();
    }

    non_identity_count += scalar_right_multiply_words(
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
    non_identity_count
}

fn scalar_right_multiply_words(left: CliffordPlanesMut<'_>, right: CliffordPlanes<'_>) -> usize {
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
    let mut non_identity_count = 0_usize;
    for (left, right) in left_words.zip(right_words) {
        let (((((left_z_signs, left_x_signs), left_inv_x2x), left_x2z), left_z2x), left_inv_z2z) =
            left;
        let (
            ((((right_z_signs, right_x_signs), right_inv_x2x), right_x2z), right_z2x),
            right_inv_z2z,
        ) = right;
        let result = scalar_product(
            [
                *left_z_signs,
                *left_x_signs,
                *left_inv_x2x,
                *left_x2z,
                *left_z2x,
                *left_inv_z2z,
            ],
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
        ] = result;
        non_identity_count += result
            .into_iter()
            .fold(0, |combined, word| combined | word)
            .count_ones() as usize;
    }
    non_identity_count
}

fn scalar_product(left: [u64; 6], right: [u64; 6]) -> [u64; 6] {
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "kernel test fixtures derive indices from exact local array and word bounds"
    )]

    use super::*;

    const VALID_TABLE_INDICES: [u8; 24] = [
        0, 1, 2, 3, 8, 9, 10, 11, 16, 17, 18, 19, 28, 29, 30, 31, 56, 57, 58, 59, 60, 61, 62, 63,
    ];

    #[test]
    fn portable_kernel_matches_scalar_reference_across_blocks_and_tails() {
        for width in [1_usize, 63, 64, 65, 255, 256, 257, 552, 1_000] {
            let (left, right) = valid_planes(width);
            let mut scalar = left.clone();
            let mut portable = left;
            let scalar_non_identity_count =
                scalar_right_multiply_words(mutable_planes(&mut scalar), immutable_planes(&right));
            let portable_non_identity_count = clifford_right_multiply_words(
                mutable_planes(&mut portable),
                immutable_planes(&right),
            );
            assert_eq!(portable, scalar, "width={width}");
            assert_eq!(
                portable_non_identity_count, scalar_non_identity_count,
                "width={width}"
            );
        }
    }

    fn valid_planes(width: usize) -> ([Vec<u64>; 6], [Vec<u64>; 6]) {
        let word_count = width.div_ceil(u64::BITS as usize);
        let mut left = std::array::from_fn(|_| vec![0; word_count]);
        let mut right = std::array::from_fn(|_| vec![0; word_count]);
        for index in 0..width {
            let left_code = VALID_TABLE_INDICES
                .get(index % VALID_TABLE_INDICES.len())
                .copied()
                .expect("left code");
            let right_code = VALID_TABLE_INDICES
                .get((index * 17 + 5) % VALID_TABLE_INDICES.len())
                .copied()
                .expect("right code");
            set_code(&mut left, index, left_code);
            set_code(&mut right, index, right_code);
        }
        (left, right)
    }

    fn set_code(planes: &mut [Vec<u64>; 6], index: usize, code: u8) {
        let word_index = index / u64::BITS as usize;
        let bit = 1_u64 << (index % u64::BITS as usize);
        for (plane_index, plane) in planes.iter_mut().enumerate() {
            if code & (1 << plane_index) != 0 {
                *plane.get_mut(word_index).expect("word") |= bit;
            }
        }
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
}
