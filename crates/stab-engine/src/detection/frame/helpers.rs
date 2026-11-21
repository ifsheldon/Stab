use stab_algebra::PauliBasis;
use stab_model::{
    CircuitInstruction, ControlledPairs, Gate, GateArguments, GateCategory, GateTargets,
    MeasureRecordOffset, Pauli, Probability, Target,
};

use super::super::error::{DetectionError, DetectionResult};
use super::word::FrameWord;

#[inline(always)]
pub(super) fn frame_word<W: FrameWord>(bits: &[W], qubit: usize) -> DetectionResult<W> {
    bits.get(qubit)
        .copied()
        .ok_or_else(|| frame_qubit_out_of_range(qubit))
}

#[inline(always)]
pub(super) fn set_frame_word<W: FrameWord>(
    bits: &mut [W],
    qubit: usize,
    value: W,
) -> DetectionResult<()> {
    let bit = bits
        .get_mut(qubit)
        .ok_or_else(|| frame_qubit_out_of_range(qubit))?;
    *bit = value;
    Ok(())
}

#[inline(always)]
pub(super) fn xor_frame_word<W: FrameWord>(
    bits: &mut [W],
    qubit: usize,
    value: W,
) -> DetectionResult<()> {
    let bit = bits
        .get_mut(qubit)
        .ok_or_else(|| frame_qubit_out_of_range(qubit))?;
    *bit ^= value;
    Ok(())
}

pub(super) fn frame_qubit_out_of_range(qubit: usize) -> DetectionError {
    DetectionError::invalid_sampler_compilation(format!(
        "qubit target {qubit} is outside the detector frame state"
    ))
}

pub(super) fn measurement_record_word<W: FrameWord>(
    measurements: &[W],
    offset: MeasureRecordOffset,
) -> DetectionResult<W> {
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

/// The probability of a gate whose argument rule is exactly one probability.
pub(super) fn single_probability_argument(
    instruction: &CircuitInstruction,
) -> DetectionResult<Probability> {
    match instruction.gate_arguments() {
        GateArguments::Probability(probability) => Ok(probability),
        _ => Err(unsupported_frame_instruction(instruction)),
    }
}

/// The optional flip probability of a measurement, zero when it is absent.
pub(super) fn measurement_flip_probability(instruction: &CircuitInstruction) -> f64 {
    instruction.probability().map_or(0.0, Probability::get)
}

/// The target pairs of a controlled-Pauli gate, which the lowering reaches only for gates whose
/// target rule is classical-control pairs.
pub(super) fn controlled_target_pairs(
    instruction: &CircuitInstruction,
) -> DetectionResult<ControlledPairs<'_>> {
    match instruction.gate_targets() {
        GateTargets::ControlledPairs(pairs) => Ok(pairs),
        _ => Err(unsupported_frame_instruction(instruction)),
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

pub(super) fn pauli_basis(pauli: Pauli) -> PauliBasis {
    match pauli {
        Pauli::X => PauliBasis::X,
        Pauli::Y => PauliBasis::Y,
        Pauli::Z => PauliBasis::Z,
    }
}

pub(super) const TWO_QUBIT_FRAME_BASES: [(Option<PauliBasis>, Option<PauliBasis>); 15] = [
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
        "detector frame execution does not support {}",
        instruction.gate().canonical_name()
    ))
}

pub(super) fn unsupported_frame_target(gate: Gate, target: &Target) -> DetectionError {
    DetectionError::invalid_sampler_compilation(format!(
        "gate {} has non-qubit frame target {target}",
        gate.canonical_name()
    ))
}
