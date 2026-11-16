//! Bit-plane batches: one packed row of shots per result bit.
//!
//! A plane holds one result bit for every shot of a batch, packed 64 shots per word with shot
//! `s` at bit `s % 64` of word `s / 64`. Producers that sample whole lanes at once (Pauli
//! frames, detection conversion, and DEM sampling) hand these planes to sinks directly, so the
//! only layout conversion left is the one a row-oriented output format itself requires.

use stab_bits::{BIT_BLOCK_WORDS, BitBlock, BitSlice};

use super::{PackedShotBatch, PackedShotBatchView, batch_width_mismatch, bit_storage_error};
use crate::{FormatError, RecordResult};

const WORD_BITS: usize = u64::BITS as usize;

/// Owned bit planes for any number of shots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitPlaneBatch {
    words: Vec<u64>,
    words_per_plane: usize,
    shot_count: usize,
    bits_per_shot: usize,
}

impl BitPlaneBatch {
    pub fn zeros(shot_count: usize, bits_per_shot: usize) -> RecordResult<Self> {
        let words_per_plane = shot_count.div_ceil(WORD_BITS);
        let len = bits_per_shot.checked_mul(words_per_plane).ok_or_else(|| {
            FormatError::invalid_data(format!(
                "bit-plane batch of {bits_per_shot} planes and {shot_count} shots is unaddressable"
            ))
        })?;
        let mut words = Vec::new();
        words.try_reserve_exact(len).map_err(|error| {
            FormatError::invalid_data(format!(
                "bit-plane batch could not reserve {len} plane words: {error}"
            ))
        })?;
        words.resize(len, 0);
        Ok(Self {
            words,
            words_per_plane,
            shot_count,
            bits_per_shot,
        })
    }

    pub fn from_shot_major(batch: PackedShotBatchView<'_>) -> RecordResult<Self> {
        let mut planes = Self::zeros(batch.shot_count(), batch.bits_per_shot())?;
        let mut column = vec![0_u64; batch.bits_per_shot()];
        for word_index in 0..planes.words_per_plane {
            let row_start = word_index * WORD_BITS;
            let row_count = WORD_BITS.min(batch.shot_count() - row_start);
            batch.copy_rows_into_bit_plane_words(row_start, row_count, &mut column)?;
            for (bit_index, word) in column.iter().copied().enumerate() {
                *planes.word_mut(bit_index, word_index)? = word;
            }
        }
        Ok(planes)
    }

    #[inline]
    pub fn shot_count(&self) -> usize {
        self.shot_count
    }

    #[inline]
    pub fn bits_per_shot(&self) -> usize {
        self.bits_per_shot
    }

    pub fn get(&self, bit_index: usize, shot_index: usize) -> Option<bool> {
        self.view().get(bit_index, shot_index)
    }

    pub fn set(&mut self, bit_index: usize, shot_index: usize, value: bool) -> RecordResult<()> {
        if shot_index >= self.shot_count {
            return Err(FormatError::invalid_data(format!(
                "bit-plane shot index {shot_index} is out of range for {} records",
                self.shot_count
            )));
        }
        let word = self.word_mut(bit_index, shot_index / WORD_BITS)?;
        let mask = 1_u64 << (shot_index % WORD_BITS);
        if value {
            *word |= mask;
        } else {
            *word &= !mask;
        }
        Ok(())
    }

    pub fn copy_shot_from_bools(&mut self, shot_index: usize, record: &[bool]) -> RecordResult<()> {
        if shot_index >= self.shot_count {
            return Err(FormatError::invalid_data(format!(
                "bit-plane shot index {shot_index} is out of range for {} records",
                self.shot_count
            )));
        }
        if record.len() != self.bits_per_shot {
            return Err(batch_width_mismatch(
                "bit-plane record",
                record.len(),
                self.bits_per_shot,
            ));
        }
        for (bit_index, value) in record.iter().copied().enumerate() {
            self.set(bit_index, shot_index, value)?;
        }
        Ok(())
    }

    pub fn plane(&self, bit_index: usize) -> RecordResult<BitSlice<'_>> {
        self.view().plane(bit_index)
    }

    pub fn view(&self) -> BitPlaneBatchView<'_> {
        BitPlaneBatchView {
            storage: PlaneStorage::Words {
                words: &self.words,
                words_per_plane: self.words_per_plane,
            },
            first_word: 0,
            shot_count: self.shot_count,
            bits_per_shot: self.bits_per_shot,
        }
    }

    pub fn view_prefix(&self, shot_count: usize) -> RecordResult<BitPlaneBatchView<'_>> {
        self.view().prefix(shot_count)
    }

    pub fn to_shot_major(&self) -> RecordResult<PackedShotBatch> {
        PackedShotBatch::from_bit_planes(self.view())
    }

    fn word_mut(&mut self, bit_index: usize, word_index: usize) -> RecordResult<&mut u64> {
        let bits_per_shot = self.bits_per_shot;
        bit_index
            .checked_mul(self.words_per_plane)
            .and_then(|start| start.checked_add(word_index))
            .filter(|_| word_index < self.words_per_plane)
            .and_then(|index| self.words.get_mut(index))
            .ok_or_else(|| {
                FormatError::invalid_data(format!(
                    "bit-plane index {bit_index} is out of range for {bits_per_shot} result bits"
                ))
            })
    }
}

/// Borrowed bit planes for any number of shots.
///
/// Storage may contain nonzero bits at shot indexes greater than or equal to
/// [`Self::shot_count`]. Logical accessors hide those inactive lanes. A plane's raw
/// [`BitSlice::words`] retain the producer's storage and must be masked by consumers that
/// inspect words directly.
#[derive(Clone, Copy, Debug)]
pub struct BitPlaneBatchView<'a> {
    storage: PlaneStorage<'a>,
    /// Stored word of each plane that holds shot zero of this view.
    first_word: usize,
    shot_count: usize,
    bits_per_shot: usize,
}

#[derive(Clone, Copy, Debug)]
enum PlaneStorage<'a> {
    /// Plane `b` is `words[b * words_per_plane..][..words_per_plane]`.
    Words {
        words: &'a [u64],
        words_per_plane: usize,
    },
    /// Plane `b` is the 256-lane block `blocks[b]`.
    Blocks(&'a [BitBlock]),
}

impl<'a> BitPlaneBatchView<'a> {
    /// Borrows `bits_per_shot` planes stored back to back, `words_per_plane` words each.
    pub fn try_from_words(
        words: &'a [u64],
        words_per_plane: usize,
        shot_count: usize,
        bits_per_shot: usize,
    ) -> RecordResult<Self> {
        let expected = bits_per_shot
            .checked_mul(words_per_plane)
            .ok_or_else(|| FormatError::invalid_data("bit-plane word storage size overflowed"))?;
        if words.len() != expected {
            return Err(batch_width_mismatch(
                "bit-plane word storage",
                words.len(),
                expected,
            ));
        }
        validate_lane_capacity(shot_count, words_per_plane)?;
        Ok(Self {
            storage: PlaneStorage::Words {
                words,
                words_per_plane,
            },
            first_word: 0,
            shot_count,
            bits_per_shot,
        })
    }

    /// Borrows one 256-lane block per result bit.
    pub fn try_from_blocks(blocks: &'a [BitBlock], shot_count: usize) -> RecordResult<Self> {
        validate_lane_capacity(shot_count, BIT_BLOCK_WORDS)?;
        Ok(Self {
            storage: PlaneStorage::Blocks(blocks),
            first_word: 0,
            shot_count,
            bits_per_shot: blocks.len(),
        })
    }

    #[inline]
    pub fn shot_count(self) -> usize {
        self.shot_count
    }

    #[inline]
    pub fn bits_per_shot(self) -> usize {
        self.bits_per_shot
    }

    #[inline]
    pub fn get(self, bit_index: usize, shot_index: usize) -> Option<bool> {
        if shot_index >= self.shot_count {
            return None;
        }
        let word = self.plane_words(bit_index)?.get(shot_index / WORD_BITS)?;
        Some(word & (1_u64 << (shot_index % WORD_BITS)) != 0)
    }

    /// Borrows one plane as a bit slice of exactly [`Self::shot_count`] lanes.
    pub fn plane(self, bit_index: usize) -> RecordResult<BitSlice<'a>> {
        let words = self.plane_words(bit_index).ok_or_else(|| {
            FormatError::invalid_data(format!(
                "bit-plane index {bit_index} is out of range for {} result bits",
                self.bits_per_shot
            ))
        })?;
        let used = words
            .get(..self.shot_count.div_ceil(WORD_BITS))
            .ok_or_else(|| FormatError::invalid_data("bit plane is shorter than its shot count"))?;
        BitSlice::new(used, self.shot_count).map_err(bit_storage_error)
    }

    /// Borrows the first `shot_count` shots of this view.
    pub fn prefix(self, shot_count: usize) -> RecordResult<Self> {
        if shot_count > self.shot_count {
            return Err(FormatError::invalid_data(format!(
                "bit-plane prefix has {shot_count} records but the view contains only {}",
                self.shot_count
            )));
        }
        Ok(Self { shot_count, ..self })
    }

    /// Borrows `shot_count` shots starting at `first_shot`, which must be a multiple of 64.
    pub fn shot_window(self, first_shot: usize, shot_count: usize) -> RecordResult<Self> {
        if !first_shot.is_multiple_of(WORD_BITS)
            || first_shot
                .checked_add(shot_count)
                .is_none_or(|end| end > self.shot_count)
        {
            return Err(FormatError::invalid_data(format!(
                "bit-plane window of {shot_count} shots at {first_shot} is not word-aligned within {} shots",
                self.shot_count
            )));
        }
        Ok(Self {
            first_word: self.first_word + first_shot / WORD_BITS,
            shot_count,
            ..self
        })
    }

    /// Copies word `word_index` of every plane into `target`, one word per result bit, with
    /// lanes at or beyond [`Self::shot_count`] cleared.
    ///
    /// Word `w` holds shots `64 * w..64 * w + 64`, so the copied column is the 64-shot block a
    /// row-oriented consumer transposes next.
    pub fn copy_word_column_into(self, word_index: usize, target: &mut [u64]) -> RecordResult<()> {
        if target.len() != self.bits_per_shot {
            return Err(batch_width_mismatch(
                "bit-plane column target",
                target.len(),
                self.bits_per_shot,
            ));
        }
        let first_shot = word_index
            .checked_mul(WORD_BITS)
            .filter(|first| *first < self.shot_count)
            .ok_or_else(|| {
                FormatError::invalid_data(format!(
                    "bit-plane word {word_index} is beyond {} shots",
                    self.shot_count
                ))
            })?;
        let mask = low_bits_mask(self.shot_count - first_shot);
        let word_index = self.first_word + word_index;
        match self.storage {
            PlaneStorage::Words {
                words,
                words_per_plane: 1,
            } if word_index == 0 && mask == u64::MAX => {
                target.copy_from_slice(words);
            }
            PlaneStorage::Words {
                words,
                words_per_plane,
            } => {
                let planes = words.chunks_exact(words_per_plane);
                for (target, plane) in target.iter_mut().zip(planes) {
                    *target = plane.get(word_index).copied().unwrap_or(0) & mask;
                }
            }
            PlaneStorage::Blocks(blocks) => {
                for (target, block) in target.iter_mut().zip(blocks) {
                    *target = block.as_words().get(word_index).copied().unwrap_or(0) & mask;
                }
            }
        }
        Ok(())
    }

    /// Borrows the planes as one word each when they are stored that way.
    pub(crate) fn single_word_planes(self) -> Option<&'a [u64]> {
        match self.storage {
            PlaneStorage::Words {
                words,
                words_per_plane: 1,
            } if self.first_word == 0 => Some(words),
            _ => None,
        }
    }

    /// Borrows the stored words of one plane from this view's first shot on, including
    /// inactive lanes.
    #[inline]
    fn plane_words(self, bit_index: usize) -> Option<&'a [u64]> {
        let plane = match self.storage {
            PlaneStorage::Words {
                words,
                words_per_plane,
            } => {
                let start = bit_index.checked_mul(words_per_plane)?;
                words.get(start..start.checked_add(words_per_plane)?)?
            }
            PlaneStorage::Blocks(blocks) => blocks.get(bit_index)?.as_words().as_slice(),
        };
        plane.get(self.first_word..)
    }
}

fn validate_lane_capacity(shot_count: usize, words_per_plane: usize) -> RecordResult<()> {
    if shot_count.div_ceil(WORD_BITS) > words_per_plane {
        return Err(FormatError::invalid_data(format!(
            "bit planes of {words_per_plane} words cannot hold {shot_count} shots"
        )));
    }
    Ok(())
}

pub(super) const fn low_bits_mask(bit_count: usize) -> u64 {
    if bit_count >= WORD_BITS {
        u64::MAX
    } else {
        (1_u64 << bit_count) - 1
    }
}
