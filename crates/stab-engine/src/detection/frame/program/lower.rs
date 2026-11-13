//! Allocation-free admission and materialization of a [`CircuitProgram`].

use std::ops::ControlFlow;

use arrayvec::ArrayVec;
use stab_algebra::PauliBasis;
use stab_model::{
    Circuit, CircuitInstruction, Gate, RepeatNestingLimit, Target,
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
use super::{
    CircuitProgram, CliffordStep, ControlledPair, FrameGate, PauliProductMeasurement,
    ProgramCounts, ProgramEntry, ProgramInstruction, ReferenceClifford, ReferenceGateId,
    TraversalEvent, admission_drift, byte_product, checked_add_bytes, materialized_retained_bytes,
    program_shape_error, storage_overflow, validate_program, visit_circuit,
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
    SinglePauliNoise,
    TwoQubitPauliNoise,
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
                                admission.retained_bytes = checked_add_bytes(
                                    admission.retained_bytes,
                                    checked_add_bytes(entry_bytes, payload_bytes(lowered, kind)?)?,
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
        // A repeat marker is emitted only once its body emits an entry, so bodies that lower to
        // nothing vanish without ever exceeding the admitted entry capacity.
        let mut repeat_markers = ArrayVec::<PendingRepeat, { RepeatNestingLimit::HARD_MAX }>::new();
        let mut frame_qubit_count = 0_usize;
        let mut actions = FrameCliffordActionCache::default();
        let mut cliffords = ReferenceCliffordTable::default();
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
                    visit_lowered_instructions(instruction, |lowered| {
                        let lowering = lower(lowered, options, &mut actions)?;
                        if let Lowering::Entry(kind) = lowering {
                            let built = build(lowered, kind, &mut cliffords)?;
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
                    repeat_markers
                        .try_push(PendingRepeat {
                            count,
                            marker: None,
                        })
                        .map_err(|_| program_shape_error())?;
                }
                TraversalEvent::EndRepeat => {
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
        let retained_bytes = materialized_retained_bytes(&entries, entries.capacity())?;
        Ok(Self {
            entries,
            cliffords: cliffords.table,
            retained_bytes,
            frame_qubit_count,
            counts: admission.counts,
        })
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
    if !matches!(instruction.gate().canonical_name(), "SPP" | "SPP_DAG") {
        return visitor(instruction);
    }
    let completion =
        stab_analysis::advanced::visit_decomposed_spp_instructions(instruction, |lowered| {
            if matches!(lowered.gate().canonical_name(), "SPP" | "SPP_DAG") {
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

fn lower(
    instruction: &CircuitInstruction,
    options: ProgramOptions,
    actions: &mut FrameCliffordActionCache,
) -> DetectionResult<Lowering> {
    let kind = match instruction.gate().canonical_name() {
        "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" | "DETECTOR" | "I_ERROR" | "II_ERROR" => {
            return Ok(Lowering::Noop);
        }
        "OBSERVABLE_INCLUDE" => {
            if !options.include_pauli_observables
                || !instruction.targets().iter().any(Target::is_pauli_target)
            {
                return Ok(Lowering::Noop);
            }
            EntryKind::Observable
        }
        "R" => EntryKind::Reset(PauliBasis::Z),
        "RX" => EntryKind::Reset(PauliBasis::X),
        "RY" => EntryKind::Reset(PauliBasis::Y),
        "M" => measure(PauliBasis::Z, false),
        "MX" => measure(PauliBasis::X, false),
        "MY" => measure(PauliBasis::Y, false),
        "MR" => measure(PauliBasis::Z, true),
        "MRX" => measure(PauliBasis::X, true),
        "MRY" => measure(PauliBasis::Y, true),
        "MXX" => EntryKind::MeasurePairs(PauliBasis::X),
        "MYY" => EntryKind::MeasurePairs(PauliBasis::Y),
        "MZZ" => EntryKind::MeasurePairs(PauliBasis::Z),
        "MPP" => pauli_product_shape(instruction)?,
        "MPAD" => EntryKind::MeasurePads,
        name @ ("X_ERROR" | "Y_ERROR" | "Z_ERROR" | "DEPOLARIZE1" | "PAULI_CHANNEL_1"
        | "DEPOLARIZE2" | "PAULI_CHANNEL_2") => {
            let pairs = matches!(name, "DEPOLARIZE2" | "PAULI_CHANNEL_2");
            if zero_probability_noise(instruction)? {
                return Ok(Lowering::Noop);
            } else if pairs {
                EntryKind::TwoQubitPauliNoise
            } else {
                EntryKind::SinglePauliNoise
            }
        }
        "E" => EntryKind::CorrelatedError { else_branch: false },
        "ELSE_CORRELATED_ERROR" => EntryKind::CorrelatedError { else_branch: true },
        "HERALDED_ERASE" => EntryKind::HeraldedErase,
        "HERALDED_PAULI_CHANNEL_1" => EntryKind::HeraldedPauliChannel,
        name => {
            let Some(action) = actions.action(instruction.gate())? else {
                if zero_probability_noise(instruction)? {
                    return Ok(Lowering::FrameIdentity);
                }
                return Err(DetectionError::invalid_sampler_compilation(format!(
                    "detector frame execution does not support {name}"
                )));
            };
            let controlled = controlled_pauli_basis(name);
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
                    _ => Err(unsupported_frame_target(name, target)),
                };
            }
            EntryKind::Clifford(match action {
                FrameCliffordAction::Identity => FrameGate::Identity,
                FrameCliffordAction::Single(map) => FrameGate::Single(map),
                FrameCliffordAction::Two(map) => FrameGate::Two(map),
            })
        }
    };
    Ok(Lowering::Entry(kind))
}

const fn measure(basis: PauliBasis, reset: bool) -> EntryKind {
    EntryKind::Measure { basis, reset }
}

fn controlled_pauli_basis(name: &str) -> Option<PauliBasis> {
    match name {
        "CX" | "XCZ" => Some(PauliBasis::X),
        "CY" | "YCZ" => Some(PauliBasis::Y),
        "CZ" => Some(PauliBasis::Z),
        _ => None,
    }
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
            | EntryKind::SinglePauliNoise
            | EntryKind::TwoQubitPauliNoise
            | EntryKind::CorrelatedError { .. }
            | EntryKind::Observable => 0,
        },
    };
    if let Lowering::Entry(EntryKind::Clifford(_)) = lowering
        && controlled_pauli_basis(instruction.gate().canonical_name()).is_some()
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

/// The heap bytes `build` retains for `kind`, computed without allocating.
fn payload_bytes(instruction: &CircuitInstruction, kind: EntryKind) -> DetectionResult<u64> {
    let targets = instruction.targets().len();
    let words = byte_product(targets, size_of::<usize>())?;
    let flags = if any_inverted(instruction) {
        byte_product(targets, 1)?
    } else {
        0
    };
    let term_bytes = |count: usize| byte_product(count, size_of::<(usize, PauliBasis)>());
    match kind {
        EntryKind::Clifford(_)
        | EntryKind::Reset(_)
        | EntryKind::SinglePauliNoise
        | EntryKind::HeraldedErase
        | EntryKind::HeraldedPauliChannel => Ok(words),
        EntryKind::Measure { .. } | EntryKind::MeasurePairs(_) => checked_add_bytes(words, flags),
        EntryKind::MeasurePads => byte_product(targets, 1),
        EntryKind::PauliProducts { terms, products } => checked_add_bytes(
            checked_add_bytes(
                term_bytes(terms)?,
                byte_product(products, size_of::<usize>())?,
            )?,
            byte_product(products, 1)?,
        ),
        EntryKind::TwoQubitPauliNoise => {
            checked_add_bytes(words, byte_product(15, size_of::<f64>())?)
        }
        EntryKind::CorrelatedError { .. } | EntryKind::Observable => term_bytes(
            instruction
                .targets()
                .iter()
                .filter(|target| target.is_pauli_target())
                .count(),
        ),
        EntryKind::ControlledPauli { .. } => byte_product(targets / 2, size_of::<ControlledPair>()),
    }
}

fn try_vec<T>(capacity: usize, context: &str) -> DetectionResult<Vec<T>> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|error| {
        DetectionError::invalid_sampler_compilation(format!(
            "unable to reserve {capacity} {context}: {error}"
        ))
    })?;
    Ok(values)
}

fn qubit_targets(instruction: &CircuitInstruction) -> DetectionResult<Vec<usize>> {
    let mut targets = try_vec(
        instruction.targets().len(),
        "resolved detector-frame targets",
    )?;
    for target in instruction.targets() {
        targets.push(qubit_index(instruction, target)?);
    }
    Ok(targets)
}

fn inversion_flags(instruction: &CircuitInstruction) -> DetectionResult<Vec<bool>> {
    if !any_inverted(instruction) {
        return Ok(Vec::new());
    }
    let mut flags = try_vec(instruction.targets().len(), "result inversion flags")?;
    flags.extend(
        instruction
            .targets()
            .iter()
            .map(Target::is_inverted_result_target),
    );
    Ok(flags)
}

fn pauli_terms(instruction: &CircuitInstruction) -> DetectionResult<Vec<(usize, PauliBasis)>> {
    let count = instruction
        .targets()
        .iter()
        .filter(|target| target.is_pauli_target())
        .count();
    let mut terms = try_vec(count, "resolved Pauli terms")?;
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
        if terms.len() == terms.capacity() {
            return Err(admission_drift());
        }
        terms.push((qubit_index(instruction, target)?, pauli_basis(pauli)));
    }
    Ok(terms)
}

fn build(
    instruction: &CircuitInstruction,
    kind: EntryKind,
    cliffords: &mut ReferenceCliffordTable,
) -> DetectionResult<ProgramInstruction> {
    Ok(match kind {
        EntryKind::Clifford(frame) => ProgramInstruction::Clifford {
            frame,
            reference: cliffords.id(instruction.gate())?,
            targets: qubit_targets(instruction)?,
        },
        EntryKind::Reset(basis) => ProgramInstruction::Reset {
            basis,
            targets: qubit_targets(instruction)?,
        },
        EntryKind::Measure { basis, reset } => ProgramInstruction::Measure {
            basis,
            reset,
            flip_probability: measurement_flip_probability(instruction)?,
            targets: qubit_targets(instruction)?,
            inverted: inversion_flags(instruction)?,
        },
        EntryKind::MeasurePairs(basis) => ProgramInstruction::MeasurePairs {
            basis,
            flip_probability: measurement_flip_probability(instruction)?,
            targets: qubit_targets(instruction)?,
            inverted: inversion_flags(instruction)?,
        },
        EntryKind::MeasurePads => {
            let mut values = try_vec(instruction.targets().len(), "measurement pad values")?;
            for target in instruction.targets() {
                let qubit = target
                    .qubit_id()
                    .ok_or_else(|| unsupported_frame_instruction(instruction))?;
                values.push(qubit.get() == 1);
            }
            ProgramInstruction::MeasurePads {
                flip_probability: measurement_flip_probability(instruction)?,
                values,
            }
        }
        EntryKind::PauliProducts { terms, products } => {
            ProgramInstruction::PauliProducts(build_pauli_products(instruction, terms, products)?)
        }
        EntryKind::SinglePauliNoise => {
            let probabilities = match instruction.gate().canonical_name() {
                "X_ERROR" => [single_probability_argument(instruction)?.get(), 0.0, 0.0],
                "Y_ERROR" => [0.0, single_probability_argument(instruction)?.get(), 0.0],
                "Z_ERROR" => [0.0, 0.0, single_probability_argument(instruction)?.get()],
                "DEPOLARIZE1" => [single_probability_argument(instruction)?.get() / 3.0; 3],
                "PAULI_CHANNEL_1" => probability_list::<3>(instruction)?,
                _ => return Err(program_shape_error()),
            };
            ProgramInstruction::SinglePauliNoise {
                probabilities,
                targets: qubit_targets(instruction)?,
            }
        }
        EntryKind::TwoQubitPauliNoise => {
            let probabilities = match instruction.gate().canonical_name() {
                "DEPOLARIZE2" => [single_probability_argument(instruction)?.get() / 15.0; 15],
                "PAULI_CHANNEL_2" => probability_list::<15>(instruction)?,
                _ => return Err(program_shape_error()),
            };
            ProgramInstruction::TwoQubitPauliNoise {
                probabilities: Box::new(probabilities),
                targets: qubit_targets(instruction)?,
            }
        }
        EntryKind::CorrelatedError { else_branch } => ProgramInstruction::CorrelatedError {
            else_branch,
            probability: single_probability_argument(instruction)?.get(),
            terms: pauli_terms(instruction)?,
        },
        EntryKind::HeraldedErase => ProgramInstruction::HeraldedErase {
            probability: single_probability_argument(instruction)?.get(),
            targets: qubit_targets(instruction)?,
        },
        EntryKind::HeraldedPauliChannel => ProgramInstruction::HeraldedPauliChannel {
            probabilities: probability_list::<4>(instruction)?,
            targets: qubit_targets(instruction)?,
        },
        EntryKind::ControlledPauli { basis, map } => {
            let mut pairs = try_vec(instruction.targets().len() / 2, "controlled Pauli pairs")?;
            for target_group in instruction.targets().chunks(2) {
                let pair =
                    match classify_controlled_pauli_target_pair(instruction.gate(), target_group) {
                        ControlledPauliTargetPair::Quantum { .. } => {
                            let [control, target] = target_group else {
                                return Err(unsupported_frame_instruction(instruction));
                            };
                            ControlledPair::Quantum {
                                control: qubit_index(instruction, control)?,
                                target: qubit_index(instruction, target)?,
                            }
                        }
                        ControlledPauliTargetPair::Classical { control, target } => {
                            let qubit =
                                usize::try_from(target.get()).map_err(|_| storage_overflow())?;
                            match control {
                                ClassicalControl::Record(offset) => {
                                    ControlledPair::Record { offset, qubit }
                                }
                                ClassicalControl::Sweep(id) => ControlledPair::Sweep { id, qubit },
                            }
                        }
                        ControlledPauliTargetPair::ClassicalNoop { .. } => continue,
                        ControlledPauliTargetPair::Unsupported => {
                            return Err(unsupported_frame_instruction(instruction));
                        }
                    };
                pairs.push(pair);
            }
            ProgramInstruction::ControlledPauli {
                basis,
                frame: map,
                reference: cliffords.id(instruction.gate())?,
                pairs,
            }
        }
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
                terms: pauli_terms(instruction)?,
            }
        }
    })
}

fn build_pauli_products(
    instruction: &CircuitInstruction,
    term_count: usize,
    product_count: usize,
) -> DetectionResult<PauliProductMeasurement> {
    let mut terms = try_vec(term_count, "detector-frame Pauli product terms")?;
    let mut product_ends = try_vec(product_count, "detector-frame Pauli products")?;
    let mut inverted = try_vec(product_count, "Pauli product inversion flags")?;
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
        if normalized.len() > terms.capacity() - terms.len()
            || product_ends.len() == product_ends.capacity()
        {
            return Err(admission_drift());
        }
        terms.extend_from_slice(&normalized);
        product_ends.push(terms.len());
        inverted.push(product_inverted);
    }
    if product_ends.len() != product_count {
        return Err(admission_drift());
    }
    Ok(PauliProductMeasurement {
        terms,
        product_ends,
        inverted,
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
    Ok(match gate.canonical_name() {
        "H" => ReferenceClifford::Hadamard,
        "CX" => ReferenceClifford::ControlledX,
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
