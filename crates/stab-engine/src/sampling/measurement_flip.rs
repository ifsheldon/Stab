use rand::{Rng, RngExt as _};

use super::ExecutionMode;

pub(super) fn sample(probability: f64, rng: &mut impl Rng, mode: ExecutionMode) -> bool {
    match mode {
        ExecutionMode::Sample => rng.random::<f64>() < probability,
        ExecutionMode::ReferenceSample => deterministic_value(probability),
    }
}

pub(super) fn is_deterministic(probability: f64) -> bool {
    probability == 0.0 || probability == 1.0
}

pub(super) fn deterministic_value(probability: f64) -> bool {
    probability == 1.0
}
