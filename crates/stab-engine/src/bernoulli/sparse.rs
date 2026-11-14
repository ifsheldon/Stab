//! Hit-position sampling for sparse probabilities on the indexed block streams.
//!
//! [`SparseBernoulli`] reproduces exactly the hits that [`super::sample_indexed_range_into`]
//! would set for a probability on the geometric-skip branch, but reports them as positions
//! instead of materialising, copying, and scanning a block mask. Each aligned block keeps its
//! independent substream, its draw order, and its gap formula, so both paths agree bit for bit.

use super::{
    INDEXED_BLOCK_SHOTS, IndexedRangeError, RandomSource as _, SPARSE_PROBABILITY_BOUND,
    SplitMixSource, geometric_gap, indexed_block_key, indexed_seed_from_keys, indexed_stream_key,
};

/// Relative guard band below the exact first-draw miss bound.
///
/// A first draw `u <= exp(1024 ln(1 - p)) (1 - 2^-20)` has `ln(u) <= 1024 ln(1 - p) - 2^-20`, so
/// even after the rounding of `ln`, the division, `exp`, and the threshold product (a few
/// units in the last place, at most about 1e-15 relative) the computed gap stays strictly
/// above 1024 for every sparse probability, whose block log-failure magnitude is below 21.
const FIRST_DRAW_MISS_GUARD: f64 = 1.0 / (1_u64 << 20) as f64;

/// A validated shot range of at most one indexed block's width.
///
/// Such a range overlaps at most two aligned blocks, whose substream keys are computed once here
/// instead of once per sampled error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IndexedShotRange {
    first_shot: u64,
    shot_count: usize,
    block_keys: [u64; 2],
}

impl IndexedShotRange {
    pub(crate) fn try_new(first_shot: u64, shot_count: usize) -> Result<Self, IndexedRangeError> {
        if shot_count > INDEXED_BLOCK_SHOTS {
            return Err(IndexedRangeError::TooManyShots);
        }
        let shot_count_u64 =
            u64::try_from(shot_count).map_err(|_| IndexedRangeError::TooManyShots)?;
        first_shot
            .checked_add(shot_count_u64)
            .ok_or(IndexedRangeError::ShotRangeOverflow)?;
        // The first block index is at most `u64::MAX / 1024`, so its successor cannot wrap.
        let first_block = first_shot / INDEXED_BLOCK_SHOTS as u64;
        Ok(Self {
            first_shot,
            shot_count,
            block_keys: [
                indexed_block_key(first_block),
                indexed_block_key(first_block + 1),
            ],
        })
    }

    pub(crate) const fn first_shot(self) -> u64 {
        self.first_shot
    }

    pub(crate) const fn shot_count(self) -> usize {
        self.shot_count
    }

    /// Visits each aligned block overlapping the range as
    /// `(block_key, block_offset, take, output_offset)`.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a remainder modulo the 1024-shot block width always fits in usize"
    )]
    fn try_for_each_block<E>(
        self,
        mut visit: impl FnMut(u64, usize, usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let block_width = INDEXED_BLOCK_SHOTS as u64;
        let mut produced = 0_usize;
        for block_key in self.block_keys {
            if produced >= self.shot_count {
                break;
            }
            // Construction proved `first_shot + shot_count` fits, and `produced < shot_count`.
            let global_shot = self.first_shot + produced as u64;
            let block_offset = (global_shot % block_width) as usize;
            let take = (INDEXED_BLOCK_SHOTS - block_offset).min(self.shot_count - produced);
            visit(block_key, block_offset, take, produced)?;
            produced += take;
        }
        Ok(())
    }
}

/// Per-probability constants of the geometric-skip sampler.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SparseBernoulli {
    log_failure: f64,
    /// First draws at or below this value certainly yield no hit within a whole block.
    first_draw_miss_bound: f64,
}

impl SparseBernoulli {
    /// Returns the sparse sampler when `probability` is on the geometric-skip branch.
    #[allow(
        clippy::cast_precision_loss,
        reason = "the 1024-shot block width is exactly representable in f64"
    )]
    pub(crate) fn new(probability: f64) -> Option<Self> {
        if !(probability > 0.0 && probability < SPARSE_PROBABILITY_BOUND) {
            return None;
        }
        let log_failure = (-probability).ln_1p();
        let block_log_failure = INDEXED_BLOCK_SHOTS as f64 * log_failure;
        Some(Self {
            log_failure,
            first_draw_miss_bound: block_log_failure.exp() * (1.0 - FIRST_DRAW_MISS_GUARD),
        })
    }

    /// Calls `on_hit` with the range-relative position of every sampled hit, in increasing order.
    ///
    /// The hits are exactly the set bits that `sample_indexed_range_into` produces for the same
    /// probability, seed, stream, and range.
    pub(crate) fn try_for_each_hit<E>(
        &self,
        seed: u64,
        stream_index: u64,
        range: IndexedShotRange,
        mut on_hit: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let stream_key = indexed_stream_key(stream_index);
        range.try_for_each_block(|block_key, block_offset, take, output_offset| {
            self.try_for_each_block_hit(
                indexed_seed_from_keys(seed, stream_key, block_key),
                block_offset,
                block_offset + take,
                |position| on_hit(position - block_offset + output_offset),
            )
        })
    }

    /// Walks one block's gap sequence, reporting block positions in `window_start..window_end`.
    ///
    /// The draws and the termination rule are those of `or_sparse_hits` over a whole block; the
    /// walk only stops early once no later position can land in the window.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "the finite nonnegative geometric gap is bounded by the remaining bit count before conversion"
    )]
    #[inline]
    fn try_for_each_block_hit<E>(
        &self,
        block_seed: u64,
        window_start: usize,
        window_end: usize,
        mut on_hit: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut rng = SplitMixSource::new(block_seed);
        let mut uniform = 1.0 - rng.next_unit_f64();
        if uniform <= self.first_draw_miss_bound {
            return Ok(());
        }
        let mut position = 0_usize;
        loop {
            let gap = geometric_gap(uniform, self.log_failure);
            if !gap.is_finite() || gap >= (INDEXED_BLOCK_SHOTS - position) as f64 {
                return Ok(());
            }
            position += gap as usize;
            if position >= window_end {
                return Ok(());
            }
            if position >= window_start {
                on_hit(position)?;
            }
            position += 1;
            if position >= INDEXED_BLOCK_SHOTS {
                return Ok(());
            }
            uniform = 1.0 - rng.next_unit_f64();
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        reason = "focused kernel tests use valid fixed-size ranges and fail immediately on fixture errors"
    )]

    use std::convert::Infallible;

    use super::super::sample_indexed_range_into;
    use super::*;

    const PROBABILITIES: [f64; 10] = [
        f64::MIN_POSITIVE,
        1e-300,
        1e-12,
        6.669_779_853_440_971e-5,
        1e-3,
        0.004,
        0.01,
        0.019,
        0.019_999_999_999,
        0.02 - f64::EPSILON / 64.0,
    ];

    fn sparse_hits(
        sparse: &SparseBernoulli,
        seed: u64,
        stream_index: u64,
        first_shot: u64,
        shot_count: usize,
    ) -> Vec<u64> {
        let mut words = vec![0_u64; shot_count.div_ceil(64)];
        let mut last = None;
        let range = IndexedShotRange::try_new(first_shot, shot_count).expect("valid range");
        sparse
            .try_for_each_hit(seed, stream_index, range, |position| {
                assert!(position < shot_count, "hit {position} escaped the range");
                assert!(last < Some(position), "hits must be strictly increasing");
                last = Some(position);
                words[position / 64] ^= 1 << (position % 64);
                Ok::<(), Infallible>(())
            })
            .expect("infallible visitor");
        words
    }

    #[test]
    fn sparse_hits_match_the_mask_sampler_on_every_range_shape() {
        let ranges: [(u64, usize); 14] = [
            (0, 1024),
            (0, 1),
            (0, 63),
            (0, 64),
            (0, 1000),
            (5, 1),
            (17, 47),
            (63, 2),
            (100, 924),
            (1000, 24),
            (1000, 64),
            (1000, 1024),
            (3 * 1024, 1024),
            (u64::MAX - 1024, 1024),
        ];
        let mut checked_hits = 0_u32;
        for probability in PROBABILITIES {
            let sparse = SparseBernoulli::new(probability).expect("sparse probability");
            for (seed, stream_index) in [(0, 0), (13, 7), (u64::MAX, 123_456)] {
                for (first_shot, shot_count) in ranges {
                    let mut expected = vec![0_u64; shot_count.div_ceil(64)];
                    sample_indexed_range_into(
                        probability,
                        seed,
                        stream_index,
                        first_shot,
                        shot_count,
                        &mut expected,
                    )
                    .expect("valid indexed range");
                    let actual = sparse_hits(&sparse, seed, stream_index, first_shot, shot_count);
                    assert_eq!(
                        actual, expected,
                        "p={probability} seed={seed} stream={stream_index} range={first_shot}+{shot_count}"
                    );
                    checked_hits += expected.iter().map(|word| word.count_ones()).sum::<u32>();
                }
            }
        }
        assert!(checked_hits > 1000, "fixture must exercise many hits");
    }

    #[test]
    fn first_draw_shortcut_never_changes_an_outcome() {
        for probability in PROBABILITIES {
            let sparse = SparseBernoulli::new(probability).expect("sparse probability");
            let bound = sparse.first_draw_miss_bound;
            let mut uniform = bound;
            for _ in 0..4096 {
                let gap = geometric_gap(uniform, sparse.log_failure);
                assert!(
                    !gap.is_finite() || gap >= 1024.0,
                    "p={probability} u={uniform} skipped a hit at gap {gap}"
                );
                uniform = uniform.next_down();
            }
            for scale in [0.5, 1e-3, 1e-12] {
                let gap = geometric_gap(bound * scale, sparse.log_failure);
                assert!(!gap.is_finite() || gap >= 1024.0);
            }
        }
    }

    #[test]
    fn only_the_geometric_skip_branch_is_sparse() {
        for probability in [0.0, -0.0, 0.02, 0.25, 0.5, 0.99, 1.0, f64::NAN] {
            assert_eq!(SparseBernoulli::new(probability), None, "p={probability}");
        }
    }
}
