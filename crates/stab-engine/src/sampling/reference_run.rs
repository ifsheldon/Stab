//! Drives the inverse-tableau reference simulator over the resolved circuit program.

use stab_algebra::PauliBasis;
use stab_model::advanced::CancellationCountdown;
use stab_model::{Cancellation, RepeatNestingLimit};

use super::api::{ReferenceSampleLoopPolicy, SamplingExecutionError};
use super::reference_bits::ReferenceBits;
use super::reference_fold::{FoldProbe, ProbeStep, SnapshotSlot};
use super::reference_simulator::{
    InverseTableauSimulator, ReferenceSimulatorError, WorkModel, reset_correction,
};
use crate::detection::frame::{
    CircuitProgram, CliffordStep, ControlledPair, ProgramEntry, ProgramInstruction,
    ReferenceClifford,
};

pub(super) use super::reference_fold::has_reference_fold_candidate;

/// Reusable simulator storage for computing reference samples of one program.
#[derive(Debug)]
pub(super) struct ReferenceWorkspace {
    simulator: InverseTableauSimulator,
    /// Snapshot storage when the plan admitted one; it is allocated on first capture.
    snapshot: Option<SnapshotSlot>,
}

impl ReferenceWorkspace {
    pub(super) fn try_new(
        qubit_count: usize,
        with_snapshot: bool,
    ) -> Result<Self, SamplingExecutionError> {
        let simulator = InverseTableauSimulator::try_new(qubit_count).map_err(simulator_error)?;
        Ok(Self {
            simulator,
            snapshot: with_snapshot.then(|| SnapshotSlot::new(qubit_count)),
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
        output: &mut ReferenceBits,
        cancellation: &Cancellation,
    ) -> Result<(), SamplingExecutionError> {
        #[cfg(test)]
        REFERENCE_RUNS.with(|runs| runs.set(runs.get().saturating_add(1)));
        self.simulator.reset_to_zero_state();
        output.clear();
        let snapshot = match loop_policy {
            ReferenceSampleLoopPolicy::Fold => self.snapshot.as_mut(),
            ReferenceSampleLoopPolicy::Iterate => None,
        };
        if let Some(slot) = snapshot.as_deref()
            && !slot.is_free()
        {
            return Err(run_invariant(
                "a reference run started with a held snapshot",
            ));
        }
        let result = run_entries(
            program,
            &mut self.simulator,
            snapshot,
            sweep_record,
            output,
            cancellation,
        );
        if let Some(slot) = self.snapshot.as_mut() {
            slot.release();
        }
        result
    }
}

struct RepeatFrame {
    body_start: usize,
    body_end: usize,
    remaining: u64,
    /// Cycle detection of this repeat while it may still fold.
    probe: Option<FoldProbe>,
}

fn run_entries(
    program: &CircuitProgram,
    simulator: &mut InverseTableauSimulator,
    mut snapshot: Option<&mut SnapshotSlot>,
    sweep_record: &[bool],
    output: &mut ReferenceBits,
    cancellation: &Cancellation,
) -> Result<(), SamplingExecutionError> {
    let entries = program.entries();
    let mut repeats = arrayvec::ArrayVec::<RepeatFrame, { RepeatNestingLimit::HARD_MAX }>::new();
    let mut countdown = CancellationCountdown::new();
    let mut index = 0;
    while index < entries.len() {
        let entry = entries
            .get(index)
            .ok_or_else(|| run_invariant("reference sampling moved beyond retained operations"))?;
        match entry {
            ProgramEntry::Instruction(instruction) => {
                countdown.tick(cancellation)?;
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
                let probe = match snapshot.as_deref_mut() {
                    Some(slot) => {
                        countdown.tick(cancellation)?;
                        FoldProbe::begin(
                            program,
                            body_start,
                            *body_end,
                            *count,
                            simulator,
                            slot,
                            output.len(),
                        )?
                    }
                    None => None,
                };
                repeats
                    .try_push(RepeatFrame {
                        body_start,
                        body_end: *body_end,
                        remaining: *count,
                        probe,
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
                if let (Some(probe), Some(slot)) = (frame.probe.as_mut(), snapshot.as_deref_mut()) {
                    // A probe step compares whole tableaux, so it polls cancellation directly.
                    cancellation.check()?;
                    match probe.end_iteration(simulator, slot, output.len())? {
                        ProbeStep::Continue => {}
                        ProbeStep::GiveUp => frame.probe = None,
                        ProbeStep::Fold {
                            period,
                            block_start,
                        } => {
                            frame.probe = None;
                            slot.release();
                            // Every later iteration repeats the last `period` ones, which
                            // depend only on the recurring state.
                            let later = frame.remaining - 1;
                            let copies = later
                                .checked_div(period)
                                .ok_or_else(|| run_invariant("a folded repeat had no period"))?;
                            output.append_repeated(block_start, copies)?;
                            #[cfg(test)]
                            record_fold(
                                entries.get(frame.body_start..frame.body_end),
                                copies.saturating_mul(period),
                                period,
                            );
                            // The leftover iterations are simulated so the state after the
                            // repeat is exact.
                            let leftover = later % period;
                            if leftover == 0 {
                                repeats.pop();
                                index += 1;
                            } else {
                                frame.remaining = leftover;
                                index = frame.body_start;
                            }
                            continue;
                        }
                    }
                }
                if frame.remaining > 1 {
                    frame.remaining -= 1;
                    index = frame.body_start;
                    countdown.tick(cancellation)?;
                } else {
                    repeats.pop();
                    index += 1;
                }
            }
        }
    }
    repeats
        .is_empty()
        .ok_or_else(|| run_invariant("reference sampling ended inside a repeat"))
}

/// A gate whose `H`/`S`/`CX` decomposition the simulator applies as one row operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectClifford {
    ControlledZ,
    Swap,
    SDag,
    Pauli(PauliBasis),
}

impl DirectClifford {
    /// Recognizes decompositions whose conjugation action is one direct row operation, so
    /// applying it gives exactly the rows the steps would.
    fn from_steps(steps: &[CliffordStep]) -> Option<Self> {
        use CliffordStep::{Cx, H, S};
        match steps {
            [H(1), Cx(0, 1), H(1)] => Some(Self::ControlledZ),
            [Cx(0, 1), Cx(1, 0), Cx(0, 1)] => Some(Self::Swap),
            [S(0), S(0), S(0)] => Some(Self::SDag),
            [H(0), S(0), S(0), H(0)] => Some(Self::Pauli(PauliBasis::X)),
            [S(0), S(0)] => Some(Self::Pauli(PauliBasis::Z)),
            [S(0), S(0), H(0), S(0), S(0), H(0)] => Some(Self::Pauli(PauliBasis::Y)),
            _ => None,
        }
    }

    fn arity(self) -> usize {
        match self {
            Self::ControlledZ | Self::Swap => 2,
            Self::SDag | Self::Pauli(_) => 1,
        }
    }

    fn apply(
        self,
        group: &[usize],
        simulator: &mut InverseTableauSimulator,
    ) -> Result<(), ReferenceSimulatorError> {
        match (self, group) {
            (Self::ControlledZ, &[left, right]) => simulator.apply_controlled_z(left, right),
            (Self::Swap, &[left, right]) => simulator.apply_swap(left, right),
            (Self::SDag, &[qubit]) => simulator.apply_s_dag(qubit),
            (Self::Pauli(basis), &[qubit]) => simulator.apply_pauli(qubit, basis),
            _ => Err(ReferenceSimulatorError::Invariant(
                "direct gate targets disagree with its arity",
            )),
        }
    }

    fn work(self, model: WorkModel) -> u64 {
        match self {
            Self::ControlledZ => model.row_multiply().saturating_mul(2),
            Self::Swap => model.row_copy().saturating_mul(4),
            Self::SDag => model.row_multiply(),
            Self::Pauli(_) => WorkModel::dispatch(),
        }
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
            let direct = steps
                .as_deref()
                .and_then(DirectClifford::from_steps)
                .filter(|direct| direct.arity() == arity);
            for group in targets.chunks(arity) {
                if let Some(direct) = direct {
                    direct.apply(group, simulator)?;
                    continue;
                }
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

/// Estimated simulator work of applying `clifford` to one target group, as charged by the
/// simulator when [`apply_clifford`] runs it.
pub(super) fn clifford_group_work(clifford: &ReferenceClifford, model: WorkModel) -> u64 {
    let swap = model.row_copy().saturating_mul(2);
    let multiply = model.row_multiply();
    match clifford {
        ReferenceClifford::Hadamard => swap,
        ReferenceClifford::ControlledX => multiply.saturating_mul(2),
        ReferenceClifford::Tableau { transform, steps } => {
            let arity = transform.target_count().max(1) as u64;
            match steps.as_deref() {
                Some(steps) => match DirectClifford::from_steps(steps) {
                    Some(direct) => direct.work(model),
                    None => steps
                        .iter()
                        .map(|step| match step {
                            CliffordStep::H(_) => swap,
                            CliffordStep::S(_) => multiply,
                            CliffordStep::Cx(..) => multiply.saturating_mul(2),
                        })
                        .fold(0, u64::saturating_add),
                },
                // Each of the 2k rows is saved, rebuilt from up to k saved rows, and stored.
                None => arity.saturating_mul(2).saturating_mul(
                    multiply
                        .saturating_mul(arity)
                        .saturating_add(model.row_copy().saturating_mul(2)),
                ),
            }
        }
    }
}

fn apply_instruction(
    program: &CircuitProgram,
    instruction: &ProgramInstruction,
    simulator: &mut InverseTableauSimulator,
    sweep_record: &[bool],
    output: &mut ReferenceBits,
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
            output.extend_from_bools(values);
            Ok(())
        }
        ProgramInstruction::HeraldedErase { targets, .. }
        | ProgramInstruction::HeraldedPauliChannel { targets, .. } => {
            output.extend_zeros(targets.len());
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
                        if output.lookback(offset) {
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
    cancellation: &Cancellation,
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
    let mut record = ReferenceBits::default();
    let mut count = 0_u64;
    let mut cursor = program.cursor(cancellation);
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
                        ControlledPair::Record { offset, qubit } if record.lookback(offset) => {
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
    match error {
        crate::detection::DetectionError::Cancelled(cancelled) => {
            SamplingExecutionError::Cancelled(cancelled)
        }
        error => SamplingExecutionError::InternalInvariant {
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
    /// Longest cycle period any fold found.
    pub(super) longest_period: u64,
}

#[cfg(test)]
thread_local! {
    static REFERENCE_EXECUTION_STATS: std::cell::Cell<ReferenceExecutionStats> =
        const { std::cell::Cell::new(ReferenceExecutionStats {
            folded_repeats: 0,
            reused_iterations: 0,
            reused_operation_dispatches: 0,
            longest_period: 0,
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
fn record_fold(body: Option<&[ProgramEntry]>, reused_iterations: u64, period: u64) {
    let dispatches = body
        .unwrap_or_default()
        .iter()
        .filter(|entry| matches!(entry, ProgramEntry::Instruction(_)))
        .count() as u128;
    REFERENCE_EXECUTION_STATS.with(|cell| {
        let mut stats = cell.get();
        stats.folded_repeats = stats.folded_repeats.saturating_add(1);
        stats.reused_iterations = stats.reused_iterations.saturating_add(reused_iterations);
        stats.reused_operation_dispatches = stats
            .reused_operation_dispatches
            .saturating_add(dispatches.saturating_mul(u128::from(reused_iterations)));
        stats.longest_period = stats.longest_period.max(period);
        cell.set(stats);
    });
}
