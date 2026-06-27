use rand::{Rng, RngExt as _};

use crate::PauliBasis;

use super::operation::{SINGLE_QUBIT_PAULI_CHANNEL_BASES, TWO_QUBIT_PAULI_CHANNEL_BASES};
use super::stabilizer_frame::StabilizerFrame;

pub(super) fn apply_heralded_pauli_channel(
    frame: &mut StabilizerFrame,
    qubit: usize,
    probabilities: &[f64; 4],
    measurements: &mut Vec<bool>,
    rng: &mut impl Rng,
) {
    let [i_probability, x_probability, y_probability, z_probability] = *probabilities;
    let mut sampled_probability = rng.random::<f64>();
    if sampled_probability < i_probability {
        measurements.push(true);
        return;
    }
    sampled_probability -= i_probability;
    for (basis, probability) in [
        (PauliBasis::X, x_probability),
        (PauliBasis::Y, y_probability),
        (PauliBasis::Z, z_probability),
    ] {
        if sampled_probability < probability {
            measurements.push(true);
            frame.apply_pauli(qubit, basis);
            return;
        }
        sampled_probability -= probability;
    }
    measurements.push(false);
}

pub(super) fn apply_single_qubit_pauli_channel(
    frame: &mut StabilizerFrame,
    qubit: usize,
    probabilities: &[f64; 3],
    total_probability: f64,
    rng: &mut impl Rng,
) {
    let mut sampled_probability = rng.random::<f64>();
    if sampled_probability >= total_probability {
        return;
    }
    for (basis, probability) in SINGLE_QUBIT_PAULI_CHANNEL_BASES
        .into_iter()
        .zip(probabilities.iter().copied())
    {
        if sampled_probability < probability {
            frame.apply_pauli(qubit, basis);
            return;
        }
        sampled_probability -= probability;
    }
}

pub(super) fn apply_correlated_error(
    frame: &mut StabilizerFrame,
    terms: &[(usize, PauliBasis)],
    probability: f64,
    else_branch: bool,
    correlated_error_occurred: &mut bool,
    rng: &mut impl Rng,
) {
    if else_branch && *correlated_error_occurred {
        return;
    }
    if rng.random::<f64>() < probability {
        for (qubit, basis) in terms {
            frame.apply_pauli(*qubit, *basis);
        }
        *correlated_error_occurred = true;
    } else if !else_branch {
        *correlated_error_occurred = false;
    }
}

pub(super) fn apply_two_qubit_pauli_channel(
    frame: &mut StabilizerFrame,
    left: usize,
    right: usize,
    probabilities: &[f64; 15],
    total_probability: f64,
    rng: &mut impl Rng,
) {
    let mut sampled_probability = rng.random::<f64>();
    if sampled_probability >= total_probability {
        return;
    }
    for ((left_basis, right_basis), probability) in TWO_QUBIT_PAULI_CHANNEL_BASES
        .into_iter()
        .zip(probabilities.iter().copied())
    {
        if sampled_probability < probability {
            if let Some(basis) = left_basis {
                frame.apply_pauli(left, basis);
            }
            if let Some(basis) = right_basis {
                frame.apply_pauli(right, basis);
            }
            return;
        }
        sampled_probability -= probability;
    }
}
