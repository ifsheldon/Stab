use rand::{Rng, RngExt as _};

use crate::detection::error::{DetectionError, DetectionResult};

#[derive(Clone, Copy)]
pub(super) enum FrameExecutionMode<'a> {
    Sample {
        active_mask: u64,
    },
    SweepCorrection {
        sweep_planes: &'a [u64],
        active_mask: u64,
    },
}

impl FrameExecutionMode<'_> {
    pub(super) fn random_mask(self, rng: &mut impl Rng, probability: f64) -> u64 {
        match self {
            Self::Sample { active_mask } => sample_bernoulli_mask(probability, active_mask, rng),
            Self::SweepCorrection { .. } => 0,
        }
    }

    pub(super) const fn samples_noise(self) -> bool {
        matches!(self, Self::Sample { .. })
    }

    pub(super) fn sweep_mask(self, id: u32) -> u64 {
        match self {
            Self::Sample { .. } => 0,
            Self::SweepCorrection {
                sweep_planes,
                active_mask,
            } => {
                usize::try_from(id)
                    .ok()
                    .and_then(|index| sweep_planes.get(index))
                    .copied()
                    .unwrap_or(0)
                    & active_mask
            }
        }
    }

    pub(super) const fn active_mask(self) -> u64 {
        match self {
            Self::Sample { active_mask } => active_mask,
            Self::SweepCorrection { active_mask, .. } => active_mask,
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

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the finite nonnegative geometric gap is bounded by the remaining lane count before conversion"
)]
pub(super) fn sample_bernoulli_mask(probability: f64, lanes: u64, rng: &mut impl Rng) -> u64 {
    if probability <= 0.0 || lanes == 0 {
        return 0;
    }
    if probability >= 1.0 {
        return lanes;
    }
    if probability == 0.5 {
        return rng.random::<u64>() & lanes;
    }
    if probability > 0.5 {
        return lanes & !sample_bernoulli_mask(1.0 - probability, lanes, rng);
    }

    let lane_count = (u64::BITS - lanes.leading_zeros()) as usize;
    let log_failure = (-probability).ln_1p();
    let mut result = 0_u64;
    let mut lane = 0_usize;
    while lane < lane_count {
        let uniform = 1.0 - rng.random::<f64>();
        let gap = (uniform.ln() / log_failure).floor();
        if !gap.is_finite() || gap >= lane_count.saturating_sub(lane) as f64 {
            break;
        }
        lane = lane.saturating_add(gap as usize);
        if lane >= lane_count {
            break;
        }
        result |= 1_u64 << lane;
        lane = lane.saturating_add(1);
    }
    result & lanes
}

pub(super) fn sample_categorical_masks<const N: usize>(
    probabilities: [f64; N],
    lanes: u64,
    rng: &mut impl Rng,
) -> [u64; N] {
    let total = probabilities.iter().copied().sum::<f64>();
    let occurred = sample_bernoulli_mask(total, lanes, rng);
    let mut masks = [0_u64; N];
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
pub(super) fn visit_sparse_categorical_events<const N: usize>(
    probabilities: [f64; N],
    item_count: usize,
    lanes: u64,
    rng: &mut impl Rng,
    mut visitor: impl FnMut(usize, usize, u64) -> DetectionResult<()>,
) -> DetectionResult<bool> {
    let total = probabilities.iter().copied().sum::<f64>();
    if total <= 0.0 || item_count == 0 || lanes == 0 {
        return Ok(true);
    }
    if total > 0.125 || total >= 1.0 {
        return Ok(false);
    }
    let lane_count = (u64::BITS - lanes.leading_zeros()) as usize;
    if lanes != active_mask(lane_count) {
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
        let lane_mask = 1_u64 << (position % lane_count);
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

pub(super) fn for_each_set_lane(mut lanes: u64, mut visitor: impl FnMut(u64)) {
    while lanes != 0 {
        let lane = lanes.isolate_lowest_one();
        visitor(lane);
        lanes &= lanes - 1;
    }
}

pub(in crate::detection) fn batch_active_mask(shot_count: usize) -> u64 {
    active_mask(shot_count)
}
