use super::BitMatrix;

const TILE_BITS: usize = u64::BITS as usize;
const TRANSPOSE_STAGES: [(usize, u64); 6] = [
    (32, 0x0000_0000_FFFF_FFFF),
    (16, 0x0000_FFFF_0000_FFFF),
    (8, 0x00FF_00FF_00FF_00FF),
    (4, 0x0F0F_0F0F_0F0F_0F0F),
    (2, 0x3333_3333_3333_3333),
    (1, 0x5555_5555_5555_5555),
];

pub(super) fn out_of_place(source: &BitMatrix, target: &mut BitMatrix) {
    if source.rows == 0 || source.cols() == 0 {
        return;
    }

    for row_base in (0..source.rows).step_by(TILE_BITS) {
        for col_base in (0..source.cols()).step_by(TILE_BITS) {
            let mut tile = load_tile(source, row_base, col_base);
            transpose_tile(&mut tile);
            write_tile(target, col_base, row_base, &tile);
        }
    }
}

pub(super) fn square_in_place(matrix: &mut BitMatrix) {
    let size = matrix.rows;
    if size == 0 {
        return;
    }

    for row_base in (0..size).step_by(TILE_BITS) {
        let mut diagonal = load_tile(matrix, row_base, row_base);
        transpose_tile(&mut diagonal);
        write_tile(matrix, row_base, row_base, &diagonal);

        let off_diagonal_start = row_base.saturating_add(TILE_BITS);
        for col_base in (off_diagonal_start..size).step_by(TILE_BITS) {
            let mut upper = load_tile(matrix, row_base, col_base);
            let mut lower = load_tile(matrix, col_base, row_base);
            transpose_tile(&mut upper);
            transpose_tile(&mut lower);
            write_tile(matrix, col_base, row_base, &upper);
            write_tile(matrix, row_base, col_base, &lower);
        }
    }
}

fn load_tile(matrix: &BitMatrix, row_base: usize, col_base: usize) -> [u64; 64] {
    let mut tile = [0_u64; TILE_BITS];
    let row_count = TILE_BITS.min(matrix.rows.saturating_sub(row_base));
    let col_word = col_base / TILE_BITS;
    let words = matrix
        .words
        .chunks_exact(matrix.row_words)
        .skip(row_base)
        .take(row_count)
        .flat_map(|row| row.iter().skip(col_word).take(1));
    for (slot, word) in tile.iter_mut().zip(words) {
        *slot = *word;
    }
    tile
}

fn write_tile(matrix: &mut BitMatrix, row_base: usize, col_base: usize, tile: &[u64; 64]) {
    let row_count = TILE_BITS.min(matrix.rows.saturating_sub(row_base));
    let col_count = TILE_BITS.min(matrix.cols().saturating_sub(col_base));
    let col_word = col_base / TILE_BITS;
    let tail_mask = low_bits_mask(col_count);
    let slots = matrix
        .words
        .chunks_exact_mut(matrix.row_words)
        .skip(row_base)
        .take(row_count)
        .flat_map(|row| row.iter_mut().skip(col_word).take(1));
    for (word, slot) in tile.iter().zip(slots) {
        *slot = *word & tail_mask;
    }
}

fn low_bits_mask(bits: usize) -> u64 {
    if bits == TILE_BITS {
        u64::MAX
    } else if bits == 0 {
        0
    } else {
        (1_u64 << bits) - 1
    }
}

fn transpose_tile(tile: &mut [u64; 64]) {
    for (shift, mask) in TRANSPOSE_STAGES {
        for block in tile.chunks_exact_mut(shift * 2) {
            let (low_rows, high_rows) = block.split_at_mut(shift);
            for (low, high) in low_rows.iter_mut().zip(high_rows) {
                let swapped = ((*low >> shift) ^ *high) & mask;
                *low ^= swapped << shift;
                *high ^= swapped;
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "fixed-size transpose tests use bounded fixture indexes and fail-fast setup"
)]
mod tests {
    use super::*;
    use crate::bits::BitLen;

    #[test]
    fn tile_transpose_moves_every_single_bit() {
        for row in 0..TILE_BITS {
            for col in 0..TILE_BITS {
                let mut tile = [0_u64; TILE_BITS];
                let Some(word) = tile.get_mut(row) else {
                    panic!("row is bounded by TILE_BITS");
                };
                *word = 1_u64 << col;
                transpose_tile(&mut tile);
                for (actual_row, actual) in tile.iter().copied().enumerate() {
                    let expected = if actual_row == col { 1_u64 << row } else { 0 };
                    assert_eq!(actual, expected, "source row={row} col={col}");
                }
            }
        }
    }

    #[test]
    fn dirty_tail_bits_do_not_enter_the_logical_transpose() {
        let mut source = BitMatrix {
            rows: 65,
            cols: BitLen::new(65),
            row_words: 2,
            words: vec![0; 130],
        };
        for row in source.words.chunks_exact_mut(2) {
            let Some(tail) = row.get_mut(1) else {
                panic!("two-word row has a tail");
            };
            *tail = u64::MAX;
        }
        let mut target = BitMatrix::zeros(65, 65).expect("target");

        out_of_place(&source, &mut target);

        for row in 0..65 {
            for col in 0..65 {
                assert_eq!(target.get(row, col), Some(row == 64));
            }
        }
        assert!(target.words.chunks_exact(2).all(|row| row[1] <= 1));
    }
}
