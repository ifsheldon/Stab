#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "qualification fixtures use reviewed in-range indexes and explicit failures keep corpus diagnostics readable"
)]

use std::collections::BTreeSet;

use stab_core::bits::{self, BIT_BLOCK_WORDS};
use stab_core::{BitBlock, BitError, BitLen, BitMatrix, BitSlice, BitVec, SparseXorVec};

const WORD_BITS: usize = u64::BITS as usize;
const BOUNDARY_WIDTHS: &[usize] = &[
    0, 1, 2, 7, 8, 31, 32, 63, 64, 65, 127, 128, 129, 191, 192, 193, 255, 256, 257, 511, 512, 513,
    1023, 1024, 1025, 4093, 65_537,
];

#[test]
fn cq2_bit_len_slice_and_access_contract() {
    for &width in BOUNDARY_WIDTHS {
        let bit_len = BitLen::new(width);
        assert_eq!(bit_len.get(), width, "width={width}");
        assert_eq!(
            bit_len.word_count(),
            width.div_ceil(WORD_BITS),
            "width={width}"
        );
        assert_eq!(BitLen::from(width), bit_len, "width={width}");

        let source_words = patterned_words(width.div_ceil(WORD_BITS).saturating_add(2), 0xA5);
        let mut owned = BitVec::from_words_truncated(width, source_words.clone());
        assert_eq!(owned.len(), width, "width={width}");
        assert_eq!(owned.is_empty(), width == 0, "width={width}");
        assert_eq!(
            owned.word_count(),
            width.div_ceil(WORD_BITS),
            "width={width}"
        );
        assert_canonical_tail(&owned);

        {
            let view = owned.as_bitslice();
            assert_eq!(view.len(), width, "width={width}");
            assert_eq!(view.is_empty(), width == 0, "width={width}");
            assert_eq!(view.words(), owned.words(), "width={width}");
            assert_eq!(view.popcount(), scalar_popcount(&owned), "width={width}");
            assert_eq!(view.get(width), None, "width={width}");
            for index in boundary_indexes(width) {
                let expected = source_words
                    .get(index / WORD_BITS)
                    .is_some_and(|word| word & (1_u64 << (index % WORD_BITS)) != 0);
                assert_eq!(
                    owned.get(index),
                    Some(expected),
                    "width={width} index={index}"
                );
                assert_eq!(
                    view.get(index),
                    Some(expected),
                    "width={width} index={index}"
                );
            }
        }

        for index in boundary_indexes(width) {
            let expected = source_words
                .get(index / WORD_BITS)
                .is_some_and(|word| word & (1_u64 << (index % WORD_BITS)) != 0);
            owned.set(index, !expected).expect("toggle in-range bit");
            assert_eq!(
                owned.get(index),
                Some(!expected),
                "width={width} index={index}"
            );
            owned.set(index, expected).expect("restore in-range bit");
        }
        assert_eq!(
            owned.set(width, true),
            Err(BitError::BitIndexOutOfRange {
                index: width,
                len: width
            }),
            "width={width}"
        );

        let mut clone = owned.clone();
        if width > 0 {
            let original = owned.get(0).expect("first bit");
            clone.set(0, !original).expect("mutate clone");
            assert_eq!(
                owned.get(0),
                Some(original),
                "clone must own independent storage"
            );
            assert_ne!(clone, owned);
        } else {
            assert_eq!(clone, owned);
        }
    }

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
    let zero_64 = [0_u64];
    let zero_65 = [0_u64, 0];
    assert_ne!(
        BitSlice::new(&zero_64, 64).expect("64-bit view"),
        BitSlice::new(&zero_65, 65).expect("65-bit view")
    );
}

#[test]
fn cq2_bit_vec_logical_ops_match_scalar_across_boundaries() {
    for &width in BOUNDARY_WIDTHS {
        let left = patterned_bitvec(width, 0x17);
        let right = patterned_bitvec(width, 0xD3);
        let mask = patterned_bitvec(width, 0x69);

        let mut xored = left.clone();
        xored
            .xor_assign(&right.as_bitslice())
            .expect("same-width XOR");
        assert_bitwise(&xored, &left, &right, &mask, LogicalOp::Xor);

        let mut anded = left.clone();
        anded
            .and_assign(&right.as_bitslice())
            .expect("same-width AND");
        assert_bitwise(&anded, &left, &right, &mask, LogicalOp::And);

        let mut ored = left.clone();
        ored.or_assign(&right.as_bitslice()).expect("same-width OR");
        assert_bitwise(&ored, &left, &right, &mask, LogicalOp::Or);

        let mut masked = left.clone();
        masked
            .masked_xor_assign(&right.as_bitslice(), &mask.as_bitslice())
            .expect("same-width masked XOR");
        assert_bitwise(&masked, &left, &right, &mask, LogicalOp::MaskedXor);

        for actual in [&xored, &anded, &ored, &masked] {
            assert_canonical_tail(actual);
            assert_eq!(actual.popcount(), scalar_popcount(actual), "width={width}");
            assert_eq!(actual.not_zero(), actual.popcount() != 0, "width={width}");
        }

        let mut cleared = left;
        cleared.clear();
        assert_eq!(cleared.words(), vec![0; width.div_ceil(WORD_BITS)]);
        assert_eq!(cleared.popcount(), 0);
        assert!(!cleared.not_zero());
    }

    let mut width_65 = BitVec::zeros(65);
    let width_64 = BitVec::zeros(64);
    assert_eq!(
        width_65.xor_assign(&width_64.as_bitslice()),
        Err(BitError::LengthMismatch {
            left: 65,
            right: 64
        })
    );
    assert_eq!(
        width_65.and_assign(&width_64.as_bitslice()),
        Err(BitError::LengthMismatch {
            left: 65,
            right: 64
        })
    );
    assert_eq!(
        width_65.or_assign(&width_64.as_bitslice()),
        Err(BitError::LengthMismatch {
            left: 65,
            right: 64
        })
    );
    assert_eq!(
        width_65.masked_xor_assign(&width_65.clone().as_bitslice(), &width_64.as_bitslice()),
        Err(BitError::LengthMismatch {
            left: 65,
            right: 64
        })
    );

    let mut preallocated = patterned_bitvec(4093, 0x31);
    let rhs = patterned_bitvec(4093, 0x83);
    let mask = patterned_bitvec(4093, 0xC7);
    let allocations = allocation_counter::measure(|| {
        for _ in 0..128 {
            preallocated
                .xor_assign(&rhs.as_bitslice())
                .expect("preallocated XOR");
            preallocated
                .and_assign(&rhs.as_bitslice())
                .expect("preallocated AND");
            preallocated
                .or_assign(&rhs.as_bitslice())
                .expect("preallocated OR");
            preallocated
                .masked_xor_assign(&rhs.as_bitslice(), &mask.as_bitslice())
                .expect("preallocated masked XOR");
            std::hint::black_box(preallocated.words());
        }
    });
    assert_eq!(
        allocations.count_total, 0,
        "logical mutation allocated: {allocations:?}"
    );
    assert_eq!(
        allocations.bytes_total, 0,
        "logical mutation allocated: {allocations:?}"
    );
}

#[test]
fn cq2_bit_vec_copy_range_and_tail_contract() {
    for &width in BOUNDARY_WIDTHS {
        let source = patterned_bitvec(width, 0xC7);
        let mut copied = BitVec::zeros(width);
        copied
            .copy_from_bitslice(&source.as_bitslice())
            .expect("same-width copy");
        assert_eq!(copied, source, "width={width}");
        assert_canonical_tail(&copied);
    }

    let dirty_words = [u64::MAX, u64::MAX];
    let dirty = BitSlice::new(&dirty_words, 65).expect("dirty-tail view");
    assert_eq!(dirty.popcount(), 65);
    let mut copied = BitVec::zeros(65);
    copied
        .copy_from_bitslice(&dirty)
        .expect("copy dirty-tail view");
    assert_eq!(copied.words(), &[u64::MAX, 1]);

    for (width, target_start, source_start, count) in [
        (0, 0, 0, 0),
        (1, 0, 0, 1),
        (65, 0, 0, 65),
        (130, 1, 2, 127),
        (257, 31, 17, 196),
        (257, 64, 3, 128),
        (1025, 255, 511, 257),
        (4093, 2047, 1023, 1024),
    ] {
        let source = patterned_bitvec(width, 0x83);
        let mut actual = patterned_bitvec(width, 0x31);
        let mut expected = bits_as_bools(&actual);
        let source_bools = bits_as_bools(&source);
        for offset in 0..count {
            expected[target_start + offset] ^= source_bools[source_start + offset];
        }
        actual
            .xor_range_from(target_start, &source.as_bitslice(), source_start, count)
            .expect("bounded range XOR");
        assert_eq!(bits_as_bools(&actual), expected, "width={width}");
        assert_canonical_tail(&actual);
    }

    let source = BitVec::zeros(65);
    let mut target = BitVec::zeros(65);
    assert!(matches!(
        target.xor_range_from(64, &source.as_bitslice(), 0, 2),
        Err(BitError::BitRangeOutOfRange {
            start: 64,
            end: 66,
            len: 65
        })
    ));
    assert!(matches!(
        target.xor_range_from(0, &source.as_bitslice(), 64, 2),
        Err(BitError::BitRangeOutOfRange {
            start: 64,
            end: 66,
            len: 65
        })
    ));
    assert!(matches!(
        target.xor_range_from(usize::MAX, &source.as_bitslice(), 0, 1),
        Err(BitError::BitRangeOutOfRange {
            start: usize::MAX,
            end: usize::MAX,
            len: 65
        })
    ));
    assert_eq!(
        target.copy_from_bitslice(&BitVec::zeros(64).as_bitslice()),
        Err(BitError::LengthMismatch {
            left: 65,
            right: 64
        })
    );
}

#[test]
fn cq2_bit_block_contract_matches_scalar_words() {
    assert_eq!(BIT_BLOCK_WORDS, 4);
    assert_eq!(BitBlock::zero().words(), [0; BIT_BLOCK_WORDS]);

    for seed in [0_u64, 1, 0x55, 0xA5, u64::MAX] {
        let left_words = std::array::from_fn(|index| patterned_word(index, seed));
        let right_words = std::array::from_fn(|index| patterned_word(index, seed ^ 0xD7));
        let left = BitBlock::from_words(left_words);
        let right = BitBlock::from_words(right_words);
        assert_eq!(left.words(), left_words);
        assert_eq!(
            left.xor(right).words(),
            std::array::from_fn(|i| left_words[i] ^ right_words[i])
        );
        assert_eq!(
            left.and(right).words(),
            std::array::from_fn(|i| left_words[i] & right_words[i])
        );
        assert_eq!(
            left.or(right).words(),
            std::array::from_fn(|i| left_words[i] | right_words[i])
        );
        assert_eq!(
            left.popcount(),
            left_words
                .iter()
                .map(|word| word.count_ones() as usize)
                .sum()
        );
    }
}

#[test]
fn cq2_bit_matrix_row_contract_matches_scalar_reference() {
    for &cols in &[0, 1, 63, 64, 65, 255, 256, 257, 1025] {
        let mut matrix = BitMatrix::zeros(4, cols).expect("bounded matrix");
        assert_eq!(matrix, matrix.clone());
        assert_eq!(matrix.rows(), 4);
        assert_eq!(matrix.cols(), cols);
        let source = patterned_bools(cols, 0x19);
        let target = patterned_bools(cols, 0xB3);
        let mask = patterned_bools(cols, 0x67);
        set_row(&mut matrix, 0, &source);
        set_row(&mut matrix, 1, &target);

        matrix.xor_row_into(0, 1).expect("row XOR");
        assert_row(&matrix, 1, &xor_bools(&target, &source));
        matrix.swap_rows(0, 1).expect("row swap");
        assert_row(&matrix, 0, &xor_bools(&target, &source));
        assert_row(&matrix, 1, &source);

        let mask_bits = bitvec_from_bools(&mask);
        matrix
            .masked_xor_row_into(1, 2, &mask_bits.as_bitslice())
            .expect("masked row XOR");
        assert_row(&matrix, 2, &masked_source(&source, &mask));

        set_row(&mut matrix, 3, &source);
        matrix.xor_row_into(3, 3).expect("self row XOR");
        assert_row(&matrix, 3, &vec![false; cols]);
        set_row(&mut matrix, 3, &source);
        matrix
            .masked_xor_row_into(3, 3, &mask_bits.as_bitslice())
            .expect("self masked row XOR");
        let expected_self = source
            .iter()
            .zip(&mask)
            .map(|(source, mask)| *source & !*mask)
            .collect::<Vec<_>>();
        assert_row(&matrix, 3, &expected_self);

        assert_eq!(
            matrix.row(4),
            Err(BitError::RowIndexOutOfRange { row: 4, rows: 4 })
        );
        assert_eq!(matrix.get(4, 0), None);
        assert_eq!(matrix.get(0, cols), None);
        assert_eq!(
            matrix.set(0, cols, true),
            Err(BitError::BitIndexOutOfRange {
                index: cols,
                len: cols
            })
        );
    }

    let identity = BitMatrix::identity(65).expect("identity matrix");
    assert_ne!(identity, BitMatrix::zeros(65, 65).expect("zero matrix"));
    for row in 0..65 {
        for col in 0..65 {
            assert_eq!(identity.get(row, col), Some(row == col));
        }
    }
    assert_eq!(
        BitMatrix::zeros(usize::MAX, 65),
        Err(BitError::MatrixSizeOverflow {
            rows: usize::MAX,
            cols: 65
        })
    );
    let mut matrix = BitMatrix::zeros(2, 65).expect("matrix");
    let short_mask = BitVec::zeros(64);
    assert_eq!(
        matrix.masked_xor_row_into(0, 1, &short_mask.as_bitslice()),
        Err(BitError::LengthMismatch {
            left: 65,
            right: 64
        })
    );

    let mut preallocated = BitMatrix::zeros(3, 4093).expect("preallocated matrix");
    let mask = patterned_bitvec(4093, 0xA9);
    for col in 0..4093 {
        preallocated
            .set(0, col, col % 3 == 0)
            .expect("set source row");
        preallocated
            .set(1, col, col % 5 == 0)
            .expect("set target row");
        preallocated
            .set(2, col, col % 7 == 0)
            .expect("set self row");
    }
    let allocations = allocation_counter::measure(|| {
        for _ in 0..128 {
            preallocated
                .xor_row_into(0, 1)
                .expect("preallocated row XOR");
            preallocated
                .masked_xor_row_into(0, 1, &mask.as_bitslice())
                .expect("preallocated masked row XOR");
            preallocated
                .masked_xor_row_into(2, 2, &mask.as_bitslice())
                .expect("preallocated self masked row XOR");
            preallocated.swap_rows(0, 1).expect("preallocated row swap");
            std::hint::black_box(preallocated.row(2).expect("row view"));
        }
    });
    assert_eq!(
        allocations.count_total, 0,
        "matrix mutation allocated: {allocations:?}"
    );
    assert_eq!(
        allocations.bytes_total, 0,
        "matrix mutation allocated: {allocations:?}"
    );
}

#[test]
fn cq2_bit_matrix_transpose_contract_matches_scalar_reference() {
    for (rows, cols) in [
        (0, 0),
        (0, 65),
        (65, 0),
        (1, 1),
        (3, 5),
        (63, 65),
        (64, 64),
        (65, 63),
        (128, 129),
    ] {
        let mut matrix = BitMatrix::zeros(rows, cols).expect("bounded matrix");
        for row in 0..rows {
            for col in 0..cols {
                if matrix_pattern(row, col) {
                    matrix.set(row, col, true).expect("set matrix bit");
                }
            }
        }
        let transposed = matrix.transpose().expect("transpose");
        assert_eq!(transposed.rows(), cols);
        assert_eq!(transposed.cols(), rows);
        for row in 0..rows {
            for col in 0..cols {
                assert_eq!(transposed.get(col, row), Some(matrix_pattern(row, col)));
            }
        }
        assert_eq!(transposed.transpose().expect("double transpose"), matrix);
    }

    for &size in &[0, 1, 63, 64, 65, 129] {
        let mut matrix = BitMatrix::zeros(size, size).expect("square matrix");
        for row in 0..size {
            for col in 0..size {
                if matrix_pattern(row, col) {
                    matrix.set(row, col, true).expect("set matrix bit");
                }
            }
        }
        let original = matrix.clone();
        matrix
            .transpose_square_in_place()
            .expect("square transpose");
        matrix
            .transpose_square_in_place()
            .expect("square double transpose");
        assert_eq!(matrix, original);
    }

    let mut rectangular = BitMatrix::zeros(2, 3).expect("rectangular matrix");
    assert_eq!(
        rectangular.transpose_square_in_place(),
        Err(BitError::NotSquareMatrix { rows: 2, cols: 3 })
    );
}

#[test]
fn cq2_sparse_xor_matches_dense_across_density_transitions() {
    assert!(SparseXorVec::new().is_empty());
    for &width in &[0_usize, 1, 63, 64, 65, 255, 256, 257, 1025, 4093] {
        for &period in &[1_usize, 2, 3, 7, 16, 64, 257] {
            let left_items = patterned_items(width, period, 0);
            let right_items = patterned_items(width, period.saturating_add(1), period / 2);
            let mut sparse = SparseXorVec::from_sorted_items(with_even_duplicates(&left_items));
            let rhs = SparseXorVec::from_sorted_items(with_even_duplicates(&right_items));
            assert_eq!(sparse.items(), left_items.as_slice());
            sparse.xor_assign(&rhs);

            let expected = symmetric_difference(&left_items, &right_items);
            assert_eq!(
                sparse.items(),
                expected.as_slice(),
                "width={width} period={period}"
            );
            assert!(
                sparse
                    .items()
                    .windows(2)
                    .all(|window| window[0] < window[1])
            );

            let mut dense = BitVec::zeros(width);
            for &item in &left_items {
                dense.set(item as usize, true).expect("dense left item");
            }
            let mut dense_rhs = BitVec::zeros(width);
            for &item in &right_items {
                dense_rhs
                    .set(item as usize, true)
                    .expect("dense right item");
            }
            dense
                .xor_assign(&dense_rhs.as_bitslice())
                .expect("dense XOR");
            let dense_items = (0..width)
                .filter(|index| dense.get(*index) == Some(true))
                .map(|index| u32::try_from(index).expect("qualification width fits u32"))
                .collect::<Vec<_>>();
            assert_eq!(
                dense_items, expected,
                "dense/sparse width={width} period={period}"
            );
        }
    }

    let mut toggled = SparseXorVec::new();
    for item in [9, 1, 7, 1, 3, 9, 2, 7, 6] {
        toggled.xor_item(item);
    }
    assert_eq!(toggled.items(), &[2, 3, 6]);
    assert!(toggled.contains(3));
    assert!(!toggled.contains(4));
    assert!(toggled.is_superset_of(&[2, 6]));
    assert!(!toggled.is_superset_of(&[2, 7]));
    assert_eq!(toggled.to_string(), "SparseXorVec{2, 3, 6}");
    assert_eq!(bits::inplace_xor_sort(vec![5, 4, 5, 5]), vec![4, 5]);
    assert!(bits::is_subset_of_sorted(&[2, 6], toggled.items()));
    assert!(!bits::is_subset_of_sorted(&[2, 7], toggled.items()));
}

#[test]
fn cq2_twiddle_helpers_match_integer_reference() {
    for value in 0_u64..=65_536 {
        assert_eq!(
            bits::is_power_of_2(value),
            value.is_power_of_two(),
            "value={value}"
        );
        let expected_lg = (value != 0).then(|| u64::BITS - 1 - value.leading_zeros());
        assert_eq!(bits::floor_lg2(value), expected_lg, "value={value}");
        for start in [0, 1, 7, 8, 31, 32, 63, 64, 65] {
            let expected_first = if start >= u64::BITS {
                None
            } else {
                let shifted = value >> start;
                (shifted != 0).then(|| start + shifted.trailing_zeros())
            };
            assert_eq!(
                bits::first_set_bit(value, start),
                expected_first,
                "value={value} start={start}"
            );
        }
    }
    assert_eq!(bits::floor_lg2(u64::MAX), Some(63));
    assert_eq!(bits::first_set_bit(u64::MAX, 63), Some(63));
}

#[derive(Clone, Copy)]
enum LogicalOp {
    Xor,
    And,
    Or,
    MaskedXor,
}

fn assert_bitwise(
    actual: &BitVec,
    left: &BitVec,
    right: &BitVec,
    mask: &BitVec,
    operation: LogicalOp,
) {
    for index in 0..actual.len() {
        let left = left.get(index).expect("left bit");
        let right = right.get(index).expect("right bit");
        let mask = mask.get(index).expect("mask bit");
        let expected = match operation {
            LogicalOp::Xor => left ^ right,
            LogicalOp::And => left & right,
            LogicalOp::Or => left | right,
            LogicalOp::MaskedXor => left ^ (right & mask),
        };
        assert_eq!(actual.get(index), Some(expected), "index={index}");
    }
}

fn patterned_bitvec(width: usize, seed: u64) -> BitVec {
    BitVec::from_words_truncated(width, patterned_words(width.div_ceil(WORD_BITS), seed))
}

fn patterned_words(count: usize, seed: u64) -> Vec<u64> {
    (0..count)
        .map(|index| patterned_word(index, seed))
        .collect()
}

fn patterned_word(index: usize, seed: u64) -> u64 {
    let index = index as u64;
    seed.wrapping_add(index.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .rotate_left((index.wrapping_mul(11).wrapping_add(seed) & 63) as u32)
        ^ 0xD6E8_FEB8_6659_FD93
}

fn boundary_indexes(width: usize) -> Vec<usize> {
    let mut indexes = [
        0,
        1,
        31,
        32,
        63,
        64,
        65,
        127,
        128,
        129,
        255,
        256,
        257,
        width.saturating_sub(1),
    ]
    .into_iter()
    .filter(|index| *index < width)
    .collect::<Vec<_>>();
    indexes.sort_unstable();
    indexes.dedup();
    indexes
}

fn assert_canonical_tail(bits: &BitVec) {
    let tail = bits.len() % WORD_BITS;
    if tail == 0 {
        return;
    }
    let last = bits.words().last().copied().expect("non-empty tail word");
    assert_eq!(
        last >> tail,
        0,
        "dirty padding above logical width {}",
        bits.len()
    );
}

fn scalar_popcount(bits: &BitVec) -> usize {
    (0..bits.len())
        .filter(|index| bits.get(*index) == Some(true))
        .count()
}

fn bits_as_bools(bits: &BitVec) -> Vec<bool> {
    (0..bits.len())
        .map(|index| bits.get(index).unwrap_or(false))
        .collect()
}

fn bitvec_from_bools(bits: &[bool]) -> BitVec {
    let mut out = BitVec::zeros(bits.len());
    for (index, value) in bits.iter().copied().enumerate() {
        out.set(index, value).expect("set in-range bit");
    }
    out
}

fn patterned_bools(width: usize, seed: usize) -> Vec<bool> {
    (0..width)
        .map(|index| index.wrapping_mul(37).wrapping_add(seed) % 17 < 7)
        .collect()
}

fn set_row(matrix: &mut BitMatrix, row: usize, values: &[bool]) {
    for (col, value) in values.iter().copied().enumerate() {
        matrix.set(row, col, value).expect("set matrix row");
    }
}

fn assert_row(matrix: &BitMatrix, row: usize, expected: &[bool]) {
    let view = matrix.row(row).expect("matrix row");
    assert_eq!(view.len(), expected.len());
    for (col, expected) in expected.iter().copied().enumerate() {
        assert_eq!(view.get(col), Some(expected), "row={row} col={col}");
    }
}

fn xor_bools(left: &[bool], right: &[bool]) -> Vec<bool> {
    left.iter()
        .zip(right)
        .map(|(left, right)| *left ^ *right)
        .collect()
}

fn masked_source(source: &[bool], mask: &[bool]) -> Vec<bool> {
    source
        .iter()
        .zip(mask)
        .map(|(source, mask)| *source & *mask)
        .collect()
}

fn matrix_pattern(row: usize, col: usize) -> bool {
    row.wrapping_mul(29)
        .wrapping_add(col.wrapping_mul(43))
        .wrapping_add(row ^ col)
        % 19
        < 8
}

fn patterned_items(width: usize, period: usize, offset: usize) -> Vec<u32> {
    (0..width)
        .filter(|index| index.wrapping_add(offset) % period == 0)
        .map(|index| u32::try_from(index).expect("qualification width fits u32"))
        .collect()
}

fn with_even_duplicates(items: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(items.len().saturating_mul(3));
    for (index, item) in items.iter().copied().enumerate() {
        out.push(item);
        if index % 2 == 0 {
            out.push(item);
            out.push(item);
        }
    }
    out.reverse();
    out
}

fn symmetric_difference(left: &[u32], right: &[u32]) -> Vec<u32> {
    let left = left.iter().copied().collect::<BTreeSet<_>>();
    let right = right.iter().copied().collect::<BTreeSet<_>>();
    left.symmetric_difference(&right).copied().collect()
}
