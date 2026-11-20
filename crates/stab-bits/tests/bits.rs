#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::needless_range_loop,
    reason = "bit-kernel property tests construct in-range fixtures and direct assertions keep failing cases readable"
)]

use proptest::prelude::*;
use stab_bits::{BitBlock, BitError, BitLen, BitMatrix, BitSlice, BitVec};

const WORD_BITS: usize = u64::BITS as usize;

/// Word, SIMD-block, and multi-block edges that every width-driven property must visit.
const BOUNDARY_WIDTHS: &[usize] = &[
    0, 1, 63, 64, 65, 127, 128, 129, 255, 256, 257, 511, 512, 513, 1023, 1024, 1025,
];

fn bit_width() -> impl Strategy<Value = usize> {
    prop_oneof![proptest::sample::select(BOUNDARY_WIDTHS), 0_usize..1100]
}

/// Source words for one operand, including dirty tail bits and surplus words that
/// `BitVec::from_words_truncated` must discard.
fn padded_words(width: usize) -> impl Strategy<Value = Vec<u64>> {
    let count = width.div_ceil(WORD_BITS);
    proptest::collection::vec(any::<u64>(), count..=count + 2)
}

fn width_with_three_operands() -> impl Strategy<Value = (usize, Vec<u64>, Vec<u64>, Vec<u64>)> {
    bit_width().prop_flat_map(|width| {
        (
            Just(width),
            padded_words(width),
            padded_words(width),
            padded_words(width),
        )
    })
}

proptest! {
    #[test]
    fn bits_bitvec_truncation_views_and_access_match_words(
        (len, words) in bit_width().prop_flat_map(|width| (Just(width), padded_words(width))),
    ) {
        // Adapted from Stim v1.16.0 src/stim/mem/bit_ref.test.cc and simd_bits.test.cc boundary cases.
        let bit_len = BitLen::new(len);
        prop_assert_eq!(bit_len.get(), len);
        prop_assert_eq!(bit_len.word_count(), len.div_ceil(WORD_BITS));
        prop_assert_eq!(BitLen::from(len), bit_len);

        let mut bits = BitVec::from_words_truncated(len, words.clone());
        prop_assert_eq!(bits.len(), len);
        prop_assert_eq!(bits.is_empty(), len == 0);
        assert_matches_reference(&bits, |index| word_bit(&words, index));

        let view = bits.as_bitslice();
        prop_assert_eq!(view.len(), len);
        prop_assert_eq!(view.is_empty(), len == 0);
        prop_assert_eq!(view.words(), bits.words());
        prop_assert_eq!(view.popcount(), bits.popcount());
        for index in 0..=len {
            prop_assert_eq!(view.get(index), bits.get(index));
        }
        // A view over the dirty source words must iterate only its logical bits.
        let dirty_view = BitSlice::new(&words[..len.div_ceil(WORD_BITS)], len)
            .expect("dirty-tail view");
        let expected = (0..len).map(|index| word_bit(&words, index)).collect::<Vec<_>>();
        prop_assert_eq!(dirty_view.iter().len(), len);
        prop_assert_eq!(dirty_view.iter().collect::<Vec<_>>(), expected.clone());
        prop_assert_eq!(
            bits.iter().rev().collect::<Vec<_>>(),
            expected.iter().rev().copied().collect::<Vec<_>>()
        );
        for skip in [0, 1, 63, 64, 65, len / 2, len] {
            prop_assert_eq!(bits.iter().nth(skip), expected.get(skip).copied());
        }
        let mut copied = BitVec::zeros(len);
        copied.copy_from_bitslice(&view).expect("same-width copy");
        prop_assert_eq!(&copied, &bits);

        for index in [0, len / 2, len.saturating_sub(1)] {
            if index < len {
                let original = word_bit(&words, index);
                bits.set(index, !original).expect("toggle in-range bit");
                prop_assert_eq!(bits.get(index), Some(!original));
                bits.set(index, original).expect("restore in-range bit");
            }
        }
        prop_assert_eq!(&bits, &copied);
        prop_assert_eq!(
            bits.set(len, true),
            Err(BitError::BitIndexOutOfRange { index: len, len })
        );
    }

    #[test]
    fn bits_bitvec_logical_ops_match_scalar_reference(
        (len, left_words, right_words, mask_words) in width_with_three_operands(),
    ) {
        let left = BitVec::from_words_truncated(len, left_words.clone());
        let right = BitVec::from_words_truncated(len, right_words.clone());
        let mask = BitVec::from_words_truncated(len, mask_words.clone());
        let l = |index| word_bit(&left_words, index);
        let r = |index| word_bit(&right_words, index);
        let m = |index| word_bit(&mask_words, index);

        let mut xored = left.clone();
        xored.xor_assign(&right.as_bitslice()).expect("same-width XOR");
        assert_matches_reference(&xored, |index| l(index) ^ r(index));

        let mut anded = left.clone();
        anded.and_assign(&right.as_bitslice()).expect("same-width AND");
        assert_matches_reference(&anded, |index| l(index) & r(index));

        let mut ored = left.clone();
        ored.or_assign(&right.as_bitslice()).expect("same-width OR");
        assert_matches_reference(&ored, |index| l(index) | r(index));

        let mut masked = left;
        masked
            .masked_xor_assign(&right.as_bitslice(), &mask.as_bitslice())
            .expect("same-width masked XOR");
        assert_matches_reference(&masked, |index| l(index) ^ (r(index) & m(index)));

        masked.clear();
        assert_matches_reference(&masked, |_| false);
    }

    #[test]
    fn bits_range_xor_matches_scalar_reference(
        (len, target_words, source_words) in bit_width()
            .prop_flat_map(|width| (Just(width), padded_words(width), padded_words(width))),
        raw_target_start in 0_usize..2048,
        raw_source_start in 0_usize..2048,
        raw_count in 0_usize..2048,
    ) {
        let target_start = raw_target_start % (len + 1);
        let source_start = raw_source_start % (len + 1);
        let max_count = (len - target_start).min(len - source_start);
        let count = raw_count % (max_count + 1);

        let mut actual = BitVec::from_words_truncated(len, target_words.clone());
        let source = BitVec::from_words_truncated(len, source_words.clone());
        actual
            .xor_range_from(target_start, &source.as_bitslice(), source_start, count)
            .expect("range xor");

        assert_matches_reference(&actual, |index| {
            let flipped = (target_start..target_start + count).contains(&index)
                && word_bit(&source_words, index - target_start + source_start);
            word_bit(&target_words, index) ^ flipped
        });
    }

    #[test]
    fn bits_bit_block_word_ops_match_scalar_reference(
        left_words in any::<[u64; 4]>(),
        right_words in any::<[u64; 4]>(),
    ) {
        // Adapted from Stim v1.16.0 src/stim/mem/simd_word.test.cc bitwise and popcount checks.
        let left = BitBlock::from_words(left_words);
        let right = BitBlock::from_words(right_words);
        let xor: [u64; 4] = std::array::from_fn(|index| left_words[index] ^ right_words[index]);
        let and: [u64; 4] = std::array::from_fn(|index| left_words[index] & right_words[index]);
        let or: [u64; 4] = std::array::from_fn(|index| left_words[index] | right_words[index]);

        prop_assert_eq!(left.words(), left_words);
        prop_assert_eq!(left.xor(right).words(), xor);
        prop_assert_eq!(left.and(right).words(), and);
        prop_assert_eq!(left.or(right).words(), or);
        prop_assert_eq!((left ^ right).words(), xor);
        prop_assert_eq!((left & right).words(), and);
        prop_assert_eq!((left | right).words(), or);
        prop_assert_eq!((!left).words(), left_words.map(|word| !word));
        prop_assert_eq!(left.popcount(), reference_popcount_words(&left_words));

        let mut assigned = left;
        assigned ^= right;
        prop_assert_eq!(assigned.words(), xor);
        assigned = left;
        assigned &= right;
        prop_assert_eq!(assigned.words(), and);
        assigned = left;
        assigned |= right;
        prop_assert_eq!(assigned.words(), or);
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
    fn bits_bit_matrix_row_ops_match_scalar_reference(
        (cols, source_words, target_words, mask_words) in width_with_three_operands(),
    ) {
        let s = |col| word_bit(&source_words, col);
        let t = |col| word_bit(&target_words, col);
        let m = |col| word_bit(&mask_words, col);
        let mut matrix = BitMatrix::zeros(3, cols).expect("matrix");
        prop_assert_eq!(matrix.rows(), 3);
        prop_assert_eq!(matrix.cols(), cols);
        set_matrix_row(&mut matrix, 0, s);
        set_matrix_row(&mut matrix, 1, t);
        let mask = BitVec::from_words_truncated(cols, mask_words.clone());

        matrix.xor_row_into(0, 1).expect("row XOR");
        assert_row(&matrix, 1, |col| t(col) ^ s(col));
        matrix
            .masked_xor_row_into(0, 1, &mask.as_bitslice())
            .expect("masked row XOR");
        let combined = |col| t(col) ^ s(col) ^ (s(col) & m(col));
        assert_row(&matrix, 1, combined);
        matrix.swap_rows(0, 1).expect("row swap");
        assert_row(&matrix, 0, combined);
        assert_row(&matrix, 1, s);

        // Aliased source and target rows must follow Stim's in-place semantics.
        set_matrix_row(&mut matrix, 2, s);
        matrix.xor_row_into(2, 2).expect("self row XOR");
        assert_row(&matrix, 2, |_| false);
        set_matrix_row(&mut matrix, 2, s);
        matrix
            .masked_xor_row_into(2, 2, &mask.as_bitslice())
            .expect("self masked row XOR");
        assert_row(&matrix, 2, |col| s(col) & !m(col));
    }
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

    let one_word = [0_u64];
    assert_eq!(
        BitSlice::new(&one_word, 65),
        Err(BitError::LengthMismatch {
            left: 64,
            right: 65
        })
    );
    let empty_words: [u64; 0] = [];
    assert!(BitSlice::new(&empty_words, 0).is_ok());
    assert_ne!(BitVec::zeros(64), BitVec::zeros(65));
    let zero_65 = [0_u64, 0];
    assert_ne!(
        BitSlice::new(&one_word, 64).expect("64-bit view"),
        BitSlice::new(&zero_65, 65).expect("65-bit view")
    );
}

#[test]
fn bits_bitvec_owned_storage_contract_preserves_tail_and_resize_semantics() {
    let mut bits = BitVec::from_bits([true, false, true]);
    assert_eq!(bits.len(), 3);
    assert_eq!(bits.words(), &[0b101]);

    bits.resize_zeros(65);
    assert_eq!(bits.len(), 65);
    assert_eq!(bits.get(0), Some(true));
    assert_eq!(bits.get(1), Some(false));
    assert_eq!(bits.get(2), Some(true));
    assert_eq!(bits.get(64), Some(false));

    {
        let mut words = bits.words_mut();
        words[0] = u64::MAX;
        words[1] = u64::MAX;
    }
    assert_eq!(bits.words(), &[u64::MAX, 1]);
    assert_eq!(bits.popcount(), 65);

    bits.resize_zeros(2);
    assert_eq!(bits.words(), &[0b11]);
    assert_eq!(bits.popcount(), 2);
}

/// Hostile lengths get a typed refusal instead of an allocator abort, a refused resize leaves the
/// vector as it was, and the fallible forms build exactly what the infallible ones do.
#[test]
fn bits_bitvec_fallible_allocation_refuses_without_side_effects() {
    let huge_words = usize::MAX.div_ceil(WORD_BITS);
    assert_eq!(
        BitVec::try_zeros(usize::MAX),
        Err(BitError::StorageAllocationFailed { words: huge_words })
    );
    let mut bits = BitVec::from_bits([true, false, true]);
    assert_eq!(
        bits.try_resize_zeros(usize::MAX),
        Err(BitError::StorageAllocationFailed { words: huge_words })
    );
    assert_eq!(bits, BitVec::from_bits([true, false, true]));

    assert_eq!(BitVec::try_zeros(130), Ok(BitVec::zeros(130)));
    let mut expected = bits.clone();
    expected.resize_zeros(65);
    bits.try_resize_zeros(65).expect("small resize");
    assert_eq!(bits, expected);
    bits.words_mut()[0] = u64::MAX;
    expected.words_mut()[0] = u64::MAX;
    expected.resize_zeros(2);
    bits.try_resize_zeros(2).expect("shrink");
    assert_eq!(bits, expected);
}

#[test]
fn bits_width_and_range_mismatches_are_rejected_without_mutation() {
    let mut left = BitVec::from_words_truncated(257, vec![0x55aa_1234_9876_cdef; 5]);
    let before = left.clone();
    let right = BitVec::from_words_truncated(256, vec![0xf0f0_0f0f_dead_beef; 4]);
    let same_width = BitVec::zeros(257);
    let mismatch = Err(BitError::LengthMismatch {
        left: 257,
        right: 256,
    });

    assert_eq!(left.xor_assign(&right.as_bitslice()), mismatch);
    assert_eq!(left.and_assign(&right.as_bitslice()), mismatch);
    assert_eq!(left.or_assign(&right.as_bitslice()), mismatch);
    assert_eq!(
        left.masked_xor_assign(&same_width.as_bitslice(), &right.as_bitslice()),
        mismatch
    );
    assert_eq!(left.copy_from_bitslice(&right.as_bitslice()), mismatch);
    assert_eq!(left, before);

    let source = BitVec::zeros(65);
    let mut target = BitVec::from_words_truncated(65, vec![u64::MAX; 2]);
    let target_before = target.clone();
    assert_eq!(
        target.xor_range_from(64, &source.as_bitslice(), 0, 2),
        Err(BitError::BitRangeOutOfRange {
            start: 64,
            end: 66,
            len: 65
        })
    );
    assert_eq!(
        target.xor_range_from(0, &source.as_bitslice(), 64, 2),
        Err(BitError::BitRangeOutOfRange {
            start: 64,
            end: 66,
            len: 65
        })
    );
    assert_eq!(
        target.xor_range_from(usize::MAX, &source.as_bitslice(), 0, 1),
        Err(BitError::BitRangeOutOfRange {
            start: usize::MAX,
            end: usize::MAX,
            len: 65
        })
    );
    assert_eq!(target, target_before);
}

#[test]
fn bits_range_xor_word_chunk_edges_match_reference() {
    for (len, target_start, source_start, count) in [
        (130, 0, 0, 130),
        (130, 1, 1, 127),
        (257, 31, 17, 196),
        (257, 64, 3, 128),
        (257, 7, 64, 129),
        (257, 193, 191, 2),
        (8192, 31, 17, 4096),
        (4099, 4098, 4098, 1),
    ] {
        let target_bools = patterned_bools(len, 3);
        let source_bools = patterned_bools(len, 5);
        let mut actual = bitvec_from_bools(&target_bools);
        let source = bitvec_from_bools(&source_bools);

        actual
            .xor_range_from(target_start, &source.as_bitslice(), source_start, count)
            .expect("range xor");

        let mut expected = target_bools;
        for offset in 0..count {
            if source_bools[source_start + offset] {
                let target = target_start + offset;
                expected[target].toggle();
            }
        }
        assert_eq!(bools_from_bitvec(&actual), expected);
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

    let identity = BitMatrix::identity(65).expect("identity matrix");
    for row in 0..65 {
        for col in 0..65 {
            assert_eq!(identity.get(row, col), Some(row == col));
        }
    }
}

#[test]
fn bits_bit_matrix_rejects_out_of_range_access_and_oversized_shapes() {
    assert_eq!(
        BitMatrix::zeros(usize::MAX, 65),
        Err(BitError::MatrixSizeOverflow {
            rows: usize::MAX,
            cols: 65
        })
    );
    assert!(BitMatrix::zeros(1, usize::MAX).is_err());

    let mut matrix = BitMatrix::zeros(2, 65).expect("matrix");
    assert_eq!(
        matrix.row(2),
        Err(BitError::RowIndexOutOfRange { row: 2, rows: 2 })
    );
    assert_eq!(matrix.get(2, 0), None);
    assert_eq!(matrix.get(0, 65), None);
    assert_eq!(
        matrix.set(0, 65, true),
        Err(BitError::BitIndexOutOfRange { index: 65, len: 65 })
    );
    let short_mask = BitVec::zeros(64);
    assert_eq!(
        matrix.masked_xor_row_into(0, 1, &short_mask.as_bitslice()),
        Err(BitError::LengthMismatch {
            left: 65,
            right: 64
        })
    );
}

#[test]
fn bits_bit_matrix_copies_complete_rows_and_rejects_width_mismatches() {
    let first = patterned_bools(130, 3);
    let second = patterned_bools(130, 9);
    let mut matrix = BitMatrix::zeros(2, 130).expect("matrix");

    matrix
        .copy_row_from_bools(1, &first)
        .expect("copy first row");
    for (column, expected) in first.iter().copied().enumerate() {
        assert_eq!(matrix.get(1, column), Some(expected));
    }

    matrix
        .copy_row_from_bools(1, &second)
        .expect("replace complete row");
    for (column, expected) in second.iter().copied().enumerate() {
        assert_eq!(matrix.get(1, column), Some(expected));
    }

    assert!(matrix.copy_row_from_bools(1, &second[..129]).is_err());
    assert!(matrix.copy_row_from_bools(2, &second).is_err());
}

#[test]
fn bits_bit_matrix_copies_packed_bytes_at_word_boundaries() {
    for cols in [1_usize, 7, 8, 9, 63, 64, 65, 127, 128, 129, 257] {
        let bytes = (0..cols.div_ceil(8))
            .map(|index| {
                u8::try_from(index)
                    .expect("fixture byte index fits u8")
                    .wrapping_mul(37)
                    .wrapping_add(0xa5)
            })
            .collect::<Vec<_>>();
        let mut matrix = BitMatrix::zeros(2, cols).expect("packed matrix");
        matrix
            .copy_row_from_le_bytes(1, &bytes)
            .expect("copy packed row");
        for bit_index in 0..cols {
            let expected = bytes[bit_index / 8] & (1 << (bit_index % 8)) != 0;
            assert_eq!(
                matrix.get(1, bit_index),
                Some(expected),
                "{cols}:{bit_index}"
            );
        }

        matrix
            .copy_row_from_le_bytes(1, &vec![0; bytes.len()])
            .expect("replace packed row");
        assert!((0..cols).all(|bit_index| matrix.get(1, bit_index) == Some(false)));
    }
}

/// The transpose property stays below 18 rows, so these fixed shapes own the
/// 64-row tile crossings and the square in-place kernel.
#[test]
fn bits_bit_matrix_transpose_crosses_64_row_tiles() {
    for (rows, cols) in [
        (0, 65),
        (65, 0),
        (63, 65),
        (64, 64),
        (65, 63),
        (128, 129),
        (255, 257),
        (256, 256),
        (257, 255),
    ] {
        let matrix = patterned_matrix(rows, cols);
        let original = matrix.clone();
        let transposed = matrix.transpose().expect("transpose");
        assert_eq!(matrix, original, "allocating transpose mutated its source");
        assert_eq!(transposed.rows(), cols);
        assert_eq!(transposed.cols(), rows);
        for row in 0..rows {
            for col in 0..cols {
                assert_eq!(transposed.get(col, row), Some(matrix_pattern(row, col)));
            }
        }
        assert_eq!(transposed.transpose().expect("double transpose"), matrix);
    }

    for &size in &[0, 1, 63, 64, 65, 129, 255, 256, 257] {
        let mut matrix = patterned_matrix(size, size);
        let original = matrix.clone();
        let expected = original
            .transpose()
            .expect("allocating reference transpose");
        matrix
            .transpose_square_in_place()
            .expect("square transpose");
        assert_eq!(matrix, expected, "in-place transpose size={size}");
        matrix
            .transpose_square_in_place()
            .expect("square double transpose");
        assert_eq!(matrix, original);
    }

    let mut rectangular = BitMatrix::zeros(2, 3).expect("rectangular matrix");
    rectangular.set(1, 2, true).expect("set rectangular bit");
    let rectangular_before = rectangular.clone();
    assert_eq!(
        rectangular.transpose_square_in_place(),
        Err(BitError::NotSquareMatrix { rows: 2, cols: 3 })
    );
    assert_eq!(rectangular, rectangular_before);
}

#[test]
fn bits_preallocated_kernels_do_not_allocate() {
    let mut bits = BitVec::from_words_truncated(4093, vec![0x55aa_1234_9876_cdef; 64]);
    let rhs = BitVec::from_words_truncated(4093, vec![0xf0f0_0f0f_dead_beef; 64]);
    let mask = BitVec::from_words_truncated(4093, vec![0x0123_4567_89ab_cdef; 64]);
    let mut matrix = patterned_matrix(3, 4093);
    let mut square = BitMatrix::zeros(256, 256).expect("in-place allocation fixture");
    for row in 0..256 {
        square
            .set(row, (row * 73 + 19) % 256, true)
            .expect("set in-place fixture bit");
    }

    let allocations = allocation_counter::measure(|| {
        for _ in 0..128 {
            bits.xor_assign(&rhs.as_bitslice()).expect("XOR");
            bits.and_assign(&rhs.as_bitslice()).expect("AND");
            bits.or_assign(&rhs.as_bitslice()).expect("OR");
            bits.masked_xor_assign(&rhs.as_bitslice(), &mask.as_bitslice())
                .expect("masked XOR");
            std::hint::black_box(bits.words());

            matrix.xor_row_into(0, 1).expect("row XOR");
            matrix
                .masked_xor_row_into(0, 1, &mask.as_bitslice())
                .expect("masked row XOR");
            matrix
                .masked_xor_row_into(2, 2, &mask.as_bitslice())
                .expect("self masked row XOR");
            matrix.swap_rows(0, 1).expect("row swap");
            std::hint::black_box(matrix.row(2).expect("row view"));
        }
        square
            .transpose_square_in_place()
            .expect("in-place transpose");
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");

    let allocating = BitMatrix::zeros(256, 256).expect("allocating transpose fixture");
    let allocating_allocations = allocation_counter::measure(|| {
        std::hint::black_box(allocating.transpose().expect("allocating transpose"));
    });
    assert_eq!(
        allocating_allocations.count_total, 1,
        "allocating transpose must allocate one result buffer: {allocating_allocations:?}"
    );
    assert_eq!(
        allocating_allocations.bytes_total,
        256 * 256 / 8,
        "allocating transpose requested unexpected storage: {allocating_allocations:?}"
    );
}

fn word_bit(words: &[u64], index: usize) -> bool {
    words
        .get(index / WORD_BITS)
        .is_some_and(|word| word >> (index % WORD_BITS) & 1 == 1)
}

/// Checks every logical bit, the out-of-range accessor, storage size, canonical
/// tail padding, and the popcount/not-zero summaries against a scalar reference.
fn assert_matches_reference(actual: &BitVec, expected: impl Fn(usize) -> bool) {
    let len = actual.len();
    for index in 0..len {
        assert_eq!(actual.get(index), Some(expected(index)), "index={index}");
    }
    assert_eq!(actual.get(len), None);
    assert_eq!(actual.word_count(), len.div_ceil(WORD_BITS));
    let tail = len % WORD_BITS;
    if tail != 0 {
        let last = actual.words().last().copied().expect("non-empty tail word");
        assert_eq!(last >> tail, 0, "dirty padding above logical width {len}");
    }
    let ones = (0..len).filter(|index| expected(*index)).count();
    assert_eq!(actual.popcount(), ones);
    assert_eq!(actual.not_zero(), ones != 0);
}

fn set_matrix_row(matrix: &mut BitMatrix, row: usize, value: impl Fn(usize) -> bool) {
    for col in 0..matrix.cols() {
        matrix.set(row, col, value(col)).expect("set matrix bit");
    }
}

fn assert_row(matrix: &BitMatrix, row: usize, expected: impl Fn(usize) -> bool) {
    let view = matrix.row(row).expect("matrix row");
    assert_eq!(view.len(), matrix.cols());
    for col in 0..matrix.cols() {
        assert_eq!(view.get(col), Some(expected(col)), "row={row} col={col}");
    }
}

fn patterned_matrix(rows: usize, cols: usize) -> BitMatrix {
    let mut matrix = BitMatrix::zeros(rows, cols).expect("bounded matrix");
    for row in 0..rows {
        for col in 0..cols {
            if matrix_pattern(row, col) {
                matrix.set(row, col, true).expect("set matrix bit");
            }
        }
    }
    matrix
}

fn matrix_pattern(row: usize, col: usize) -> bool {
    row.wrapping_mul(29)
        .wrapping_add(col.wrapping_mul(43))
        .wrapping_add(row ^ col)
        % 19
        < 8
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

fn patterned_bools(len: usize, seed: usize) -> Vec<bool> {
    (0..len)
        .map(|index| (index.wrapping_mul(17).wrapping_add(seed) % 11) < 5)
        .collect()
}

fn reference_popcount_words(words: &[u64]) -> usize {
    words.iter().map(|word| word.count_ones() as usize).sum()
}
