use stab_model::{Cancellation, MeasureRecordOffset, RepeatNestingLimit};
use std::ops::{BitXor, BitXorAssign, Not};

use crate::ReferenceSample;
use crate::detection::{DetectionError, DetectionResult};
use crate::work_poll::{STEP_UNITS, WORD_OP_UNITS, WorkCountdown};

/// Detectors and observables with at most this many terms are charged as if they had exactly
/// this many, one constant charge as cheap as a step count in the hot loop.
pub(super) const SHORT_PARITY_TERMS: usize = 64;

/// Detectors and observables with more terms than this sum their parity in polled chunks.
const LONG_PARITY_TERMS: usize = 1 << 12;

use super::{
    CompactProgram, ConversionOperation, compilation_overflow, compilation_shape_mismatch,
    repeated_usize_total, unavailable_measurement,
};

#[derive(Clone, Copy)]
pub(super) enum RecordValues<'a, Value> {
    Values(&'a [Value]),
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

/// Why [`ConversionRunner::run`] returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConversionStop {
    /// The whole program ran.
    Finished,
    /// The next detector or observable reads a measurement that is not available yet.
    NeedsMeasurements,
    /// The next detector has no slot left in the detector window.
    DetectorWindowFull,
}

/// Where a conversion reads measurement values and writes detector values.
pub(super) struct ConversionIo<'a, 'b, Value> {
    pub(super) measurements: RecordValues<'a, Value>,
    /// Absolute measurement index of the first value of `measurements`.
    pub(super) measurement_base: usize,
    /// The reference sample XOR-ed into every parity, whose bits apply to every lane of a value.
    pub(super) reference_sample: Option<&'a ReferenceSample>,
    /// Measurements below this absolute index are available.
    pub(super) available_measurements: usize,
    pub(super) detectors: &'b mut [Value],
    /// Absolute detector index of `detectors[0]`.
    pub(super) detector_base: usize,
    pub(super) observables: &'b mut [Value],
}

/// A conversion program's execution state, resumable after it stops for more input or output
/// room, so a streaming batch converts its measurement windows as they are recorded.
pub(crate) struct ConversionRunner {
    program_counter: usize,
    repeat_stack: [RepeatExecutionFrame; RepeatNestingLimit::HARD_MAX],
    repeat_depth: usize,
    pub(super) cursor: ConversionCursor,
    countdown: WorkCountdown,
}

impl Default for ConversionRunner {
    fn default() -> Self {
        Self {
            program_counter: 0,
            repeat_stack: [RepeatExecutionFrame::EMPTY; RepeatNestingLimit::HARD_MAX],
            repeat_depth: 0,
            cursor: ConversionCursor::default(),
            countdown: WorkCountdown::new(),
        }
    }
}

impl ConversionRunner {
    /// Runs `program` from where it last stopped until it ends or needs what `io` lacks.
    pub(super) fn run<Value>(
        &mut self,
        program: &CompactProgram,
        io: &mut ConversionIo<'_, '_, Value>,
        cancellation: &Cancellation,
    ) -> DetectionResult<ConversionStop>
    where
        Value: BitXor<Output = Value> + BitXorAssign + Copy + Default + Not<Output = Value>,
    {
        // The loop state lives in locals so the hot loop keeps it in registers; it is stored
        // back whenever the run returns, so a later call resumes where this one stopped.
        let mut program_counter = self.program_counter;
        let mut repeat_depth = self.repeat_depth;
        let mut cursor = self.cursor;
        let mut countdown = self.countdown;
        let repeat_stack = &mut self.repeat_stack;
        let operations = program.operations.as_slice();
        let arena = program.arena();
        let result = (|| {
            while let Some(operation) = operations.get(program_counter) {
                match operation {
                    ConversionOperation::AdvanceMeasurements(count) => {
                        cursor.measurement = cursor
                            .measurement
                            .checked_add(*count)
                            .ok_or_else(compilation_overflow)?;
                        program_counter = next_program_counter(program_counter)?;
                    }
                    ConversionOperation::Detector(terms)
                    | ConversionOperation::WideDetector(terms) => {
                        if cursor.measurement > io.available_measurements {
                            return Ok(ConversionStop::NeedsMeasurements);
                        }
                        let slot =
                            cursor
                                .detector
                                .checked_sub(io.detector_base)
                                .ok_or_else(|| {
                                    DetectionError::invalid_sampler_compilation(
                                        "compact conversion detector fell behind its window",
                                    )
                                })?;
                        let Some(detector) = io.detectors.get_mut(slot) else {
                            return Ok(ConversionStop::DetectorWindowFull);
                        };
                        *detector = charged_parity(
                            arena.terms(*terms)?,
                            cursor.measurement,
                            io.measurements,
                            io.measurement_base,
                            io.reference_sample,
                            &mut countdown,
                            cancellation,
                        )?;
                        cursor.detector = cursor
                            .detector
                            .checked_add(1)
                            .ok_or_else(compilation_overflow)?;
                        program_counter = next_program_counter(program_counter)?;
                    }
                    ConversionOperation::Observable { id, terms } => {
                        if cursor.measurement > io.available_measurements {
                            return Ok(ConversionStop::NeedsMeasurements);
                        }
                        let value = charged_parity(
                            arena.terms(*terms)?,
                            cursor.measurement,
                            io.measurements,
                            io.measurement_base,
                            io.reference_sample,
                            &mut countdown,
                            cancellation,
                        )?;
                        let observable = io.observables.get_mut(*id).ok_or_else(|| {
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
                        validate_repeat_marker(operations, program_counter, *body_end)?;
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
                        validate_repeat_marker(operations, program_counter, *body_end)?;
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
                            countdown.charge(STEP_UNITS, cancellation)?;
                        }
                    }
                }
            }
            if repeat_depth != 0 {
                return Err(compilation_shape_mismatch());
            }
            Ok(ConversionStop::Finished)
        })();
        self.program_counter = program_counter;
        self.repeat_depth = repeat_depth;
        self.cursor = cursor;
        self.countdown = countdown;
        result
    }
}

/// Runs a whole conversion program over complete records.
///
/// This is the loop of [`ConversionRunner::run`] without its window checks and resumable
/// state; materializing sessions and measurement-to-detection conversion run it per batch, and
/// routing them through the resumable runner measured 5 to 10 percent slower on detection.
pub(super) fn execute_program<Value>(
    program: &CompactProgram,
    measurements: RecordValues<'_, Value>,
    reference_sample: Option<&ReferenceSample>,
    detectors: &mut [Value],
    observables: &mut [Value],
    cursor: &mut ConversionCursor,
    cancellation: &Cancellation,
) -> DetectionResult<()>
where
    Value: BitXor<Output = Value> + BitXorAssign + Copy + Default + Not<Output = Value>,
{
    let mut countdown = WorkCountdown::new();
    let mut repeat_stack = [RepeatExecutionFrame::EMPTY; RepeatNestingLimit::HARD_MAX];
    let mut repeat_depth = 0_usize;
    let mut program_counter = 0_usize;
    let operations = program.operations.as_slice();
    let arena = program.arena();
    while let Some(operation) = operations.get(program_counter) {
        match operation {
            ConversionOperation::AdvanceMeasurements(count) => {
                cursor.measurement = cursor
                    .measurement
                    .checked_add(*count)
                    .ok_or_else(compilation_overflow)?;
                program_counter = next_program_counter(program_counter)?;
            }
            ConversionOperation::Detector(terms) => {
                countdown.charge(short_parity_units::<Value>(), cancellation)?;
                let value = parity_of_terms(
                    arena.terms(*terms)?,
                    cursor.measurement,
                    measurements,
                    0,
                    reference_sample,
                )?;
                let detector = detectors.get_mut(cursor.detector).ok_or_else(|| {
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
            ConversionOperation::WideDetector(terms) => {
                let value = long_parity(
                    arena.terms(*terms)?,
                    cursor.measurement,
                    measurements,
                    0,
                    reference_sample,
                    &mut countdown,
                    cancellation,
                )?;
                let detector = detectors.get_mut(cursor.detector).ok_or_else(|| {
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
                let value = charged_parity(
                    arena.terms(*terms)?,
                    cursor.measurement,
                    measurements,
                    0,
                    reference_sample,
                    &mut countdown,
                    cancellation,
                )?;
                let observable = observables.get_mut(*id).ok_or_else(|| {
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
                validate_repeat_marker(operations, program_counter, *body_end)?;
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
                validate_repeat_marker(operations, program_counter, *body_end)?;
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
                    countdown.charge(STEP_UNITS, cancellation)?;
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

/// The parity of `terms`, charged to `countdown` first: a constant charge for at most
/// [`SHORT_PARITY_TERMS`] terms, and [`long_parity`] for more.
#[inline(always)]
fn charged_parity<Value>(
    terms: &[MeasureRecordOffset],
    measurement_cursor: usize,
    measurements: RecordValues<'_, Value>,
    measurement_base: usize,
    reference_sample: Option<&ReferenceSample>,
    countdown: &mut WorkCountdown,
    cancellation: &Cancellation,
) -> DetectionResult<Value>
where
    Value: BitXor<Output = Value> + BitXorAssign + Copy + Default + Not<Output = Value>,
{
    if terms.len() > SHORT_PARITY_TERMS {
        return long_parity(
            terms,
            measurement_cursor,
            measurements,
            measurement_base,
            reference_sample,
            countdown,
            cancellation,
        );
    }
    countdown.charge(short_parity_units::<Value>(), cancellation)?;
    parity_of_terms(
        terms,
        measurement_cursor,
        measurements,
        measurement_base,
        reference_sample,
    )
}

/// The parity of more than [`SHORT_PARITY_TERMS`] terms, summed in chunks of at most
/// [`LONG_PARITY_TERMS`] terms each charged before it is read.
#[cold]
#[inline(never)]
fn long_parity<Value>(
    terms: &[MeasureRecordOffset],
    measurement_cursor: usize,
    measurements: RecordValues<'_, Value>,
    measurement_base: usize,
    reference_sample: Option<&ReferenceSample>,
    countdown: &mut WorkCountdown,
    cancellation: &Cancellation,
) -> DetectionResult<Value>
where
    Value: BitXor<Output = Value> + BitXorAssign + Copy + Default + Not<Output = Value>,
{
    let units = term_units::<Value>();
    let mut parity = Value::default();
    for chunk in terms.chunks(LONG_PARITY_TERMS) {
        countdown.charge(chunk.len() * units, cancellation)?;
        parity ^= parity_of_terms(
            chunk,
            measurement_cursor,
            measurements,
            measurement_base,
            reference_sample,
        )?;
    }
    Ok(parity)
}

/// The constant charge of one parity of at most [`SHORT_PARITY_TERMS`] terms.
const fn short_parity_units<Value>() -> usize {
    STEP_UNITS + SHORT_PARITY_TERMS * term_units::<Value>()
}

/// Work units of reading one term of a `Value` record: one unit per 64-bit word it spans.
const fn term_units<Value>() -> usize {
    WORD_OP_UNITS * size_of::<Value>().div_ceil(size_of::<u64>())
}

/// The absolute index of the measurement `offset` reads when `measurement_cursor` measurements
/// precede the reader.
///
/// An offset is negative by construction, so the index exists exactly when the offset does not
/// reach before the first measurement, and it is then below the cursor: one checked subtraction
/// decides what the plan's signed offset arithmetic decides, on the hottest path of conversion.
#[inline(always)]
fn term_index(measurement_cursor: usize, offset: MeasureRecordOffset) -> DetectionResult<usize> {
    measurement_cursor
        .checked_sub(crate::detection::frame::lookback_distance(offset))
        .ok_or_else(|| unavailable_measurement(offset))
}

fn parity_of_terms<Value>(
    terms: &[MeasureRecordOffset],
    measurement_cursor: usize,
    measurements: RecordValues<'_, Value>,
    measurement_base: usize,
    reference_sample: Option<&ReferenceSample>,
) -> DetectionResult<Value>
where
    Value: BitXor<Output = Value> + BitXorAssign + Copy + Default + Not<Output = Value>,
{
    let mut parity = Value::default();
    let mut reference_parity = false;
    for offset in terms {
        let index = term_index(measurement_cursor, *offset)?;
        if let Some(reference) = reference_sample {
            reference_parity ^= reference.get(index).ok_or_else(|| {
                DetectionError::invalid_result_format(format!(
                    "reference sample index {index} is out of range"
                ))
            })?;
        }
        // An index below the base wraps past every slice length and reads as out of range.
        let measurement = match measurements {
            RecordValues::Values(record) => record
                .get(index.wrapping_sub(measurement_base))
                .copied()
                .ok_or_else(|| {
                    DetectionError::invalid_result_format(format!(
                        "measurement index {index} is out of range"
                    ))
                })?,
            RecordValues::Zero => Value::default(),
        };
        parity ^= measurement;
    }
    // A reference bit is common to every shot, so an odd reference parity flips every lane.
    Ok(if reference_parity { !parity } else { parity })
}
