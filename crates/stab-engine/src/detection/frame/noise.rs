use rand::{Rng, RngExt as _};

use super::word::FrameWord;
use crate::detection::error::{DetectionError, DetectionResult};
use crate::work_poll::{NOISE_EVENT_UNITS, WorkPoll};

#[derive(Clone, Copy)]
pub(super) enum FrameExecutionMode<'a, W> {
    Sample {
        active_lanes: usize,
    },
    SweepCorrection {
        sweep_planes: &'a [W],
        active_lanes: usize,
    },
}

impl<W: FrameWord> FrameExecutionMode<'_, W> {
    pub(super) fn random_mask(self, rng: &mut impl Rng, probability: f64) -> W {
        match self {
            Self::Sample { active_lanes } => {
                W::sample_mask(probability, W::active_mask(active_lanes), rng)
            }
            Self::SweepCorrection { .. } => W::default(),
        }
    }

    /// Returns the random frame component a collapse writes, or `None` when it must stay put.
    ///
    /// Sampling randomizes the component the collapsed state stabilizes, so later measurements
    /// anticommuting with the collapse come out uniformly random. Sweep correction follows Stim's
    /// measurements-to-detection-events frame, which disables that randomization and leaves
    /// the component untouched: overwriting it with zero would discard a sweep-controlled
    /// Pauli that a later basis change turns into a measured flip.
    pub(super) fn collapse_mask(self, rng: &mut impl Rng) -> Option<W> {
        match self {
            Self::Sample { .. } => Some(self.random_mask(rng, 0.5)),
            Self::SweepCorrection { .. } => None,
        }
    }

    pub(super) const fn samples_noise(self) -> bool {
        matches!(self, Self::Sample { .. })
    }

    pub(super) fn sweep_mask(self, id: u32) -> W {
        match self {
            Self::Sample { .. } => W::default(),
            Self::SweepCorrection {
                sweep_planes,
                active_lanes,
            } => {
                usize::try_from(id)
                    .ok()
                    .and_then(|index| sweep_planes.get(index))
                    .copied()
                    .unwrap_or_default()
                    & W::active_mask(active_lanes)
            }
        }
    }

    pub(super) fn active_mask(self) -> W {
        match self {
            Self::Sample { active_lanes } | Self::SweepCorrection { active_lanes, .. } => {
                W::active_mask(active_lanes)
            }
        }
    }
}

pub(super) fn sample_categorical_masks<W: FrameWord, const N: usize>(
    probabilities: [f64; N],
    lanes: W,
    rng: &mut impl Rng,
) -> [W; N] {
    let total = probabilities.iter().copied().sum::<f64>();
    let occurred = W::sample_mask(total, lanes, rng);
    let mut masks = [W::default(); N];
    for_each_set_lane(occurred, |lane_mask| {
        let mut value = rng.random::<f64>() * total;
        for (mask, probability) in masks.iter_mut().zip(probabilities) {
            if value < probability {
                *mask |= lane_mask;
                return;
            }
            value -= probability;
        }
        if let Some(mask) = masks.last_mut() {
            *mask |= lane_mask;
        }
    });
    masks
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the finite nonnegative geometric gap is bounded by the remaining event positions before conversion"
)]
pub(super) fn visit_sparse_categorical_events<W: FrameWord, const N: usize>(
    probabilities: [f64; N],
    item_count: usize,
    lanes: W,
    rng: &mut impl Rng,
    poll: &mut WorkPoll<'_>,
    visitor: impl FnMut(usize, usize, W) -> DetectionResult<()>,
) -> DetectionResult<bool> {
    let total = probabilities.iter().copied().sum::<f64>();
    let Some(lane_count) = lanes.prefix_len() else {
        return Ok(false);
    };
    if total <= 0.0 || item_count == 0 || lane_count == 0 {
        return Ok(true);
    }
    if total > 0.125 || total >= 1.0 {
        return Ok(false);
    }
    let position_count = item_count.checked_mul(lane_count).ok_or_else(|| {
        DetectionError::invalid_sampler_compilation(
            "categorical detector-frame event space overflowed",
        )
    })?;
    if position_count <= UNPOLLED_POSITIONS {
        // At most 2^20 positions at a rate of at most 1/8 expect at most 2^17 events; they are
        // charged up front, which keeps the common channel's event loop free of polls.
        let expected_events = (position_count as f64 * total).ceil() as usize + 1;
        poll.charge(expected_events.saturating_mul(NOISE_EVENT_UNITS))?;
        sparse_event_loop::<W, N, false>(
            probabilities,
            total,
            position_count,
            lane_count,
            rng,
            poll,
            visitor,
        )
    } else {
        polled_sparse_event_loop(
            probabilities,
            total,
            position_count,
            lane_count,
            rng,
            poll,
            visitor,
        )
    }
}

/// The polled walk of a channel with more than [`UNPOLLED_POSITIONS`] positions, out of line so
/// the common unpolled walk compiles alone into its caller.
#[inline(never)]
fn polled_sparse_event_loop<W: FrameWord, const N: usize>(
    probabilities: [f64; N],
    total: f64,
    position_count: usize,
    lane_count: usize,
    rng: &mut impl Rng,
    poll: &mut WorkPoll<'_>,
    visitor: impl FnMut(usize, usize, W) -> DetectionResult<()>,
) -> DetectionResult<bool> {
    sparse_event_loop::<W, N, true>(
        probabilities,
        total,
        position_count,
        lane_count,
        rng,
        poll,
        visitor,
    )
}

/// Visits the events of `position_count` positions; when `POLLED`, it charges `poll` once per
/// [`EVENTS_PER_CHARGE`] events.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the finite nonnegative geometric gap is bounded by the remaining event positions before conversion"
)]
#[inline(always)]
fn sparse_event_loop<W: FrameWord, const N: usize, const POLLED: bool>(
    probabilities: [f64; N],
    total: f64,
    position_count: usize,
    lane_count: usize,
    rng: &mut impl Rng,
    poll: &mut WorkPoll<'_>,
    mut visitor: impl FnMut(usize, usize, W) -> DetectionResult<()>,
) -> DetectionResult<bool> {
    let log_failure = (-total).ln_1p();
    let mut position = 0_usize;
    let mut charged_until = 0_usize;
    let mut events = 0_usize;
    while position < position_count {
        if POLLED {
            if events == charged_until {
                poll.charge(EVENTS_PER_CHARGE * NOISE_EVENT_UNITS)?;
                charged_until = events + EVENTS_PER_CHARGE;
            }
            events += 1;
        }
        let uniform = 1.0 - rng.random::<f64>();
        let gap = (uniform.ln() / log_failure).floor();
        if !gap.is_finite() || gap >= position_count.saturating_sub(position) as f64 {
            break;
        }
        position = position.saturating_add(gap as usize);
        if position >= position_count {
            break;
        }
        let item_index = position / lane_count;
        let lane_mask = W::one_hot(position % lane_count);
        let mut category_value = rng.random::<f64>() * total;
        let mut category = N.saturating_sub(1);
        for (index, probability) in probabilities.iter().copied().enumerate() {
            if category_value < probability {
                category = index;
                break;
            }
            category_value -= probability;
        }
        visitor(item_index, category, lane_mask)?;
        position = position.saturating_add(1);
    }
    Ok(true)
}

/// Event positions (items times lanes) up to which a sparse channel is charged up front and
/// runs without polls.
const UNPOLLED_POSITIONS: usize = 1 << 20;

/// Sparse noise events charged to the work poll at once.
const EVENTS_PER_CHARGE: usize = 256;

pub(super) fn for_each_set_lane<W: FrameWord>(mut lanes: W, mut visitor: impl FnMut(W)) {
    while let Some(lane) = lanes.isolate_lowest_one() {
        visitor(lane);
        lanes = lanes.clear_lowest_one();
    }
}
