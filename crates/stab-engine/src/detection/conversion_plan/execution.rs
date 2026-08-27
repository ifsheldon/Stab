use stab_model::{MeasureRecordOffset, RepeatNestingLimit};

use crate::detection::{DetectionError, DetectionRecordBuffer, DetectionResult};

use super::{
    ConversionOperation, compilation_overflow, compilation_shape_mismatch,
    measurement_index_from_offset, repeated_usize_total,
};

#[derive(Clone, Copy)]
pub(super) enum MeasurementValues<'a> {
    Difference(&'a [bool]),
    Zero,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ConversionCursor {
    pub(super) measurement: usize,
    pub(super) detector: usize,
}

#[derive(Clone, Copy)]
struct RepeatExecutionFrame {
    body_start: usize,
    body_end: usize,
    remaining: u64,
}

impl RepeatExecutionFrame {
    const EMPTY: Self = Self {
        body_start: 0,
        body_end: 0,
        remaining: 0,
    };
}

pub(super) fn execute_program(
    program: &[ConversionOperation],
    measurements: MeasurementValues<'_>,
    reference_sample: &[bool],
    record: &mut DetectionRecordBuffer,
    cursor: &mut ConversionCursor,
) -> DetectionResult<()> {
    let mut repeat_stack = [RepeatExecutionFrame::EMPTY; RepeatNestingLimit::HARD_MAX];
    let mut repeat_depth = 0_usize;
    let mut program_counter = 0_usize;
    while let Some(operation) = program.get(program_counter) {
        match operation {
            ConversionOperation::AdvanceMeasurements(count) => {
                cursor.measurement = cursor
                    .measurement
                    .checked_add(*count)
                    .ok_or_else(compilation_overflow)?;
                program_counter = next_program_counter(program_counter)?;
            }
            ConversionOperation::Detector(terms) => {
                let value =
                    parity_of_terms(terms, cursor.measurement, measurements, reference_sample)?;
                let detector = record.detectors.get_mut(cursor.detector).ok_or_else(|| {
                    DetectionError::invalid_sampler_compilation(format!(
                        "compact conversion detector index {} is out of range",
                        cursor.detector
                    ))
                })?;
                *detector = value;
                cursor.detector = cursor
                    .detector
                    .checked_add(1)
                    .ok_or_else(compilation_overflow)?;
                program_counter = next_program_counter(program_counter)?;
            }
            ConversionOperation::Observable { id, terms } => {
                let value =
                    parity_of_terms(terms, cursor.measurement, measurements, reference_sample)?;
                let observable = record.observables.get_mut(*id).ok_or_else(|| {
                    DetectionError::invalid_sampler_compilation(format!(
                        "compact conversion observable index {id} is out of range"
                    ))
                })?;
                *observable ^= value;
                program_counter = next_program_counter(program_counter)?;
            }
            ConversionOperation::Repeat {
                count,
                body_end,
                measurement_count,
                detector_count,
                requires_iteration: false,
            } => {
                validate_repeat_marker(program, program_counter, *body_end)?;
                cursor.measurement = repeated_usize_total(
                    cursor.measurement,
                    *measurement_count,
                    *count,
                    "runtime measurement cursor",
                )?;
                cursor.detector = repeated_usize_total(
                    cursor.detector,
                    *detector_count,
                    *count,
                    "runtime detector cursor",
                )?;
                program_counter = next_program_counter(*body_end)?;
            }
            ConversionOperation::Repeat {
                count,
                body_end,
                requires_iteration: true,
                ..
            } => {
                validate_repeat_marker(program, program_counter, *body_end)?;
                if *count == 0 {
                    program_counter = next_program_counter(*body_end)?;
                    continue;
                }
                let frame = repeat_stack
                    .get_mut(repeat_depth)
                    .ok_or_else(compilation_shape_mismatch)?;
                *frame = RepeatExecutionFrame {
                    body_start: next_program_counter(program_counter)?,
                    body_end: *body_end,
                    remaining: *count,
                };
                repeat_depth = repeat_depth
                    .checked_add(1)
                    .ok_or_else(compilation_overflow)?;
                program_counter = next_program_counter(program_counter)?;
            }
            ConversionOperation::EndRepeat => {
                let frame_index = repeat_depth
                    .checked_sub(1)
                    .ok_or_else(compilation_shape_mismatch)?;
                let frame = repeat_stack
                    .get_mut(frame_index)
                    .ok_or_else(compilation_shape_mismatch)?;
                if frame.body_end != program_counter || frame.remaining == 0 {
                    return Err(compilation_shape_mismatch());
                }
                frame.remaining -= 1;
                if frame.remaining == 0 {
                    repeat_depth = frame_index;
                    program_counter = next_program_counter(program_counter)?;
                } else {
                    program_counter = frame.body_start;
                }
            }
        }
    }
    if repeat_depth != 0 {
        return Err(compilation_shape_mismatch());
    }
    Ok(())
}

pub(super) fn validate_repeat_marker(
    program: &[ConversionOperation],
    marker_index: usize,
    body_end: usize,
) -> DetectionResult<()> {
    if body_end <= marker_index
        || !matches!(program.get(body_end), Some(ConversionOperation::EndRepeat))
    {
        return Err(compilation_shape_mismatch());
    }
    Ok(())
}

fn next_program_counter(program_counter: usize) -> DetectionResult<usize> {
    program_counter
        .checked_add(1)
        .ok_or_else(compilation_overflow)
}

fn parity_of_terms(
    terms: &[MeasureRecordOffset],
    measurement_cursor: usize,
    measurements: MeasurementValues<'_>,
    reference_sample: &[bool],
) -> DetectionResult<bool> {
    let mut parity = false;
    for offset in terms {
        let index = measurement_index_from_offset(measurement_cursor, *offset)?;
        let reference = reference_sample.get(index).copied().ok_or_else(|| {
            DetectionError::invalid_result_format(format!(
                "reference sample index {index} is out of range"
            ))
        })?;
        let measurement = match measurements {
            MeasurementValues::Difference(record) => {
                record.get(index).copied().ok_or_else(|| {
                    DetectionError::invalid_result_format(format!(
                        "measurement index {index} is out of range"
                    ))
                })?
            }
            MeasurementValues::Zero => false,
        };
        parity ^= measurement ^ reference;
    }
    Ok(parity)
}
