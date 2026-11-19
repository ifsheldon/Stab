//! Materialization of one lowered instruction into its compiled entry and arena payload.

use stab_model::{
    CircuitInstruction, Gate, GateKind, Target,
    advanced::{
        ClassicalControl, ControlledPauliTargetPair, classify_controlled_pauli_target_pair,
    },
};

use super::super::super::helpers::{
    measurement_flip_probability, pauli_basis, probability_list, single_probability_argument,
    unsupported_frame_instruction,
};
use super::super::arena::{ArenaSpan, PauliProduct, PauliTerm, ProgramArena};
use super::super::{
    CliffordStep, ControlledPair, FrameGate, NoiselessFusion, PauliProductMeasurement,
    ProgramInstruction, ReferenceClifford, ReferenceGateId, program_shape_error,
};
use super::{EntryKind, SingleQubitNoise, TwoQubitNoise, any_inverted};
use crate::detection::error::{DetectionError, DetectionResult};
use crate::sampling::LocalTableauTransform;

/// The frame qubit of a qubit target.
fn frame_qubit(instruction: &CircuitInstruction, target: &Target) -> DetectionResult<u32> {
    target
        .qubit_id()
        .map(|qubit| qubit.get())
        .ok_or_else(|| unsupported_frame_instruction(instruction))
}

/// The out-of-line form the general builder uses, so its arms share one copy of the loop
/// instead of each widening the builder's frame.
#[inline(never)]
fn qubit_targets(
    instruction: &CircuitInstruction,
    arena: &mut ProgramArena,
) -> DetectionResult<ArenaSpan<u32>> {
    qubit_targets_inline(instruction, arena)
}

#[inline(always)]
fn qubit_targets_inline(
    instruction: &CircuitInstruction,
    arena: &mut ProgramArena,
) -> DetectionResult<ArenaSpan<u32>> {
    let start = arena.mark::<u32>();
    for target in instruction.targets() {
        arena.push(frame_qubit(instruction, target)?)?;
    }
    arena.span_since(start)
}

fn inversion_flags(
    instruction: &CircuitInstruction,
    arena: &mut ProgramArena,
) -> DetectionResult<ArenaSpan<bool>> {
    if !any_inverted(instruction) {
        return Ok(ArenaSpan::EMPTY);
    }
    arena.extend(
        instruction
            .targets()
            .iter()
            .map(Target::is_inverted_result_target),
    )
}

fn pauli_terms(
    instruction: &CircuitInstruction,
    arena: &mut ProgramArena,
) -> DetectionResult<ArenaSpan<PauliTerm>> {
    let start = arena.mark::<PauliTerm>();
    for target in instruction.targets() {
        // Pinned Stim consults only the Pauli X/Z bits of correlated-error targets, so combiner
        // targets and inversion bits are ignored decoration (frame_simulator.inl:767-775).
        // Observable record targets belong to detection conversion.
        if target.is_combiner() || target.measurement_record_offset().is_some() {
            continue;
        }
        let Some(pauli) = target.pauli_type() else {
            return Err(unsupported_frame_instruction(instruction));
        };
        arena.push((frame_qubit(instruction, target)?, pauli_basis(pauli)))?;
    }
    arena.span_since(start)
}

#[inline(always)]
pub(super) fn build_clifford(
    instruction: &CircuitInstruction,
    frame: FrameGate,
    cliffords: &mut ReferenceCliffordTable,
    arena: &mut ProgramArena,
) -> DetectionResult<ProgramInstruction> {
    Ok(ProgramInstruction::Clifford {
        frame,
        reference: cliffords.id(instruction.gate())?,
        targets: qubit_targets_inline(instruction, arena)?,
    })
}

#[inline(never)]
fn build_clifford_general(
    instruction: &CircuitInstruction,
    frame: FrameGate,
    cliffords: &mut ReferenceCliffordTable,
    arena: &mut ProgramArena,
) -> DetectionResult<ProgramInstruction> {
    build_clifford(instruction, frame, cliffords, arena)
}

// Only the out-of-line general path calls this, so it is left to the optimizer: forcing it inline
// puts every arm's temporaries into one caller frame in unoptimized builds.
pub(super) fn build(
    instruction: &CircuitInstruction,
    kind: EntryKind,
    fusion: NoiselessFusion,
    cliffords: &mut ReferenceCliffordTable,
    arena: &mut ProgramArena,
) -> DetectionResult<ProgramInstruction> {
    Ok(match kind {
        EntryKind::Clifford(frame) => build_clifford_general(instruction, frame, cliffords, arena)?,
        EntryKind::Reset(basis) => ProgramInstruction::Reset {
            basis,
            targets: qubit_targets(instruction, arena)?,
            fusion,
        },
        EntryKind::Measure { basis, reset } => ProgramInstruction::Measure {
            basis,
            reset,
            flip_probability: measurement_flip_probability(instruction)?,
            targets: qubit_targets(instruction, arena)?,
            inverted: inversion_flags(instruction, arena)?,
            fusion,
        },
        EntryKind::MeasurePairs(basis) => ProgramInstruction::MeasurePairs {
            basis,
            flip_probability: measurement_flip_probability(instruction)?,
            targets: qubit_targets(instruction, arena)?,
            inverted: inversion_flags(instruction, arena)?,
            fusion,
        },
        EntryKind::MeasurePads => {
            let start = arena.mark::<bool>();
            for target in instruction.targets() {
                arena.push(frame_qubit(instruction, target)? == 1)?;
            }
            ProgramInstruction::MeasurePads {
                flip_probability: measurement_flip_probability(instruction)?,
                values: arena.span_since(start)?,
            }
        }
        EntryKind::PauliProducts { .. } => {
            ProgramInstruction::PauliProducts(build_pauli_products(instruction, arena)?)
        }
        EntryKind::SinglePauliNoise(noise) => {
            let probabilities = match noise {
                SingleQubitNoise::X => [single_probability_argument(instruction)?.get(), 0.0, 0.0],
                SingleQubitNoise::Y => [0.0, single_probability_argument(instruction)?.get(), 0.0],
                SingleQubitNoise::Z => [0.0, 0.0, single_probability_argument(instruction)?.get()],
                SingleQubitNoise::Depolarize => {
                    [single_probability_argument(instruction)?.get() / 3.0; 3]
                }
                SingleQubitNoise::PauliChannel => probability_list::<3>(instruction)?,
            };
            ProgramInstruction::SinglePauliNoise {
                probabilities: arena.extend(probabilities)?,
                targets: qubit_targets(instruction, arena)?,
            }
        }
        EntryKind::TwoQubitPauliNoise(noise) => {
            let probabilities = match noise {
                TwoQubitNoise::Depolarize => {
                    [single_probability_argument(instruction)?.get() / 15.0; 15]
                }
                TwoQubitNoise::PauliChannel => probability_list::<15>(instruction)?,
            };
            ProgramInstruction::TwoQubitPauliNoise {
                probabilities: arena.extend(probabilities)?,
                targets: qubit_targets(instruction, arena)?,
            }
        }
        EntryKind::CorrelatedError { else_branch } => ProgramInstruction::CorrelatedError {
            else_branch,
            probability: single_probability_argument(instruction)?.get(),
            terms: pauli_terms(instruction, arena)?,
        },
        EntryKind::HeraldedErase => ProgramInstruction::HeraldedErase {
            probability: single_probability_argument(instruction)?.get(),
            targets: qubit_targets(instruction, arena)?,
        },
        EntryKind::HeraldedPauliChannel => ProgramInstruction::HeraldedPauliChannel {
            probabilities: arena.extend(probability_list::<4>(instruction)?)?,
            targets: qubit_targets(instruction, arena)?,
        },
        EntryKind::ControlledPauli { basis, map } => ProgramInstruction::ControlledPauli {
            basis,
            frame: map,
            reference: cliffords.id(instruction.gate())?,
            pairs: controlled_pairs(instruction, arena)?,
        },
        EntryKind::Observable => {
            let observable = instruction.observable_id_argument()?.ok_or_else(|| {
                DetectionError::invalid_result_format("OBSERVABLE_INCLUDE missing id")
            })?;
            let id = usize::try_from(observable.get()).map_err(|_| {
                DetectionError::invalid_result_format(format!(
                    "observable id {} does not fit usize",
                    observable.get()
                ))
            })?;
            ProgramInstruction::Observable {
                id,
                terms: pauli_terms(instruction, arena)?,
            }
        }
    })
}

fn controlled_pairs(
    instruction: &CircuitInstruction,
    arena: &mut ProgramArena,
) -> DetectionResult<ArenaSpan<ControlledPair>> {
    let start = arena.mark::<ControlledPair>();
    for target_group in instruction.targets().chunks(2) {
        let pair = match classify_controlled_pauli_target_pair(instruction.gate(), target_group) {
            ControlledPauliTargetPair::Quantum { .. } => {
                let [control, target] = target_group else {
                    return Err(unsupported_frame_instruction(instruction));
                };
                ControlledPair::Quantum {
                    control: frame_qubit(instruction, control)?,
                    target: frame_qubit(instruction, target)?,
                }
            }
            ControlledPauliTargetPair::Classical { control, target } => {
                let qubit = target.get();
                match control {
                    ClassicalControl::Record(offset) => ControlledPair::Record { offset, qubit },
                    ClassicalControl::Sweep(id) => ControlledPair::Sweep { id, qubit },
                }
            }
            ControlledPauliTargetPair::ClassicalNoop { .. } => continue,
            ControlledPauliTargetPair::Unsupported => {
                return Err(unsupported_frame_instruction(instruction));
            }
        };
        arena.push(pair)?;
    }
    arena.span_since(start)
}

fn build_pauli_products(
    instruction: &CircuitInstruction,
    arena: &mut ProgramArena,
) -> DetectionResult<PauliProductMeasurement> {
    let terms = arena.mark::<PauliTerm>();
    let products = arena.mark::<PauliProduct>();
    // One pair of buffers serves every product of the instruction.
    let mut raw_terms = Vec::new();
    let mut normalized = Vec::new();
    for target_group in instruction.target_groups() {
        raw_terms.clear();
        for target in target_group {
            if target.is_combiner() {
                continue;
            }
            let Some(pauli) = target.pauli_type() else {
                return Err(unsupported_frame_instruction(instruction));
            };
            raw_terms.push((
                frame_qubit(instruction, target)?,
                pauli_basis(pauli),
                target.is_inverted_result_target(),
            ));
        }
        let product_inverted = crate::sampling::pauli_product::normalize_terms_into(
            &raw_terms,
            false,
            &mut normalized,
        )?;
        arena.push(PauliProduct::new(normalized.len(), product_inverted)?)?;
        arena.extend(normalized.iter().copied())?;
    }
    Ok(PauliProductMeasurement {
        terms: arena.span_since(terms)?,
        products: arena.span_since(products)?,
        flip_probability: measurement_flip_probability(instruction)?,
    })
}

/// Resolves each distinct gate's reference-sample action once per program, numbering the
/// actions in order of first use.
pub(super) struct ReferenceCliffordTable {
    ids: [Option<ReferenceGateId>; GateKind::COUNT],
    pub(super) table: Vec<ReferenceClifford>,
}

impl Default for ReferenceCliffordTable {
    fn default() -> Self {
        Self {
            ids: [None; GateKind::COUNT],
            table: Vec::new(),
        }
    }
}

impl ReferenceCliffordTable {
    #[inline(always)]
    pub(super) fn id(&mut self, gate: Gate) -> DetectionResult<ReferenceGateId> {
        match self.ids.get(gate.kind() as usize) {
            Some(Some(id)) => Ok(*id),
            _ => self.resolve(gate),
        }
    }

    #[cold]
    fn resolve(&mut self, gate: Gate) -> DetectionResult<ReferenceGateId> {
        let slot = self
            .ids
            .get_mut(gate.kind() as usize)
            .ok_or_else(program_shape_error)?;
        let clifford = reference_clifford(gate)?;
        self.table.push(clifford);
        let id = u16::try_from(self.table.len() - 1)
            .map(ReferenceGateId)
            .map_err(|_| program_shape_error())?;
        *slot = Some(id);
        Ok(id)
    }
}

fn reference_clifford(gate: Gate) -> DetectionResult<ReferenceClifford> {
    // `H` and `CX` have dedicated reference kernels; every other gate takes the general tableau
    // path, which rejects gates without a Clifford action.
    Ok(match gate.kind() {
        GateKind::H => ReferenceClifford::Hadamard,
        GateKind::CX => ReferenceClifford::ControlledX,
        _ => {
            let tableau = match stab_analysis::single_qubit_clifford_for_gate(gate) {
                Ok(clifford) => clifford.tableau(),
                Err(_) => stab_analysis::gate_tableau(gate)?,
            };
            ReferenceClifford::Tableau {
                transform: LocalTableauTransform::from_tableau(&tableau)?,
                steps: decomposition_steps(gate, tableau.len()),
            }
        }
    })
}

/// Parses the gate's pinned Stim `H`/`S`/`CX` decomposition, which is exact including signs,
/// or returns `None` when the gate has none in that form.
fn decomposition_steps(gate: Gate, arity: usize) -> Option<Vec<CliffordStep>> {
    let decomposition = stab_model::advanced::gate_decomposition(gate)?;
    let mut steps = Vec::new();
    for line in decomposition.as_stim_str().lines() {
        let mut tokens = line
            .split('#')
            .next()
            .unwrap_or_default()
            .split_whitespace();
        let Some(name) = tokens.next() else {
            continue;
        };
        let mut qubits = Vec::new();
        for token in tokens {
            let qubit = token.parse::<u8>().ok()?;
            if usize::from(qubit) >= arity {
                return None;
            }
            qubits.push(qubit);
        }
        match name {
            "H" => steps.extend(qubits.iter().map(|&qubit| CliffordStep::H(qubit))),
            "S" => steps.extend(qubits.iter().map(|&qubit| CliffordStep::S(qubit))),
            "CX" | "CNOT" => {
                let (pairs, unpaired) = qubits.as_chunks::<2>();
                if !unpaired.is_empty() {
                    return None;
                }
                steps.extend(
                    pairs
                        .iter()
                        .map(|&[control, target]| CliffordStep::Cx(control, target)),
                );
            }
            _ => return None,
        }
    }
    Some(steps)
}
