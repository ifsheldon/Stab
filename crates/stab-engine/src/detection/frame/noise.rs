use rand::{Rng, RngExt as _};

use super::word::FrameWord;
use crate::detection::error::{DetectionError, DetectionResult};

#[derive(Clone, Copy)]
pub(super) enum FrameExecutionMode<'a> {
    Sample {
        active_lanes: usize,
    },
    SweepCorrection {
        sweep_planes: &'a [u64],
        active_lanes: usize,
    },
}

impl FrameExecutionMode<'_> {
    pub(super) fn random_mask<W: FrameWord>(self, rng: &mut impl Rng, probability: f64) -> W {
        match self {
            Self::Sample { active_lanes } => {
                W::sample_mask(probability, W::active_mask(active_lanes), rng)
            }
            Self::SweepCorrection { .. } => W::default(),
        }
    }

    pub(super) const fn samples_noise(self) -> bool {
        matches!(self, Self::Sample { .. })
    }

    pub(super) fn sweep_mask<W: FrameWord>(self, id: u32) -> W {
        match self {
            Self::Sample { .. } => W::default(),
            Self::SweepCorrection {
                sweep_planes,
                active_lanes,
            } => {
                let word = usize::try_from(id)
                    .ok()
                    .and_then(|index| sweep_planes.get(index))
                    .copied()
                    .unwrap_or(0)
                    & active_mask(active_lanes);
                W::from_low_word(word)
            }
        }
    }

    pub(super) fn active_mask<W: FrameWord>(self) -> W {
        match self {
            Self::Sample { active_lanes } | Self::SweepCorrection { active_lanes, .. } => {
                W::active_mask(active_lanes)
            }
        }
    }
}

fn active_mask(shot_count: usize) -> u64 {
    if shot_count >= u64::BITS as usize {
        u64::MAX
    } else if shot_count == 0 {
        0
    } else {
        (1_u64 << shot_count) - 1
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
    mut visitor: impl FnMut(usize, usize, W) -> DetectionResult<()>,
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
    let log_failure = (-total).ln_1p();
    let mut position = 0_usize;
    while position < position_count {
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

pub(super) fn for_each_set_lane<W: FrameWord>(mut lanes: W, mut visitor: impl FnMut(W)) {
    while let Some(lane) = lanes.isolate_lowest_one() {
        visitor(lane);
        lanes = lanes.clear_lowest_one();
    }
}

pub(in crate::detection) fn batch_active_mask(shot_count: usize) -> u64 {
    active_mask(shot_count)
}
