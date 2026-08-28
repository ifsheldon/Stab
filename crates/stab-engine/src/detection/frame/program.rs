use arrayvec::ArrayVec;
use stab_model::{Circuit, CircuitInstruction, CircuitItem, RepeatNestingLimit, Target};
use std::ops::ControlFlow;

use super::helpers::{
    measurement_flip_probability, probability_list, single_probability_argument,
    zero_probability_noise,
};
use super::word::FrameWord;
use crate::detection::error::{
    DetectionError, DetectionResourceLimitError as ResourceLimitError, DetectionResult,
};

#[derive(Clone, Debug)]
pub(super) enum FrameProgramEntry {
    Execute {
        instruction: CircuitInstruction,
        tableau: Option<FrameTableauTransform>,
    },
    Fast {
        instruction: FastFrameInstruction,
    },
    Repeat {
        count: u64,
        body_end: usize,
    },
    EndRepeat,
}

const MAX_LOCAL_TABLEAU_QUBITS: usize = 8;

#[derive(Clone, Copy, Debug)]
pub(super) struct FrameTableauTransform {
    target_count: u8,
    x_outputs: [u16; MAX_LOCAL_TABLEAU_QUBITS],
    z_outputs: [u16; MAX_LOCAL_TABLEAU_QUBITS],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FastFrameOperation {
    Hadamard,
    ControlledNot,
    Tableau,
    ResetZ,
    MeasureZ,
    MeasureResetZ,
    MeasureXX,
    MeasureYY,
    MeasureZZ,
    SinglePauliNoise,
    TwoQubitPauliNoise,
}

#[derive(Clone, Debug)]
pub(super) struct FastFrameInstruction {
    pub(super) operation: FastFrameOperation,
    pub(super) targets: Vec<usize>,
    pub(super) probabilities: Vec<f64>,
    pub(super) tableau: Option<FrameTableauTransform>,
}

impl FrameTableauTransform {
    pub(super) fn compile(instruction: &CircuitInstruction) -> DetectionResult<Option<Self>> {
        if !stab_analysis::gate_has_tableau(instruction.gate()) {
            return Ok(None);
        }
        let tableau = stab_analysis::gate_tableau(instruction.gate())?;
        if tableau.len() > MAX_LOCAL_TABLEAU_QUBITS {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "gate {} has {} tableau targets, exceeding the detector-frame inline limit {MAX_LOCAL_TABLEAU_QUBITS}",
                instruction.gate().canonical_name(),
                tableau.len()
            )));
        }
        let mut transform = Self {
            target_count: u8::try_from(tableau.len()).map_err(|_| storage_overflow())?,
            x_outputs: [0; MAX_LOCAL_TABLEAU_QUBITS],
            z_outputs: [0; MAX_LOCAL_TABLEAU_QUBITS],
        };
        for (input, (x_output, z_output)) in transform
            .x_outputs
            .iter_mut()
            .zip(transform.z_outputs.iter_mut())
            .take(tableau.len())
            .enumerate()
        {
            *x_output = encode_output(tableau.x_output(input).map_err(|error| {
                DetectionError::invalid_sampler_compilation(error.to_string())
            })?)?;
            *z_output = encode_output(tableau.z_output(input).map_err(|error| {
                DetectionError::invalid_sampler_compilation(error.to_string())
            })?)?;
        }
        Ok(Some(transform))
    }

    pub(super) const fn target_count(self) -> usize {
        self.target_count as usize
    }

    pub(super) fn apply_word_planes<W: FrameWord>(
        self,
        input_xs: &[W],
        input_zs: &[W],
    ) -> Option<([W; MAX_LOCAL_TABLEAU_QUBITS], [W; MAX_LOCAL_TABLEAU_QUBITS])> {
        if input_xs.len() != self.target_count() || input_zs.len() != self.target_count() {
            return None;
        }
        let mut output_xs = [W::default(); MAX_LOCAL_TABLEAU_QUBITS];
        let mut output_zs = [W::default(); MAX_LOCAL_TABLEAU_QUBITS];
        for (input_index, (&input_x, &input_z)) in input_xs.iter().zip(input_zs).enumerate() {
            let x_output = *self.x_outputs.get(input_index)?;
            let z_output = *self.z_outputs.get(input_index)?;
            for output_index in 0..self.target_count() {
                if (x_output >> output_index) & 1 != 0 {
                    *output_xs.get_mut(output_index)? ^= input_x;
                }
                if (x_output >> (MAX_LOCAL_TABLEAU_QUBITS + output_index)) & 1 != 0 {
                    *output_zs.get_mut(output_index)? ^= input_x;
                }
                if (z_output >> output_index) & 1 != 0 {
                    *output_xs.get_mut(output_index)? ^= input_z;
                }
                if (z_output >> (MAX_LOCAL_TABLEAU_QUBITS + output_index)) & 1 != 0 {
                    *output_zs.get_mut(output_index)? ^= input_z;
                }
            }
        }
        Some((output_xs, output_zs))
    }
}

fn encode_output(output: &stab_algebra::PauliString) -> DetectionResult<u16> {
    if output.len() > MAX_LOCAL_TABLEAU_QUBITS {
        return Err(storage_overflow());
    }
    let mut encoded = 0_u16;
    for index in 0..output.len() {
        let basis = output.get(index).ok_or_else(program_shape_error)?;
        encoded |= u16::from(basis.x_bit()) << index;
        encoded |= u16::from(basis.z_bit()) << (MAX_LOCAL_TABLEAU_QUBITS + index);
    }
    Ok(encoded)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::detection) struct FrameProgramAdmission {
    entry_count: usize,
    retained_bytes: u64,
    qubit_count: usize,
}

impl FrameProgramAdmission {
    pub(in crate::detection) const fn retained_bytes(self) -> u64 {
        self.retained_bytes
    }
}

#[derive(Clone, Debug)]
pub(super) struct FrameProgram {
    entries: Vec<FrameProgramEntry>,
    retained_bytes: u64,
    qubit_count: usize,
}

impl FrameProgram {
    pub(super) fn admit(
        circuit: &Circuit,
        retained_base_bytes: u64,
        max_combined_bytes: u64,
        include_pauli_observables: bool,
    ) -> DetectionResult<FrameProgramAdmission> {
        let mut admission = FrameProgramAdmission::default();
        visit_circuit(circuit, |event| {
            match event {
                TraversalEvent::Instruction(instruction) => {
                    visit_lowered_frame_instructions(instruction, |lowered| {
                        validate_frame_instruction(lowered)?;
                        if frame_instruction_is_noop(lowered, include_pauli_observables)? {
                            return Ok(());
                        }
                        add_admitted_entry(&mut admission)?;
                        admission.retained_bytes = checked_add_bytes(
                            admission.retained_bytes,
                            instruction_payload_bytes(lowered)?,
                        )?;
                        admission.qubit_count =
                            admission.qubit_count.max(instruction_qubit_count(lowered)?);
                        validate_admission_bytes(
                            admission,
                            retained_base_bytes,
                            max_combined_bytes,
                        )?;
                        Ok(())
                    })?;
                }
                TraversalEvent::BeginRepeat(_) | TraversalEvent::EndRepeat => {
                    add_admitted_entry(&mut admission)?;
                    validate_admission_bytes(admission, retained_base_bytes, max_combined_bytes)?;
                }
            }
            Ok(())
        })?;
        Ok(admission)
    }

    pub(super) fn materialize(
        circuit: &Circuit,
        admission: FrameProgramAdmission,
        include_pauli_observables: bool,
    ) -> DetectionResult<Self> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(admission.entry_count)
            .map_err(|error| {
                DetectionError::invalid_sampler_compilation(format!(
                    "unable to reserve {} compact detector-frame entries: {error}",
                    admission.entry_count
                ))
            })?;
        let mut repeat_markers = ArrayVec::<usize, { RepeatNestingLimit::HARD_MAX }>::new();
        let mut qubit_count = 0_usize;
        visit_circuit(circuit, |event| {
            match event {
                TraversalEvent::Instruction(instruction) => {
                    visit_lowered_frame_instructions(instruction, |lowered| {
                        if frame_instruction_is_noop(lowered, include_pauli_observables)? {
                            return Ok(());
                        }
                        qubit_count = qubit_count.max(instruction_qubit_count(lowered)?);
                        let tableau = FrameTableauTransform::compile(lowered)?;
                        if let Some(operation) = fast_frame_operation(lowered) {
                            entries.push(FrameProgramEntry::Fast {
                                instruction: FastFrameInstruction {
                                    operation,
                                    targets: try_fast_targets(lowered)?,
                                    probabilities: try_fast_probabilities(lowered, operation)?,
                                    tableau,
                                },
                            });
                        } else {
                            entries.push(FrameProgramEntry::Execute {
                                instruction: try_clone_execution_instruction(lowered)?,
                                tableau,
                            });
                        }
                        Ok(())
                    })?;
                }
                TraversalEvent::BeginRepeat(count) => {
                    let marker = entries.len();
                    repeat_markers.try_push(marker).map_err(|_| {
                        DetectionError::invalid_sampler_compilation(
                            "detector-frame program exceeded admitted repeat nesting",
                        )
                    })?;
                    entries.push(FrameProgramEntry::Repeat {
                        count,
                        body_end: usize::MAX,
                    });
                }
                TraversalEvent::EndRepeat => {
                    let body_end = entries.len();
                    entries.push(FrameProgramEntry::EndRepeat);
                    let marker = repeat_markers.pop().ok_or_else(program_shape_error)?;
                    let Some(FrameProgramEntry::Repeat {
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
            || qubit_count != admission.qubit_count
        {
            return Err(DetectionError::invalid_sampler_compilation(
                "detector-frame materialization disagreed with allocation-free admission",
            ));
        }
        validate_program(&entries)?;
        let retained_bytes = materialized_retained_bytes(&entries, entries.capacity())?;
        Ok(Self {
            entries,
            retained_bytes,
            qubit_count,
        })
    }

    pub(super) const fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }

    pub(super) const fn qubit_count(&self) -> usize {
        self.qubit_count
    }

    pub(super) fn cursor(&self) -> FrameProgramCursor<'_> {
        FrameProgramCursor {
            entries: &self.entries,
            index: 0,
            repeats: ArrayVec::new(),
        }
    }
}

#[derive(Clone, Copy)]
enum TraversalEvent<'a> {
    Instruction(&'a CircuitInstruction),
    BeginRepeat(u64),
    EndRepeat,
}

struct TraversalFrame<'a> {
    items: std::slice::Iter<'a, CircuitItem>,
    end_repeat: bool,
}

fn visit_circuit<'a>(
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
                visitor(TraversalEvent::BeginRepeat(repeat.repeat_count().get()))?;
                frames
                    .try_push(TraversalFrame {
                        items: repeat.body().items().iter(),
                        end_repeat: true,
                    })
                    .map_err(|_| {
                        ResourceLimitError::detection_repeat_nesting(
                            RepeatNestingLimit::HARD_MAX + 1,
                            RepeatNestingLimit::HARD_MAX,
                        )
                    })?;
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

fn validate_frame_instruction(instruction: &CircuitInstruction) -> DetectionResult<()> {
    match instruction.gate().canonical_name() {
        "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" | "DETECTOR" | "OBSERVABLE_INCLUDE"
        | "I_ERROR" | "II_ERROR" => Ok(()),
        "R"
        | "RX"
        | "RY"
        | "M"
        | "MX"
        | "MY"
        | "MR"
        | "MRX"
        | "MRY"
        | "MXX"
        | "MYY"
        | "MZZ"
        | "MPP"
        | "MPAD"
        | "X_ERROR"
        | "Y_ERROR"
        | "Z_ERROR"
        | "DEPOLARIZE1"
        | "DEPOLARIZE2"
        | "PAULI_CHANNEL_1"
        | "PAULI_CHANNEL_2"
        | "E"
        | "ELSE_CORRELATED_ERROR"
        | "HERALDED_ERASE"
        | "HERALDED_PAULI_CHANNEL_1" => Ok(()),
        "CX" | "CY" | "CZ" | "XCZ" | "YCZ" => validate_controlled_pauli_targets(instruction),
        _ if stab_analysis::gate_has_tableau(instruction.gate()) => Ok(()),
        _ if zero_probability_noise(instruction)? => Ok(()),
        name => Err(DetectionError::invalid_sampler_compilation(format!(
            "detector frame execution does not support {name}"
        ))),
    }
}

fn frame_instruction_is_noop(
    instruction: &CircuitInstruction,
    include_pauli_observables: bool,
) -> DetectionResult<bool> {
    match instruction.gate().canonical_name() {
        "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" | "DETECTOR" | "I_ERROR" | "II_ERROR" => Ok(true),
        "OBSERVABLE_INCLUDE" => Ok(!include_pauli_observables
            || !instruction.targets().iter().any(Target::is_pauli_target)),
        "X_ERROR" | "Y_ERROR" | "Z_ERROR" | "DEPOLARIZE1" | "DEPOLARIZE2" | "PAULI_CHANNEL_1"
        | "PAULI_CHANNEL_2" => zero_probability_noise(instruction),
        _ => Ok(false),
    }
}

fn add_admitted_entry(admission: &mut FrameProgramAdmission) -> DetectionResult<()> {
    admission.entry_count = admission
        .entry_count
        .checked_add(1)
        .ok_or_else(storage_overflow)?;
    admission.retained_bytes = checked_add_bytes(
        admission.retained_bytes,
        u64::try_from(size_of::<FrameProgramEntry>()).map_err(|_| storage_overflow())?,
    )?;
    Ok(())
}

fn visit_lowered_frame_instructions(
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

fn validate_controlled_pauli_targets(instruction: &CircuitInstruction) -> DetectionResult<()> {
    use stab_model::advanced::{ControlledPauliTargetPair, classify_controlled_pauli_target_pair};
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

fn fast_frame_operation(instruction: &CircuitInstruction) -> Option<FastFrameOperation> {
    match instruction.gate().canonical_name() {
        "H" => Some(FastFrameOperation::Hadamard),
        "R" => Some(FastFrameOperation::ResetZ),
        "M" => Some(FastFrameOperation::MeasureZ),
        "MR" => Some(FastFrameOperation::MeasureResetZ),
        "MXX" => Some(FastFrameOperation::MeasureXX),
        "MYY" => Some(FastFrameOperation::MeasureYY),
        "MZZ" => Some(FastFrameOperation::MeasureZZ),
        "X_ERROR" | "Y_ERROR" | "Z_ERROR" | "DEPOLARIZE1" | "PAULI_CHANNEL_1" => {
            Some(FastFrameOperation::SinglePauliNoise)
        }
        "DEPOLARIZE2" | "PAULI_CHANNEL_2" => Some(FastFrameOperation::TwoQubitPauliNoise),
        "CX" if instruction.targets().chunks_exact(2).all(|targets| {
            matches!(
                stab_model::advanced::classify_controlled_pauli_target_pair(
                    instruction.gate(),
                    targets
                ),
                stab_model::advanced::ControlledPauliTargetPair::Quantum { .. }
            )
        }) =>
        {
            Some(FastFrameOperation::ControlledNot)
        }
        _ if stab_analysis::gate_has_tableau(instruction.gate())
            && instruction
                .targets()
                .iter()
                .all(|target| target.qubit_id().is_some()) =>
        {
            Some(FastFrameOperation::Tableau)
        }
        _ => None,
    }
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

fn instruction_payload_bytes(instruction: &CircuitInstruction) -> DetectionResult<u64> {
    if let Some(operation) = fast_frame_operation(instruction) {
        return checked_add_bytes(
            byte_product(instruction.targets().len(), size_of::<usize>())?,
            byte_product(fast_probability_count(operation), size_of::<f64>())?,
        );
    }
    let bytes = stab_model::advanced::circuit_instruction_minimum_retained_heap_bytes(instruction)
        .ok_or_else(storage_overflow)?;
    u64::try_from(bytes).map_err(|_| storage_overflow())
}

fn materialized_retained_bytes(
    entries: &[FrameProgramEntry],
    entry_capacity: usize,
) -> DetectionResult<u64> {
    let mut retained = byte_product(entry_capacity, size_of::<FrameProgramEntry>())?;
    for entry in entries {
        let payload = match entry {
            FrameProgramEntry::Execute { instruction, .. } => {
                stab_model::advanced::circuit_instruction_retained_heap_bytes(instruction)
                    .ok_or_else(storage_overflow)?
            }
            FrameProgramEntry::Fast { instruction } => instruction
                .targets
                .capacity()
                .checked_mul(size_of::<usize>())
                .and_then(|target_bytes| {
                    instruction
                        .probabilities
                        .capacity()
                        .checked_mul(size_of::<f64>())
                        .and_then(|probability_bytes| target_bytes.checked_add(probability_bytes))
                })
                .ok_or_else(storage_overflow)?,
            FrameProgramEntry::Repeat { .. } | FrameProgramEntry::EndRepeat => continue,
        };
        retained = checked_add_bytes(
            retained,
            u64::try_from(payload).map_err(|_| storage_overflow())?,
        )?;
    }
    Ok(retained)
}

fn try_fast_targets(instruction: &CircuitInstruction) -> DetectionResult<Vec<usize>> {
    let mut targets = Vec::new();
    targets
        .try_reserve_exact(instruction.targets().len())
        .map_err(|error| {
            DetectionError::invalid_sampler_compilation(format!(
                "unable to reserve {} resolved detector-frame targets: {error}",
                instruction.targets().len()
            ))
        })?;
    for target in instruction.targets() {
        let qubit = target.qubit_id().ok_or_else(program_shape_error)?;
        targets.push(usize::try_from(qubit.get()).map_err(|_| storage_overflow())?);
    }
    Ok(targets)
}

const fn fast_probability_count(operation: FastFrameOperation) -> usize {
    match operation {
        FastFrameOperation::MeasureZ
        | FastFrameOperation::MeasureResetZ
        | FastFrameOperation::MeasureXX
        | FastFrameOperation::MeasureYY
        | FastFrameOperation::MeasureZZ => 1,
        FastFrameOperation::SinglePauliNoise => 3,
        FastFrameOperation::TwoQubitPauliNoise => 15,
        FastFrameOperation::Hadamard
        | FastFrameOperation::ControlledNot
        | FastFrameOperation::Tableau
        | FastFrameOperation::ResetZ => 0,
    }
}

fn try_fast_probabilities(
    instruction: &CircuitInstruction,
    operation: FastFrameOperation,
) -> DetectionResult<Vec<f64>> {
    let mut probabilities = Vec::new();
    probabilities
        .try_reserve_exact(fast_probability_count(operation))
        .map_err(|error| {
            DetectionError::invalid_sampler_compilation(format!(
                "unable to reserve {} detector-frame probabilities: {error}",
                fast_probability_count(operation)
            ))
        })?;
    match operation {
        FastFrameOperation::Hadamard
        | FastFrameOperation::ControlledNot
        | FastFrameOperation::Tableau
        | FastFrameOperation::ResetZ => {}
        FastFrameOperation::MeasureZ
        | FastFrameOperation::MeasureResetZ
        | FastFrameOperation::MeasureXX
        | FastFrameOperation::MeasureYY
        | FastFrameOperation::MeasureZZ => {
            probabilities.push(measurement_flip_probability(instruction)?);
        }
        FastFrameOperation::SinglePauliNoise => {
            let values = match instruction.gate().canonical_name() {
                "X_ERROR" => [single_probability_argument(instruction)?.get(), 0.0, 0.0],
                "Y_ERROR" => [0.0, single_probability_argument(instruction)?.get(), 0.0],
                "Z_ERROR" => [0.0, 0.0, single_probability_argument(instruction)?.get()],
                "DEPOLARIZE1" => {
                    let probability = single_probability_argument(instruction)?.get() / 3.0;
                    [probability; 3]
                }
                "PAULI_CHANNEL_1" => probability_list::<3>(instruction)?,
                _ => return Err(program_shape_error()),
            };
            probabilities.extend_from_slice(&values);
        }
        FastFrameOperation::TwoQubitPauliNoise => {
            let values = match instruction.gate().canonical_name() {
                "DEPOLARIZE2" => {
                    let probability = single_probability_argument(instruction)?.get() / 15.0;
                    [probability; 15]
                }
                "PAULI_CHANNEL_2" => probability_list::<15>(instruction)?,
                _ => return Err(program_shape_error()),
            };
            probabilities.extend_from_slice(&values);
        }
    }
    Ok(probabilities)
}

fn try_clone_execution_instruction(
    instruction: &CircuitInstruction,
) -> DetectionResult<CircuitInstruction> {
    let mut args = Vec::new();
    args.try_reserve_exact(instruction.args().len())
        .map_err(|error| {
            DetectionError::invalid_sampler_compilation(format!(
                "unable to reserve {} detector-frame argument slots: {error}",
                instruction.args().len()
            ))
        })?;
    args.extend_from_slice(instruction.args());
    let mut targets = Vec::new();
    targets
        .try_reserve_exact(instruction.targets().len())
        .map_err(|error| {
            DetectionError::invalid_sampler_compilation(format!(
                "unable to reserve {} detector-frame target slots: {error}",
                instruction.targets().len()
            ))
        })?;
    targets.extend(instruction.targets().iter().cloned());
    stab_model::advanced::circuit_instruction_with_tag_bytes(
        instruction.gate(),
        args,
        targets,
        None,
    )
    .map_err(Into::into)
}

#[derive(Clone, Copy)]
struct RepeatExecutionFrame {
    body_start: usize,
    body_end: usize,
    remaining: u64,
}

pub(super) struct FrameProgramCursor<'a> {
    entries: &'a [FrameProgramEntry],
    index: usize,
    repeats: ArrayVec<RepeatExecutionFrame, { RepeatNestingLimit::HARD_MAX }>,
}

impl<'a> FrameProgramCursor<'a> {
    pub(super) fn next_instruction(&mut self) -> DetectionResult<Option<FrameInstruction<'a>>> {
        loop {
            let Some(entry) = self.entries.get(self.index) else {
                if self.repeats.is_empty() && self.index == self.entries.len() {
                    return Ok(None);
                }
                return Err(program_shape_error());
            };
            match entry {
                FrameProgramEntry::Execute {
                    instruction,
                    tableau,
                } => {
                    self.index = self.index.checked_add(1).ok_or_else(storage_overflow)?;
                    return Ok(Some(FrameInstruction::Execute {
                        instruction,
                        tableau: tableau.as_ref(),
                    }));
                }
                FrameProgramEntry::Fast { instruction } => {
                    self.index = self.index.checked_add(1).ok_or_else(storage_overflow)?;
                    return Ok(Some(FrameInstruction::Fast(instruction)));
                }
                FrameProgramEntry::Repeat { count, body_end } => {
                    if *count == 0
                        || *body_end <= self.index
                        || !matches!(
                            self.entries.get(*body_end),
                            Some(FrameProgramEntry::EndRepeat)
                        )
                    {
                        return Err(program_shape_error());
                    }
                    let body_start = self.index.checked_add(1).ok_or_else(storage_overflow)?;
                    self.repeats
                        .try_push(RepeatExecutionFrame {
                            body_start,
                            body_end: *body_end,
                            remaining: *count,
                        })
                        .map_err(|_| program_shape_error())?;
                    self.index = body_start;
                }
                FrameProgramEntry::EndRepeat => {
                    let frame = self.repeats.last_mut().ok_or_else(program_shape_error)?;
                    if frame.body_end != self.index || frame.remaining == 0 {
                        return Err(program_shape_error());
                    }
                    if frame.remaining > 1 {
                        frame.remaining -= 1;
                        self.index = frame.body_start;
                    } else {
                        self.repeats.pop();
                        self.index = self.index.checked_add(1).ok_or_else(storage_overflow)?;
                    }
                }
            }
        }
    }
}

pub(super) enum FrameInstruction<'a> {
    Execute {
        instruction: &'a CircuitInstruction,
        tableau: Option<&'a FrameTableauTransform>,
    },
    Fast(&'a FastFrameInstruction),
}

fn validate_program(entries: &[FrameProgramEntry]) -> DetectionResult<()> {
    let mut repeat_ends = ArrayVec::<usize, { RepeatNestingLimit::HARD_MAX }>::new();
    for (index, entry) in entries.iter().enumerate() {
        match entry {
            FrameProgramEntry::Execute { .. } | FrameProgramEntry::Fast { .. } => {}
            FrameProgramEntry::Repeat { count, body_end } => {
                if *count == 0
                    || *body_end <= index
                    || !matches!(entries.get(*body_end), Some(FrameProgramEntry::EndRepeat))
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
            FrameProgramEntry::EndRepeat => {
                if repeat_ends.pop() != Some(index) {
                    return Err(program_shape_error());
                }
            }
        }
    }
    repeat_ends
        .is_empty()
        .then_some(())
        .ok_or_else(program_shape_error)
}

fn byte_product(count: usize, item_size: usize) -> DetectionResult<u64> {
    let bytes = count.checked_mul(item_size).ok_or_else(storage_overflow)?;
    u64::try_from(bytes).map_err(|_| storage_overflow())
}

fn checked_add_bytes(left: u64, right: u64) -> DetectionResult<u64> {
    left.checked_add(right).ok_or_else(storage_overflow)
}

fn validate_admission_bytes(
    admission: FrameProgramAdmission,
    retained_base_bytes: u64,
    max_combined_bytes: u64,
) -> DetectionResult<()> {
    let combined = checked_add_bytes(retained_base_bytes, admission.retained_bytes)?;
    if combined > max_combined_bytes {
        return Err(
            ResourceLimitError::detection_compiled_bytes(combined, max_combined_bytes).into(),
        );
    }
    Ok(())
}

fn storage_overflow() -> DetectionError {
    DetectionError::invalid_sampler_compilation(
        "compact detector-frame retained byte count overflowed",
    )
}

fn program_shape_error() -> DetectionError {
    DetectionError::invalid_sampler_compilation(
        "compact detector-frame program has an invalid repeat shape",
    )
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "frame-program tests use direct structural fixture assertions"
    )]

    use super::*;

    #[test]
    fn structural_validation_does_not_execute_repeat_iterations() {
        let entries = [
            FrameProgramEntry::Repeat {
                count: u64::MAX,
                body_end: 1,
            },
            FrameProgramEntry::EndRepeat,
        ];

        validate_program(&entries).expect("validate compact repeat tape structurally");
    }

    #[test]
    fn spp_lowering_stops_at_the_first_compiled_byte_rejection() {
        let circuit = Circuit::from_stim_str("SPP X0 X1*Z1\n").expect("parse SPP fixture");
        let error = FrameProgram::admit(&circuit, 0, 0, true)
            .expect_err("the first lowered instruction exceeds a zero-byte budget");
        assert!(matches!(
            error,
            DetectionError::ResourceLimit(ref resource)
                if resource.kind() == crate::detection::DetectionResourceKind::CompiledBytes
        ));
    }
}
