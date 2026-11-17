//! Stim's collapse order for X- and Y-basis measurements and resets in the reference sample.
//!
//! Stim's reference sample (`TableauSimulator::reference_sample_circuit`, and the
//! `ReferenceSampleTree` that folds its loops) forces every random outcome to false in the
//! measured basis, but it does not collapse every gate in target order:
//!
//! - `MX`, `MY`, `MRX`, `MRY`, `RX`, and `RY` collapse the distinct targets of one instruction
//!   in ascending qubit order (`collapse_x` and `collapse_y` gather them in a `std::set`) before
//!   any result is recorded or any qubit is reset.
//! - `MXX` and `MYY` split their pairs into segments of disjoint pairs and collapse each
//!   segment's pairs in ascending order of their first qubit.
//! - `M`, `MR`, `R`, `MZZ`, and `MPP` collapse in target order, which is the simulator's own
//!   sequential order.
//!
//! The instruction is the one of Stim's noiseless circuit, which fuses adjacent instructions of
//! the same gate and tag once noise channels and flip arguments are stripped. When correlated
//! targets collapse in a different order, a different but equally valid outcome is forced, so
//! `m2d` would disagree with Stim on non-deterministic detectors.
//!
//! Collapsing a fused group in Stim's order up front, and then running its instructions
//! sequentially, reproduces Stim exactly: every target is deterministic afterwards, and the
//! sequential pass only records results and applies reset corrections, which are Paulis on
//! already-collapsed qubits that commute with every other target's observable.

use stab_algebra::PauliBasis;

use super::api::SamplingExecutionError;
use super::reference_run::simulator_error;
use super::reference_simulator::{InverseTableauSimulator, ReferenceSimulatorError, SimulatorWork};
use crate::detection::frame::{
    ArenaSpan, NoiselessFusion, ProgramArena, ProgramEntry, ProgramInstruction,
};
use crate::work_poll::WorkPoll;

/// The collapsing gate family whose collapse order spans a fused instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OrderedGate {
    Reset(PauliBasis),
    Measure { basis: PauliBasis, reset: bool },
    Pairs(PauliBasis),
}

impl OrderedGate {
    fn of(instruction: &ProgramInstruction) -> Option<(Self, NoiselessFusion, ArenaSpan<u32>)> {
        let (gate, fusion, targets) = match instruction {
            ProgramInstruction::Reset {
                basis,
                targets,
                fusion,
            } => (Self::Reset(*basis), *fusion, targets),
            ProgramInstruction::Measure {
                basis,
                reset,
                targets,
                fusion,
                ..
            } => (
                Self::Measure {
                    basis: *basis,
                    reset: *reset,
                },
                *fusion,
                targets,
            ),
            ProgramInstruction::MeasurePairs {
                basis,
                targets,
                fusion,
                ..
            } => (Self::Pairs(*basis), *fusion, targets),
            _ => return None,
        };
        matches!(gate.basis(), PauliBasis::X | PauliBasis::Y).then_some((gate, fusion, *targets))
    }

    fn basis(self) -> PauliBasis {
        match self {
            Self::Reset(basis) | Self::Measure { basis, .. } | Self::Pairs(basis) => basis,
        }
    }
}

/// Whether the reference simulator ignores `instruction` entirely, as Stim's noiseless circuit
/// drops it before fusing its neighbours.
fn dropped_by_noiseless_circuit(instruction: &ProgramInstruction) -> bool {
    matches!(
        instruction,
        ProgramInstruction::SinglePauliNoise { .. }
            | ProgramInstruction::TwoQubitPauliNoise { .. }
            | ProgramInstruction::CorrelatedError { .. }
    )
}

/// The target lists of the fused group that starts at `entries[start]`, in program order.
fn fused_targets<'a>(
    entries: &'a [ProgramEntry],
    arena: &'a ProgramArena,
    start: usize,
    gate: OrderedGate,
) -> impl Iterator<Item = Result<&'a [u32], ReferenceSimulatorError>> {
    let mut rest = entries.get(start..).unwrap_or_default().iter();
    let mut first = true;
    let mut done = false;
    std::iter::from_fn(move || {
        if done {
            return None;
        }
        let joined = rest
            .by_ref()
            .map_while(|entry| match entry {
                ProgramEntry::Instruction(instruction) => Some(instruction),
                ProgramEntry::Repeat { .. } | ProgramEntry::EndRepeat => None,
            })
            .find(|instruction| first || !dropped_by_noiseless_circuit(instruction))
            .and_then(OrderedGate::of)
            .filter(|(next_gate, fusion, _)| {
                first || (*next_gate == gate && *fusion == NoiselessFusion::WithPrevious)
            });
        first = false;
        done = joined.is_none();
        joined.map(|(_, _, targets)| {
            arena.get(targets).map_err(|_| {
                ReferenceSimulatorError::Invariant("fused collapse targets escaped their arena")
            })
        })
    })
}

/// Each item of `items`, or the error that replaced its list.
fn flatten_resolved<'a, T: Copy + 'a>(
    items: impl Iterator<Item = Result<&'a [T], ReferenceSimulatorError>>,
) -> impl Iterator<Item = Result<T, ReferenceSimulatorError>> {
    items.flat_map(|resolved| {
        let (values, error) = match resolved {
            Ok(values) => (values, None),
            Err(error) => (&[][..], Some(error)),
        };
        values.iter().copied().map(Ok).chain(error.map(Err))
    })
}

/// Collapses the fused group that starts at `entries[index]` in Stim's order, when that order
/// differs from the sequential one.
///
/// Instructions that only extend a group do nothing here: their group was collapsed when its
/// first instruction ran, and the same entries always run together because fusion never
/// crosses a repeat-block edge.
pub(super) fn collapse_fused_group(
    entries: &[ProgramEntry],
    arena: &ProgramArena,
    index: usize,
    simulator: &mut InverseTableauSimulator,
    poll: &mut WorkPoll<'_>,
    work: &mut SimulatorWork,
) -> Result<(), SamplingExecutionError> {
    let Some(ProgramEntry::Instruction(instruction)) = entries.get(index) else {
        return Ok(());
    };
    let Some((gate, NoiselessFusion::Starts, _)) = OrderedGate::of(instruction) else {
        return Ok(());
    };
    let basis = gate.basis();
    match gate {
        OrderedGate::Reset(_) | OrderedGate::Measure { .. } => {
            let targets = || {
                flatten_resolved(fused_targets(entries, arena, index, gate))
                    .map(|qubit| qubit.map(|qubit| qubit as usize))
            };
            if is_sorted(targets()).map_err(simulator_error)? {
                return Ok(());
            }
            simulator
                .collapse_ascending(targets(), basis, poll, work)
                .map_err(simulator_error)
        }
        OrderedGate::Pairs(_) => {
            let pairs = || {
                let groups = fused_targets(entries, arena, index, gate)
                    .map(|targets| targets.map(|targets| targets.as_chunks::<2>().0));
                flatten_resolved(groups)
                    .map(|pair| pair.map(|[first, second]| (first as usize, second as usize)))
            };
            if is_sorted(pairs().map(|pair| pair.map(|(first, _)| first)))
                .map_err(simulator_error)?
            {
                return Ok(());
            }
            simulator
                .collapse_pair_segments(pairs(), basis, poll, work)
                .map_err(simulator_error)
        }
    }
}

/// Whether `values` never decrease, in which case sequential collapse already follows Stim's
/// ascending order (repeated values are deterministic by the time they recur).
fn is_sorted(
    values: impl Iterator<Item = Result<usize, ReferenceSimulatorError>>,
) -> Result<bool, ReferenceSimulatorError> {
    let mut previous = None;
    for value in values {
        let value = value?;
        if previous.is_some_and(|previous| previous > value) {
            return Ok(false);
        }
        previous = Some(value);
    }
    Ok(true)
}
