//! Drives the inverse-tableau reference simulator over a compact sampling program.

use stab_model::RepeatNestingLimit;
use stab_model::advanced::ClassicalControl;

use super::api::{ReferenceSampleLoopPolicy, SamplingExecutionError};
use super::execute::record_lookback;
use super::operation::{SampleOperation, SampleProgram, SampleProgramEntry};
use super::reference_simulator::{
    InverseTableauSimulator, ReferenceSimulatorError, ReferenceStateSnapshot, reset_correction,
};

/// Reusable simulator storage for computing reference samples of one plan.
#[derive(Debug)]
pub(super) struct ReferenceWorkspace {
    simulator: InverseTableauSimulator,
    snapshot: Option<ReferenceStateSnapshot>,
}

impl ReferenceWorkspace {
    pub(super) fn try_new(
        qubit_count: usize,
        with_snapshot: bool,
    ) -> Result<Self, SamplingExecutionError> {
        let simulator = InverseTableauSimulator::try_new(qubit_count).map_err(simulator_error)?;
        let snapshot = with_snapshot
            .then(|| ReferenceStateSnapshot::try_new(qubit_count))
            .transpose()
            .map_err(simulator_error)?;
        Ok(Self {
            simulator,
            snapshot,
        })
    }

    /// Computes the noiseless reference record of `program` into `output`.
    ///
    /// Random measurements take the false outcome, noise and result flips are ignored, and
    /// sweep-controlled Paulis read `sweep_record` (missing bits are false).
    pub(super) fn run(
        &mut self,
        program: &SampleProgram,
        loop_policy: ReferenceSampleLoopPolicy,
        sweep_record: &[bool],
        output: &mut Vec<bool>,
    ) -> Result<(), SamplingExecutionError> {
        #[cfg(test)]
        REFERENCE_RUNS.with(|runs| runs.set(runs.get().saturating_add(1)));
        self.simulator.reset_to_zero_state();
        output.clear();
        let snapshot = match loop_policy {
            ReferenceSampleLoopPolicy::Fold => self.snapshot.as_mut(),
            ReferenceSampleLoopPolicy::Iterate => None,
        };
        run_entries(program, &mut self.simulator, snapshot, sweep_record, output)
    }
}

#[derive(Clone, Copy)]
struct RepeatFrame {
    body_start: usize,
    body_end: usize,
    remaining: u64,
}

fn run_entries(
    program: &SampleProgram,
    simulator: &mut InverseTableauSimulator,
    mut snapshot: Option<&mut ReferenceStateSnapshot>,
    sweep_record: &[bool],
    output: &mut Vec<bool>,
) -> Result<(), SamplingExecutionError> {
    let entries = program.entries();
    let mut repeats = arrayvec::ArrayVec::<RepeatFrame, { RepeatNestingLimit::HARD_MAX }>::new();
    let mut index = 0;
    while index < entries.len() {
        let entry = entries
            .get(index)
            .ok_or_else(|| run_invariant("reference sampling moved beyond retained operations"))?;
        match entry {
            SampleProgramEntry::Execute(operation) => {
                apply_operation(operation, simulator, sweep_record, output)?;
                index += 1;
            }
            SampleProgramEntry::Repeat { count, body_end } => {
                if *count == 0
                    || *body_end <= index
                    || !matches!(entries.get(*body_end), Some(SampleProgramEntry::EndRepeat))
                {
                    return Err(run_invariant(
                        "reference sampling found an invalid repeat marker",
                    ));
                }
                let body_start = index + 1;
                let after_body = body_end.checked_add(1).ok_or_else(|| {
                    run_invariant("reference sampling repeat-end index overflowed")
                })?;
                let mut remaining = *count;
                if let Some(fold_snapshot) = snapshot.as_deref_mut()
                    && program.reference_fold_is_profitable(
                        body_start,
                        *body_end,
                        *count,
                        simulator.qubit_count(),
                    )
                    && simulator.capture(fold_snapshot)
                {
                    let output_start = output.len();
                    run_range(
                        program,
                        body_start,
                        *body_end,
                        simulator,
                        sweep_record,
                        output,
                    )?;
                    if simulator
                        .same_state_as(fold_snapshot)
                        .map_err(simulator_error)?
                    {
                        // The iteration returned to the state it started from and the body
                        // reads no measurement record, so every later iteration repeats it.
                        append_repeated_bits(output, output_start, count - 1)?;
                        #[cfg(test)]
                        record_fold(
                            count - 1,
                            (program.compact_operation_count(body_start, *body_end) as u128)
                                .saturating_mul(u128::from(count - 1)),
                        );
                        index = after_body;
                        continue;
                    }
                    remaining -= 1;
                }
                if remaining == 0 {
                    index = after_body;
                    continue;
                }
                repeats
                    .try_push(RepeatFrame {
                        body_start,
                        body_end: *body_end,
                        remaining,
                    })
                    .map_err(|_| {
                        run_invariant("reference sampling exceeded the admitted repeat nesting")
                    })?;
                index = body_start;
            }
            SampleProgramEntry::EndRepeat => {
                let Some(frame) = repeats.last_mut() else {
                    return Err(run_invariant("reference sampling ended an absent repeat"));
                };
                if frame.body_end != index || frame.remaining == 0 {
                    return Err(run_invariant(
                        "reference sampling repeat end disagrees with its marker",
                    ));
                }
                if frame.remaining > 1 {
                    frame.remaining -= 1;
                    index = frame.body_start;
                } else {
                    repeats.pop();
                    index += 1;
                }
            }
        }
    }
    if repeats.is_empty() {
        Ok(())
    } else {
        Err(run_invariant("reference sampling ended inside a repeat"))
    }
}

fn run_range(
    program: &SampleProgram,
    start: usize,
    end: usize,
    simulator: &mut InverseTableauSimulator,
    sweep_record: &[bool],
    output: &mut Vec<bool>,
) -> Result<(), SamplingExecutionError> {
    let mut cursor = program.cursor_range(start, end);
    while let Some(operation) = cursor.next_operation()? {
        apply_operation(operation, simulator, sweep_record, output)?;
    }
    Ok(())
}

fn apply_operation(
    operation: &SampleOperation,
    simulator: &mut InverseTableauSimulator,
    sweep_record: &[bool],
    output: &mut Vec<bool>,
) -> Result<(), SamplingExecutionError> {
    match operation {
        SampleOperation::ApplyHadamard { qubit } => simulator.apply_hadamard(*qubit),
        SampleOperation::ApplyControlledX { control, target } => {
            simulator.apply_controlled_x(*control, *target)
        }
        SampleOperation::ApplyTableau { targets, transform } => {
            simulator.apply_local_clifford(targets, transform.inverse())
        }
        SampleOperation::Reset { qubit, basis } => simulator.reset(*qubit, *basis),
        SampleOperation::Measure {
            qubit,
            basis,
            inverted,
            flip_probability: _,
            reset,
        } => {
            // Stim's reference sample is noiseless, so result-flip probabilities are ignored.
            let measured = simulator.measure(*qubit, *basis).map_err(simulator_error)?;
            output.push(measured ^ *inverted);
            if *reset && measured {
                simulator.apply_pauli(*qubit, reset_correction(*basis))
            } else {
                Ok(())
            }
        }
        SampleOperation::MeasureProduct {
            terms,
            inverted,
            flip_probability: _,
        } => {
            let measured = simulator.measure_product(terms).map_err(simulator_error)?;
            output.push(measured ^ *inverted);
            Ok(())
        }
        SampleOperation::Pad {
            value,
            flip_probability: _,
        } => {
            output.push(*value);
            Ok(())
        }
        SampleOperation::HeraldedPauliChannel { .. } => {
            output.push(false);
            Ok(())
        }
        SampleOperation::SingleQubitPauliChannel { .. }
        | SampleOperation::TwoQubitPauliChannel { .. }
        | SampleOperation::CorrelatedError { .. } => Ok(()),
        SampleOperation::ClassicallyControlledPauli {
            control,
            qubit,
            basis,
        } => {
            let active = match control {
                ClassicalControl::Record(offset) => record_lookback(output, *offset),
                ClassicalControl::Sweep(sweep_id) => usize::try_from(*sweep_id)
                    .ok()
                    .and_then(|index| sweep_record.get(index))
                    .copied()
                    .unwrap_or(false),
            };
            if active {
                simulator.apply_pauli(*qubit, *basis)
            } else {
                Ok(())
            }
        }
    }
    .map_err(simulator_error)
}

fn append_repeated_bits(
    output: &mut Vec<bool>,
    output_start: usize,
    repetitions: u64,
) -> Result<(), SamplingExecutionError> {
    let pattern_len = output.len().saturating_sub(output_start);
    if pattern_len == 0 {
        return Ok(());
    }
    let total = usize::try_from(repetitions)
        .ok()
        .and_then(|repetitions| repetitions.checked_mul(pattern_len))
        .ok_or_else(|| run_invariant("reference folding output length overflowed"))?;
    output.try_reserve(total).map_err(|error| {
        SamplingExecutionError::SessionStorageAllocation {
            message: format!("reference folding output capacity {total}: {error}"),
        }
    })?;
    for _ in 0..repetitions {
        output.extend_from_within(output_start..output_start + pattern_len);
    }
    Ok(())
}

pub(super) fn simulator_error(error: ReferenceSimulatorError) -> SamplingExecutionError {
    match error {
        ReferenceSimulatorError::Allocation { .. } => {
            SamplingExecutionError::SessionStorageAllocation {
                message: error.to_string(),
            }
        }
        ReferenceSimulatorError::Invariant(_) => SamplingExecutionError::InternalInvariant {
            message: error.to_string(),
        },
    }
}

fn run_invariant(message: &str) -> SamplingExecutionError {
    SamplingExecutionError::InternalInvariant {
        message: message.to_owned(),
    }
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
    static REFERENCE_RUNS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(super) fn reset_reference_execution_stats() {
    REFERENCE_EXECUTION_STATS.set(ReferenceExecutionStats::default());
}

#[cfg(test)]
pub(super) fn take_reference_execution_stats() -> ReferenceExecutionStats {
    REFERENCE_EXECUTION_STATS.replace(ReferenceExecutionStats::default())
}

/// Number of reference simulations run on this thread, for plan-cache tests.
#[cfg(test)]
pub(super) fn reference_run_count() -> u64 {
    REFERENCE_RUNS.get()
}

#[cfg(test)]
fn record_fold(reused_iterations: u64, reused_operation_dispatches: u128) {
    REFERENCE_EXECUTION_STATS.with(|cell| {
        let mut stats = cell.get();
        stats.folded_repeats = stats.folded_repeats.saturating_add(1);
        stats.reused_iterations = stats.reused_iterations.saturating_add(reused_iterations);
        stats.reused_operation_dispatches = stats
            .reused_operation_dispatches
            .saturating_add(reused_operation_dispatches);
        cell.set(stats);
    });
}
