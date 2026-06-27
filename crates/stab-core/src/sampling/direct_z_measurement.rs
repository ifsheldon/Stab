use rand::{Rng, RngExt as _};

use crate::PauliBasis;

use super::operation::SampleOperation;

#[derive(Clone, Copy, Debug, PartialEq)]
struct DirectZMeasurementPlan {
    pauli_flip_probability: Option<f64>,
    measurement_flip_probability: f64,
    inverted: bool,
}

pub(super) fn sample_zero_one_bytes<R>(
    operations: &[SampleOperation],
    measurement_count: usize,
    shots: usize,
    rng: &mut R,
) -> Option<Vec<u8>>
where
    R: Rng,
{
    let plan = direct_z_measurement_plan(operations, measurement_count)?;
    let mut bytes = Vec::with_capacity(shots.checked_mul(2).unwrap_or(0));
    for _ in 0..shots {
        let mut bit = plan.inverted;
        if let Some(probability) = plan.pauli_flip_probability {
            bit ^= rng.random::<f64>() < probability;
        }
        bit ^= rng.random::<f64>() < plan.measurement_flip_probability;
        bytes.push(if bit { b'1' } else { b'0' });
        bytes.push(b'\n');
    }
    Some(bytes)
}

fn direct_z_measurement_plan(
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

fn z_measurement_pauli_flip_probability(probabilities: &[f64; 3]) -> f64 {
    let [x_probability, y_probability, _z_probability] = *probabilities;
    x_probability + y_probability
}
