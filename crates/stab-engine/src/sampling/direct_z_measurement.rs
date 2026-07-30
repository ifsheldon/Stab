use rand::{Rng, RngExt as _};

use stab_algebra::PauliBasis;

use super::measurement_flip;
use super::operation::SampleOperation;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DirectZMeasurementPlan {
    pauli_flip_probability: Option<f64>,
    measurement_flip_probability: f64,
    inverted: bool,
}

pub(super) fn compile(
    operations: &[SampleOperation],
    measurement_count: usize,
) -> Option<DirectZMeasurementPlan> {
    if measurement_count != 1 {
        return None;
    }
    match operations {
        [
            SampleOperation::SingleQubitPauliChannel {
                qubit,
                probabilities,
                ..
            },
            SampleOperation::Measure {
                qubit: measure_qubit,
                basis,
                inverted,
                flip_probability,
                reset,
            },
        ] if qubit == measure_qubit && *basis == PauliBasis::Z && !reset => {
            Some(DirectZMeasurementPlan {
                pauli_flip_probability: Some(z_measurement_pauli_flip_probability(probabilities)),
                measurement_flip_probability: *flip_probability,
                inverted: *inverted,
            })
        }
        [
            SampleOperation::Measure {
                basis,
                inverted,
                flip_probability,
                reset,
                ..
            },
        ] if *basis == PauliBasis::Z && !reset => Some(DirectZMeasurementPlan {
            pauli_flip_probability: None,
            measurement_flip_probability: *flip_probability,
            inverted: *inverted,
        }),
        _ => None,
    }
}

impl DirectZMeasurementPlan {
    pub(super) const fn reference_bit(self) -> bool {
        self.inverted
    }

    pub(super) fn determined_measurement_count(self, unknown_input: bool) -> u64 {
        u64::from(
            !unknown_input && measurement_flip::is_deterministic(self.measurement_flip_probability),
        )
    }

    #[inline(always)]
    pub(super) fn sample(self, rng: &mut impl Rng) -> bool {
        let mut bit = self.inverted;
        if let Some(probability) = self.pauli_flip_probability {
            bit ^= rng.random::<f64>() < probability;
        }
        bit ^= rng.random::<f64>() < self.measurement_flip_probability;
        bit
    }
}

fn z_measurement_pauli_flip_probability(probabilities: &[f64; 3]) -> f64 {
    let [x_probability, y_probability, _z_probability] = *probabilities;
    x_probability + y_probability
}
