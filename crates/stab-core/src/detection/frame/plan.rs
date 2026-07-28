use std::borrow::Cow;

use rand::Rng;

use super::ScalarDetectionFrame;
use super::helpers::{
    is_frame_bit_target, is_frame_qubit_or_bit_target, unsupported_frame_instruction,
    zero_probability_noise,
};
use crate::detection::{ConversionPlan, DetectionConversionLimits};
use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, RepeatBlock,
    ResourceLimitError, Target, circuit::CircuitAssembler,
};

struct AdmittedFrameConversion {
    plan: ConversionPlan,
    execution_storage: FrameExecutionStorage,
}

impl AdmittedFrameConversion {
    fn admit(
        circuit: &Circuit,
        limits: DetectionConversionLimits,
    ) -> CircuitResult<AdmittedFrameConversion> {
        let admission = ConversionPlan::admission_from_visitor(limits, |plan| {
            append_frame_conversion_plan(circuit, plan)
        })?;
        let execution_storage = frame_execution_storage(circuit)?;
        admit_combined_compiled_storage(
            admission.compiled_storage_bytes()?,
            execution_storage.retained_bytes,
            limits.max_compiled_bytes(),
        )?;
        let plan = ConversionPlan::materialize_from_admission(admission, |plan| {
            append_frame_conversion_plan(circuit, plan)
        })?;
        Ok(Self {
            plan,
            execution_storage,
        })
    }

    fn materialize_execution_circuit(&self, circuit: &Circuit) -> CircuitResult<Circuit> {
        let mut result = CircuitAssembler::new();
        append_frame_execution_circuit(circuit, &mut result, self.execution_storage.root_items)?;
        Ok(result.finish())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FrameExecutionStorage {
    root_items: usize,
    retained_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameExecutionInstructionDisposition {
    Borrowed,
    Filtered { retained_targets: usize },
    Omitted,
}

#[derive(Clone, Debug)]
pub(in crate::detection) struct DirectDetectorFramePlan {
    executable: Circuit,
    conversion: ConversionPlan,
    limits: DetectionConversionLimits,
}

impl DirectDetectorFramePlan {
    pub(in crate::detection) fn compile(
        circuit: &Circuit,
        limits: DetectionConversionLimits,
    ) -> CircuitResult<Self> {
        let admitted = AdmittedFrameConversion::admit(circuit, limits)?;
        let executable = admitted.materialize_execution_circuit(circuit)?;
        Ok(Self {
            executable,
            conversion: admitted.plan,
            limits,
        })
    }

    pub(in crate::detection) fn measurement_count(&self) -> usize {
        self.conversion.measurement_count
    }

    pub(in crate::detection) fn qubit_count(&self) -> usize {
        self.executable.count_qubits()
    }

    pub(in crate::detection) fn detector_count(&self) -> usize {
        self.conversion.detector_terms.len()
    }

    pub(in crate::detection) fn observable_count(&self) -> usize {
        self.conversion.observable_terms.len()
    }

    #[cfg(test)]
    pub(in crate::detection) fn compiled_bytes(&self) -> CircuitResult<u64> {
        self.conversion
            .compiled_storage_bytes()?
            .checked_add(frame_execution_storage(&self.executable)?.retained_bytes)
            .ok_or_else(storage_overflow)
    }

    pub(in crate::detection) fn state(&self) -> CircuitResult<DirectDetectorFrameState> {
        Ok(DirectDetectorFrameState {
            frame: ScalarDetectionFrame::try_reusable(
                self.executable.count_qubits(),
                self.measurement_count(),
                self.detector_count(),
                self.observable_count(),
            )?,
        })
    }

    pub(in crate::detection) fn sample<'a>(
        &self,
        state: &'a mut DirectDetectorFrameState,
        rng: &mut impl Rng,
    ) -> CircuitResult<(&'a [bool], &'a [bool])> {
        state.frame.reset(rng);
        state
            .frame
            .execute_circuit(&self.executable, self.limits.max_repeat_unroll(), rng)?;
        if state.frame.measurements.len() != self.measurement_count() {
            return Err(CircuitError::invalid_result_format(format!(
                "frame detection sampled {} measurement bits but expected {}",
                state.frame.measurements.len(),
                self.measurement_count()
            )));
        }
        if state.frame.detectors.len() != self.detector_count() {
            return Err(CircuitError::invalid_result_format(format!(
                "frame detection sampled {} detector bits but expected {}",
                state.frame.detectors.len(),
                self.detector_count()
            )));
        }
        if state.frame.observables.len() != self.observable_count() {
            return Err(CircuitError::invalid_result_format(format!(
                "frame detection sampled {} observable bits but expected {}",
                state.frame.observables.len(),
                self.observable_count()
            )));
        }
        Ok((&state.frame.detectors, &state.frame.observables))
    }
}

#[derive(Debug)]
pub(in crate::detection) struct DirectDetectorFrameState {
    pub(super) frame: ScalarDetectionFrame,
}

pub(in crate::detection) fn frame_conversion_plan_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> CircuitResult<ConversionPlan> {
    ConversionPlan::from_visitor(limits, |plan| append_frame_conversion_plan(circuit, plan))
}

fn append_frame_conversion_plan(circuit: &Circuit, plan: &mut ConversionPlan) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction)
                if matches!(instruction.gate().canonical_name(), "SPP" | "SPP_DAG") =>
            {
                let decomposed = decomposed_frame_instruction(instruction)?;
                append_frame_conversion_plan(&decomposed, plan)?;
            }
            CircuitItem::Instruction(instruction) => {
                match frame_execution_instruction_disposition(instruction)? {
                    FrameExecutionInstructionDisposition::Borrowed => {
                        validate_frame_detection_instruction(instruction)?;
                        plan.visit_instruction(instruction)?;
                    }
                    FrameExecutionInstructionDisposition::Filtered { .. } => {
                        validate_frame_detection_instruction(instruction)?;
                        plan.visit_frame_instruction_without_sweep(instruction)?;
                    }
                    FrameExecutionInstructionDisposition::Omitted => continue,
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                plan.visit_repeated_body(repeat.repeat_count().get(), |plan| {
                    append_frame_conversion_plan(repeat.body(), plan)
                })?;
            }
        }
    }
    Ok(())
}

fn validate_frame_detection_instruction(instruction: &CircuitInstruction) -> CircuitResult<()> {
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
        "SPP" | "SPP_DAG" => Err(CircuitError::invalid_sampler_compilation(
            "frame detection must decompose SPP instructions before validation",
        )),
        "CX" | "CY" => validate_frame_controlled_pauli_targets(instruction),
        "CZ" => validate_frame_cz_targets(instruction),
        "XCZ" | "YCZ" => validate_frame_x_or_y_controlled_z_targets(instruction),
        _ if crate::analysis::gate_has_tableau(instruction.gate()) => Ok(()),
        _ if zero_probability_noise(instruction)? => Ok(()),
        name => Err(CircuitError::invalid_sampler_compilation(format!(
            "M9 detector frame subset does not support {name}"
        ))),
    }
}

pub(super) fn decomposed_frame_instruction(
    instruction: &CircuitInstruction,
) -> CircuitResult<Circuit> {
    crate::analysis::decomposed_single_instruction(instruction).map_err(|error| {
        CircuitError::invalid_sampler_compilation(format!(
            "{} cannot be executed by frame detection via decomposition: {error}",
            instruction.gate().canonical_name()
        ))
    })
}

fn validate_frame_controlled_pauli_targets(instruction: &CircuitInstruction) -> CircuitResult<()> {
    for target_group in instruction.targets().chunks(2) {
        let [control, target] = target_group else {
            return Err(unsupported_frame_instruction(instruction));
        };
        if (control.qubit_id().is_some() || is_frame_bit_target(control))
            && target.qubit_id().is_some()
        {
            continue;
        }
        return Err(unsupported_frame_instruction(instruction));
    }
    Ok(())
}

fn validate_frame_cz_targets(instruction: &CircuitInstruction) -> CircuitResult<()> {
    for target_group in instruction.targets().chunks(2) {
        let [left, right] = target_group else {
            return Err(unsupported_frame_instruction(instruction));
        };
        if is_frame_qubit_or_bit_target(left) && is_frame_qubit_or_bit_target(right) {
            continue;
        }
        return Err(unsupported_frame_instruction(instruction));
    }
    Ok(())
}

fn validate_frame_x_or_y_controlled_z_targets(
    instruction: &CircuitInstruction,
) -> CircuitResult<()> {
    for target_group in instruction.targets().chunks(2) {
        let [left, right] = target_group else {
            return Err(unsupported_frame_instruction(instruction));
        };
        if left.qubit_id().is_some() && right.qubit_id().is_some() {
            continue;
        }
        if left.qubit_id().is_some() && right.measurement_record_offset().is_some() {
            continue;
        }
        if left.qubit_id().is_some() && right.is_sweep_bit_target() {
            continue;
        }
        return Err(unsupported_frame_instruction(instruction));
    }
    Ok(())
}

fn frame_execution_storage(circuit: &Circuit) -> CircuitResult<FrameExecutionStorage> {
    let mut storage = FrameExecutionStorage::default();
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction)
                if matches!(instruction.gate().canonical_name(), "SPP" | "SPP_DAG") =>
            {
                let decomposed = decomposed_frame_instruction(instruction)?;
                storage = checked_add_storage(storage, frame_execution_storage(&decomposed)?)?;
            }
            CircuitItem::Instruction(instruction) => {
                let target_count = match frame_execution_instruction_disposition(instruction)? {
                    FrameExecutionInstructionDisposition::Borrowed => instruction.targets().len(),
                    FrameExecutionInstructionDisposition::Filtered { retained_targets } => {
                        retained_targets
                    }
                    FrameExecutionInstructionDisposition::Omitted => continue,
                };
                storage.root_items = storage
                    .root_items
                    .checked_add(1)
                    .ok_or_else(storage_overflow)?;
                storage.retained_bytes = checked_add_bytes(
                    storage.retained_bytes,
                    instruction_retained_bytes(instruction.args().len(), target_count)?,
                )?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                storage.root_items = storage
                    .root_items
                    .checked_add(1)
                    .ok_or_else(storage_overflow)?;
                storage.retained_bytes = checked_add_bytes(
                    storage.retained_bytes,
                    u64::try_from(size_of::<CircuitItem>()).map_err(|_| storage_overflow())?,
                )?;
                let body = frame_execution_storage(repeat.body())?;
                storage.retained_bytes =
                    checked_add_bytes(storage.retained_bytes, body.retained_bytes)?;
            }
        }
    }
    Ok(storage)
}

fn instruction_retained_bytes(arg_count: usize, target_count: usize) -> CircuitResult<u64> {
    let item = u64::try_from(size_of::<CircuitItem>()).map_err(|_| storage_overflow())?;
    let args = byte_product(arg_count, size_of::<f64>())?;
    let targets = byte_product(target_count, size_of::<Target>())?;
    checked_add_bytes(checked_add_bytes(item, args)?, targets)
}

fn checked_add_storage(
    left: FrameExecutionStorage,
    right: FrameExecutionStorage,
) -> CircuitResult<FrameExecutionStorage> {
    Ok(FrameExecutionStorage {
        root_items: left
            .root_items
            .checked_add(right.root_items)
            .ok_or_else(storage_overflow)?,
        retained_bytes: checked_add_bytes(left.retained_bytes, right.retained_bytes)?,
    })
}

fn byte_product(count: usize, item_size: usize) -> CircuitResult<u64> {
    let bytes = count.checked_mul(item_size).ok_or_else(storage_overflow)?;
    u64::try_from(bytes).map_err(|_| storage_overflow())
}

fn checked_add_bytes(left: u64, right: u64) -> CircuitResult<u64> {
    left.checked_add(right).ok_or_else(storage_overflow)
}

fn storage_overflow() -> CircuitError {
    CircuitError::invalid_sampler_compilation(
        "direct detector-frame retained byte count overflowed",
    )
}

fn admit_combined_compiled_storage(
    conversion_bytes: u64,
    execution_bytes: u64,
    limit_bytes: u64,
) -> CircuitResult<u64> {
    let combined = conversion_bytes
        .checked_add(execution_bytes)
        .ok_or_else(storage_overflow)?;
    if combined > limit_bytes {
        return Err(ResourceLimitError::detection_compiled_bytes(combined, limit_bytes).into());
    }
    Ok(combined)
}

fn append_frame_execution_circuit(
    circuit: &Circuit,
    result: &mut CircuitAssembler,
    root_item_capacity: usize,
) -> CircuitResult<()> {
    result.try_reserve_exact(root_item_capacity)?;
    append_frame_execution_items(circuit, result)
}

fn append_frame_execution_items(
    circuit: &Circuit,
    result: &mut CircuitAssembler,
) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction)
                if matches!(instruction.gate().canonical_name(), "SPP" | "SPP_DAG") =>
            {
                let decomposed = decomposed_frame_instruction(instruction)?;
                append_frame_execution_items(&decomposed, result)?;
            }
            CircuitItem::Instruction(instruction) => {
                if let Some(instruction) = frame_execution_instruction(instruction)? {
                    result.try_append_instruction(try_clone_execution_instruction(
                        instruction.as_ref(),
                    )?)?;
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let shape = frame_execution_storage(repeat.body())?;
                let mut body = CircuitAssembler::new();
                append_frame_execution_circuit(repeat.body(), &mut body, shape.root_items)?;
                result.try_append_repeat_block(RepeatBlock::new(
                    repeat.repeat_count(),
                    body.finish(),
                    None,
                ))?;
            }
        }
    }
    Ok(())
}

fn try_clone_execution_instruction(
    instruction: &CircuitInstruction,
) -> CircuitResult<CircuitInstruction> {
    let mut args = Vec::new();
    args.try_reserve_exact(instruction.args().len())
        .map_err(|error| {
            CircuitError::invalid_sampler_compilation(format!(
                "unable to reserve {} direct-frame argument slots: {error}",
                instruction.args().len()
            ))
        })?;
    args.extend_from_slice(instruction.args());
    let mut targets = Vec::new();
    targets
        .try_reserve_exact(instruction.targets().len())
        .map_err(|error| {
            CircuitError::invalid_sampler_compilation(format!(
                "unable to reserve {} direct-frame target slots: {error}",
                instruction.targets().len()
            ))
        })?;
    targets.extend(instruction.targets().iter().cloned());
    CircuitInstruction::new_with_tag_bytes(instruction.gate(), args, targets, None)
}

fn frame_execution_instruction<'a>(
    instruction: &'a CircuitInstruction,
) -> CircuitResult<Option<Cow<'a, CircuitInstruction>>> {
    let retained_targets = match frame_execution_instruction_disposition(instruction)? {
        FrameExecutionInstructionDisposition::Borrowed => {
            return Ok(Some(Cow::Borrowed(instruction)));
        }
        FrameExecutionInstructionDisposition::Filtered { retained_targets } => retained_targets,
        FrameExecutionInstructionDisposition::Omitted => return Ok(None),
    };

    let mut targets = Vec::new();
    targets
        .try_reserve_exact(retained_targets)
        .map_err(|error| {
            CircuitError::invalid_sampler_compilation(format!(
                "unable to reserve {retained_targets} filtered direct-frame targets: {error}"
            ))
        })?;
    for target_group in instruction.targets().chunks(2) {
        let [left, right] = target_group else {
            return Err(unsupported_frame_instruction(instruction));
        };
        if left.qubit_id().is_some() && right.is_sweep_bit_target() {
            continue;
        }
        targets.extend(target_group.iter().cloned());
    }
    debug_assert_eq!(targets.len(), retained_targets);
    let mut args = Vec::new();
    args.try_reserve_exact(instruction.args().len())
        .map_err(|error| {
            CircuitError::invalid_sampler_compilation(format!(
                "unable to reserve {} filtered direct-frame arguments: {error}",
                instruction.args().len()
            ))
        })?;
    args.extend_from_slice(instruction.args());
    Ok(Some(Cow::Owned(CircuitInstruction::new_with_tag_bytes(
        instruction.gate(),
        args,
        targets,
        None,
    )?)))
}

fn frame_execution_instruction_disposition(
    instruction: &CircuitInstruction,
) -> CircuitResult<FrameExecutionInstructionDisposition> {
    if !matches!(instruction.gate().canonical_name(), "XCZ" | "YCZ") {
        return Ok(FrameExecutionInstructionDisposition::Borrowed);
    }

    let mut retained_targets = 0_usize;
    let mut removed_sweep_target = false;
    for target_group in instruction.targets().chunks(2) {
        let [left, right] = target_group else {
            return Ok(FrameExecutionInstructionDisposition::Borrowed);
        };
        if left.qubit_id().is_some() && right.is_sweep_bit_target() {
            removed_sweep_target = true;
            continue;
        }
        retained_targets = retained_targets
            .checked_add(target_group.len())
            .ok_or_else(storage_overflow)?;
    }
    if !removed_sweep_target {
        return Ok(FrameExecutionInstructionDisposition::Borrowed);
    }
    if retained_targets == 0 {
        return Ok(FrameExecutionInstructionDisposition::Omitted);
    }
    Ok(FrameExecutionInstructionDisposition::Filtered { retained_targets })
}
