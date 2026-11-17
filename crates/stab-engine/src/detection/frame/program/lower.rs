//! Allocation-free admission and materialization of a [`CircuitProgram`].

use std::ops::ControlFlow;

use arrayvec::ArrayVec;
use stab_algebra::PauliBasis;
use stab_model::{
    Circuit, CircuitInstruction, Gate, GateKind, RepeatNestingLimit, Target,
    advanced::{
        ClassicalControl, ControlledPauliTargetPair, classify_controlled_pauli_target_pair,
    },
};

use super::super::clifford::{FrameCliffordAction, FrameCliffordActionCache, TwoQubitFrameMap};
use super::super::helpers::{
    measurement_flip_probability, pauli_basis, probability_list, qubit_index,
    single_probability_argument, unsupported_frame_instruction, unsupported_frame_target,
    zero_probability_noise,
};
use super::arena::{ArenaCounts, ArenaSpan, PauliProduct, PauliTerm, ProgramArena};
use super::{
    CircuitProgram, CliffordStep, ControlledPair, FrameGate, NoiselessFusion,
    PauliProductMeasurement, ProgramCounts, ProgramEntry, ProgramInstruction, ReferenceClifford,
    ReferenceGateId, TraversalEvent, admission_drift, byte_product, checked_add_bytes,
    materialized_retained_bytes, program_shape_error, storage_overflow, validate_program,
    visit_circuit, window_needs,
};
use crate::detection::error::{
    DetectionError, DetectionResourceLimitError as ResourceLimitError, DetectionResult,
};
use crate::sampling::LocalTableauTransform;

/// Lowering choices that change which instructions a program retains.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProgramOptions {
    /// Retain `OBSERVABLE_INCLUDE` Pauli terms, which detection frames record.
    pub(crate) include_pauli_observables: bool,
}

impl ProgramOptions {
    pub(crate) const SAMPLING: Self = Self {
        include_pauli_observables: false,
    };
    pub(crate) const DETECTION: Self = Self {
        include_pauli_observables: true,
    };
}

/// The allocation-free shape of a program, computed before any program storage is reserved.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProgramAdmission {
    entry_count: usize,
    arena: ArenaCounts,
    retained_bytes: u64,
    frame_qubit_count: usize,
    counts: ProgramCounts,
    options: ProgramOptions,
}

impl ProgramAdmission {
    pub(crate) const fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }

    pub(crate) const fn counts(&self) -> ProgramCounts {
        self.counts
    }
}

/// How one lowered instruction is represented in the program.
#[derive(Clone, Copy, Debug)]
enum Lowering {
    /// The instruction changes neither a frame, a reference state, nor a record.
    Noop,
    /// Zero-probability noise that compiles to nothing but still sizes the frame, because the
    /// frame width fixes how much randomness a batch reset draws.
    FrameIdentity,
    Entry(EntryKind),
}

#[derive(Clone, Copy, Debug)]
enum EntryKind {
    Clifford(FrameGate),
    Reset(PauliBasis),
    Measure {
        basis: PauliBasis,
        reset: bool,
    },
    MeasurePairs(PauliBasis),
    MeasurePads,
    PauliProducts {
        terms: usize,
        products: usize,
    },
    SinglePauliNoise(SingleQubitNoise),
    TwoQubitPauliNoise(TwoQubitNoise),
    CorrelatedError {
        else_branch: bool,
    },
    HeraldedErase,
    HeraldedPauliChannel,
    ControlledPauli {
        basis: PauliBasis,
        map: TwoQubitFrameMap,
    },
    Observable,
}

/// The single-qubit Pauli channel a noise gate lowers to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SingleQubitNoise {
    X,
    Y,
    Z,
    Depolarize,
    PauliChannel,
}

/// The two-qubit Pauli channel a noise gate lowers to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TwoQubitNoise {
    Depolarize,
    PauliChannel,
}

/// Running counts of the admission walk, with folded repeat totals.
#[derive(Clone, Copy, Debug, Default)]
struct CountState {
    measurements: u64,
    sweep_bits: u64,
}

#[derive(Clone, Copy)]
struct PendingRepeat {
    count: u64,
    marker: Option<usize>,
}

#[derive(Clone, Copy)]
struct RepeatAdmission {
    count: u64,
    entries_before: usize,
    arena_before: ArenaCounts,
    bytes_before: u64,
    counts_before: CountState,
}

impl CircuitProgram {
    /// Admits a program without allocating its storage, rejecting it once `retained_base_bytes`
    /// plus the program's retained bytes would exceed `max_combined_bytes`.
    pub(crate) fn admit(
        circuit: &Circuit,
        options: ProgramOptions,
        retained_base_bytes: u64,
        max_combined_bytes: u64,
    ) -> DetectionResult<ProgramAdmission> {
        let mut admission = ProgramAdmission {
            options,
            ..ProgramAdmission::default()
        };
        let mut counts = CountState::default();
        let mut simulated_qubits = 0_usize;
        let mut repeats = ArrayVec::<RepeatAdmission, { RepeatNestingLimit::HARD_MAX }>::new();
        let mut actions = FrameCliffordActionCache::default();
        let entry_bytes = byte_product(1, size_of::<ProgramEntry>())?;
        let check = |admission: &ProgramAdmission| {
            let combined = checked_add_bytes(retained_base_bytes, admission.retained_bytes)?;
            if combined > max_combined_bytes {
                return Err(DetectionError::from(
                    ResourceLimitError::detection_compiled_bytes(combined, max_combined_bytes),
                ));
            }
            Ok(())
        };
        visit_circuit(circuit, |event| {
            match event {
                TraversalEvent::Instruction(instruction) => {
                    // The source instruction, not its lowering, sizes Stim's simulated qubit count, so
                    // instructions that lower to nothing still count.
                    simulated_qubits = simulated_qubits.max(instruction_qubit_count(instruction)?);
                    visit_lowered_instructions(instruction, |lowered| {
                        let lowering = lower(lowered, options, &mut actions)?;
                        record_effects(lowered, lowering, &mut counts)?;
                        match lowering {
                            Lowering::Noop => return Ok(()),
                            Lowering::FrameIdentity => {}
                            Lowering::Entry(kind) => {
                                admission.entry_count = admission
                                    .entry_count
                                    .checked_add(1)
                                    .ok_or_else(storage_overflow)?;
                                let payload = arena_demand(lowered, kind)?;
                                admission.arena.add(payload)?;
                                admission.retained_bytes = checked_add_bytes(
                                    admission.retained_bytes,
                                    checked_add_bytes(entry_bytes, payload.bytes()?)?,
                                )?;
                            }
                        }
                        admission.frame_qubit_count = admission
                            .frame_qubit_count
                            .max(instruction_qubit_count(lowered)?);
                        check(&admission)
                    })?;
                }
                TraversalEvent::BeginRepeat(count) => {
                    repeats
                        .try_push(RepeatAdmission {
                            count,
                            entries_before: admission.entry_count,
                            arena_before: admission.arena,
                            bytes_before: admission.retained_bytes,
                            counts_before: counts,
                        })
                        .map_err(|_| program_shape_error())?;
                    admission.entry_count = admission
                        .entry_count
                        .checked_add(1)
                        .ok_or_else(storage_overflow)?;
                    admission.retained_bytes =
                        checked_add_bytes(admission.retained_bytes, entry_bytes)?;
                    check(&admission)?;
                }
                TraversalEvent::EndRepeat => {
                    let repeat = repeats.pop().ok_or_else(program_shape_error)?;
                    integrate_repeat(&mut counts, repeat)?;
                    if admission.entry_count == repeat.entries_before + 1 {
                        // A body that lowers to nothing is elided with its marker.
                        admission.entry_count = repeat.entries_before;
                        admission.arena = repeat.arena_before;
                        admission.retained_bytes = repeat.bytes_before;
                    } else {
                        admission.entry_count = admission
                            .entry_count
                            .checked_add(1)
                            .ok_or_else(storage_overflow)?;
                        admission.retained_bytes =
                            checked_add_bytes(admission.retained_bytes, entry_bytes)?;
                        check(&admission)?;
                    }
                }
            }
            Ok(())
        })?;
        admission.counts = ProgramCounts {
            measurements: usize::try_from(counts.measurements).map_err(|_| {
                DetectionError::invalid_sampler_compilation(
                    "measurement record count cannot fit in usize during sampler compilation",
                )
            })?,
            sweep_bits: usize::try_from(counts.sweep_bits).map_err(|_| {
                DetectionError::invalid_sampler_compilation(
                    "sweep bit count cannot fit in usize during sampler compilation",
                )
            })?,
            simulated_qubits,
        };
        Ok(admission)
    }

    /// Builds the program that `admission` admitted.
    pub(crate) fn materialize(
        circuit: &Circuit,
        admission: &ProgramAdmission,
    ) -> DetectionResult<Self> {
        let options = admission.options;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(admission.entry_count)
            .map_err(|error| {
                DetectionError::invalid_sampler_compilation(format!(
                    "unable to reserve {} compact detector-frame entries: {error}",
                    admission.entry_count
                ))
            })?;
        let mut arena = ProgramArena::try_with_capacity(admission.arena)?;
        // A repeat marker is emitted only once its body emits an entry, so bodies that lower to
        // nothing vanish without ever exceeding the admitted entry capacity.
        let mut repeat_markers = ArrayVec::<PendingRepeat, { RepeatNestingLimit::HARD_MAX }>::new();
        let mut frame_qubit_count = 0_usize;
        let mut actions = FrameCliffordActionCache::default();
        let mut cliffords = ReferenceCliffordTable::default();
        let mut noiseless_previous: Option<&CircuitInstruction> = None;
        let push = |entries: &mut Vec<ProgramEntry>, entry: ProgramEntry| {
            if entries.len() == entries.capacity() {
                return Err(admission_drift());
            }
            entries.push(entry);
            Ok(())
        };
        visit_circuit(circuit, |event| {
            match event {
                TraversalEvent::Instruction(instruction) => {
                    let fusion = noiseless_fusion(noiseless_previous, instruction);
                    if kept_by_noiseless_circuit(instruction.gate()) {
                        noiseless_previous = Some(instruction);
                    }
                    visit_lowered_instructions(instruction, |lowered| {
                        let lowering = lower(lowered, options, &mut actions)?;
                        if let Lowering::Entry(kind) = lowering {
                            let built = build(lowered, kind, fusion, &mut cliffords, &mut arena)?;
                            for pending in &mut repeat_markers {
                                if pending.marker.is_none() {
                                    pending.marker = Some(entries.len());
                                    push(
                                        &mut entries,
                                        ProgramEntry::Repeat {
                                            count: pending.count,
                                            body_end: usize::MAX,
                                        },
                                    )?;
                                }
                            }
                            push(&mut entries, ProgramEntry::Instruction(built))?;
                        }
                        if !matches!(lowering, Lowering::Noop) {
                            frame_qubit_count =
                                frame_qubit_count.max(instruction_qubit_count(lowered)?);
                        }
                        Ok(())
                    })?;
                }
                TraversalEvent::BeginRepeat(count) => {
                    // A repeat block is its own operation, so nothing fuses across its edges.
                    noiseless_previous = None;
                    repeat_markers
                        .try_push(PendingRepeat {
                            count,
                            marker: None,
                        })
                        .map_err(|_| program_shape_error())?;
                }
                TraversalEvent::EndRepeat => {
                    noiseless_previous = None;
                    let pending = repeat_markers.pop().ok_or_else(program_shape_error)?;
                    let Some(marker) = pending.marker else {
                        return Ok(());
                    };
                    let body_end = entries.len();
                    push(&mut entries, ProgramEntry::EndRepeat)?;
                    let Some(ProgramEntry::Repeat {
                        body_end: stored_end,
                        ..
                    }) = entries.get_mut(marker)
                    else {
                        return Err(program_shape_error());
                    };
                    *stored_end = body_end;
                }
            }
            Ok(())
        })?;
        if !repeat_markers.is_empty()
            || entries.len() != admission.entry_count
            || frame_qubit_count != admission.frame_qubit_count
        {
            return Err(admission_drift());
        }
        validate_program(&entries)?;
        let retained_bytes = materialized_retained_bytes(entries.capacity(), &arena)?;
        if retained_bytes > admission.retained_bytes {
            return Err(admission_drift());
        }
        let window_needs = window_needs(&entries, &arena)?;
        Ok(Self {
            entries,
            arena,
            cliffords: cliffords.table,
            retained_bytes,
            frame_qubit_count,
            counts: admission.counts,
            window_needs,
        })
    }
}

/// Whether Stim's `Circuit::aliased_noiseless_circuit` keeps an instruction of `gate`.
///
/// It drops noise channels that produce no results, keeps result-producing gates without their
/// flip arguments (heralded noise becomes `MPAD`), and keeps every other instruction, including
/// annotations such as `TICK` and `DETECTOR` that the compiled program elides.
fn kept_by_noiseless_circuit(gate: Gate) -> bool {
    !gate.is_noisy() || gate.produces_measurements()
}

/// Whether Stim's noiseless reference circuit fuses `instruction` into `previous`, the last
/// instruction it kept in the same block, for the X- and Y-basis collapsing gates whose
/// collapse order spans a fused instruction.
///
/// Stim fuses two operations of the same gate with equal arguments and tags; result-producing
/// gates lose their arguments first, so only the gate and tag must agree for them.
fn noiseless_fusion(
    previous: Option<&CircuitInstruction>,
    instruction: &CircuitInstruction,
) -> NoiselessFusion {
    let gate = instruction.gate();
    let ordered = matches!(
        gate.kind(),
        GateKind::RX
            | GateKind::RY
            | GateKind::MX
            | GateKind::MY
            | GateKind::MRX
            | GateKind::MRY
            | GateKind::MXX
            | GateKind::MYY
    );
    match previous {
        Some(previous)
            if ordered
                && previous.gate().kind() == gate.kind()
                && previous.tag_bytes() == instruction.tag_bytes()
                && (gate.produces_measurements() || previous.args() == instruction.args()) =>
        {
            NoiselessFusion::WithPrevious
        }
        _ => NoiselessFusion::Starts,
    }
}

fn integrate_repeat(counts: &mut CountState, repeat: RepeatAdmission) -> DetectionResult<()> {
    let before = repeat.counts_before;
    let body_measurements = counts
        .measurements
        .checked_sub(before.measurements)
        .ok_or_else(|| {
            DetectionError::invalid_sampler_compilation(
                "repeat-body measurement count moved backwards during sampler compilation",
            )
        })?;
    let repeated = body_measurements.checked_mul(repeat.count).ok_or_else(|| {
        DetectionError::invalid_sampler_compilation(
            "repeated measurement record count overflows during sampler compilation",
        )
    })?;
    counts.measurements = before.measurements.checked_add(repeated).ok_or_else(|| {
        DetectionError::invalid_sampler_compilation(
            "measurement record count overflows during sampler compilation",
        )
    })?;
    Ok(())
}

/// Calls `visitor` with `instruction`, or with each instruction of its single-instruction
/// decomposition for `SPP` and `SPP_DAG`.
fn visit_lowered_instructions(
    instruction: &CircuitInstruction,
    mut visitor: impl FnMut(&CircuitInstruction) -> DetectionResult<()>,
) -> DetectionResult<()> {
    if !is_pauli_product_phase(instruction.gate()) {
        return visitor(instruction);
    }
    let completion =
        stab_analysis::advanced::visit_decomposed_spp_instructions(instruction, |lowered| {
            if is_pauli_product_phase(lowered.gate()) {
                return ControlFlow::Break(DetectionError::invalid_sampler_compilation(
                    "single-instruction frame lowering retained its source instruction",
                ));
            }
            match visitor(&lowered) {
                Ok(()) => ControlFlow::Continue(()),
                Err(error) => ControlFlow::Break(error),
            }
        })
        .map_err(|error| {
            DetectionError::invalid_sampler_compilation(format!(
                "{} cannot be compiled for detector-frame execution: {error}",
                instruction.gate().canonical_name()
            ))
        })?;
    if let ControlFlow::Break(error) = completion {
        return Err(error);
    }
    Ok(())
}

fn is_pauli_product_phase(gate: Gate) -> bool {
    matches!(gate.kind(), GateKind::SPP | GateKind::SPP_DAG)
}

fn lower(
    instruction: &CircuitInstruction,
    options: ProgramOptions,
    actions: &mut FrameCliffordActionCache,
) -> DetectionResult<Lowering> {
    let kind = match instruction.gate().kind() {
        GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS
        | GateKind::DETECTOR
        | GateKind::I_ERROR
        | GateKind::II_ERROR => {
            return Ok(Lowering::Noop);
        }
        GateKind::OBSERVABLE_INCLUDE => {
            if !options.include_pauli_observables
                || !instruction.targets().iter().any(Target::is_pauli_target)
            {
                return Ok(Lowering::Noop);
            }
            EntryKind::Observable
        }
        GateKind::R => EntryKind::Reset(PauliBasis::Z),
        GateKind::RX => EntryKind::Reset(PauliBasis::X),
        GateKind::RY => EntryKind::Reset(PauliBasis::Y),
        GateKind::M => measure(PauliBasis::Z, false),
        GateKind::MX => measure(PauliBasis::X, false),
        GateKind::MY => measure(PauliBasis::Y, false),
        GateKind::MR => measure(PauliBasis::Z, true),
        GateKind::MRX => measure(PauliBasis::X, true),
        GateKind::MRY => measure(PauliBasis::Y, true),
        GateKind::MXX => EntryKind::MeasurePairs(PauliBasis::X),
        GateKind::MYY => EntryKind::MeasurePairs(PauliBasis::Y),
        GateKind::MZZ => EntryKind::MeasurePairs(PauliBasis::Z),
        GateKind::MPP => pauli_product_shape(instruction)?,
        GateKind::MPAD => EntryKind::MeasurePads,
        GateKind::X_ERROR => return single_noise(instruction, SingleQubitNoise::X),
        GateKind::Y_ERROR => return single_noise(instruction, SingleQubitNoise::Y),
        GateKind::Z_ERROR => return single_noise(instruction, SingleQubitNoise::Z),
        GateKind::DEPOLARIZE1 => return single_noise(instruction, SingleQubitNoise::Depolarize),
        GateKind::PAULI_CHANNEL_1 => {
            return single_noise(instruction, SingleQubitNoise::PauliChannel);
        }
        GateKind::DEPOLARIZE2 => return pair_noise(instruction, TwoQubitNoise::Depolarize),
        GateKind::PAULI_CHANNEL_2 => return pair_noise(instruction, TwoQubitNoise::PauliChannel),
        GateKind::E => EntryKind::CorrelatedError { else_branch: false },
        GateKind::ELSE_CORRELATED_ERROR => EntryKind::CorrelatedError { else_branch: true },
        GateKind::HERALDED_ERASE => EntryKind::HeraldedErase,
        GateKind::HERALDED_PAULI_CHANNEL_1 => EntryKind::HeraldedPauliChannel,
        // `SPP` and `SPP_DAG` arrive here only if their decomposition retained them, and
        // `REPEAT` never forms an instruction; both fail as unsupported frame actions.
        GateKind::XCX
        | GateKind::XCY
        | GateKind::XCZ
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::YCZ
        | GateKind::CX
        | GateKind::CY
        | GateKind::CZ
        | GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
        | GateKind::I
        | GateKind::X
        | GateKind::Y
        | GateKind::Z
        | GateKind::C_XYZ
        | GateKind::C_ZYX
        | GateKind::C_NXYZ
        | GateKind::C_XNYZ
        | GateKind::C_XYNZ
        | GateKind::C_NZYX
        | GateKind::C_ZNYX
        | GateKind::C_ZYNX
        | GateKind::SQRT_X
        | GateKind::SQRT_X_DAG
        | GateKind::SQRT_Y
        | GateKind::SQRT_Y_DAG
        | GateKind::S
        | GateKind::S_DAG
        | GateKind::II
        | GateKind::SQRT_XX
        | GateKind::SQRT_XX_DAG
        | GateKind::SQRT_YY
        | GateKind::SQRT_YY_DAG
        | GateKind::SQRT_ZZ
        | GateKind::SQRT_ZZ_DAG
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG
        | GateKind::SPP
        | GateKind::SPP_DAG
        | GateKind::REPEAT => return lower_clifford(instruction, actions),
    };
    Ok(Lowering::Entry(kind))
}

/// Lowers a Pauli channel, or a channel whose probabilities are all zero to nothing.
fn single_noise(
    instruction: &CircuitInstruction,
    noise: SingleQubitNoise,
) -> DetectionResult<Lowering> {
    Ok(if zero_probability_noise(instruction)? {
        Lowering::Noop
    } else {
        Lowering::Entry(EntryKind::SinglePauliNoise(noise))
    })
}

fn pair_noise(instruction: &CircuitInstruction, noise: TwoQubitNoise) -> DetectionResult<Lowering> {
    Ok(if zero_probability_noise(instruction)? {
        Lowering::Noop
    } else {
        Lowering::Entry(EntryKind::TwoQubitPauliNoise(noise))
    })
}

/// Lowers a unitary gate through its cached frame action, or a zero-probability instruction
/// without one to a frame identity.
fn lower_clifford(
    instruction: &CircuitInstruction,
    actions: &mut FrameCliffordActionCache,
) -> DetectionResult<Lowering> {
    let gate = instruction.gate();
    let Some(action) = actions.action(gate)? else {
        if zero_probability_noise(instruction)? {
            return Ok(Lowering::FrameIdentity);
        }
        return Err(DetectionError::invalid_sampler_compilation(format!(
            "detector frame execution does not support {}",
            gate.canonical_name()
        )));
    };
    let controlled = gate.controlled_pauli().map(pauli_basis);
    if controlled.is_some() {
        validate_controlled_pauli_targets(instruction)?;
    }
    if let Some(target) = instruction
        .targets()
        .iter()
        .find(|target| target.qubit_id().is_none())
    {
        return match (action, controlled) {
            (FrameCliffordAction::Two(map), Some(basis)) => {
                Ok(Lowering::Entry(EntryKind::ControlledPauli { basis, map }))
            }
            _ => Err(unsupported_frame_target(gate, target)),
        };
    }
    Ok(Lowering::Entry(EntryKind::Clifford(match action {
        FrameCliffordAction::Identity => FrameGate::Identity,
        FrameCliffordAction::Single(map) => FrameGate::Single(map),
        FrameCliffordAction::Two(map) => FrameGate::Two(map),
    })))
}

const fn measure(basis: PauliBasis, reset: bool) -> EntryKind {
    EntryKind::Measure { basis, reset }
}

fn validate_controlled_pauli_targets(instruction: &CircuitInstruction) -> DetectionResult<()> {
    for target_group in instruction.targets().chunks(2) {
        if matches!(
            classify_controlled_pauli_target_pair(instruction.gate(), target_group),
            ControlledPauliTargetPair::Unsupported
        ) {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "sampling execution does not support the {} target shape",
                instruction.gate().canonical_name()
            )));
        }
    }
    Ok(())
}

/// Counts one product per Pauli target that is not joined to its predecessor by a combiner,
/// and reserves one arena slot per raw term, since normalization only removes terms.
fn pauli_product_shape(instruction: &CircuitInstruction) -> DetectionResult<EntryKind> {
    let (mut terms, mut products) = (0_usize, 0_usize);
    let mut joined = false;
    for target in instruction.targets() {
        if target.is_combiner() {
            joined = true;
            continue;
        }
        terms = terms.checked_add(1).ok_or_else(storage_overflow)?;
        if !joined {
            products = products.checked_add(1).ok_or_else(storage_overflow)?;
        }
        joined = false;
    }
    Ok(EntryKind::PauliProducts { terms, products })
}

/// Records measurements, sweep bits, and record-lookback validity.
fn record_effects(
    instruction: &CircuitInstruction,
    lowering: Lowering,
    counts: &mut CountState,
) -> DetectionResult<()> {
    let targets = instruction.targets().len();
    let measurements = match lowering {
        Lowering::Noop | Lowering::FrameIdentity => 0,
        Lowering::Entry(kind) => match kind {
            EntryKind::Measure { .. } | EntryKind::MeasurePads | EntryKind::HeraldedErase => {
                targets
            }
            EntryKind::HeraldedPauliChannel => targets,
            EntryKind::MeasurePairs(_) => targets / 2,
            EntryKind::PauliProducts { products, .. } => products,
            EntryKind::ControlledPauli { .. } => {
                record_controls(instruction, counts)?;
                0
            }
            EntryKind::Clifford(_)
            | EntryKind::Reset(_)
            | EntryKind::SinglePauliNoise(_)
            | EntryKind::TwoQubitPauliNoise(_)
            | EntryKind::CorrelatedError { .. }
            | EntryKind::Observable => 0,
        },
    };
    if let Lowering::Entry(EntryKind::Clifford(_)) = lowering
        && instruction.gate().controlled_pauli().is_some()
    {
        record_controls(instruction, counts)?;
    }
    counts.measurements = counts
        .measurements
        .checked_add(u64::try_from(measurements).map_err(|_| {
            DetectionError::invalid_sampler_compilation(
                "measurement record count cannot fit in u64 during sampler compilation",
            )
        })?)
        .ok_or_else(|| {
            DetectionError::invalid_sampler_compilation(
                "measurement record count overflows during sampler compilation",
            )
        })?;
    Ok(())
}

/// Validates the classical controls of a controlled-Pauli instruction and returns the number of
/// target pairs that execute an operation.
fn record_controls(
    instruction: &CircuitInstruction,
    counts: &mut CountState,
) -> DetectionResult<usize> {
    let mut operations = 0_usize;
    for target_group in instruction.targets().chunks(2) {
        match classify_controlled_pauli_target_pair(instruction.gate(), target_group) {
            ControlledPauliTargetPair::Quantum { .. } => operations += 1,
            ControlledPauliTargetPair::Classical { control, .. } => {
                admit_control(instruction, counts, control)?;
                operations += 1;
            }
            ControlledPauliTargetPair::ClassicalNoop { first, second } => {
                for control in [first, second] {
                    if let ClassicalControl::Sweep(id) = control {
                        add_sweep_bit(counts, id)?;
                    }
                }
            }
            ControlledPauliTargetPair::Unsupported => {
                return Err(unsupported_frame_instruction(instruction));
            }
        }
    }
    Ok(operations)
}

fn admit_control(
    instruction: &CircuitInstruction,
    counts: &mut CountState,
    control: ClassicalControl,
) -> DetectionResult<()> {
    match control {
        ClassicalControl::Record(offset) => {
            let required = u64::from(offset.get().unsigned_abs());
            if required == 0 {
                return Err(DetectionError::invalid_sampler_compilation(format!(
                    "measurement record target rec[{}] is not a valid lookback while compiling {} feedback",
                    offset.stim_text(),
                    instruction.gate().canonical_name()
                )));
            }
            if required > counts.measurements {
                return Err(DetectionError::invalid_sampler_compilation(format!(
                    "measurement record target rec[{}] is not available while compiling {} feedback",
                    offset.get(),
                    instruction.gate().canonical_name()
                )));
            }
            Ok(())
        }
        ClassicalControl::Sweep(id) => add_sweep_bit(counts, id),
    }
}

fn add_sweep_bit(counts: &mut CountState, id: u32) -> DetectionResult<()> {
    let width = u64::from(id)
        .checked_add(1)
        .ok_or_else(|| DetectionError::invalid_sampler_compilation("sweep bit count overflowed"))?;
    counts.sweep_bits = counts.sweep_bits.max(width);
    Ok(())
}

fn instruction_qubit_count(instruction: &CircuitInstruction) -> DetectionResult<usize> {
    if instruction.gate().targets_are_pad_values() {
        return Ok(0);
    }
    instruction
        .targets()
        .iter()
        .filter_map(Target::qubit_id)
        .try_fold(0_usize, |count, qubit| {
            let next = usize::try_from(qubit.get())
                .ok()
                .and_then(|index| index.checked_add(1))
                .ok_or_else(storage_overflow)?;
            Ok(count.max(next))
        })
}

fn any_inverted(instruction: &CircuitInstruction) -> bool {
    instruction
        .targets()
        .iter()
        .any(Target::is_inverted_result_target)
}

/// The arena elements `build` stores for `kind`, computed without allocating.
fn arena_demand(instruction: &CircuitInstruction, kind: EntryKind) -> DetectionResult<ArenaCounts> {
    let targets = instruction.targets().len();
    let qubits = ArenaCounts {
        qubits: targets,
        ..ArenaCounts::default()
    };
    let probabilities = |count: usize| ArenaCounts {
        probabilities: count,
        ..qubits
    };
    let none = ArenaCounts::default();
    Ok(match kind {
        EntryKind::Clifford(_) | EntryKind::Reset(_) | EntryKind::HeraldedErase => qubits,
        EntryKind::SinglePauliNoise(_) => probabilities(3),
        EntryKind::TwoQubitPauliNoise(_) => probabilities(15),
        EntryKind::HeraldedPauliChannel => probabilities(4),
        EntryKind::Measure { .. } | EntryKind::MeasurePairs(_) => ArenaCounts {
            flags: if any_inverted(instruction) {
                targets
            } else {
                0
            },
            ..qubits
        },
        EntryKind::MeasurePads => ArenaCounts {
            flags: targets,
            ..none
        },
        EntryKind::PauliProducts { terms, products } => ArenaCounts {
            terms,
            products,
            ..none
        },
        EntryKind::CorrelatedError { .. } | EntryKind::Observable => ArenaCounts {
            terms: instruction
                .targets()
                .iter()
                .filter(|target| target.is_pauli_target())
                .count(),
            ..none
        },
        EntryKind::ControlledPauli { .. } => ArenaCounts {
            pairs: targets / 2,
            ..none
        },
    })
}

/// The frame qubit of a qubit target.
fn frame_qubit(instruction: &CircuitInstruction, target: &Target) -> DetectionResult<u32> {
    target
        .qubit_id()
        .map(|qubit| qubit.get())
        .ok_or_else(|| unsupported_frame_instruction(instruction))
}

fn qubit_targets(
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
        arena.push((qubit_index(instruction, target)?, pauli_basis(pauli)))?;
    }
    arena.span_since(start)
}

fn build(
    instruction: &CircuitInstruction,
    kind: EntryKind,
    fusion: NoiselessFusion,
    cliffords: &mut ReferenceCliffordTable,
    arena: &mut ProgramArena,
) -> DetectionResult<ProgramInstruction> {
    Ok(match kind {
        EntryKind::Clifford(frame) => ProgramInstruction::Clifford {
            frame,
            reference: cliffords.id(instruction.gate())?,
            targets: qubit_targets(instruction, arena)?,
        },
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
    for target_group in instruction.target_groups() {
        let mut raw_terms = Vec::new();
        for target in target_group {
            if target.is_combiner() {
                continue;
            }
            let Some(pauli) = target.pauli_type() else {
                return Err(unsupported_frame_instruction(instruction));
            };
            raw_terms.push((
                qubit_index(instruction, target)?,
                pauli_basis(pauli),
                target.is_inverted_result_target(),
            ));
        }
        let (normalized, product_inverted) =
            crate::sampling::pauli_product::normalize_terms(raw_terms, false)?;
        arena.push(PauliProduct::new(normalized.len(), product_inverted)?)?;
        arena.extend(normalized)?;
    }
    Ok(PauliProductMeasurement {
        terms: arena.span_since(terms)?,
        products: arena.span_since(products)?,
        flip_probability: measurement_flip_probability(instruction)?,
    })
}

/// Resolves each distinct gate's reference-sample action once per program.
#[derive(Default)]
struct ReferenceCliffordTable {
    gates: Vec<Gate>,
    table: Vec<ReferenceClifford>,
}

impl ReferenceCliffordTable {
    fn id(&mut self, gate: Gate) -> DetectionResult<ReferenceGateId> {
        let index = match self.gates.iter().position(|cached| *cached == gate) {
            Some(index) => index,
            None => {
                let clifford = reference_clifford(gate)?;
                self.gates.push(gate);
                self.table.push(clifford);
                self.table.len() - 1
            }
        };
        u16::try_from(index)
            .map(ReferenceGateId)
            .map_err(|_| program_shape_error())
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
