use rand::Rng;
use stab_model::RepeatNestingLimit;
use stab_model::advanced::ClassicalControl;

use super::api::{ReferenceSampleLoopPolicy, SamplingExecutionError};
use super::operation::{SampleOperation, SampleProgram, SampleProgramEntry};
use super::stabilizer_frame::{
    MeasurementRandomness, StabilizerFrame, StabilizerStateSnapshot, reset_correction,
};
use super::{ExecutionMode, measurement_flip, noise};
use stab_model::MeasureRecordOffset;

pub(super) struct ExecutionBuffers<'a> {
    pub(super) frame: &'a mut StabilizerFrame,
    pub(super) record: &'a mut Vec<bool>,
    pub(super) output: &'a mut Vec<bool>,
    pub(super) correlated_error_occurred: &'a mut bool,
}

pub(super) fn count_determined_operations<R>(
    operations: &SampleProgram,
    frame: &mut StabilizerFrame,
    record: &mut Vec<bool>,
    rng: &mut R,
) -> Result<u64, SamplingExecutionError>
where
    R: Rng,
{
    let mut count = 0;
    let mut cursor = operations.cursor();
    while let Some(operation) = cursor.next_operation()? {
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
            SampleOperation::ClassicallyControlledPauli {
                control,
                qubit,
                basis,
            } => {
                if matches!(control, ClassicalControl::Record(offset) if record_lookback(record, *offset))
                {
                    frame.apply_pauli(*qubit, *basis);
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
    operations: &SampleProgram,
    buffers: &mut ExecutionBuffers<'_>,
    rng: &mut impl Rng,
    mode: ExecutionMode,
    sweep_record: &[bool],
) -> Result<(), SamplingExecutionError> {
    let mut cursor = operations.cursor();
    while let Some(operation) = cursor.next_operation()? {
        execute_operation(operation, buffers, rng, mode, sweep_record);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ReferenceRepeatFrame {
    body_start: usize,
    body_end: usize,
    remaining: u64,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ReferenceExecutionStats {
    pub(super) folded_repeats: u64,
    pub(super) reused_iterations: u64,
    pub(super) reused_operation_dispatches: u128,
}

#[cfg(test)]
thread_local! {
    static REFERENCE_EXECUTION_STATS: std::cell::Cell<ReferenceExecutionStats> =
        const { std::cell::Cell::new(ReferenceExecutionStats {
            folded_repeats: 0,
            reused_iterations: 0,
            reused_operation_dispatches: 0,
        }) };
}

#[cfg(test)]
pub(super) fn reset_reference_execution_stats() {
    REFERENCE_EXECUTION_STATS.set(ReferenceExecutionStats::default());
}

#[cfg(test)]
pub(super) fn take_reference_execution_stats() -> ReferenceExecutionStats {
    REFERENCE_EXECUTION_STATS.replace(ReferenceExecutionStats::default())
}

fn record_reference_fold(reused_iterations: u64, reused_operation_dispatches: u128) {
    #[cfg(test)]
    REFERENCE_EXECUTION_STATS.with(|cell| {
        let mut stats = cell.get();
        stats.folded_repeats = stats.folded_repeats.saturating_add(1);
        stats.reused_iterations = stats.reused_iterations.saturating_add(reused_iterations);
        stats.reused_operation_dispatches = stats
            .reused_operation_dispatches
            .saturating_add(reused_operation_dispatches);
        cell.set(stats);
    });
    #[cfg(not(test))]
    let _ = (reused_iterations, reused_operation_dispatches);
}

pub(super) fn execute_reference_operations(
    program: &SampleProgram,
    buffers: &mut ExecutionBuffers<'_>,
    rng: &mut impl Rng,
    sweep_record: &[bool],
    loop_policy: ReferenceSampleLoopPolicy,
    snapshot: Option<&mut StabilizerStateSnapshot>,
) -> Result<(), SamplingExecutionError> {
    let Some(snapshot) = snapshot else {
        execute_operations(
            program,
            buffers,
            rng,
            ExecutionMode::ReferenceSample,
            sweep_record,
        )?;
        return Ok(());
    };
    if loop_policy == ReferenceSampleLoopPolicy::Iterate {
        execute_operations(
            program,
            buffers,
            rng,
            ExecutionMode::ReferenceSample,
            sweep_record,
        )?;
        return Ok(());
    }

    let entries = program.entries();
    let mut repeats =
        arrayvec::ArrayVec::<ReferenceRepeatFrame, { RepeatNestingLimit::HARD_MAX }>::new();
    let mut index = 0;
    while index < entries.len() {
        let entry =
            entries
                .get(index)
                .ok_or_else(|| SamplingExecutionError::InternalInvariant {
                    message: "reference sampling moved beyond retained operations".to_owned(),
                })?;
        match entry {
            SampleProgramEntry::Execute(operation) => {
                execute_operation(
                    operation,
                    buffers,
                    rng,
                    ExecutionMode::ReferenceSample,
                    sweep_record,
                );
                index += 1;
            }
            SampleProgramEntry::Repeat { count, body_end } => {
                if *count == 0
                    || *body_end <= index
                    || !matches!(entries.get(*body_end), Some(SampleProgramEntry::EndRepeat))
                {
                    return Err(SamplingExecutionError::InternalInvariant {
                        message: "reference sampling found an invalid repeat marker".to_owned(),
                    });
                }
                let body_start = index.checked_add(1).ok_or_else(|| {
                    SamplingExecutionError::InternalInvariant {
                        message: "reference sampling repeat-body index overflowed".to_owned(),
                    }
                })?;
                let may_fold = program.reference_fold_is_profitable(
                    body_start,
                    *body_end,
                    *count,
                    buffers.frame.len(),
                ) && snapshot.capture(buffers.frame);
                if may_fold {
                    let output_start = buffers.output.len();
                    let correlated_before = *buffers.correlated_error_occurred;
                    execute_operations_range(
                        program,
                        body_start,
                        *body_end,
                        buffers,
                        rng,
                        ExecutionMode::ReferenceSample,
                        sweep_record,
                    )?;
                    if snapshot.matches(buffers.frame)
                        && *buffers.correlated_error_occurred == correlated_before
                    {
                        append_repeated_reference_bits(
                            buffers,
                            output_start,
                            count.saturating_sub(1),
                        )?;
                        let reused_iterations = count.saturating_sub(1);
                        record_reference_fold(
                            reused_iterations,
                            (program.compact_operation_count(body_start, *body_end) as u128)
                                .saturating_mul(u128::from(reused_iterations)),
                        );
                        index = body_end.checked_add(1).ok_or_else(|| {
                            SamplingExecutionError::InternalInvariant {
                                message: "reference sampling repeat-end index overflowed"
                                    .to_owned(),
                            }
                        })?;
                        continue;
                    }
                    if *count == 1 {
                        index = body_end.checked_add(1).ok_or_else(|| {
                            SamplingExecutionError::InternalInvariant {
                                message: "reference sampling repeat-end index overflowed"
                                    .to_owned(),
                            }
                        })?;
                        continue;
                    }
                    if repeats
                        .try_push(ReferenceRepeatFrame {
                            body_start,
                            body_end: *body_end,
                            remaining: count - 1,
                        })
                        .is_err()
                    {
                        return Err(SamplingExecutionError::InternalInvariant {
                            message: "reference sampling exceeded the admitted repeat nesting"
                                .to_owned(),
                        });
                    }
                    index = body_start;
                    continue;
                }
                if repeats
                    .try_push(ReferenceRepeatFrame {
                        body_start,
                        body_end: *body_end,
                        remaining: *count,
                    })
                    .is_err()
                {
                    return Err(SamplingExecutionError::InternalInvariant {
                        message: "reference sampling exceeded the admitted repeat nesting"
                            .to_owned(),
                    });
                }
                index = body_start;
            }
            SampleProgramEntry::EndRepeat => {
                let Some(frame) = repeats.last_mut() else {
                    return Err(SamplingExecutionError::InternalInvariant {
                        message: "reference sampling ended an absent repeat".to_owned(),
                    });
                };
                if frame.body_end != index || frame.remaining == 0 {
                    return Err(SamplingExecutionError::InternalInvariant {
                        message: "reference sampling repeat end disagrees with its marker"
                            .to_owned(),
                    });
                }
                if frame.remaining > 1 {
                    frame.remaining -= 1;
                    index = frame.body_start;
                } else {
                    repeats.pop();
                    index = index.checked_add(1).ok_or_else(|| {
                        SamplingExecutionError::InternalInvariant {
                            message: "reference sampling repeat-end index overflowed".to_owned(),
                        }
                    })?;
                }
            }
        }
    }
    if repeats.is_empty() {
        Ok(())
    } else {
        Err(SamplingExecutionError::InternalInvariant {
            message: "reference sampling ended inside a repeat".to_owned(),
        })
    }
}

fn execute_operations_range(
    program: &SampleProgram,
    start: usize,
    end: usize,
    buffers: &mut ExecutionBuffers<'_>,
    rng: &mut impl Rng,
    mode: ExecutionMode,
    sweep_record: &[bool],
) -> Result<(), SamplingExecutionError> {
    let mut cursor = program.cursor_range(start, end);
    while let Some(operation) = cursor.next_operation()? {
        execute_operation(operation, buffers, rng, mode, sweep_record);
    }
    Ok(())
}

fn append_repeated_reference_bits(
    buffers: &mut ExecutionBuffers<'_>,
    output_start: usize,
    repetitions: u64,
) -> Result<(), SamplingExecutionError> {
    let pattern_len = buffers.output.len().saturating_sub(output_start);
    if pattern_len == 0 {
        return Ok(());
    }
    for _ in 0..repetitions {
        for offset in 0..pattern_len {
            let index = output_start.checked_add(offset).ok_or_else(|| {
                SamplingExecutionError::InternalInvariant {
                    message: "reference folding output-pattern index overflowed".to_owned(),
                }
            })?;
            let bit = buffers.output.get(index).copied().ok_or_else(|| {
                SamplingExecutionError::InternalInvariant {
                    message: "reference folding lost its measured output pattern".to_owned(),
                }
            })?;
            buffers.output.push(bit);
            buffers.record.push(bit);
        }
    }
    Ok(())
}

fn execute_operation(
    operation: &SampleOperation,
    buffers: &mut ExecutionBuffers<'_>,
    rng: &mut impl Rng,
    mode: ExecutionMode,
    sweep_record: &[bool],
) {
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
        SampleOperation::ClassicallyControlledPauli {
            control,
            qubit,
            basis,
        } => {
            let active = match control {
                ClassicalControl::Record(offset) => measurement_record_bit(buffers.record, *offset),
                ClassicalControl::Sweep(sweep_id) => usize::try_from(*sweep_id)
                    .ok()
                    .and_then(|index| sweep_record.get(index))
                    .copied()
                    .unwrap_or(false),
            };
            if active {
                buffers.frame.apply_pauli(*qubit, *basis);
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
