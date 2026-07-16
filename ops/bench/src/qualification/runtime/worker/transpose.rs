use std::hint::black_box;
use std::sync::atomic::{Ordering, compiler_fence};

use stab_core::BitMatrix;

use super::WorkerError;

pub(super) const TRANSPOSE_MIN_DIMENSION: u64 = 256;
pub(super) const TRANSPOSE_MAX_DIMENSION: u64 = 16_384;
pub(super) const TRANSPOSE_DIMENSION_ALIGNMENT: u64 = 256;

const FIXTURE_SEED: u64 = 0xd1b5_4a32_d192_ed03;
const ROW_AFFINE: u64 = 0x0000_0001_0000_01b3;
const LANE_AFFINE: u64 = 0x0000_0000_9e37_79b9;
const SET_BITS_PER_ROW: u64 = 8;
const IN_PLACE_MARKER: u64 = 3;
const ALLOCATING_MARKER: u64 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TransposeKind {
    InPlace,
    Allocating,
}

impl TransposeKind {
    pub(super) const fn workload(self) -> &'static str {
        match self {
            Self::InPlace => "bit-matrix-transpose-in-place",
            Self::Allocating => "bit-matrix-transpose-allocating",
        }
    }

    pub(super) const fn measurement(self) -> &'static str {
        match self {
            Self::InPlace => "in-place-transpose",
            Self::Allocating => "allocating-transpose",
        }
    }

    const fn marker(self) -> u64 {
        match self {
            Self::InPlace => IN_PLACE_MARKER,
            Self::Allocating => ALLOCATING_MARKER,
        }
    }
}

pub(super) struct TransposeFixture {
    kind: TransposeKind,
    dimension: u64,
    matrix: BitMatrix,
    result: Option<BitMatrix>,
    pub(super) input_bytes: u64,
    pub(super) input_digest: [u64; 4],
}

impl TransposeFixture {
    pub(super) fn prepare(kind: TransposeKind, work_items: u64) -> Result<Self, WorkerError> {
        let (dimension, input_bytes) = checked_transpose_shape(work_items)?;
        let host_dimension = usize::try_from(dimension)
            .map_err(|_| WorkerError::TransposeDimensionRange(dimension))?;
        let mut matrix = BitMatrix::zeros(host_dimension, host_dimension)?;
        for row in 0..dimension {
            for lane in 0..SET_BITS_PER_ROW {
                let column = fixture_column(row, lane, dimension)?;
                matrix.set(
                    usize::try_from(row)
                        .map_err(|_| WorkerError::TransposeDimensionRange(dimension))?,
                    usize::try_from(column)
                        .map_err(|_| WorkerError::TransposeDimensionRange(dimension))?,
                    true,
                )?;
            }
        }
        let input_digest = matrix_digest(&matrix)?;

        match kind {
            TransposeKind::InPlace => {
                matrix.transpose_square_in_place()?;
                matrix.transpose_square_in_place()?;
                if matrix_digest(&matrix)? != input_digest {
                    return Err(WorkerError::TransposePrimingState(kind.workload()));
                }
            }
            TransposeKind::Allocating => {
                for _ in 0..2 {
                    let warmed = matrix.transpose()?;
                    black_box(&warmed);
                }
            }
        }

        Ok(Self {
            kind,
            dimension,
            matrix,
            result: None,
            input_bytes,
            input_digest,
        })
    }

    pub(super) fn execute(&mut self, iterations: u64) -> Result<(), WorkerError> {
        match self.kind {
            TransposeKind::InPlace => {
                for _ in 0..iterations {
                    compiler_fence(Ordering::SeqCst);
                    black_box(&mut self.matrix).transpose_square_in_place()?;
                }
            }
            TransposeKind::Allocating => {
                let mut result = None;
                for _ in 0..iterations {
                    compiler_fence(Ordering::SeqCst);
                    let next = black_box(&self.matrix).transpose()?;
                    black_box(&next);
                    result = Some(next);
                }
                self.result = result;
            }
        }
        Ok(())
    }

    pub(super) fn output_digest(
        &self,
        iterations: u64,
        work_items: u64,
    ) -> Result<[u64; 4], WorkerError> {
        match self.kind {
            TransposeKind::InPlace => {
                let final_digest = matrix_digest(&self.matrix)?;
                Ok(byte_digest_words(&[
                    iterations,
                    work_items,
                    self.dimension,
                    self.kind.marker(),
                    self.input_digest[0],
                    self.input_digest[1],
                    self.input_digest[2],
                    self.input_digest[3],
                    final_digest[0],
                    final_digest[1],
                    final_digest[2],
                    final_digest[3],
                ]))
            }
            TransposeKind::Allocating => {
                let result = self
                    .result
                    .as_ref()
                    .ok_or(WorkerError::MissingTransposeResult)?;
                let result_digest = matrix_digest(result)?;
                let source_digest = matrix_digest(&self.matrix)?;
                if source_digest != self.input_digest {
                    return Err(WorkerError::TransposeSourceChanged);
                }
                Ok(byte_digest_words(&[
                    iterations,
                    work_items,
                    self.dimension,
                    self.kind.marker(),
                    self.input_digest[0],
                    self.input_digest[1],
                    self.input_digest[2],
                    self.input_digest[3],
                    result_digest[0],
                    result_digest[1],
                    result_digest[2],
                    result_digest[3],
                    source_digest[0],
                    source_digest[1],
                    source_digest[2],
                    source_digest[3],
                ]))
            }
        }
    }

    #[cfg(test)]
    pub(super) const fn dimension(&self) -> u64 {
        self.dimension
    }

    #[cfg(test)]
    pub(super) const fn matrix(&self) -> &BitMatrix {
        &self.matrix
    }

    #[cfg(test)]
    pub(super) const fn result(&self) -> Option<&BitMatrix> {
        self.result.as_ref()
    }
}

pub(super) fn checked_transpose_shape(work_items: u64) -> Result<(u64, u64), WorkerError> {
    let dimension = work_items.isqrt();
    if dimension.saturating_mul(dimension) != work_items {
        return Err(WorkerError::TransposeWorkNotSquare(work_items));
    }
    if dimension < TRANSPOSE_MIN_DIMENSION {
        return Err(WorkerError::TransposeDimensionMinimum {
            actual: dimension,
            minimum: TRANSPOSE_MIN_DIMENSION,
        });
    }
    if !dimension.is_multiple_of(TRANSPOSE_DIMENSION_ALIGNMENT) {
        return Err(WorkerError::TransposeDimensionAlignment {
            actual: dimension,
            alignment: TRANSPOSE_DIMENSION_ALIGNMENT,
        });
    }
    if dimension > TRANSPOSE_MAX_DIMENSION {
        return Err(WorkerError::TransposeDimensionLimit {
            actual: dimension,
            maximum: TRANSPOSE_MAX_DIMENSION,
        });
    }
    let input_bytes = work_items
        .checked_div(8)
        .and_then(|bytes| bytes.checked_add(16))
        .ok_or(WorkerError::TransposeByteCountOverflow)?;
    Ok((dimension, input_bytes))
}

fn fixture_column(row: u64, lane: u64, dimension: u64) -> Result<u64, WorkerError> {
    let affine = row
        .checked_mul(ROW_AFFINE)
        .and_then(|value| {
            lane.checked_mul(LANE_AFFINE)
                .and_then(|lane| value.checked_add(lane))
        })
        .ok_or(WorkerError::TransposeAffineOverflow)?;
    let offset = row
        .checked_mul(17)
        .and_then(|value| {
            lane.checked_mul(31)
                .and_then(|lane| value.checked_add(lane))
        })
        .ok_or(WorkerError::TransposeAffineOverflow)?;
    Ok(
        splitmix64(FIXTURE_SEED ^ affine ^ dimension.rotate_left(29)).wrapping_add(offset)
            % dimension,
    )
}

fn splitmix64(input: u64) -> u64 {
    let mut value = input.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn matrix_digest(matrix: &BitMatrix) -> Result<[u64; 4], WorkerError> {
    let mut digest = ByteDigest::new();
    digest.update_u64(
        u64::try_from(matrix.rows()).map_err(|_| WorkerError::TransposeDimensionRange(u64::MAX))?,
    );
    digest.update_u64(
        u64::try_from(matrix.cols()).map_err(|_| WorkerError::TransposeDimensionRange(u64::MAX))?,
    );
    for row in 0..matrix.rows() {
        for word in matrix.row(row)?.words() {
            digest.update_u64(*word);
        }
    }
    Ok(digest.finish())
}

fn byte_digest_words(words: &[u64]) -> [u64; 4] {
    let mut digest = ByteDigest::new();
    for word in words {
        digest.update_u64(*word);
    }
    digest.finish()
}

struct ByteDigest {
    state: [u64; 4],
    byte_index: u64,
}

impl ByteDigest {
    const fn new() -> Self {
        Self {
            state: [
                0x6a09_e667_f3bc_c908,
                0xbb67_ae85_84ca_a73b,
                0x3c6e_f372_fe94_f82b,
                0xa54f_f53a_5f1d_36f1,
            ],
            byte_index: 0,
        }
    }

    fn update_u64(&mut self, word: u64) {
        for byte in word.to_le_bytes() {
            let value =
                u64::from(byte).wrapping_add(self.byte_index.wrapping_mul(0x9e37_79b9_7f4a_7c15));
            for (lane_state, lane) in self.state.iter_mut().zip(0_u32..) {
                *lane_state ^= value.rotate_left(lane * 13);
                *lane_state = lane_state
                    .wrapping_mul(0x0100_0000_01b3_u64.wrapping_add(u64::from(lane) * 2))
                    .rotate_left(9 + lane);
            }
            self.byte_index = self.byte_index.wrapping_add(1);
        }
    }

    const fn finish(self) -> [u64; 4] {
        self.state
    }
}

#[cfg(test)]
pub(super) fn independently_encoded_matrix(matrix: &BitMatrix) -> Result<Vec<u8>, WorkerError> {
    let capacity = matrix
        .rows()
        .checked_mul(matrix.cols())
        .and_then(|bits| bits.checked_div(8))
        .and_then(|bytes| bytes.checked_add(16))
        .ok_or(WorkerError::TransposeByteCountOverflow)?;
    let mut encoded = Vec::with_capacity(capacity);
    encoded.extend_from_slice(
        &u64::try_from(matrix.rows())
            .map_err(|_| WorkerError::TransposeDimensionRange(u64::MAX))?
            .to_le_bytes(),
    );
    encoded.extend_from_slice(
        &u64::try_from(matrix.cols())
            .map_err(|_| WorkerError::TransposeDimensionRange(u64::MAX))?
            .to_le_bytes(),
    );
    for row in 0..matrix.rows() {
        for word in matrix.row(row)?.words() {
            encoded.extend_from_slice(&word.to_le_bytes());
        }
    }
    Ok(encoded)
}

#[cfg(test)]
pub(super) fn independently_computed_digest(matrix: &BitMatrix) -> Result<[u64; 4], WorkerError> {
    Ok(super::byte_digest(&independently_encoded_matrix(matrix)?))
}
