//! Allocation-free admission and materialization of a [`CircuitProgram`].

use std::ops::ControlFlow;

use arrayvec::ArrayVec;
use stab_algebra::PauliBasis;
use stab_model::{
    Circuit, CircuitInstruction, Gate, GateKind, RepeatNestingLimit, Target,
    advanced::{ClassicalControl, ControlledPauliTargetPair},
};

use super::super::clifford::{FrameCliffordAction, FrameCliffordActionCache, TwoQubitFrameMap};
use super::super::helpers::{
    pauli_basis, unsupported_frame_instruction, unsupported_frame_target, zero_probability_noise,
};
use super::arena::{ArenaCounts, ProgramArena};
use super::{
    CircuitProgram, FrameGate, NoiselessFusion, ProgramCounts, ProgramEntry, ProgramInstruction,
    TraversalEvent, admission_drift, byte_product, checked_add_bytes, materialized_retained_bytes,
    program_shape_error, storage_overflow, validate_program, visit_circuit,
};
use crate::detection::error::{
    DetectionError, DetectionResourceLimitError as ResourceLimitError, DetectionResult,
};

mod build;
#[cfg(test)]
mod tests;

use build::{ReferenceCliffordTable, build, build_clifford};

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
    /// Counts one entry and its arena payload.
    #[inline(always)]
    fn admit_entry(&mut self, entry_bytes: u64, payload: ArenaCounts) -> DetectionResult<()> {
        self.entry_count = self
            .entry_count
            .checked_add(1)
            .ok_or_else(storage_overflow)?;
        self.arena.add(payload)?;
        self.retained_bytes = checked_add_bytes(
            self.retained_bytes,
            checked_add_bytes(entry_bytes, payload.bytes()?)?,
        )?;
        Ok(())
    }

    /// Counts one entry whose only payload is `targets` qubit targets, as [`Self::admit_entry`]
    /// does for that payload.
    #[inline(always)]
    fn admit_qubit_entry(&mut self, entry_bytes: u64, targets: usize) -> DetectionResult<()> {
        self.entry_count = self
            .entry_count
            .checked_add(1)
            .ok_or_else(storage_overflow)?;
        self.arena.add_qubits(targets)?;
        let payload_bytes = byte_product(targets, size_of::<u32>())?;
        self.retained_bytes = checked_add_bytes(
            self.retained_bytes,
            checked_add_bytes(entry_bytes, payload_bytes)?,
        )?;
        Ok(())
    }

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
        let mut admitter = Admitter {
            admission: ProgramAdmission {
                options,
                ..ProgramAdmission::default()
            },
            counts: CountState::default(),
            simulated_qubits: 0,
            actions: FrameCliffordActionCache::default(),
            entry_bytes: byte_product(1, size_of::<ProgramEntry>())?,
            retained_base_bytes,
            max_combined_bytes,
        };
        let mut repeats = ArrayVec::<RepeatAdmission, { RepeatNestingLimit::HARD_MAX }>::new();
        visit_circuit(circuit, |event| match event {
            TraversalEvent::Instruction(instruction) => admitter.instruction(instruction),
            TraversalEvent::BeginRepeat(count) => {
                repeats
                    .try_push(RepeatAdmission {
                        count,
                        entries_before: admitter.admission.entry_count,
                        counts_before: admitter.counts,
                    })
                    .map_err(|_| program_shape_error())?;
                admitter.add_marker()
            }
            TraversalEvent::EndRepeat => {
                let repeat = repeats.pop().ok_or_else(program_shape_error)?;
                admitter.end_repeat(repeat)
            }
        })?;
        let Admitter {
            mut admission,
            counts,
            simulated_qubits,
            ..
        } = admitter;
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
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(admission.entry_count)
            .map_err(|error| {
                DetectionError::storage_allocation(format!(
                    "unable to reserve {} compact detector-frame entries: {error}",
                    admission.entry_count
                ))
            })?;
        let mut repeat_markers = Vec::new();
        repeat_markers
            .try_reserve_exact(RepeatNestingLimit::HARD_MAX)
            .map_err(|error| {
                DetectionError::storage_allocation(format!(
                    "unable to reserve detector-frame repeat markers: {error}"
                ))
            })?;
        let mut materializer = Materializer {
            options: admission.options,
            entries,
            arena: ProgramArena::try_with_capacity(admission.arena)?,
            repeat_markers,
            frame_qubit_count: 0,
            actions: FrameCliffordActionCache::default(),
            cliffords: ReferenceCliffordTable::default(),
            noiseless_previous: None,
        };
        visit_circuit(circuit, |event| match event {
            TraversalEvent::Instruction(instruction) => materializer.instruction(instruction),
            TraversalEvent::BeginRepeat(count) => {
                // A repeat block is its own operation, so nothing fuses across its edges.
                materializer.noiseless_previous = None;
                let markers = &mut materializer.repeat_markers;
                if markers.len() >= RepeatNestingLimit::HARD_MAX {
                    return Err(program_shape_error());
                }
                markers.push(PendingRepeat {
                    count,
                    marker: None,
                });
                Ok(())
            }
            TraversalEvent::EndRepeat => materializer.end_repeat(),
        })?;
        let Materializer {
            entries,
            arena,
            repeat_markers,
            frame_qubit_count,
            cliffords,
            ..
        } = materializer;
        if !repeat_markers.is_empty()
            || entries.len() != admission.entry_count
            || frame_qubit_count != admission.frame_qubit_count
        {
            return Err(admission_drift());
        }
        let window_needs = validate_program(&entries, &arena)?;
        let retained_bytes = materialized_retained_bytes(entries.capacity(), &arena)?;
        if retained_bytes > admission.retained_bytes {
            return Err(admission_drift());
        }
        let window_needs = window_needs?;
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

/// The running state of one admission walk.
struct Admitter {
    admission: ProgramAdmission,
    counts: CountState,
    simulated_qubits: usize,
    actions: FrameCliffordActionCache,
    entry_bytes: u64,
    retained_base_bytes: u64,
    max_combined_bytes: u64,
}

impl Admitter {
    /// Refuses the program once its retained bytes exceed the budget.
    #[inline]
    fn check(&self) -> DetectionResult<()> {
        let combined = checked_add_bytes(self.retained_base_bytes, self.admission.retained_bytes)?;
        if combined > self.max_combined_bytes {
            return Err(compiled_bytes_limit(combined, self.max_combined_bytes));
        }
        Ok(())
    }

    #[inline(always)]
    fn instruction(&mut self, instruction: &CircuitInstruction) -> DetectionResult<()> {
        let Some(plain) = plain_clifford(instruction, &mut self.actions)? else {
            return self.general_instruction(instruction);
        };
        self.simulated_qubits = self.simulated_qubits.max(plain.qubits);
        self.admission
            .admit_qubit_entry(self.entry_bytes, plain.targets)?;
        self.admission.frame_qubit_count = self.admission.frame_qubit_count.max(plain.qubits);
        self.check()
    }

    /// Admits any instruction through the general lowering, kept out of line so the walk over
    /// plain Clifford instructions stays small.
    #[inline(never)]
    fn general_instruction(&mut self, instruction: &CircuitInstruction) -> DetectionResult<()> {
        // The source instruction, not its lowering, sizes Stim's simulated qubit count, so
        // instructions that lower to nothing still count.
        self.simulated_qubits = self
            .simulated_qubits
            .max(instruction_qubit_count(instruction)?);
        let options = self.admission.options;
        visit_lowered_instructions(instruction, |lowered| {
            let lowering = lower(lowered, options, &mut self.actions)?;
            record_effects(lowered, lowering, &mut self.counts)?;
            match lowering {
                Lowering::Noop => return Ok(()),
                Lowering::FrameIdentity => {}
                Lowering::Entry(kind) => {
                    self.admission
                        .admit_entry(self.entry_bytes, arena_demand(lowered, kind))?;
                }
            }
            self.admission.frame_qubit_count = self
                .admission
                .frame_qubit_count
                .max(instruction_qubit_count(lowered)?);
            self.check()
        })
    }

    /// Counts one repeat marker entry.
    fn add_marker(&mut self) -> DetectionResult<()> {
        self.admission.entry_count = self
            .admission
            .entry_count
            .checked_add(1)
            .ok_or_else(storage_overflow)?;
        self.admission.retained_bytes =
            checked_add_bytes(self.admission.retained_bytes, self.entry_bytes)?;
        self.check()
    }

    fn end_repeat(&mut self, repeat: RepeatAdmission) -> DetectionResult<()> {
        integrate_repeat(&mut self.counts, repeat)?;
        if self.admission.entry_count == repeat.entries_before + 1 {
            // A body that lowers to nothing is elided with its marker; it added no entry, so it
            // added no arena payload either.
            self.admission.entry_count = repeat.entries_before;
            self.admission.retained_bytes = self
                .admission
                .retained_bytes
                .checked_sub(self.entry_bytes)
                .ok_or_else(admission_drift)?;
            return Ok(());
        }
        self.add_marker()
    }
}

#[cold]
fn compiled_bytes_limit(combined: u64, max_combined_bytes: u64) -> DetectionError {
    DetectionError::from(ResourceLimitError::detection_compiled_bytes(
        combined,
        stab_model::MemoryBudget::from_bytes(max_combined_bytes),
    ))
}

/// The running state of one materialization walk.
struct Materializer<'a> {
    options: ProgramOptions,
    entries: Vec<ProgramEntry>,
    arena: ProgramArena,
    // A repeat marker is emitted only once its body emits an entry, so bodies that lower to
    // nothing vanish without ever exceeding the admitted entry capacity.
    // Heap storage reserved once for the whole nesting envelope: the walk runs on the caller's
    // stack, which an inline array of every level would dominate.
    repeat_markers: Vec<PendingRepeat>,
    frame_qubit_count: usize,
    actions: FrameCliffordActionCache,
    cliffords: ReferenceCliffordTable,
    noiseless_previous: Option<&'a CircuitInstruction>,
}

impl<'a> Materializer<'a> {
    #[inline(always)]
    fn instruction(&mut self, instruction: &'a CircuitInstruction) -> DetectionResult<()> {
        let Some(plain) = plain_clifford(instruction, &mut self.actions)? else {
            return self.general_instruction(instruction);
        };
        // A unitary gate never fuses, but it is the previous instruction of the one after it.
        if kept_by_noiseless_circuit(instruction.gate()) {
            self.noiseless_previous = Some(instruction);
        }
        let built = build_clifford(
            instruction,
            plain.frame,
            &mut self.cliffords,
            &mut self.arena,
        )?;
        emit(&mut self.entries, &mut self.repeat_markers, built)?;
        self.frame_qubit_count = self.frame_qubit_count.max(plain.qubits);
        Ok(())
    }

    /// Materializes any instruction through the general lowering, kept out of line so the walk
    /// over plain Clifford instructions stays small.
    #[inline(never)]
    fn general_instruction(&mut self, instruction: &'a CircuitInstruction) -> DetectionResult<()> {
        let fusion = noiseless_fusion(self.noiseless_previous, instruction);
        if kept_by_noiseless_circuit(instruction.gate()) {
            self.noiseless_previous = Some(instruction);
        }
        let options = self.options;
        visit_lowered_instructions(instruction, |lowered| {
            let lowering = lower(lowered, options, &mut self.actions)?;
            if let Lowering::Entry(kind) = lowering {
                let built = build(lowered, kind, fusion, &mut self.cliffords, &mut self.arena)?;
                emit(&mut self.entries, &mut self.repeat_markers, built)?;
            }
            if !matches!(lowering, Lowering::Noop) {
                self.frame_qubit_count = self
                    .frame_qubit_count
                    .max(instruction_qubit_count(lowered)?);
            }
            Ok(())
        })
    }

    fn end_repeat(&mut self) -> DetectionResult<()> {
        self.noiseless_previous = None;
        let pending = self.repeat_markers.pop().ok_or_else(program_shape_error)?;
        let Some(marker) = pending.marker else {
            return Ok(());
        };
        let body_end = self.entries.len();
        push(&mut self.entries, ProgramEntry::EndRepeat)?;
        let Some(ProgramEntry::Repeat {
            body_end: stored_end,
            ..
        }) = self.entries.get_mut(marker)
        else {
            return Err(program_shape_error());
        };
        *stored_end = body_end;
        Ok(())
    }
}

/// Appends an entry within the admitted capacity.
#[inline(always)]
fn push(entries: &mut Vec<ProgramEntry>, entry: ProgramEntry) -> DetectionResult<()> {
    if entries.len() == entries.capacity() {
        return Err(admission_drift());
    }
    entries.push(entry);
    Ok(())
}

/// Appends an instruction, first emitting the marker of every enclosing repeat whose body had
/// emitted nothing yet.
#[inline(always)]
fn emit(
    entries: &mut Vec<ProgramEntry>,
    repeat_markers: &mut [PendingRepeat],
    instruction: ProgramInstruction,
) -> DetectionResult<()> {
    // Markers are emitted outermost first and all at once, so the unmarked repeats are always
    // the innermost ones and a marked innermost repeat means every repeat is marked.
    if repeat_markers
        .last()
        .is_some_and(|pending| pending.marker.is_none())
    {
        emit_markers(entries, repeat_markers)?;
    }
    push(entries, ProgramEntry::Instruction(instruction))
}

#[cold]
fn emit_markers(
    entries: &mut Vec<ProgramEntry>,
    repeat_markers: &mut [PendingRepeat],
) -> DetectionResult<()> {
    for pending in repeat_markers {
        if pending.marker.is_none() {
            pending.marker = Some(entries.len());
            push(
                entries,
                ProgramEntry::Repeat {
                    count: pending.count,
                    body_end: usize::MAX,
                },
            )?;
        }
    }
    Ok(())
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

/// A unitary instruction whose targets all name qubits, the shape of almost every instruction
/// of a large circuit.
#[derive(Clone, Copy)]
struct PlainClifford {
    frame: FrameGate,
    /// One past the largest target qubit.
    qubits: usize,
    targets: usize,
}

/// Recognizes a [`PlainClifford`] in one pass over its targets, or returns `None` to send the
/// instruction through the general lowering, which derives the same entry, qubit counts, and
/// record effects for every instruction this accepts and fails the same way when the gate's
/// frame action cannot be derived.
#[inline(always)]
fn plain_clifford(
    instruction: &CircuitInstruction,
    actions: &mut FrameCliffordActionCache,
) -> DetectionResult<Option<PlainClifford>> {
    let gate = instruction.gate();
    if !gate.is_unitary() || is_pauli_product_phase(gate) {
        return Ok(None);
    }
    let targets = instruction.targets();
    let mut largest = None::<u32>;
    for target in targets {
        let Some(qubit) = target.qubit_id() else {
            return Ok(None);
        };
        largest = largest.max(Some(qubit.get()));
    }
    // Every target pair of a controlled Pauli on qubits is a quantum pair, which records nothing;
    // an unpaired target takes the general lowering and its error.
    if gate.controlled_pauli().is_some() && !targets.len().is_multiple_of(2) {
        return Ok(None);
    }
    let qubits = match largest {
        None => 0,
        Some(qubit) => match usize::try_from(qubit).ok().and_then(|q| q.checked_add(1)) {
            Some(qubits) => qubits,
            None => return Ok(None),
        },
    };
    let Some(action) = actions.action(gate)? else {
        return Ok(None);
    };
    Ok(Some(PlainClifford {
        frame: match action {
            FrameCliffordAction::Identity => FrameGate::Identity,
            FrameCliffordAction::Single(map) => FrameGate::Single(map),
            FrameCliffordAction::Two(map) => FrameGate::Two(map),
        },
        qubits,
        targets: targets.len(),
    }))
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
            ControlledPauliTargetPair::classify(instruction.gate(), target_group),
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
        match ControlledPauliTargetPair::classify(instruction.gate(), target_group) {
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
fn arena_demand(instruction: &CircuitInstruction, kind: EntryKind) -> ArenaCounts {
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
    match kind {
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
    }
}
