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
