//! Cost model and cycle detection for folding repeat blocks of the reference simulation.
//!
//! The reference sample is a deterministic function of the circuit, and the output of a repeat
//! iteration depends only on the stabilizer state it starts from when its body reads no
//! measurement record or sweep bit. So once the state after iteration `i` equals the state after
//! an earlier iteration `s`, the iterations `s + 1..=i` repeat forever with period `i - s`, and
//! their output bits can be copied instead of simulated.
//!
//! Detection follows Brent's algorithm on iteration-end states: one snapshot is kept, re-taken
//! after iterations whose count is a power of two, and compared after every iteration. A cycle
//! with transient `mu` and period `lambda` is found within about `2 * max(mu, lambda) + lambda`
//! iterations.
//!
//! Probing costs a snapshot capture and one state comparison per iteration. Both are weighed in
//! the simulator's own [`WorkModel`] units against the iterations a fold would save, using the
//! work the simulator actually measured for the last iteration. A probe continues only while a
//! match after the next iteration would save more than that next probe step costs, while the
//! probe's total cost stays within `1 / PROBE_BUDGET_DIVISOR` of the whole loop's work, and for
//! at most [`MAX_PROBED_ITERATIONS`] iterations; so a loop that never repeats pays a bounded
//! overhead.

use arrayvec::ArrayVec;
use stab_model::RepeatNestingLimit;

use super::api::SamplingExecutionError;
use super::reference_run::{clifford_group_work, simulator_error};
use super::reference_simulator::{
    InverseTableauSimulator, ReferenceStateSnapshot, SimulatorWork, WorkModel,
};
use crate::detection::frame::{CircuitProgram, ControlledPair, ProgramEntry, ProgramInstruction};
use crate::work_poll::WorkPoll;

/// After its first step, a probe may spend at most this fraction (one over the divisor) of the
/// loop's work.
const PROBE_BUDGET_DIVISOR: u128 = 8;

/// The first probe step, a capture and one comparison, must save this many times its cost.
const FIRST_PROBE_MARGIN: u128 = 2;

/// Iterations a probe may run before it stops looking for a cycle.
const MAX_PROBED_ITERATIONS: u64 = 64;

/// Reusable snapshot storage of one reference run, allocated on first capture.
#[derive(Debug)]
pub(super) struct SnapshotSlot {
    qubit_count: usize,
    storage: Option<ReferenceStateSnapshot>,
    held: bool,
}

impl SnapshotSlot {
    pub(super) fn new(qubit_count: usize) -> Self {
        Self {
            qubit_count,
            storage: None,
            held: false,
        }
    }

    /// Estimated work of capturing a snapshot now, including its first allocation.
    fn capture_work(&self, model: WorkModel) -> u64 {
        if self.storage.is_some() {
            model.capture()
        } else {
            model.capture().saturating_add(model.snapshot_allocation())
        }
    }

    /// Whether no repeat currently holds the snapshot.
    pub(super) fn is_free(&self) -> bool {
        !self.held
    }

    pub(super) fn release(&mut self) {
        self.held = false;
    }

    fn capture(
        &mut self,
        simulator: &mut InverseTableauSimulator,
    ) -> Result<bool, SamplingExecutionError> {
        let snapshot = match &mut self.storage {
            Some(snapshot) => snapshot,
            empty => empty.insert(
                ReferenceStateSnapshot::try_new(self.qubit_count).map_err(simulator_error)?,
            ),
        };
        self.held = simulator.capture(snapshot);
        Ok(self.held)
    }

    fn held_snapshot(&mut self) -> Result<&mut ReferenceStateSnapshot, SamplingExecutionError> {
        match (&mut self.storage, self.held) {
            (Some(snapshot), true) => Ok(snapshot),
            _ => Err(probe_invariant(
                "a fold probe compared without its snapshot",
            )),
        }
    }
}

/// What the run does after a probed iteration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProbeStep {
    /// Keep probing the next iteration.
    Continue,
    /// Stop probing; the snapshot is released and the remaining iterations run normally.
    GiveUp,
    /// The state after this iteration recurred. The output bits from `block_start` on are those
    /// of the last `period` iterations, which repeat for every later iteration.
    Fold { period: u64, block_start: usize },
}

/// The snapshot a probe holds: the iteration count it was taken after and the output length.
#[derive(Clone, Copy, Debug)]
struct HeldSnapshot {
    iteration: u64,
    output_len: usize,
}

/// Cycle detection state of one repeat block.
#[derive(Clone, Copy, Debug)]
pub(super) struct FoldProbe {
    count: u64,
    completed: u64,
    /// Static estimate of one iteration with nested repeats iterated, as a probed one runs.
    unfolded_work: u64,
    has_nested_repeat: bool,
    iteration_start: u64,
    held: Option<HeldSnapshot>,
    spent: u64,
    /// Estimated cost of the admitted first probe step, which the budget allows on top.
    allowance: Option<u64>,
}

impl FoldProbe {
    /// Starts probing the repeat `entries[body_start..body_end]` of `count` iterations, if its
    /// iterations can recur at all. The snapshot may be taken before the first iteration.
    pub(super) fn begin(
        program: &CircuitProgram,
        body_start: usize,
        body_end: usize,
        count: u64,
        simulator: &mut InverseTableauSimulator,
        slot: &mut SnapshotSlot,
        output_len: usize,
    ) -> Result<Option<Self>, SamplingExecutionError> {
        let body = program
            .entries()
            .get(body_start..body_end)
            .ok_or_else(|| probe_invariant("a fold probe body is outside the program"))?;
        if count < 2 || !slot.is_free() || !body_is_foldable(body) {
            return Ok(None);
        }
        let model = simulator.work_model();
        let capture_work = slot.capture_work(model);
        let mut probe = Self {
            count,
            completed: 0,
            unfolded_work: saturate(iteration_work(program, body, model, false)),
            has_nested_repeat: body
                .iter()
                .any(|entry| matches!(entry, ProgramEntry::Repeat { .. })),
            iteration_start: 0,
            held: None,
            spent: 0,
            allowance: None,
        };
        // A nested repeat could fold on its own during the first iteration but not while this
        // probe holds the snapshot, so such a probe first measures one iteration.
        if !probe.has_nested_repeat
            && probe.worth_probing(count, probe.unfolded_work, capture_work, || {
                let before = simulator.work();
                let support = simulator.current_support_rows();
                let cost = capture_work.saturating_add(model.fresh_comparison(support));
                (cost, simulator.work().saturating_sub(before))
            })
        {
            probe.capture(simulator, slot, output_len)?;
        }
        probe.iteration_start = simulator.work();
        Ok(Some(probe))
    }

    /// Ends one probed iteration: compares the state with the held snapshot, then decides
    /// whether probing the next iteration is still worth its cost.
    pub(super) fn end_iteration(
        &mut self,
        simulator: &mut InverseTableauSimulator,
        slot: &mut SnapshotSlot,
        output_len: usize,
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> Result<ProbeStep, SamplingExecutionError> {
        self.completed = self.completed.saturating_add(1);
        let iteration_work = simulator.work().saturating_sub(self.iteration_start);
        let model = simulator.work_model();
        if let Some(held) = self.held {
            let before = simulator.work();
            let recurred = simulator
                .same_state_as(slot.held_snapshot()?, poll, work)
                .map_err(simulator_error)?;
            self.charge(simulator.work().saturating_sub(before));
            if recurred {
                return Ok(ProbeStep::Fold {
                    period: self.completed.saturating_sub(held.iteration),
                    block_start: held.output_len,
                });
            }
        }
        let remaining = self.count.saturating_sub(self.completed);
        // Brent's schedule re-takes the snapshot after power-of-two iteration counts.
        let recapture = self.held.is_none() || self.completed.is_power_of_two();
        // While the snapshot was free, nested repeats could fold; a probed iteration cannot.
        let penalty = if self.has_nested_repeat && self.held.is_none() {
            self.unfolded_work.saturating_sub(iteration_work)
        } else {
            0
        };
        let capture_work = slot.capture_work(model);
        let snapshot_estimate = if recapture {
            None
        } else {
            Some(slot.held_snapshot()?.full_comparison_work(model))
        };
        let floor = if recapture { capture_work } else { 0 };
        let worth = self.worth_probing(
            remaining,
            iteration_work,
            floor.saturating_add(penalty),
            || {
                let (cost, estimating) = snapshot_estimate.map_or_else(
                    || {
                        let before = simulator.work();
                        let support = simulator.current_support_rows();
                        let cost = capture_work.saturating_add(model.fresh_comparison(support));
                        (cost, simulator.work().saturating_sub(before))
                    },
                    |cost| (cost, 0),
                );
                (cost.saturating_add(penalty), estimating)
            },
        );
        if !worth {
            self.held = None;
            slot.release();
            return Ok(ProbeStep::GiveUp);
        }
        if recapture {
            self.capture(simulator, slot, output_len)?;
            if self.held.is_none() {
                return Ok(ProbeStep::GiveUp);
            }
        }
        self.iteration_start = simulator.work();
        Ok(ProbeStep::Continue)
    }

    /// Whether probing after the next iteration pays: a match then saves `remaining - 1`
    /// iterations, which must cost more than the next probe step. The first probe step must
    /// cost at most half of what it can save; later steps must also keep the probe's total
    /// within the first step plus `1 / PROBE_BUDGET_DIVISOR` of the loop's work.
    ///
    /// `floor` is a cheap lower bound of the step's cost; `next_step` returns its estimate and
    /// the work spent estimating it, and is only called when the floor passes.
    fn worth_probing(
        &mut self,
        remaining: u64,
        iteration_work: u64,
        floor: u64,
        next_step: impl FnOnce() -> (u64, u64),
    ) -> bool {
        if self.completed >= MAX_PROBED_ITERATIONS || remaining < 2 {
            return false;
        }
        #[cfg(test)]
        if FORCED_PROBES.get() {
            let (_, estimating) = next_step();
            self.charge(estimating);
            return true;
        }
        let savings = u128::from(remaining - 1).saturating_mul(u128::from(iteration_work));
        let budget = u128::from(self.count).saturating_mul(u128::from(iteration_work))
            / PROBE_BUDGET_DIVISOR;
        let affordable = |cost: u64, spent: u64, allowance: Option<u64>| {
            let cost = u128::from(cost);
            match allowance {
                None => savings > cost.saturating_mul(FIRST_PROBE_MARGIN),
                Some(allowance) => {
                    savings > cost
                        && u128::from(spent).saturating_add(cost)
                            <= u128::from(allowance).saturating_add(budget)
                }
            }
        };
        if !affordable(floor, self.spent, self.allowance) {
            return false;
        }
        let (cost, estimating) = next_step();
        self.charge(estimating);
        let worth = affordable(cost, self.spent, self.allowance);
        if worth && self.allowance.is_none() {
            self.allowance = Some(cost);
        }
        worth
    }

    fn capture(
        &mut self,
        simulator: &mut InverseTableauSimulator,
        slot: &mut SnapshotSlot,
        output_len: usize,
    ) -> Result<(), SamplingExecutionError> {
        let before = simulator.work();
        let captured = slot.capture(simulator)?;
        self.charge(simulator.work().saturating_sub(before));
        self.held = captured.then_some(HeldSnapshot {
            iteration: self.completed,
            output_len,
        });
        Ok(())
    }

    fn charge(&mut self, work: u64) {
        self.spent = self.spent.saturating_add(work);
    }
}

/// Whether any repeat of `program` may be folded, so a reference run needs a state snapshot.
///
/// This is a necessary condition only: a repeat qualifies when copying its iterations could
/// save more than capturing one snapshot and comparing an identical tableau against it, with
/// every measurement assumed to collapse.
pub(super) fn has_reference_fold_candidate(program: &CircuitProgram, qubit_count: usize) -> bool {
    let model = WorkModel::for_qubits(qubit_count);
    let entries = program.entries();
    entries
        .iter()
        .enumerate()
        .any(|(index, entry)| match entry {
            ProgramEntry::Repeat { count, body_end }
                if *count >= 2
                    && *body_end > index
                    && matches!(entries.get(*body_end), Some(ProgramEntry::EndRepeat)) =>
            {
                let body = entries.get(index + 1..*body_end).unwrap_or_default();
                body_is_foldable(body)
                    && iteration_work(program, body, model, true)
                        .saturating_mul(u128::from(*count - 1))
                        > u128::from(model.capture().saturating_add(model.tableau_pass()))
            }
            _ => false,
        })
}

/// Whether the iterations of `body` depend only on the state they start from: no instruction
/// reads the measurement record or a sweep bit.
fn body_is_foldable(body: &[ProgramEntry]) -> bool {
    !body.iter().any(|entry| {
        matches!(
            entry,
            ProgramEntry::Instruction(ProgramInstruction::ControlledPauli { pairs, .. })
                if pairs.iter().any(|pair| !matches!(pair, ControlledPair::Quantum { .. }))
        )
    })
}

/// Static estimate of one iteration of `body` with nested repeats iterated.
///
/// Measurements are charged as deterministic, a lower bound, unless `collapse` charges each as
/// a random-outcome collapse that updates half of the rows.
fn iteration_work(
    program: &CircuitProgram,
    body: &[ProgramEntry],
    model: WorkModel,
    collapse: bool,
) -> u128 {
    let mut multipliers = ArrayVec::<u128, { RepeatNestingLimit::HARD_MAX }>::new();
    let mut total = 0_u128;
    let mut scale = 1_u128;
    for entry in body {
        match entry {
            ProgramEntry::Repeat { count, .. } => {
                if multipliers.try_push(scale).is_err() {
                    return u128::MAX;
                }
                scale = scale.saturating_mul(u128::from(*count));
            }
            ProgramEntry::EndRepeat => {
                // A body holds whole repeat blocks; an unbalanced one is never folded.
                let Some(outer) = multipliers.pop() else {
                    return 0;
                };
                scale = outer;
            }
            ProgramEntry::Instruction(instruction) => {
                let units = instruction_work(program, instruction, model, collapse);
                total = total.saturating_add(scale.saturating_mul(u128::from(units)));
            }
        }
    }
    total
}

fn instruction_work(
    program: &CircuitProgram,
    instruction: &ProgramInstruction,
    model: WorkModel,
    collapse: bool,
) -> u64 {
    let rows = |x: bool, z: bool| u64::from(x) + u64::from(z);
    let product = |multiplied: u64| {
        model
            .row_multiply()
            .saturating_mul(multiplied)
            .saturating_add(model.row_copy())
    };
    // A random outcome scans every row and updates about half of them.
    let collapse_work = if collapse {
        model
            .collapse_scan()
            .saturating_add(model.row_multiply().saturating_mul(model.qubit_count()))
    } else {
        0
    };
    let measurement = |multiplied: u64| product(multiplied).saturating_add(collapse_work);
    let per_target = |count: usize, units: u64| (count as u64).saturating_mul(units);
    match instruction {
        ProgramInstruction::Clifford {
            reference, targets, ..
        } => match program.reference_clifford(*reference) {
            Ok(clifford) => {
                let arity = clifford.arity().max(1);
                per_target(targets.len() / arity, clifford_group_work(clifford, model))
            }
            Err(_) => 0,
        },
        ProgramInstruction::Reset { basis, targets }
        | ProgramInstruction::Measure { basis, targets, .. } => per_target(
            targets.len(),
            measurement(rows(basis.x_bit(), basis.z_bit())),
        ),
        ProgramInstruction::MeasurePairs { basis, targets, .. } => per_target(
            targets.len() / 2,
            measurement(2 * rows(basis.x_bit(), basis.z_bit())),
        ),
        ProgramInstruction::PauliProducts(products) => products
            .products()
            .map(|terms| match terms {
                Ok((terms, _)) => product(
                    terms
                        .iter()
                        .map(|(_, basis)| rows(basis.x_bit(), basis.z_bit()))
                        .sum(),
                ),
                Err(_) => 0,
            })
            .fold(0, u64::saturating_add)
            .saturating_add(per_target(products.product_count(), collapse_work)),
        ProgramInstruction::ControlledPauli {
            reference, pairs, ..
        } => match program.reference_clifford(*reference) {
            Ok(clifford) => per_target(pairs.len(), clifford_group_work(clifford, model)),
            Err(_) => 0,
        },
        ProgramInstruction::MeasurePads { .. }
        | ProgramInstruction::SinglePauliNoise { .. }
        | ProgramInstruction::TwoQubitPauliNoise { .. }
        | ProgramInstruction::CorrelatedError { .. }
        | ProgramInstruction::HeraldedErase { .. }
        | ProgramInstruction::HeraldedPauliChannel { .. }
        | ProgramInstruction::Observable { .. } => WorkModel::dispatch(),
    }
}

#[cfg(test)]
thread_local! {
    static FORCED_PROBES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Makes every repeat on this thread probe for cycles regardless of cost, within the
/// iteration bound, so tests can exercise cycle detection on small circuits.
#[cfg(test)]
pub(super) fn force_fold_probes(forced: bool) {
    FORCED_PROBES.set(forced);
}

fn saturate(work: u128) -> u64 {
    u64::try_from(work).unwrap_or(u64::MAX)
}

fn probe_invariant(message: &str) -> SamplingExecutionError {
    SamplingExecutionError::InternalInvariant {
        message: message.to_owned(),
    }
}
