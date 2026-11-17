//! The engine's single compiled circuit representation.
//!
//! A [`CircuitProgram`] is one flat tape of resolved instructions with folded repeat markers.
//! Lowering resolves gate identity, targets, probabilities, record lookbacks, sweep bits, Pauli
//! products, and reference-sample Clifford actions once at compile time, so no executor matches
//! gate names or re-parses arguments. The bit-plane Pauli frame, the inverse-tableau reference
//! simulator, deterministic-measurement counting, and the test-only generator-list frame walk this
//! tape through one [`ProgramCursor`].

use arrayvec::ArrayVec;
use stab_algebra::PauliBasis;
use stab_model::{
    Cancellation, Circuit, CircuitInstruction, CircuitItem, MeasureRecordOffset, RepeatNestingLimit,
};

use super::clifford::{SingleQubitFrameMap, TwoQubitFrameMap};
use crate::detection::error::{
    DetectionError, DetectionResourceLimitError as ResourceLimitError, DetectionResult,
};
use crate::sampling::LocalTableauTransform;
use crate::work_poll::{STEP_UNITS, WorkPoll};

mod lower;
#[cfg(test)]
mod tests;

pub(crate) use lower::ProgramOptions;

/// One entry of the flat program tape.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ProgramEntry {
    Instruction(ProgramInstruction),
    Repeat { count: u64, body_end: usize },
    EndRepeat,
}

/// The Pauli-frame action of a resolved unitary gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FrameGate {
    /// The gate cannot change a Pauli frame, as for Pauli gates and identities, but it still
    /// changes the reference sample's signs.
    Identity,
    Single(SingleQubitFrameMap),
    Two(TwoQubitFrameMap),
}

/// Index of a gate's reference-sample action in [`CircuitProgram::reference_clifford`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReferenceGateId(u16);

/// The stabilizer-state action of one unitary gate, resolved once per distinct gate.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ReferenceClifford {
    Hadamard,
    ControlledX,
    /// Any other unitary gate, with its forward tableau for the general frame and, when the
    /// gate has one, its exact `H`/`S`/`CX` decomposition for the inverse-tableau simulator.
    Tableau {
        transform: LocalTableauTransform,
        steps: Option<Vec<CliffordStep>>,
    },
}

/// One step of a gate's `H`/`S`/`CX` decomposition, on indices into the gate's target group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CliffordStep {
    H(u8),
    S(u8),
    Cx(u8, u8),
}

impl ReferenceClifford {
    pub(crate) fn arity(&self) -> usize {
        match self {
            Self::Hadamard => 1,
            Self::ControlledX => 2,
            Self::Tableau { transform, .. } => transform.target_count(),
        }
    }
}

/// One target pair of a controlled Pauli whose targets include a classical control.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ControlledPair {
    Quantum {
        control: usize,
        target: usize,
    },
    Record {
        offset: MeasureRecordOffset,
        qubit: usize,
    },
    Sweep {
        id: u32,
        qubit: usize,
    },
}

/// Whether Stim's noiseless reference circuit fuses an instruction into the one before it.
///
/// Stim computes its reference sample on `Circuit::aliased_noiseless_circuit`, which drops
/// noise channels and result-flip arguments and then fuses adjacent instructions of the same
/// gate and tag. The collapse order of an X- or Y-basis measurement or reset spans the whole
/// fused instruction, so the reference simulator needs to know which compiled instructions
/// Stim sees as one. Only X- and Y-basis `Reset`, `Measure`, and `MeasurePairs` instructions
/// are ever marked [`NoiselessFusion::WithPrevious`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum NoiselessFusion {
    /// The instruction starts a new instruction of the noiseless circuit.
    #[default]
    Starts,
    /// The instruction extends the previous kept instruction, with only noise between them.
    WithPrevious,
}

/// A resolved circuit instruction.
///
/// Targets are qubit indices. `inverted` is empty when no target of the instruction carries a
/// result inversion and otherwise holds one flag per target.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ProgramInstruction {
    Clifford {
        frame: FrameGate,
        reference: ReferenceGateId,
        targets: Vec<usize>,
    },
    Reset {
        basis: PauliBasis,
        targets: Vec<usize>,
        fusion: NoiselessFusion,
    },
    Measure {
        basis: PauliBasis,
        reset: bool,
        flip_probability: f64,
        targets: Vec<usize>,
        inverted: Vec<bool>,
        fusion: NoiselessFusion,
    },
    MeasurePairs {
        basis: PauliBasis,
        flip_probability: f64,
        targets: Vec<usize>,
        inverted: Vec<bool>,
        fusion: NoiselessFusion,
    },
    MeasurePads {
        flip_probability: f64,
        values: Vec<bool>,
    },
    PauliProducts(PauliProductMeasurement),
    SinglePauliNoise {
        probabilities: [f64; 3],
        targets: Vec<usize>,
    },
    TwoQubitPauliNoise {
        probabilities: Box<[f64; 15]>,
        targets: Vec<usize>,
    },
    CorrelatedError {
        else_branch: bool,
        probability: f64,
        terms: Vec<(usize, PauliBasis)>,
    },
    HeraldedErase {
        probability: f64,
        targets: Vec<usize>,
    },
    HeraldedPauliChannel {
        probabilities: [f64; 4],
        targets: Vec<usize>,
    },
    ControlledPauli {
        basis: PauliBasis,
        frame: TwoQubitFrameMap,
        reference: ReferenceGateId,
        pairs: Vec<ControlledPair>,
    },
    Observable {
        id: usize,
        terms: Vec<(usize, PauliBasis)>,
    },
}

/// The normalized Pauli products of one `MPP` instruction, stored in one term arena.
///
/// Product `i` occupies `terms[product_ends[i - 1]..product_ends[i]]`, so execution walks
/// precomputed slices and allocates nothing per batch. `inverted[i]` is the static sign of
/// product `i`, which only the reference sample observes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PauliProductMeasurement {
    terms: Vec<(usize, PauliBasis)>,
    product_ends: Vec<usize>,
    inverted: Vec<bool>,
    flip_probability: f64,
}

impl PauliProductMeasurement {
    pub(crate) const fn flip_probability(&self) -> f64 {
        self.flip_probability
    }

    pub(crate) fn product_count(&self) -> usize {
        self.product_ends.len()
    }

    /// Yields each product's terms and static inversion.
    pub(crate) fn products(
        &self,
    ) -> impl Iterator<Item = DetectionResult<(&[(usize, PauliBasis)], bool)>> + '_ {
        let mut start = 0_usize;
        self.product_ends
            .iter()
            .zip(&self.inverted)
            .map(move |(&end, &inverted)| {
                let product = self.terms.get(start..end).ok_or_else(|| {
                    DetectionError::invalid_sampler_compilation(
                        "compiled Pauli product escaped its term arena",
                    )
                })?;
                start = end;
                Ok((product, inverted))
            })
    }

    fn retained_bytes(&self) -> Option<usize> {
        self.terms
            .capacity()
            .checked_mul(size_of::<(usize, PauliBasis)>())?
            .checked_add(
                self.product_ends
                    .capacity()
                    .checked_mul(size_of::<usize>())?,
            )?
            .checked_add(self.inverted.capacity())
    }
}

impl ProgramInstruction {
    fn retained_bytes(&self) -> Option<usize> {
        let words = |targets: &Vec<usize>| targets.capacity().checked_mul(size_of::<usize>());
        let terms = |terms: &Vec<(usize, PauliBasis)>| {
            terms
                .capacity()
                .checked_mul(size_of::<(usize, PauliBasis)>())
        };
        match self {
            Self::Clifford { targets, .. }
            | Self::Reset { targets, .. }
            | Self::SinglePauliNoise { targets, .. }
            | Self::HeraldedErase { targets, .. }
            | Self::HeraldedPauliChannel { targets, .. } => words(targets),
            Self::Measure {
                targets, inverted, ..
            }
            | Self::MeasurePairs {
                targets, inverted, ..
            } => words(targets)?.checked_add(inverted.capacity()),
            Self::MeasurePads { values, .. } => Some(values.capacity()),
            Self::PauliProducts(measurement) => measurement.retained_bytes(),
            Self::TwoQubitPauliNoise { targets, .. } => {
                words(targets)?.checked_add(size_of::<[f64; 15]>())
            }
            Self::CorrelatedError { terms: list, .. } | Self::Observable { terms: list, .. } => {
                terms(list)
            }
            Self::ControlledPauli { pairs, .. } => {
                pairs.capacity().checked_mul(size_of::<ControlledPair>())
            }
        }
    }
}

/// Retention a frame needs to run a program over bounded windows of measurement planes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MeasurementWindowNeeds {
    /// Most planes back from the newest that a classical control reads.
    pub(crate) feedback_lookback: usize,
    /// Most measurement planes one instruction records.
    pub(crate) largest_instruction: usize,
}

/// Counts that admission derives from the resolved program.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProgramCounts {
    pub(crate) measurements: usize,
    pub(crate) sweep_bits: usize,
    /// One past the largest qubit target of any instruction whose targets are not pad values,
    /// including instructions that lower to nothing, which is the qubit count Stim simulates.
    pub(crate) simulated_qubits: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CircuitProgram {
    entries: Vec<ProgramEntry>,
    cliffords: Vec<ReferenceClifford>,
    retained_bytes: u64,
    frame_qubit_count: usize,
    counts: ProgramCounts,
}

impl CircuitProgram {
    /// A program that executes nothing, for plans whose shots observe no measurement.
    pub(crate) fn empty(counts: ProgramCounts) -> Self {
        Self {
            entries: Vec::new(),
            cliffords: Vec::new(),
            retained_bytes: 0,
            frame_qubit_count: 0,
            counts,
        }
    }

    #[cfg(test)]
    pub(crate) fn compile(circuit: &Circuit, options: ProgramOptions) -> DetectionResult<Self> {
        let admission = Self::admit(circuit, options, 0, u64::MAX)?;
        Self::materialize(circuit, &admission)
    }

    pub(crate) const fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }

    /// Number of qubits a Pauli frame for this program holds.
    pub(crate) const fn frame_qubit_count(&self) -> usize {
        self.frame_qubit_count
    }

    pub(crate) const fn counts(&self) -> ProgramCounts {
        self.counts
    }

    pub(crate) fn entries(&self) -> &[ProgramEntry] {
        &self.entries
    }

    pub(crate) fn reference_clifford(
        &self,
        id: ReferenceGateId,
    ) -> DetectionResult<&ReferenceClifford> {
        self.cliffords.get(usize::from(id.0)).ok_or_else(|| {
            DetectionError::invalid_sampler_compilation(
                "compiled gate referenced a missing reference action",
            )
        })
    }

    /// A cursor over the whole tape that polls `cancellation` while it expands repeats.
    pub(crate) fn cursor<'a>(&'a self, cancellation: &'a Cancellation) -> ProgramCursor<'a> {
        ProgramCursor::new(&self.entries, 0, self.entries.len(), cancellation)
    }

    /// What a frame must retain to run this program over bounded windows of measurement planes:
    /// the deepest classical-control record lookback and the most measurements one instruction
    /// records, since a window is handed over only between instructions.
    pub(crate) fn measurement_window_needs(&self) -> MeasurementWindowNeeds {
        let mut needs = MeasurementWindowNeeds::default();
        for entry in &self.entries {
            let ProgramEntry::Instruction(instruction) = entry else {
                continue;
            };
            let recorded = match instruction {
                ProgramInstruction::Measure { targets, .. }
                | ProgramInstruction::HeraldedErase { targets, .. }
                | ProgramInstruction::HeraldedPauliChannel { targets, .. } => targets.len(),
                ProgramInstruction::MeasurePairs { targets, .. } => targets.len() / 2,
                ProgramInstruction::MeasurePads { values, .. } => values.len(),
                ProgramInstruction::PauliProducts(measurement) => measurement.product_count(),
                ProgramInstruction::ControlledPauli { pairs, .. } => {
                    for pair in pairs {
                        if let ControlledPair::Record { offset, .. } = pair {
                            needs.feedback_lookback =
                                needs.feedback_lookback.max(lookback_distance(*offset));
                        }
                    }
                    0
                }
                ProgramInstruction::Clifford { .. }
                | ProgramInstruction::Reset { .. }
                | ProgramInstruction::SinglePauliNoise { .. }
                | ProgramInstruction::TwoQubitPauliNoise { .. }
                | ProgramInstruction::CorrelatedError { .. }
                | ProgramInstruction::Observable { .. } => 0,
            };
            needs.largest_instruction = needs.largest_instruction.max(recorded);
        }
        needs
    }

    #[cfg(test)]
    pub(crate) fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

/// Executes the flat tape, expanding every repeat iteration.
///
/// The cursor charges its [`WorkPoll`] for every returned instruction and repeat iteration, and
/// the executor charges the same poll for the targets of each instruction, so a caller walking
/// an astronomically repeated body or one very wide instruction observes cancellation within a
/// bounded amount of work and receives [`DetectionError::Cancelled`].
pub(crate) struct ProgramCursor<'a> {
    entries: &'a [ProgramEntry],
    index: usize,
    end: usize,
    repeats: ArrayVec<RepeatExecutionFrame, { RepeatNestingLimit::HARD_MAX }>,
    poll: WorkPoll<'a>,
}

#[derive(Clone, Copy)]
struct RepeatExecutionFrame {
    body_start: usize,
    body_end: usize,
    remaining: u64,
}

impl<'a> ProgramCursor<'a> {
    fn new(
        entries: &'a [ProgramEntry],
        start: usize,
        end: usize,
        cancellation: &'a Cancellation,
    ) -> Self {
        Self {
            entries,
            index: start,
            end: end.min(entries.len()),
            repeats: ArrayVec::new(),
            poll: WorkPoll::new(cancellation),
        }
    }

    /// The poll the executor charges for the work of each returned instruction.
    pub(crate) const fn poll(&mut self) -> &mut WorkPoll<'a> {
        &mut self.poll
    }

    #[inline]
    pub(crate) fn next_instruction(&mut self) -> DetectionResult<Option<&'a ProgramInstruction>> {
        loop {
            if self.index >= self.end {
                if self.repeats.is_empty() {
                    return Ok(None);
                }
                return Err(program_shape_error());
            }
            let entry = self
                .entries
                .get(self.index)
                .ok_or_else(program_shape_error)?;
            match entry {
                ProgramEntry::Instruction(instruction) => {
                    self.poll.charge(STEP_UNITS)?;
                    self.index += 1;
                    return Ok(Some(instruction));
                }
                ProgramEntry::Repeat { count, body_end } => {
                    if *count == 0
                        || *body_end <= self.index
                        || *body_end >= self.end
                        || !matches!(self.entries.get(*body_end), Some(ProgramEntry::EndRepeat))
                    {
                        return Err(program_shape_error());
                    }
                    let body_start = self.index + 1;
                    self.repeats
                        .try_push(RepeatExecutionFrame {
                            body_start,
                            body_end: *body_end,
                            remaining: *count,
                        })
                        .map_err(|_| program_shape_error())?;
                    self.index = body_start;
                }
                ProgramEntry::EndRepeat => {
                    let frame = self.repeats.last_mut().ok_or_else(program_shape_error)?;
                    if frame.body_end != self.index || frame.remaining == 0 {
                        return Err(program_shape_error());
                    }
                    if frame.remaining > 1 {
                        frame.remaining -= 1;
                        self.index = frame.body_start;
                        self.poll.charge(STEP_UNITS)?;
                    } else {
                        self.repeats.pop();
                        self.index += 1;
                    }
                }
            }
        }
    }
}

/// One step of the shared folded circuit walk.
#[derive(Clone, Copy)]
pub(crate) enum TraversalEvent<'a> {
    Instruction(&'a CircuitInstruction),
    BeginRepeat(u64),
    EndRepeat,
}

struct TraversalFrame<'a> {
    items: std::slice::Iter<'a, CircuitItem>,
    end_repeat: bool,
}

/// Walks a circuit once in source order without expanding repeats.
///
/// This is the engine's one circuit walker: program lowering and detection conversion both
/// consume its events, so they agree on traversal order and nesting admission.
pub(crate) fn visit_circuit<'a>(
    circuit: &'a Circuit,
    mut visitor: impl FnMut(TraversalEvent<'a>) -> DetectionResult<()>,
) -> DetectionResult<()> {
    let mut frames = ArrayVec::<TraversalFrame<'a>, { RepeatNestingLimit::HARD_MAX + 1 }>::new();
    frames.push(TraversalFrame {
        items: circuit.items().iter(),
        end_repeat: false,
    });
    loop {
        let item = frames.last_mut().and_then(|frame| frame.items.next());
        match item {
            Some(CircuitItem::Instruction(instruction)) => {
                visitor(TraversalEvent::Instruction(instruction))?;
            }
            Some(CircuitItem::RepeatBlock(repeat)) => {
                if frames.is_full() {
                    return Err(ResourceLimitError::detection_repeat_nesting(
                        RepeatNestingLimit::HARD_MAX + 1,
                        RepeatNestingLimit::HARD_MAX,
                    )
                    .into());
                }
                visitor(TraversalEvent::BeginRepeat(repeat.repeat_count().get()))?;
                frames.push(TraversalFrame {
                    items: repeat.body().items().iter(),
                    end_repeat: true,
                });
            }
            None => {
                let Some(completed) = frames.pop() else {
                    return Err(program_shape_error());
                };
                if completed.end_repeat {
                    visitor(TraversalEvent::EndRepeat)?;
                } else {
                    return Ok(());
                }
            }
        }
    }
}

fn validate_program(entries: &[ProgramEntry]) -> DetectionResult<()> {
    let mut repeat_ends = ArrayVec::<usize, { RepeatNestingLimit::HARD_MAX }>::new();
    for (index, entry) in entries.iter().enumerate() {
        match entry {
            ProgramEntry::Instruction(_) => {}
            ProgramEntry::Repeat { count, body_end } => {
                if *count == 0
                    || *body_end <= index + 1
                    || !matches!(entries.get(*body_end), Some(ProgramEntry::EndRepeat))
                    || repeat_ends
                        .last()
                        .is_some_and(|parent_end| body_end >= parent_end)
                {
                    return Err(program_shape_error());
                }
                repeat_ends
                    .try_push(*body_end)
                    .map_err(|_| program_shape_error())?;
            }
            ProgramEntry::EndRepeat => {
                if repeat_ends.pop() != Some(index) {
                    return Err(program_shape_error());
                }
            }
        }
    }
    repeat_ends.is_empty().ok_or_else(program_shape_error)
}

fn materialized_retained_bytes(
    entries: &[ProgramEntry],
    entry_capacity: usize,
) -> DetectionResult<u64> {
    let mut retained = byte_product(entry_capacity, size_of::<ProgramEntry>())?;
    for entry in entries {
        let ProgramEntry::Instruction(instruction) = entry else {
            continue;
        };
        let payload = instruction.retained_bytes().ok_or_else(storage_overflow)?;
        retained = checked_add_bytes(
            retained,
            u64::try_from(payload).map_err(|_| storage_overflow())?,
        )?;
    }
    Ok(retained)
}

fn byte_product(count: usize, item_size: usize) -> DetectionResult<u64> {
    let bytes = count.checked_mul(item_size).ok_or_else(storage_overflow)?;
    u64::try_from(bytes).map_err(|_| storage_overflow())
}

fn checked_add_bytes(left: u64, right: u64) -> DetectionResult<u64> {
    left.checked_add(right).ok_or_else(storage_overflow)
}

fn storage_overflow() -> DetectionError {
    DetectionError::invalid_sampler_compilation(
        "compact detector-frame retained byte count overflowed",
    )
}

fn admission_drift() -> DetectionError {
    DetectionError::invalid_sampler_compilation(
        "detector-frame materialization disagreed with allocation-free admission",
    )
}

/// Planes back from the newest that `offset` reads.
pub(crate) fn lookback_distance(offset: MeasureRecordOffset) -> usize {
    usize::try_from(offset.get().unsigned_abs()).unwrap_or(usize::MAX)
}

fn program_shape_error() -> DetectionError {
    DetectionError::invalid_sampler_compilation(
        "compact detector-frame program has an invalid repeat shape",
    )
}
