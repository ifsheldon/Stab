use rand::{Rng, RngExt as _};

use super::ExecutionMode;

pub(super) fn sample(probability: f64, rng: &mut impl Rng, mode: ExecutionMode) -> bool {
    match mode {
        ExecutionMode::Sample => rng.random::<f64>() < probability,
        // Stim's reference sample is strictly noiseless: `aliased_noiseless_circuit` drops
        // result-flip probabilities before the reference run, so even a certain flip must not
        // invert the reference bit. The direct-Z fast path already encodes this by using only
        // the inversion flag as its reference bit.
        ExecutionMode::ReferenceSample => false,
    }
}
