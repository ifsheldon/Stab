use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

use rand::Rng;
use stab_bits::{BIT_BLOCK_WORDS, BitBlock};

use stab_records::{BitPlaneBatchView, RecordResult};

use crate::bernoulli::fill_words as fill_bernoulli_words;
use crate::detection::error::DetectionResult;

use super::BitPlaneDetectionFrame;
use super::helpers::{frame_word, set_frame_word, xor_frame_word};

pub(crate) trait FrameWord:
    BitAnd<Output = Self>
    + BitAndAssign
    + BitOr<Output = Self>
    + BitOrAssign
    + BitXor<Output = Self>
    + BitXorAssign
    + Copy
    + Default
    + Eq
    + Not<Output = Self>
{
    const LANE_COUNT: usize;

    fn active_mask(lanes: usize) -> Self;
    /// Moves lane `lanes + i` to lane `i` and clears the vacated high lanes.
    fn shift_lanes_down(self, lanes: usize) -> Self;
    fn prefix_len(self) -> Option<usize>;
    fn one_hot(index: usize) -> Self;
    fn isolate_lowest_one(self) -> Option<Self>;
    fn clear_lowest_one(self) -> Self;
    fn sample_mask(probability: f64, active: Self, rng: &mut impl Rng) -> Self;
    /// Borrows the first `shot_count` lanes of one plane per result bit as a batch view.
    fn plane_view(planes: &[Self], shot_count: usize) -> RecordResult<BitPlaneBatchView<'_>>;
    #[cfg(test)]
    fn extract_u64(self, start: usize, count: usize) -> u64;
}

/// The 64-lane frame word: one `u64` per plane, used by narrow sessions.
impl FrameWord for u64 {
    const LANE_COUNT: usize = u64::BITS as usize;

    #[inline(always)]
    fn active_mask(lanes: usize) -> Self {
        low_bits_mask(lanes)
    }

    #[inline(always)]
    fn shift_lanes_down(self, lanes: usize) -> Self {
        self.checked_shr(u32::try_from(lanes).unwrap_or(u32::MAX))
            .unwrap_or(0)
    }

    fn prefix_len(self) -> Option<usize> {
        let lanes = self.trailing_ones() as usize;
        (self == low_bits_mask(lanes)).then_some(lanes)
    }

    #[inline(always)]
    fn one_hot(index: usize) -> Self {
        u32::try_from(index)
            .ok()
            .and_then(|index| 1_u64.checked_shl(index))
            .unwrap_or(0)
    }

    #[inline(always)]
    fn isolate_lowest_one(self) -> Option<Self> {
        (self != 0).then(|| u64::isolate_lowest_one(self))
    }

    #[inline(always)]
    fn clear_lowest_one(self) -> Self {
        self & self.wrapping_sub(1)
    }

    fn sample_mask(probability: f64, active: Self, rng: &mut impl Rng) -> Self {
        let mut word = [0_u64; 1];
        fill_bernoulli_words(probability, &mut word, rng);
        let [word] = word;
        word & active
    }

    fn plane_view(planes: &[Self], shot_count: usize) -> RecordResult<BitPlaneBatchView<'_>> {
        BitPlaneBatchView::try_from_words(planes, 1, shot_count, planes.len())
    }

    #[cfg(test)]
    fn extract_u64(self, start: usize, count: usize) -> u64 {
        self.shift_lanes_down(start) & low_bits_mask(count)
    }
}

impl FrameWord for BitBlock {
    const LANE_COUNT: usize = BIT_BLOCK_WORDS * u64::BITS as usize;

    fn active_mask(lanes: usize) -> Self {
        let full_words = lanes.min(Self::LANE_COUNT) / u64::BITS as usize;
        let tail = lanes.min(Self::LANE_COUNT) % u64::BITS as usize;
        let words = std::array::from_fn(|index| {
            if index < full_words {
                u64::MAX
            } else if index == full_words {
                low_bits_mask(tail)
            } else {
                0
            }
        });
        Self::from_words(words)
    }

    fn shift_lanes_down(self, lanes: usize) -> Self {
        let words = self.words();
        let word_shift = lanes / u64::BITS as usize;
        let bit_shift = lanes % u64::BITS as usize;
        let word = |index: usize| words.get(index).copied().unwrap_or(0);
        Self::from_words(std::array::from_fn(|index| {
            let low = word(index + word_shift);
            if bit_shift == 0 {
                low
            } else {
                (low >> bit_shift)
                    | (word(index + word_shift + 1) << (u64::BITS as usize - bit_shift))
            }
        }))
    }

    fn prefix_len(self) -> Option<usize> {
        let mut lanes = 0_usize;
        let mut ended = false;
        for word in self.words() {
            if ended {
                if word != 0 {
                    return None;
                }
                continue;
            }
            if word == u64::MAX {
                lanes += u64::BITS as usize;
                continue;
            }
            let tail = word.trailing_ones() as usize;
            if word != low_bits_mask(tail) {
                return None;
            }
            lanes += tail;
            ended = true;
        }
        Some(lanes)
    }

    fn one_hot(index: usize) -> Self {
        let mut words = [0; BIT_BLOCK_WORDS];
        if let Some(word) = words.get_mut(index / u64::BITS as usize) {
            *word = 1_u64 << (index % u64::BITS as usize);
        }
        Self::from_words(words)
    }

    fn isolate_lowest_one(self) -> Option<Self> {
        let mut isolated = [0; BIT_BLOCK_WORDS];
        for (index, word) in self.words().into_iter().enumerate() {
            if word == 0 {
                continue;
            }
            if let Some(output) = isolated.get_mut(index) {
                *output = word.isolate_lowest_one();
            }
            return Some(Self::from_words(isolated));
        }
        None
    }

    fn clear_lowest_one(self) -> Self {
        let mut words = self.words();
        if let Some(word) = words.iter_mut().find(|word| **word != 0) {
            *word &= word.wrapping_sub(1);
        }
        Self::from_words(words)
    }

    fn sample_mask(probability: f64, active: Self, rng: &mut impl Rng) -> Self {
        let mut words = [0; BIT_BLOCK_WORDS];
        fill_bernoulli_words(probability, &mut words, rng);
        Self::from_words(words) & active
    }

    fn plane_view(planes: &[Self], shot_count: usize) -> RecordResult<BitPlaneBatchView<'_>> {
        BitPlaneBatchView::try_from_blocks(planes, shot_count)
    }

    #[cfg(test)]
    fn extract_u64(self, start: usize, count: usize) -> u64 {
        let words = self.words();
        let word_index = start / u64::BITS as usize;
        let bit_offset = start % u64::BITS as usize;
        let low = words.get(word_index).copied().unwrap_or(0) >> bit_offset;
        let value = if bit_offset == 0 {
            low
        } else {
            low | (words.get(word_index + 1).copied().unwrap_or(0)
                << (u64::BITS as usize - bit_offset))
        };
        value & low_bits_mask(count)
    }
}

const fn low_bits_mask(bit_count: usize) -> u64 {
    if bit_count >= u64::BITS as usize {
        u64::MAX
    } else if bit_count == 0 {
        0
    } else {
        (1_u64 << bit_count) - 1
    }
}

impl<W: FrameWord> BitPlaneDetectionFrame<W> {
    #[inline(always)]
    pub(super) fn x_word(&self, qubit: usize) -> DetectionResult<W> {
        frame_word(&self.xs, qubit)
    }

    #[inline(always)]
    pub(super) fn z_word(&self, qubit: usize) -> DetectionResult<W> {
        frame_word(&self.zs, qubit)
    }

    #[inline(always)]
    pub(super) fn set_x_word(&mut self, qubit: usize, value: W) -> DetectionResult<()> {
        set_frame_word(&mut self.xs, qubit, value)
    }

    #[inline(always)]
    pub(super) fn set_z_word(&mut self, qubit: usize, value: W) -> DetectionResult<()> {
        set_frame_word(&mut self.zs, qubit, value)
    }

    #[inline(always)]
    pub(super) fn xor_x_word(&mut self, qubit: usize, value: W) -> DetectionResult<()> {
        xor_frame_word(&mut self.xs, qubit, value)
    }

    #[inline(always)]
    pub(super) fn xor_z_word(&mut self, qubit: usize, value: W) -> DetectionResult<()> {
        xor_frame_word(&mut self.zs, qubit, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_block_prefixes_and_cross_word_segments_are_exact() {
        for lanes in [0, 1, 63, 64, 65, 127, 128, 192, 255, 256] {
            let mask = BitBlock::active_mask(lanes);
            assert_eq!(mask.prefix_len(), Some(lanes));
        }
        assert_eq!(
            BitBlock::from_words([u64::MAX, 0b101, 0, 0]).prefix_len(),
            None
        );

        let block = BitBlock::from_words([
            0x0123_4567_89ab_cdef,
            0xfedc_ba98_7654_3210,
            0x0f0f_f0f0_aaaa_5555,
            0x1234_5678_9abc_def0,
        ]);
        assert_eq!(block.extract_u64(0, 64), 0x0123_4567_89ab_cdef);
        assert_eq!(block.extract_u64(60, 64), 0xedcb_a987_6543_2100);
        assert_eq!(block.extract_u64(192, 64), 0x1234_5678_9abc_def0);
        assert_eq!(block.extract_u64(255, 1), 0);
    }

    #[test]
    fn u64_prefixes_shifts_and_lane_masks_are_exact_at_the_word_edges() {
        for lanes in [0, 1, 37, 63, 64] {
            assert_eq!(u64::active_mask(lanes).prefix_len(), Some(lanes));
        }
        assert_eq!(0b101_u64.prefix_len(), None);
        let word = 0x0123_4567_89ab_cdef_u64;
        for shift in [0, 1, 37, 63, 64] {
            for lane in 0..u64::LANE_COUNT {
                let expected = if lane + shift < u64::LANE_COUNT {
                    word.extract_u64(lane + shift, 1)
                } else {
                    0
                };
                assert_eq!(
                    word.shift_lanes_down(shift).extract_u64(lane, 1),
                    expected,
                    "shift {shift} lane {lane}"
                );
            }
        }
        assert_eq!(u64::one_hot(63), 1 << 63);
        assert_eq!(u64::one_hot(64), 0);
    }

    #[test]
    fn bit_block_lane_shifts_keep_the_undelivered_suffix_in_order() {
        let block = BitBlock::from_words([
            0x0123_4567_89ab_cdef,
            0xfedc_ba98_7654_3210,
            0x0f0f_f0f0_aaaa_5555,
            0x1234_5678_9abc_def0,
        ]);
        for shift in [0, 1, 37, 63, 64, 65, 128, 200, 255, 256] {
            let shifted = block.shift_lanes_down(shift);
            for lane in 0..BitBlock::LANE_COUNT {
                let expected = if lane + shift < BitBlock::LANE_COUNT {
                    block.extract_u64(lane + shift, 1)
                } else {
                    0
                };
                assert_eq!(
                    shifted.extract_u64(lane, 1),
                    expected,
                    "shift {shift} lane {lane}"
                );
            }
        }
    }
}
