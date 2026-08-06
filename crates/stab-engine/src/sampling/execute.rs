use rand::Rng;

use super::api::SamplingExecutionError;
use super::operation::SampleOperation;
use super::stabilizer_frame::{MeasurementRandomness, StabilizerFrame, reset_correction};
use super::{ExecutionMode, measurement_flip, noise};
use stab_model::MeasureRecordOffset;

pub(super) struct ExecutionBuffers<'a> {
    pub(super) frame: &'a mut StabilizerFrame,
    pub(super) record: &'a mut Vec<bool>,
    pub(super) output: &'a mut Vec<bool>,
    pub(super) correlated_error_occurred: &'a mut bool,
}

pub(super) fn count_determined_operations<R>(
    operations: &[SampleOperation],
    frame: &mut StabilizerFrame,
    record: &mut Vec<bool>,
    rng: &mut R,
) -> Result<u64, SamplingExecutionError>
where
    R: Rng,
{
    let mut count = 0;
    for operation in operations {
        match operation {
            SampleOperation::ApplyHadamard { qubit } => {
                frame.apply_hadamard(*qubit);
            }
            SampleOperation::ApplyControlledX { control, target } => {
                frame.apply_controlled_x(*control, *target);
            }
            SampleOperation::ApplyTableau { targets, transform } => {
                frame.apply_tableau(targets, transform);
            }
            SampleOperation::Reset { qubit, basis } => {
                frame.reset(
                    *qubit,
                    *basis,
                    rng,
                    MeasurementRandomness::DeterministicFalse,
                );
            }
            SampleOperation::Measure {
                qubit,
                basis,
                inverted,
                flip_probability: _,
                reset,
            } => {
                // Stim's count_determined_measurements strips measurement arguments, so flip
                // probabilities never affect determinism or the propagated record values.
                if frame.measure_is_deterministic(*qubit, *basis) {
                    count += 1;
                }
                let measured = frame.measure(
                    *qubit,
                    *basis,
                    *inverted,
                    rng,
                    MeasurementRandomness::DeterministicFalse,
                );
                record.push(measured);
                if *reset && (measured ^ *inverted) {
                    frame.apply_pauli(*qubit, reset_correction(*basis));
                }
            }
            SampleOperation::MeasureProduct {
                terms,
                inverted,
                flip_probability: _,
            } => {
                if frame.pauli_product_measurement_is_deterministic(terms) {
                    count += 1;
                }
                let measured = frame.measure_pauli_product(
                    terms,
                    *inverted,
                    rng,
                    MeasurementRandomness::DeterministicFalse,
                );
                record.push(measured);
            }
            SampleOperation::Pad { .. } => {
                return Err(
                    SamplingExecutionError::UnsupportedDeterminedMeasurementGate { gate: "MPAD" },
                );
            }
            SampleOperation::FeedbackPauli {
                offset,
                qubit,
                basis,
            } => {
                if record_lookback(record, *offset) {
                    frame.apply_pauli(*qubit, *basis);
                }
            }
            SampleOperation::SweepPauli { .. } => {}
            SampleOperation::Repeat { count: reps, body } => {
                for _ in 0..*reps {
                    count += count_determined_operations(body, frame, record, rng)?;
                }
            }
            SampleOperation::HeraldedPauliChannel { .. } => {
                return Err(
                    SamplingExecutionError::UnsupportedDeterminedMeasurementGate {
                        gate: "heralded noise",
                    },
                );
            }
            SampleOperation::SingleQubitPauliChannel { .. }
            | SampleOperation::TwoQubitPauliChannel { .. }
            | SampleOperation::CorrelatedError { .. } => {}
        }
    }
    Ok(count)
}

pub(super) fn record_lookback(record: &[bool], offset: MeasureRecordOffset) -> bool {
    let index = i64::try_from(record.len())
        .ok()
        .and_then(|len| len.checked_add(i64::from(offset.get())))
        .and_then(|index| usize::try_from(index).ok());
    index
        .and_then(|index| record.get(index))
        .copied()
        .unwrap_or(false)
}

pub(super) fn execute_operations(
    operations: &[SampleOperation],
    buffers: &mut ExecutionBuffers<'_>,
    rng: &mut impl Rng,
    mode: ExecutionMode,
    sweep_record: &[bool],
) {
    for operation in operations {
        match operation {
            SampleOperation::ApplyHadamard { qubit } => {
                buffers.frame.apply_hadamard(*qubit);
            }
            SampleOperation::ApplyControlledX { control, target } => {
                buffers.frame.apply_controlled_x(*control, *target);
            }
            SampleOperation::ApplyTableau { targets, transform } => {
                buffers.frame.apply_tableau(targets, transform);
            }
            SampleOperation::Reset { qubit, basis } => {
                buffers
                    .frame
                    .reset(*qubit, *basis, rng, mode.measurement_randomness());
            }
            SampleOperation::Measure {
                qubit,
                basis,
                inverted,
                flip_probability,
                reset,
            } => {
                let noisy_flip = measurement_flip::sample(*flip_probability, rng, mode);
                let result = buffers.frame.measure(
                    *qubit,
                    *basis,
                    *inverted ^ noisy_flip,
                    rng,
                    mode.measurement_randomness(),
                );
                buffers.record.push(result);
                buffers.output.push(result);
                if *reset {
                    buffers
                        .frame
                        .reset(*qubit, *basis, rng, mode.measurement_randomness());
                }
            }
            SampleOperation::MeasureProduct {
                terms,
                inverted,
                flip_probability,
            } => {
                let noisy_flip = measurement_flip::sample(*flip_probability, rng, mode);
                let result = buffers.frame.measure_pauli_product(
                    terms,
                    *inverted ^ noisy_flip,
                    rng,
                    mode.measurement_randomness(),
                );
                buffers.record.push(result);
                buffers.output.push(result);
            }
            SampleOperation::Pad {
                value,
                flip_probability,
            } => {
                let result = *value ^ measurement_flip::sample(*flip_probability, rng, mode);
                buffers.record.push(result);
                buffers.output.push(result);
            }
            SampleOperation::SingleQubitPauliChannel {
                qubit,
                probabilities,
                total_probability,
            } => {
                if mode.includes_noise() {
                    noise::apply_single_qubit_pauli_channel(
                        buffers.frame,
                        *qubit,
                        probabilities,
                        *total_probability,
                        rng,
                    );
                }
            }
            SampleOperation::TwoQubitPauliChannel {
                left,
                right,
                probabilities,
                total_probability,
            } => {
                if mode.includes_noise() {
                    noise::apply_two_qubit_pauli_channel(
                        buffers.frame,
                        *left,
                        *right,
                        probabilities,
                        *total_probability,
                        rng,
                    );
                }
            }
            SampleOperation::CorrelatedError {
                else_branch,
                probability,
                terms,
            } => {
                if mode.includes_noise() {
                    noise::apply_correlated_error(
                        buffers.frame,
                        terms,
                        *probability,
                        *else_branch,
                        buffers.correlated_error_occurred,
                        rng,
                    );
                } else if !else_branch {
                    *buffers.correlated_error_occurred = false;
                }
            }
            SampleOperation::HeraldedPauliChannel {
                qubit,
                probabilities,
            } => {
                let herald = if mode.includes_noise() {
                    noise::apply_heralded_pauli_channel(buffers.frame, *qubit, probabilities, rng)
                } else {
                    false
                };
                buffers.record.push(herald);
                buffers.output.push(herald);
            }
            SampleOperation::FeedbackPauli {
                offset,
                qubit,
                basis,
            } => {
                if measurement_record_bit(buffers.record, *offset) {
                    buffers.frame.apply_pauli(*qubit, *basis);
                }
            }
            SampleOperation::SweepPauli {
                sweep_id,
                qubit,
                basis,
            } => {
                if sweep_record.get(*sweep_id).copied().unwrap_or(false) {
                    buffers.frame.apply_pauli(*qubit, *basis);
                }
            }
            SampleOperation::Repeat { count, body } => {
                for _ in 0..*count {
                    execute_operations(body, buffers, rng, mode, sweep_record);
                }
            }
        }
    }
}

fn measurement_record_bit(measurements: &[bool], offset: MeasureRecordOffset) -> bool {
    let Ok(len) = i64::try_from(measurements.len()) else {
        return false;
    };
    let index = len + i64::from(offset.get());
    let Ok(index) = usize::try_from(index) else {
        return false;
    };
    measurements.get(index).copied().unwrap_or(false)
}
