use rand::{Rng, RngExt as _};
use stab_algebra::PauliBasis;
use stab_model::{
    CircuitInstruction, GateCategory, MeasureRecordOffset, Pauli, Probability, QubitId, Target,
};

use super::super::error::{DetectionError, DetectionResult};

pub(super) fn frame_bit(bits: &[bool], qubit: usize) -> DetectionResult<bool> {
    bits.get(qubit)
        .copied()
        .ok_or_else(|| frame_qubit_out_of_range(qubit))
}

pub(super) fn set_frame_bit(bits: &mut [bool], qubit: usize, value: bool) -> DetectionResult<()> {
    let bit = bits
        .get_mut(qubit)
        .ok_or_else(|| frame_qubit_out_of_range(qubit))?;
    *bit = value;
    Ok(())
}

pub(super) fn xor_frame_bit(bits: &mut [bool], qubit: usize, value: bool) -> DetectionResult<()> {
    let bit = bits
        .get_mut(qubit)
        .ok_or_else(|| frame_qubit_out_of_range(qubit))?;
    *bit ^= value;
    Ok(())
}

fn frame_qubit_out_of_range(qubit: usize) -> DetectionError {
    DetectionError::invalid_sampler_compilation(format!(
        "qubit target {qubit} is outside the detector frame state"
    ))
}

pub(super) fn measurement_record_bit(
    measurements: &[bool],
    offset: MeasureRecordOffset,
) -> DetectionResult<bool> {
    let len = i64::try_from(measurements.len())
        .map_err(|_| DetectionError::invalid_result_format("measurement count does not fit i64"))?;
    let index = len + i64::from(offset.get());
    let index = usize::try_from(index).map_err(|_| {
        DetectionError::invalid_result_format(format!(
            "measurement record target rec[{}] is not available",
            offset.stim_text()
        ))
    })?;
    measurements.get(index).copied().ok_or_else(|| {
        DetectionError::invalid_result_format(format!(
            "measurement record target rec[{}] is not available",
            offset.stim_text()
        ))
    })
}

pub(super) fn sample_flip(probability: f64, rng: &mut impl Rng) -> bool {
    rng.random::<f64>() < probability
}

pub(super) fn single_probability_argument(
    instruction: &CircuitInstruction,
) -> DetectionResult<Probability> {
    if instruction.args().len() != 1 {
        return Err(unsupported_frame_instruction(instruction));
    }
    instruction
        .probability_argument()?
        .ok_or_else(|| unsupported_frame_instruction(instruction))
}

pub(super) fn measurement_flip_probability(
    instruction: &CircuitInstruction,
) -> DetectionResult<f64> {
    match instruction.probability_argument()? {
        None => Ok(0.0),
        Some(probability) => Ok(probability.get()),
    }
}

pub(super) fn probability_list<const N: usize>(
    instruction: &CircuitInstruction,
) -> DetectionResult<[f64; N]> {
    if instruction.args().len() != N {
        return Err(unsupported_frame_instruction(instruction));
    }
    let mut values = [0.0; N];
    values.copy_from_slice(instruction.args());
    Ok(values)
}

pub(super) fn zero_probability_noise(instruction: &CircuitInstruction) -> DetectionResult<bool> {
    if !matches!(
        instruction.gate().category(),
        GateCategory::Noise | GateCategory::HeraldedNoise
    ) {
        return Ok(false);
    }
    Ok(instruction
        .args()
        .iter()
        .all(|probability| *probability == 0.0))
}

pub(super) fn qubit_index(
    instruction: &CircuitInstruction,
    target: &Target,
) -> DetectionResult<usize> {
    let Some(qubit) = target.qubit_id() else {
        return Err(unsupported_frame_instruction(instruction));
    };
    qubit_id_index(qubit)
}

pub(super) fn qubit_id_index(qubit: QubitId) -> DetectionResult<usize> {
    usize::try_from(qubit.get()).map_err(|_| {
        DetectionError::invalid_sampler_compilation(format!(
            "qubit target {} cannot fit in this platform's usize",
            qubit.get()
        ))
    })
}

pub(super) fn pauli_basis(pauli: Pauli) -> PauliBasis {
    match pauli {
        Pauli::X => PauliBasis::X,
        Pauli::Y => PauliBasis::Y,
        Pauli::Z => PauliBasis::Z,
    }
}

pub(super) fn sample_single_pauli(
    probabilities: [f64; 3],
    rng: &mut impl Rng,
) -> Option<PauliBasis> {
    let mut sampled_probability = rng.random::<f64>();
    for (basis, probability) in [
        (PauliBasis::X, probabilities[0]),
        (PauliBasis::Y, probabilities[1]),
        (PauliBasis::Z, probabilities[2]),
    ] {
        if sampled_probability < probability {
            return Some(basis);
        }
        sampled_probability -= probability;
    }
    None
}

pub(super) fn sample_two_qubit_pauli(
    probabilities: [f64; 15],
    rng: &mut impl Rng,
) -> Option<(Option<PauliBasis>, Option<PauliBasis>)> {
    let mut sampled_probability = rng.random::<f64>();
    for (bases, probability) in TWO_QUBIT_FRAME_BASES.into_iter().zip(probabilities) {
        if sampled_probability < probability {
            return Some(bases);
        }
        sampled_probability -= probability;
    }
    None
}

const TWO_QUBIT_FRAME_BASES: [(Option<PauliBasis>, Option<PauliBasis>); 15] = [
    (None, Some(PauliBasis::X)),
    (None, Some(PauliBasis::Y)),
    (None, Some(PauliBasis::Z)),
    (Some(PauliBasis::X), None),
    (Some(PauliBasis::X), Some(PauliBasis::X)),
    (Some(PauliBasis::X), Some(PauliBasis::Y)),
    (Some(PauliBasis::X), Some(PauliBasis::Z)),
    (Some(PauliBasis::Y), None),
    (Some(PauliBasis::Y), Some(PauliBasis::X)),
    (Some(PauliBasis::Y), Some(PauliBasis::Y)),
    (Some(PauliBasis::Y), Some(PauliBasis::Z)),
    (Some(PauliBasis::Z), None),
    (Some(PauliBasis::Z), Some(PauliBasis::X)),
    (Some(PauliBasis::Z), Some(PauliBasis::Y)),
    (Some(PauliBasis::Z), Some(PauliBasis::Z)),
];

pub(super) fn unsupported_frame_instruction(instruction: &CircuitInstruction) -> DetectionError {
    DetectionError::invalid_sampler_compilation(format!(
        "M9 detector frame subset does not support {}",
        instruction.gate().canonical_name()
    ))
}

pub(super) fn unsupported_frame_target(gate_name: &str, target: &Target) -> DetectionError {
    DetectionError::invalid_sampler_compilation(format!(
        "gate {gate_name} has non-qubit frame target {target}"
    ))
}
