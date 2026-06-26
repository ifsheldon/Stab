#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::needless_range_loop,
    reason = "M5 property tests construct in-range fixtures and direct assertions keep failing cases readable"
)]

use std::collections::BTreeSet;

use proptest::prelude::*;
use stab_core::bits;
use stab_core::{BitBlock, BitMatrix, BitSlice, BitVec, SparseXorVec};

#[test]
fn bits_bit_ref_and_tail_boundaries_follow_stim() {
    // Adapted from Stim v1.16.0 src/stim/mem/bit_ref.test.cc and simd_bits.test.cc boundary cases.
    for len in [0, 1, 63, 64, 65, 127, 128, 129] {
        let mut bits = BitVec::zeros(len);
        assert_eq!(bits.len(), len);
        assert_eq!(bits.get(len), None);
        if len > 0 {
            bits.set(len - 1, true).expect("set last bit");
            assert_eq!(bits.get(len - 1), Some(true));
            bits.set(len - 1, false).expect("clear last bit");
            assert_eq!(bits.get(len - 1), Some(false));
        }
        assert!(bits.set(len, true).is_err());
    }

    let truncated = BitVec::from_words_truncated(65, vec![u64::MAX, u64::MAX, u64::MAX]);
    assert_eq!(truncated.word_count(), 2);
    assert_eq!(truncated.words(), &[u64::MAX, 1]);
    assert_eq!(truncated.popcount(), 65);
}

#[test]
fn bits_dirty_tail_padding_is_ignored_and_canonicalized() {
    let dirty_words = [u64::MAX, u64::MAX];
    let dirty = BitSlice::new(&dirty_words, 65).expect("dirty slice");
    assert_eq!(dirty.popcount(), 65);

    let mut copied = BitVec::zeros(65);
    copied
        .copy_from_bitslice(&dirty)
        .expect("copy dirty-padded slice");
    assert_eq!(copied.words(), &[u64::MAX, 1]);
    assert_eq!(copied.popcount(), 65);

    let mut xored = BitVec::zeros(65);
    xored.xor_assign(&dirty).expect("xor dirty-padded slice");
    assert_eq!(xored.words(), &[u64::MAX, 1]);

    let mut ored = BitVec::zeros(65);
    ored.or_assign(&dirty).expect("or dirty-padded slice");
    assert_eq!(ored.words(), &[u64::MAX, 1]);
    assert!(ored.not_zero());
}

#[test]
fn bits_bit_block_word_ops_match_scalar_reference() {
    // Adapted from Stim v1.16.0 src/stim/mem/simd_word.test.cc bitwise and popcount checks.
    let left = BitBlock::from_words([0xFFFF_0000_FFFF_0000, 0x0123_4567_89AB_CDEF, 0, u64::MAX]);
    let right = BitBlock::from_words([
        0x00FF_00FF_00FF_00FF,
        0xFFFF_0000_FFFF_0000,
        u64::MAX,
        0x0F0F_0F0F_0F0F_0F0F,
    ]);

    assert_eq!(
        left.xor(right).words(),
        [
            0xFF00_00FF_FF00_00FF,
            0xFEDC_4567_7654_CDEF,
            u64::MAX,
            0xF0F0_F0F0_F0F0_F0F0,
        ]
    );
    assert_eq!(
        left.and(right).words(),
        [
            0x00FF_0000_00FF_0000,
            0x0123_0000_89AB_0000,
            0,
            0x0F0F_0F0F_0F0F_0F0F,
        ]
    );
    assert_eq!(
        left.or(right).popcount(),
        reference_popcount_words(&left.or(right).words())
    );
}

#[test]
fn bits_bit_util_scalar_helpers_cover_boundaries() {
    let mut left = BitVec::from_words_truncated(256, vec![0b1010_u64, 0b0101, u64::MAX, 0]);
    let right = BitVec::from_words_truncated(256, vec![0b1100_u64, 0b0011, 0, u64::MAX]);
    let mask = BitVec::from_words_truncated(256, vec![0b0110_u64, u64::MAX, u64::MAX, 0]);

    left.masked_xor_assign(&right.as_bitslice(), &mask.as_bitslice())
        .expect("masked xor");
    assert_eq!(left.words(), &[0b1110, 0b0110, u64::MAX, 0]);
    assert_eq!(left.popcount(), 69);
}

#[test]
fn bits_bitvec_multi_block_boundaries_match_reference() {
    for len in [255, 256, 257, 511, 512, 513, 1024, 1025] {
        let left_bools = patterned_bools(len, 3);
        let right_bools = patterned_bools(len, 5);
        let mask_bools = patterned_bools(len, 7);
        let left = bitvec_from_bools(&left_bools);
        let right = bitvec_from_bools(&right_bools);
        let mask = bitvec_from_bools(&mask_bools);

        let mut xor_actual = left.clone();
        xor_actual.xor_assign(&right.as_bitslice()).expect("xor");
        assert_eq!(
            bools_from_bitvec(&xor_actual),
            left_bools
                .iter()
                .zip(&right_bools)
                .map(|(left, right)| *left ^ *right)
                .collect::<Vec<_>>()
        );

        let mut masked_actual = left;
        masked_actual
            .masked_xor_assign(&right.as_bitslice(), &mask.as_bitslice())
            .expect("masked xor");
        assert_eq!(
            bools_from_bitvec(&masked_actual),
            xor_expected_with_mask(&left_bools, &right_bools, &mask_bools)
        );
    }
}

proptest! {
    #[test]
    fn bits_bitvec_simd_ops_match_scalar_reference(
        len in 0usize..257,
        left_seed in proptest::collection::vec(any::<bool>(), 0..257),
        right_seed in proptest::collection::vec(any::<bool>(), 0..257),
        mask_seed in proptest::collection::vec(any::<bool>(), 0..257),
    ) {
        let left_bools = resize_bools(left_seed, len);
        let right_bools = resize_bools(right_seed, len);
        let mask_bools = resize_bools(mask_seed, len);
        let left = bitvec_from_bools(&left_bools);
        let right = bitvec_from_bools(&right_bools);
        let mask = bitvec_from_bools(&mask_bools);

        let mut xor_actual = left.clone();
        xor_actual.xor_assign(&right.as_bitslice()).expect("xor");
        let xor_expected = left_bools.iter().zip(&right_bools).map(|(left, right)| *left ^ *right).collect::<Vec<_>>();
        prop_assert_eq!(bools_from_bitvec(&xor_actual), xor_expected);

        let mut and_actual = left.clone();
        and_actual.and_assign(&right.as_bitslice()).expect("and");
        let and_expected = left_bools.iter().zip(&right_bools).map(|(left, right)| *left & *right).collect::<Vec<_>>();
        prop_assert_eq!(bools_from_bitvec(&and_actual), and_expected);

        let mut or_actual = left.clone();
        or_actual.or_assign(&right.as_bitslice()).expect("or");
        let or_expected = left_bools.iter().zip(&right_bools).map(|(left, right)| *left | *right).collect::<Vec<_>>();
        prop_assert_eq!(bools_from_bitvec(&or_actual), or_expected);

        let mut masked_actual = left;
        masked_actual
            .masked_xor_assign(&right.as_bitslice(), &mask.as_bitslice())
            .expect("masked xor");
        let masked_expected = xor_expected_with_mask(&left_bools, &right_bools, &mask_bools);
        prop_assert_eq!(bools_from_bitvec(&masked_actual), masked_expected);
    }

    #[test]
    fn bits_range_xor_matches_scalar_reference(
        len in 1usize..257,
        target_seed in proptest::collection::vec(any::<bool>(), 1..257),
        source_seed in proptest::collection::vec(any::<bool>(), 1..257),
        raw_target_start in 0usize..257,
        raw_source_start in 0usize..257,
        raw_count in 0usize..257,
    ) {
        let target_bools = resize_bools(target_seed, len);
        let source_bools = resize_bools(source_seed, len);
        let target_start = raw_target_start % len;
        let source_start = raw_source_start % len;
        let max_count = (len - target_start).min(len - source_start);
        let count = raw_count % (max_count + 1);

        let mut actual = bitvec_from_bools(&target_bools);
        let source = bitvec_from_bools(&source_bools);
        actual
            .xor_range_from(target_start, &source.as_bitslice(), source_start, count)
            .expect("range xor");

        let mut expected = target_bools;
        for offset in 0..count {
            if source_bools[source_start + offset] {
                let target = target_start + offset;
                expected[target] = !expected[target];
            }
        }
        prop_assert_eq!(bools_from_bitvec(&actual), expected);
    }

    #[test]
    fn bits_bit_matrix_transpose_matches_scalar_reference(
        rows in 0usize..18,
        cols in 0usize..130,
        set_seed in proptest::collection::vec((0usize..18, 0usize..130), 0..80),
    ) {
        let mut matrix = BitMatrix::zeros(rows, cols).expect("matrix");
        let mut expected = vec![vec![false; cols]; rows];
        for (row, col) in set_seed {
            if row < rows && col < cols {
                matrix.set(row, col, true).expect("set matrix bit");
                expected[row][col] = true;
            }
        }

        let transposed = matrix.transpose().expect("transpose");
        prop_assert_eq!(transposed.rows(), cols);
        prop_assert_eq!(transposed.cols(), rows);
        for row in 0..rows {
            for col in 0..cols {
                prop_assert_eq!(transposed.get(col, row), Some(expected[row][col]));
            }
        }
    }

    #[test]
    fn bits_bit_matrix_masked_row_xor_matches_scalar_reference(
        cols in 0usize..257,
        source_seed in proptest::collection::vec(any::<bool>(), 0..257),
        target_seed in proptest::collection::vec(any::<bool>(), 0..257),
        mask_seed in proptest::collection::vec(any::<bool>(), 0..257),
    ) {
        let source_bools = resize_bools(source_seed, cols);
        let target_bools = resize_bools(target_seed, cols);
        let mask_bools = resize_bools(mask_seed, cols);
        let mut matrix = BitMatrix::zeros(2, cols).expect("matrix");
        set_matrix_row(&mut matrix, 0, &source_bools);
        set_matrix_row(&mut matrix, 1, &target_bools);
        let mask = bitvec_from_bools(&mask_bools);

        matrix
            .masked_xor_row_into(0, 1, &mask.as_bitslice())
            .expect("masked row xor");

        let expected = xor_expected_with_mask(&target_bools, &source_bools, &mask_bools);
        for (col, expected) in expected.into_iter().enumerate() {
            prop_assert_eq!(matrix.get(1, col), Some(expected));
        }
    }

    #[test]
    fn bits_sparse_xor_vec_matches_symmetric_difference(
        left in proptest::collection::vec(0u32..80, 0..80),
        right in proptest::collection::vec(0u32..80, 0..80),
    ) {
        let mut actual = SparseXorVec::from_sorted_items(left.clone());
        let rhs = SparseXorVec::from_sorted_items(right.clone());
        actual.xor_assign(&rhs);

        let expected = symmetric_difference_reference(&inplace_reference(left), &inplace_reference(right));
        prop_assert_eq!(actual.items(), expected.as_slice());
    }
}

#[test]
fn bits_bit_matrix_row_operations_match_stim_bit_table_cases() {
    // Adapted from Stim v1.16.0 src/stim/mem/simd_bit_table.test.cc row operations.
    let mut matrix = BitMatrix::zeros(6, 500).expect("matrix");
    matrix.set(0, 10, true).expect("set");
    matrix.set(0, 490, true).expect("set");
    matrix.set(5, 490, true).expect("set");
    matrix.xor_row_into(0, 1).expect("xor row");
    matrix.xor_row_into(5, 1).expect("xor row");
    assert_eq!(matrix.get(1, 10), Some(true));
    assert_eq!(matrix.get(1, 490), Some(false));

    let mut mask = BitVec::zeros(500);
    mask.set(10, true).expect("set mask");
    matrix
        .masked_xor_row_into(0, 2, &mask.as_bitslice())
        .expect("masked row xor");
    assert_eq!(matrix.get(2, 10), Some(true));
    assert_eq!(matrix.get(2, 490), Some(false));

    matrix.swap_rows(0, 5).expect("swap rows");
    assert_eq!(matrix.get(5, 10), Some(true));
    assert_eq!(matrix.get(0, 490), Some(true));

    let mut identity = BitMatrix::identity(4).expect("identity");
    identity.set(3, 1, true).expect("set");
    let mut transposed = identity.clone();
    transposed
        .transpose_square_in_place()
        .expect("square transpose");
    assert_eq!(transposed.get(1, 3), Some(true));
    transposed
        .transpose_square_in_place()
        .expect("square transpose");
    assert_eq!(transposed, identity);
}

#[test]
fn bits_bit_matrix_rejects_storage_size_overflow() {
    assert!(BitMatrix::zeros(usize::MAX, 65).is_err());
}

#[test]
fn bits_sparse_xor_vec_ports_stim_examples() {
    // Adapted from Stim v1.16.0 src/stim/mem/sparse_xor_vec.test.cc.
    let mut left = SparseXorVec::new();
    left.xor_item(1);
    left.xor_item(3);
    let mut right = SparseXorVec::new();
    right.xor_item(3);
    right.xor_item(2);
    left.xor_assign(&right);
    assert_eq!(left.items(), &[1, 2]);
    assert_eq!(right.items(), &[2, 3]);

    left.xor_item(2);
    left.xor_item(6);
    left.xor_item(9);
    left.xor_item(2);
    assert_eq!(left.items(), &[1, 2, 6, 9]);
    assert!(left.contains(6));
    assert!(left.is_superset_of(&[1, 9]));
    assert!(!left.is_superset_of(&[1, 3]));
    assert_eq!(left.to_string(), "SparseXorVec{1, 2, 6, 9}");

    assert_eq!(bits::inplace_xor_sort(vec![5, 4, 5, 5]), vec![4, 5]);
    assert_eq!(bits::inplace_xor_sort(vec![4, 5, 5, 4]), Vec::<u32>::new());
}

#[test]
fn bits_twiddle_helpers_match_upstream_examples() {
    // Adapted from Stim v1.16.0 src/stim/util_bot/twiddle.test.cc.
    let powers = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let expected = [
        false, true, true, false, true, false, false, false, true, false,
    ];
    for (value, expected) in powers.into_iter().zip(expected) {
        assert_eq!(bits::is_power_of_2(value), expected);
    }

    assert_eq!(bits::floor_lg2(1), Some(0));
    assert_eq!(bits::floor_lg2(2), Some(1));
    assert_eq!(bits::floor_lg2(3), Some(1));
    assert_eq!(bits::floor_lg2(9), Some(3));
    assert_eq!(bits::floor_lg2(0), None);

    let value = 0b0001_1100_1000_u64;
    assert_eq!(bits::first_set_bit(value, 0), Some(3));
    assert_eq!(bits::first_set_bit(value, 4), Some(6));
    assert_eq!(bits::first_set_bit(value, 8), Some(8));
    assert_eq!(bits::first_set_bit(0, 0), None);
}

fn bitvec_from_bools(bits: &[bool]) -> BitVec {
    let mut out = BitVec::zeros(bits.len());
    for (index, bit) in bits.iter().enumerate() {
        out.set(index, *bit).expect("set bit");
    }
    out
}

fn bools_from_bitvec(bits: &BitVec) -> Vec<bool> {
    (0..bits.len())
        .map(|index| bits.get(index).unwrap_or(false))
        .collect()
}

fn resize_bools(mut bits: Vec<bool>, len: usize) -> Vec<bool> {
    bits.resize(len, false);
    bits
}

fn patterned_bools(len: usize, seed: usize) -> Vec<bool> {
    (0..len)
        .map(|index| (index.wrapping_mul(17).wrapping_add(seed) % 11) < 5)
        .collect()
}

fn set_matrix_row(matrix: &mut BitMatrix, row: usize, bits: &[bool]) {
    for (col, bit) in bits.iter().enumerate() {
        matrix.set(row, col, *bit).expect("set matrix bit");
    }
}

fn xor_expected_with_mask(left: &[bool], right: &[bool], mask: &[bool]) -> Vec<bool> {
    left.iter()
        .zip(right)
        .zip(mask)
        .map(|((left, right), mask)| *left ^ (*right & *mask))
        .collect()
}

fn inplace_reference(items: Vec<u32>) -> Vec<u32> {
    let mut set = BTreeSet::new();
    for item in items {
        if !set.insert(item) {
            set.remove(&item);
        }
    }
    set.into_iter().collect()
}

fn symmetric_difference_reference(left: &[u32], right: &[u32]) -> Vec<u32> {
    let left = left.iter().copied().collect::<BTreeSet<_>>();
    let right = right.iter().copied().collect::<BTreeSet<_>>();
    left.symmetric_difference(&right).copied().collect()
}

fn reference_popcount_words(words: &[u64]) -> usize {
    words.iter().map(|word| word.count_ones() as usize).sum()
}
