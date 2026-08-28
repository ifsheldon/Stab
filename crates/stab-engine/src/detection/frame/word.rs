use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

use rand::Rng;
use stab_bits::{BIT_BLOCK_WORDS, BitBlock};

use crate::bernoulli::{fill_words as fill_bernoulli_words, sample_mask as sample_bernoulli_mask};
use crate::detection::error::DetectionResult;

use super::BitPlaneDetectionFrame;
use super::helpers::{frame_word, set_frame_word, xor_frame_word};

pub(in crate::detection) trait FrameWord:
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
    fn from_low_word(word: u64) -> Self;
    fn prefix_len(self) -> Option<usize>;
    fn one_hot(index: usize) -> Self;
    fn isolate_lowest_one(self) -> Option<Self>;
    fn clear_lowest_one(self) -> Self;
    fn sample_mask(probability: f64, active: Self, rng: &mut impl Rng) -> Self;
    fn extract_u64(self, start: usize, count: usize) -> u64;
}

impl FrameWord for u64 {
    const LANE_COUNT: usize = u64::BITS as usize;

    fn active_mask(lanes: usize) -> Self {
        low_bits_mask(lanes)
    }

    fn from_low_word(word: u64) -> Self {
        word
    }

    fn prefix_len(self) -> Option<usize> {
        let lanes = self.trailing_ones() as usize;
        (self == low_bits_mask(lanes)).then_some(lanes)
    }

    fn one_hot(index: usize) -> Self {
        if index < Self::LANE_COUNT {
            1_u64 << index
        } else {
            0
        }
    }

    fn isolate_lowest_one(self) -> Option<Self> {
        (self != 0).then(|| u64::isolate_lowest_one(self))
    }

    fn clear_lowest_one(self) -> Self {
        self & self.wrapping_sub(1)
    }

    fn sample_mask(probability: f64, active: Self, rng: &mut impl Rng) -> Self {
        sample_bernoulli_mask(probability, active, rng)
    }

    fn extract_u64(self, start: usize, count: usize) -> u64 {
        if start >= Self::LANE_COUNT {
            return 0;
        }
        (self >> start) & low_bits_mask(count)
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

    fn from_low_word(word: u64) -> Self {
        let mut words = [0; BIT_BLOCK_WORDS];
        if let Some(low) = words.first_mut() {
            *low = word;
        }
        Self::from_words(words)
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
}
