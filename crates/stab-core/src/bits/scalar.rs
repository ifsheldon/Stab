pub(super) fn pauli_right_multiply_words(
    left_x: &mut [u64],
    left_z: &mut [u64],
    right_x: &[u64],
    right_z: &[u64],
) -> (u64, u64, bool) {
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
    (count_bit_1, count_bit_2, has_terms)
}
