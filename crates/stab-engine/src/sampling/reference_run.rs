//! Drives the inverse-tableau reference simulator over the resolved circuit program.

use stab_model::RepeatNestingLimit;

use super::api::{ReferenceSampleLoopPolicy, SamplingExecutionError};
use super::reference_simulator::{
    InverseTableauSimulator, ReferenceSimulatorError, ReferenceStateSnapshot, reset_correction,
};
use crate::detection::frame::{
    CircuitProgram, CliffordStep, ControlledPair, ProgramEntry, ProgramInstruction,
    ReferenceClifford,
};
use stab_model::MeasureRecordOffset;

/// Below this many repeated work units a repeat is iterated rather than snapshotted.
const MIN_REFERENCE_FOLD_WORK: u128 = 64;

/// Reusable simulator storage for computing reference samples of one program.
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
        program: &CircuitProgram,
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

/// Whether any repeat of `program` may be folded, so a reference run needs a state snapshot.
pub(super) fn has_reference_fold_candidate(program: &CircuitProgram, qubit_count: usize) -> bool {
    let entries = program.entries();
    entries
        .iter()
        .enumerate()
        .any(|(index, entry)| match entry {
            ProgramEntry::Repeat { count, body_end }
                if *body_end > index
                    && matches!(entries.get(*body_end), Some(ProgramEntry::EndRepeat)) =>
            {
                reference_fold_is_profitable(program, index + 1, *body_end, *count, qubit_count)
            }
            _ => false,
        })
}

fn reference_fold_is_profitable(
    program: &CircuitProgram,
    start: usize,
    end: usize,
    repeat_count: u64,
    qubit_count: usize,
) -> bool {
    let body = program.entries().get(start..end).unwrap_or_default();
    if body.iter().any(|entry| {
        matches!(
            entry,
            ProgramEntry::Instruction(ProgramInstruction::ControlledPauli { pairs, .. })
                if pairs.iter().any(|pair| !matches!(pair, ControlledPair::Quantum { .. }))
        )
    }) {
        return false;
    }
    let repeated_work = reference_work_units(program, body, qubit_count)
        .saturating_mul(u128::from(repeat_count.saturating_sub(1)));
    let snapshot_work = (qubit_count as u128)
        .saturating_mul(qubit_count as u128)
        .saturating_mul(2);
    repeated_work >= MIN_REFERENCE_FOLD_WORK.max(snapshot_work)
}

/// Estimated simulator work of one pass over `body`, per target dispatch: a Clifford costs one
/// row update per qubit and target, a collapse one per qubit squared, and the rest one unit.
fn reference_work_units(
    program: &CircuitProgram,
    body: &[ProgramEntry],
    qubit_count: usize,
) -> u128 {
    let qubits = (qubit_count as u128).max(1);
    let collapse = qubits.saturating_mul(qubits);
    let gate_work = |arity: usize, clifford: Option<&ReferenceClifford>| match clifford {
        Some(ReferenceClifford::Hadamard | ReferenceClifford::ControlledX) => qubits,
        _ => qubits.saturating_mul(arity.max(1) as u128),
    };
    body.iter()
        .filter_map(|entry| match entry {
            ProgramEntry::Instruction(instruction) => Some(instruction),
            ProgramEntry::Repeat { .. } | ProgramEntry::EndRepeat => None,
        })
        .fold(0_u128, |work, instruction| {
            let units = match instruction {
                ProgramInstruction::Clifford {
                    reference, targets, ..
                } => {
                    let clifford = program.reference_clifford(*reference).ok();
                    let arity = clifford.map_or(1, ReferenceClifford::arity).max(1);
                    gate_work(arity, clifford).saturating_mul((targets.len() / arity) as u128)
                }
                ProgramInstruction::Reset { targets, .. }
                | ProgramInstruction::Measure { targets, .. } => {
                    collapse.saturating_mul(targets.len() as u128)
                }
                ProgramInstruction::MeasurePairs { targets, .. } => {
                    collapse.saturating_mul((targets.len() / 2) as u128)
                }
                ProgramInstruction::PauliProducts(measurement) => {
                    collapse.saturating_mul(measurement.product_count() as u128)
                }
                ProgramInstruction::MeasurePads { values, .. } => values.len() as u128,
                ProgramInstruction::SinglePauliNoise { targets, .. }
                | ProgramInstruction::HeraldedErase { targets, .. }
                | ProgramInstruction::HeraldedPauliChannel { targets, .. } => targets.len() as u128,
                ProgramInstruction::TwoQubitPauliNoise { targets, .. } => {
                    (targets.len() / 2) as u128
                }
                ProgramInstruction::CorrelatedError { .. } => 1,
                ProgramInstruction::ControlledPauli {
                    reference, pairs, ..
                } => {
                    let clifford = program.reference_clifford(*reference).ok();
                    pairs.iter().fold(0_u128, |units, pair| {
                        units.saturating_add(match pair {
                            ControlledPair::Quantum { .. } => gate_work(2, clifford),
                            ControlledPair::Record { .. } | ControlledPair::Sweep { .. } => 1,
                        })
                    })
                }
                ProgramInstruction::Observable { .. } => 0,
            };
            work.saturating_add(units)
        })
}

#[derive(Clone, Copy)]
struct RepeatFrame {
    body_start: usize,
    body_end: usize,
    remaining: u64,
}

fn run_entries(
    program: &CircuitProgram,
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
            ProgramEntry::Instruction(instruction) => {
                apply_instruction(program, instruction, simulator, sweep_record, output)?;
                index += 1;
            }
            ProgramEntry::Repeat { count, body_end } => {
                if *count == 0
                    || *body_end <= index
                    || !matches!(entries.get(*body_end), Some(ProgramEntry::EndRepeat))
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
                    && reference_fold_is_profitable(
                        program,
                        body_start,
                        *body_end,
                        *count,
                        simulator.qubit_count(),
                    )
                    && simulator.capture(fold_snapshot)
                {
                    let output_start = output.len();
                    let mut cursor = program.cursor_range(body_start, *body_end);
                    while let Some(instruction) =
                        cursor.next_instruction().map_err(program_error)?
                    {
                        apply_instruction(program, instruction, simulator, sweep_record, output)?;
                    }
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
                            (entries
                                .get(body_start..*body_end)
                                .unwrap_or_default()
                                .iter()
                                .filter(|entry| matches!(entry, ProgramEntry::Instruction(_)))
                                .count() as u128)
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
            ProgramEntry::EndRepeat => {
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

/// Applies a resolved unitary gate to every target group of `targets`.
fn apply_clifford(
    clifford: &ReferenceClifford,
    targets: &[usize],
    simulator: &mut InverseTableauSimulator,
) -> Result<(), ReferenceSimulatorError> {
    match clifford {
        ReferenceClifford::Hadamard => {
            for &qubit in targets {
                simulator.apply_hadamard(qubit)?;
            }
        }
        ReferenceClifford::ControlledX => {
            for pair in targets.chunks(2) {
                let &[control, target] = pair else {
                    return Err(ReferenceSimulatorError::Invariant(
                        "controlled-X targets are not paired",
                    ));
                };
                simulator.apply_controlled_x(control, target)?;
            }
        }
        ReferenceClifford::Tableau { transform, steps } => {
            let arity = transform.target_count().max(1);
            for group in targets.chunks(arity) {
                let Some(steps) = steps else {
                    simulator.apply_local_clifford(group, transform.inverse())?;
                    continue;
                };
                let qubit = |index: u8| {
                    group.get(usize::from(index)).copied().ok_or(
                        ReferenceSimulatorError::Invariant(
                            "decomposition step escaped its target group",
                        ),
                    )
                };
                for step in steps {
                    match *step {
                        CliffordStep::H(target) => simulator.apply_hadamard(qubit(target)?)?,
                        CliffordStep::S(target) => simulator.apply_s(qubit(target)?)?,
                        CliffordStep::Cx(control, target) => {
                            simulator.apply_controlled_x(qubit(control)?, qubit(target)?)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn apply_instruction(
    program: &CircuitProgram,
    instruction: &ProgramInstruction,
    simulator: &mut InverseTableauSimulator,
    sweep_record: &[bool],
    output: &mut Vec<bool>,
) -> Result<(), SamplingExecutionError> {
    match instruction {
        ProgramInstruction::Clifford {
            reference, targets, ..
        } => apply_clifford(
            program
                .reference_clifford(*reference)
                .map_err(program_error)?,
            targets,
            simulator,
        )
        .map_err(simulator_error),
        ProgramInstruction::Reset { basis, targets } => targets
            .iter()
            .try_for_each(|&qubit| simulator.reset(qubit, *basis))
            .map_err(simulator_error),
        ProgramInstruction::Measure {
            basis,
            reset,
            targets,
            inverted,
            ..
        } => {
            // Stim's reference sample is noiseless, so result-flip probabilities are ignored.
            for (index, &qubit) in targets.iter().enumerate() {
                let measured = simulator.measure(qubit, *basis).map_err(simulator_error)?;
                output.push(measured ^ inverted.get(index).copied().unwrap_or(false));
                if *reset && measured {
                    simulator
                        .apply_pauli(qubit, reset_correction(*basis))
                        .map_err(simulator_error)?;
                }
            }
            Ok(())
        }
        ProgramInstruction::MeasurePairs {
            basis,
            targets,
            inverted,
            ..
        } => {
            for (index, pair) in targets.chunks(2).enumerate() {
                let &[left, right] = pair else {
                    return Err(run_invariant("pair measurement targets are not paired"));
                };
                let measured = simulator
                    .measure_product(&[(left, *basis), (right, *basis)])
                    .map_err(simulator_error)?;
                let flip = inverted.get(2 * index).copied().unwrap_or(false)
                    ^ inverted.get(2 * index + 1).copied().unwrap_or(false);
                output.push(measured ^ flip);
            }
            Ok(())
        }
        ProgramInstruction::PauliProducts(measurement) => {
            for product in measurement.products() {
                let (terms, inverted) = product.map_err(program_error)?;
                let measured = simulator.measure_product(terms).map_err(simulator_error)?;
                output.push(measured ^ inverted);
            }
            Ok(())
        }
        ProgramInstruction::MeasurePads { values, .. } => {
            output.extend_from_slice(values);
            Ok(())
        }
        ProgramInstruction::HeraldedErase { targets, .. }
        | ProgramInstruction::HeraldedPauliChannel { targets, .. } => {
            output.extend(std::iter::repeat_n(false, targets.len()));
            Ok(())
        }
        ProgramInstruction::SinglePauliNoise { .. }
        | ProgramInstruction::TwoQubitPauliNoise { .. }
        | ProgramInstruction::CorrelatedError { .. }
        | ProgramInstruction::Observable { .. } => Ok(()),
        ProgramInstruction::ControlledPauli {
            basis,
            reference,
            pairs,
            ..
        } => {
            let clifford = program
                .reference_clifford(*reference)
                .map_err(program_error)?;
            for pair in pairs {
                match *pair {
                    ControlledPair::Quantum { control, target } => {
                        apply_clifford(clifford, &[control, target], simulator)
                    }
                    ControlledPair::Record { offset, qubit } => {
                        if record_lookback(output, offset) {
                            simulator.apply_pauli(qubit, *basis)
                        } else {
                            Ok(())
                        }
                    }
                    ControlledPair::Sweep { id, qubit } => {
                        let active = usize::try_from(id)
                            .ok()
                            .and_then(|index| sweep_record.get(index))
                            .copied()
                            .unwrap_or(false);
                        if active {
                            simulator.apply_pauli(qubit, *basis)
                        } else {
                            Ok(())
                        }
                    }
                }
                .map_err(simulator_error)?;
            }
            Ok(())
        }
    }
}

/// Counts the measurements of `program` whose outcome is determined by earlier operations.
///
/// With `unknown_input`, each qubit starts maximally entangled with a private reference qubit
/// that no operation touches, so a measurement is determined only by the circuit's own earlier
/// measurements and resets. Flip probabilities and noise never matter, matching Stim, which
/// strips instruction arguments before counting.
pub(super) fn count_determined_measurements(
    program: &CircuitProgram,
    qubit_count: usize,
    unknown_input: bool,
) -> Result<u64, SamplingExecutionError> {
    let simulated = if unknown_input {
        qubit_count
            .checked_mul(2)
            .ok_or_else(|| run_invariant("unknown-input qubit count overflowed"))?
    } else {
        qubit_count
    };
    let mut simulator = InverseTableauSimulator::try_new(simulated).map_err(simulator_error)?;
    if unknown_input {
        for qubit in 0..qubit_count {
            simulator.apply_hadamard(qubit).map_err(simulator_error)?;
            simulator
                .apply_controlled_x(qubit, qubit + qubit_count)
                .map_err(simulator_error)?;
        }
    }
    let mut record = Vec::new();
    let mut count = 0_u64;
    let mut cursor = program.cursor();
    while let Some(instruction) = cursor.next_instruction().map_err(program_error)? {
        match instruction {
            ProgramInstruction::Measure {
                basis,
                reset,
                targets,
                inverted,
                ..
            } => {
                for (index, &qubit) in targets.iter().enumerate() {
                    let terms = [(qubit, *basis)];
                    count += u64::from(
                        simulator
                            .is_deterministic(&terms)
                            .map_err(simulator_error)?,
                    );
                    let measured = simulator.measure(qubit, *basis).map_err(simulator_error)?;
                    record.push(measured ^ inverted.get(index).copied().unwrap_or(false));
                    if *reset && measured {
                        simulator
                            .apply_pauli(qubit, reset_correction(*basis))
                            .map_err(simulator_error)?;
                    }
                }
            }
            ProgramInstruction::MeasurePairs {
                basis,
                targets,
                inverted,
                ..
            } => {
                for (index, pair) in targets.chunks(2).enumerate() {
                    let &[left, right] = pair else {
                        return Err(run_invariant("pair measurement targets are not paired"));
                    };
                    let terms = [(left, *basis), (right, *basis)];
                    count += u64::from(
                        simulator
                            .is_deterministic(&terms)
                            .map_err(simulator_error)?,
                    );
                    let measured = simulator.measure_product(&terms).map_err(simulator_error)?;
                    let flip = inverted.get(2 * index).copied().unwrap_or(false)
                        ^ inverted.get(2 * index + 1).copied().unwrap_or(false);
                    record.push(measured ^ flip);
                }
            }
            ProgramInstruction::PauliProducts(measurement) => {
                for product in measurement.products() {
                    let (terms, inverted) = product.map_err(program_error)?;
                    count += u64::from(simulator.is_deterministic(terms).map_err(simulator_error)?);
                    let measured = simulator.measure_product(terms).map_err(simulator_error)?;
                    record.push(measured ^ inverted);
                }
            }
            ProgramInstruction::MeasurePads { .. } => {
                return Err(
                    SamplingExecutionError::UnsupportedDeterminedMeasurementGate { gate: "MPAD" },
                );
            }
            ProgramInstruction::HeraldedErase { .. }
            | ProgramInstruction::HeraldedPauliChannel { .. } => {
                return Err(
                    SamplingExecutionError::UnsupportedDeterminedMeasurementGate {
                        gate: "heralded noise",
                    },
                );
            }
            ProgramInstruction::ControlledPauli {
                basis,
                reference,
                pairs,
                ..
            } => {
                let clifford = program
                    .reference_clifford(*reference)
                    .map_err(program_error)?;
                for pair in pairs {
                    match *pair {
                        ControlledPair::Quantum { control, target } => {
                            apply_clifford(clifford, &[control, target], &mut simulator)
                        }
                        ControlledPair::Record { offset, qubit }
                            if record_lookback(&record, offset) =>
                        {
                            simulator.apply_pauli(qubit, *basis)
                        }
                        ControlledPair::Record { .. } | ControlledPair::Sweep { .. } => Ok(()),
                    }
                    .map_err(simulator_error)?;
                }
            }
            other => apply_instruction(program, other, &mut simulator, &[], &mut record)?,
        }
    }
    Ok(count)
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

pub(super) fn program_error(error: crate::detection::DetectionError) -> SamplingExecutionError {
    SamplingExecutionError::InternalInvariant {
        message: error.to_string(),
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

/// The record bit `offset` steps back from the end of `record`, or false before its start.
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
