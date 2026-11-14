//! The lane width of a sampling session's Pauli-frame batches.

use stab_bits::{BIT_BLOCK_WORDS, BitBlock};

use crate::ShotCount;

/// Shots one Pauli-frame batch of a circuit-sampling or detection-sampling session samples
/// together.
///
/// A session samples whole batches of this many lanes and keeps lanes it has not delivered for
/// its next run, so its seeded output is a function of the plan, the seed, and this width:
/// partitioning a seeded run across calls on one session reproduces the combined call at any
/// width, while two sessions with the same seed and different widths draw different streams with
/// the same distribution.
///
/// The default is the widest width, which has the lowest per-shot cost on most circuits once a
/// request fills its batches. A narrow width does a quarter of the frame work and holds a quarter
/// of the plane storage per batch, which is what a request for a few shots on a wide circuit
/// needs; use
/// [`LaneWidth::for_total_shots`] when the total shot count is known before the session is
/// created.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LaneWidth {
    /// 64 lanes, one `u64` per frame plane.
    Lanes64,
    /// 256 lanes, one [`BitBlock`] per frame plane.
    #[default]
    Lanes256,
}

impl LaneWidth {
    /// Every width, narrowest first.
    pub const ALL: &'static [Self] = &[Self::Lanes64, Self::Lanes256];

    /// The widest width, which is also the default.
    pub const WIDEST: Self = Self::Lanes256;

    /// Requests of at least this many shots always use [`LaneWidth::WIDEST`]: two of its
    /// batches, past which the wide frame's lower per-lane cost outweighs the lanes a narrower
    /// width would save.
    pub const WIDEST_FROM_TOTAL_SHOTS: u64 = 2 * Self::WIDEST.shots_per_batch() as u64;

    /// The width a session expecting `total_shots` shots in all should use.
    ///
    /// Below [`Self::WIDEST_FROM_TOTAL_SHOTS`] this is the width that samples the fewest lanes for
    /// the whole request, preferring the wider width when two sample the same number; from there
    /// on it is [`Self::WIDEST`]. With the shipped widths, totals of 1 to 192 and 257 to 448
    /// shots select [`LaneWidth::Lanes64`], and every other total selects
    /// [`LaneWidth::Lanes256`].
    pub const fn for_total_shots(total_shots: ShotCount) -> Self {
        let total = total_shots.get();
        if total >= Self::WIDEST_FROM_TOTAL_SHOTS {
            return Self::WIDEST;
        }
        // Visit widest first and replace only on strictly fewer lanes, so ties keep the wider.
        let mut best = Self::WIDEST;
        let mut remaining = Self::ALL;
        while let [narrower @ .., width] = remaining {
            if width.sampled_lanes(total) < best.sampled_lanes(total) {
                best = *width;
            }
            remaining = narrower;
        }
        best
    }

    /// Lanes sampled to deliver `total` shots in whole batches of this width.
    const fn sampled_lanes(self, total: u64) -> u64 {
        let lanes = self.shots_per_batch() as u64;
        total.div_ceil(lanes) * lanes
    }

    /// Shots sampled together in one batch, and the most shots one delivered batch holds.
    pub const fn shots_per_batch(self) -> usize {
        match self {
            Self::Lanes64 => u64::BITS as usize,
            Self::Lanes256 => BIT_BLOCK_WORDS * u64::BITS as usize,
        }
    }

    /// Bytes of one frame plane at this width.
    pub(crate) const fn plane_bytes(self) -> usize {
        match self {
            Self::Lanes64 => size_of::<u64>(),
            Self::Lanes256 => size_of::<BitBlock>(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_shot_selection_samples_the_fewest_lanes_below_two_wide_batches() {
        let narrow =
            |shots: u64| LaneWidth::for_total_shots(ShotCount::new(shots)) == LaneWidth::Lanes64;
        let narrow_totals = (0..=1024)
            .filter(|shots| narrow(*shots))
            .collect::<Vec<_>>();
        let expected = (1..=192).chain(257..=448).collect::<Vec<_>>();
        assert_eq!(narrow_totals, expected);
        assert!(!narrow(u64::MAX));
    }
}
