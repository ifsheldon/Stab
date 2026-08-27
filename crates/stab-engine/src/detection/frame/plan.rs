use rand::Rng;
use stab_model::advanced::{
    CircuitBuilder as CircuitAssembler, ControlledPauliTargetPair,
    classify_controlled_pauli_target_pair,
};
use stab_model::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock, Target};

use super::ScalarDetectionFrame;
use super::helpers::{unsupported_frame_instruction, zero_probability_noise};
use crate::detection::error::{
    DetectionError, DetectionResourceLimitError as ResourceLimitError, DetectionResult,
};
use crate::detection::{ConversionPlan, DetectionConversionLimits};

struct AdmittedFrameConversion {
    plan: ConversionPlan,
    execution_storage: FrameExecutionStorage,
}

impl AdmittedFrameConversion {
    fn admit(
        circuit: &Circuit,
        limits: DetectionConversionLimits,
    ) -> DetectionResult<AdmittedFrameConversion> {
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

    fn materialize_execution_circuit(&self, circuit: &Circuit) -> DetectionResult<Circuit> {
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
    ) -> DetectionResult<Self> {
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
    pub(in crate::detection) fn compiled_bytes(&self) -> DetectionResult<u64> {
        self.conversion
            .compiled_storage_bytes()?
            .checked_add(frame_execution_storage(&self.executable)?.retained_bytes)
            .ok_or_else(storage_overflow)
    }

    pub(in crate::detection) fn state(&self) -> DetectionResult<DirectDetectorFrameState> {
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
    ) -> DetectionResult<(&'a [bool], &'a [bool])> {
        state.frame.reset(rng);
        state
            .frame
            .execute_circuit(&self.executable, self.limits.max_repeat_unroll(), rng)?;
        if state.frame.measurements.len() != self.measurement_count() {
            return Err(DetectionError::invalid_result_format(format!(
                "frame detection sampled {} measurement bits but expected {}",
                state.frame.measurements.len(),
                self.measurement_count()
            )));
        }
        if state.frame.detectors.len() != self.detector_count() {
            return Err(DetectionError::invalid_result_format(format!(
                "frame detection sampled {} detector bits but expected {}",
                state.frame.detectors.len(),
                self.detector_count()
            )));
        }
        if state.frame.observables.len() != self.observable_count() {
            return Err(DetectionError::invalid_result_format(format!(
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
) -> DetectionResult<ConversionPlan> {
    ConversionPlan::from_visitor(limits, |plan| append_frame_conversion_plan(circuit, plan))
}

fn append_frame_conversion_plan(
    circuit: &Circuit,
    plan: &mut ConversionPlan,
) -> DetectionResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction)
                if matches!(instruction.gate().canonical_name(), "SPP" | "SPP_DAG") =>
            {
                let decomposed = decomposed_frame_instruction(instruction)?;
                append_frame_conversion_plan(&decomposed, plan)?;
            }
            CircuitItem::Instruction(instruction) => {
                validate_frame_detection_instruction(instruction)?;
                plan.visit_instruction(instruction)?;
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

fn validate_frame_detection_instruction(instruction: &CircuitInstruction) -> DetectionResult<()> {
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
        "SPP" | "SPP_DAG" => Err(DetectionError::invalid_sampler_compilation(
            "frame detection must decompose SPP instructions before validation",
        )),
        "CX" | "CY" | "CZ" | "XCZ" | "YCZ" => validate_frame_controlled_pauli_targets(instruction),
        _ if stab_analysis::gate_has_tableau(instruction.gate()) => Ok(()),
        _ if zero_probability_noise(instruction)? => Ok(()),
        name => Err(DetectionError::invalid_sampler_compilation(format!(
            "M9 detector frame subset does not support {name}"
        ))),
    }
}

pub(super) fn decomposed_frame_instruction(
    instruction: &CircuitInstruction,
) -> DetectionResult<Circuit> {
    stab_analysis::advanced::decomposed_single_instruction(instruction).map_err(|error| {
        DetectionError::invalid_sampler_compilation(format!(
            "{} cannot be executed by frame detection via decomposition: {error}",
            instruction.gate().canonical_name()
        ))
    })
}

fn validate_frame_controlled_pauli_targets(
    instruction: &CircuitInstruction,
) -> DetectionResult<()> {
    for target_group in instruction.targets().chunks(2) {
        if matches!(
            classify_controlled_pauli_target_pair(instruction.gate(), target_group),
            ControlledPauliTargetPair::Unsupported
        ) {
            return Err(unsupported_frame_instruction(instruction));
        }
    }
    Ok(())
}

fn frame_execution_storage(circuit: &Circuit) -> DetectionResult<FrameExecutionStorage> {
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
                storage.root_items = storage
                    .root_items
                    .checked_add(1)
                    .ok_or_else(storage_overflow)?;
                storage.retained_bytes = checked_add_bytes(
                    storage.retained_bytes,
                    instruction_retained_bytes(
                        instruction.args().len(),
                        instruction.targets().len(),
                    )?,
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

fn instruction_retained_bytes(arg_count: usize, target_count: usize) -> DetectionResult<u64> {
    let item = u64::try_from(size_of::<CircuitItem>()).map_err(|_| storage_overflow())?;
    let args = byte_product(arg_count, size_of::<f64>())?;
    let targets = byte_product(target_count, size_of::<Target>())?;
    checked_add_bytes(checked_add_bytes(item, args)?, targets)
}

fn checked_add_storage(
    left: FrameExecutionStorage,
    right: FrameExecutionStorage,
) -> DetectionResult<FrameExecutionStorage> {
    Ok(FrameExecutionStorage {
        root_items: left
            .root_items
            .checked_add(right.root_items)
            .ok_or_else(storage_overflow)?,
        retained_bytes: checked_add_bytes(left.retained_bytes, right.retained_bytes)?,
    })
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
        "direct detector-frame retained byte count overflowed",
    )
}

fn admit_combined_compiled_storage(
    conversion_bytes: u64,
    execution_bytes: u64,
    limit_bytes: u64,
) -> DetectionResult<u64> {
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
) -> DetectionResult<()> {
    result.try_reserve_exact(root_item_capacity)?;
    append_frame_execution_items(circuit, result)
}

fn append_frame_execution_items(
    circuit: &Circuit,
    result: &mut CircuitAssembler,
) -> DetectionResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction)
                if matches!(instruction.gate().canonical_name(), "SPP" | "SPP_DAG") =>
            {
                let decomposed = decomposed_frame_instruction(instruction)?;
                append_frame_execution_items(&decomposed, result)?;
            }
            CircuitItem::Instruction(instruction) => {
                result.try_append_instruction(try_clone_execution_instruction(instruction)?)?;
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
) -> DetectionResult<CircuitInstruction> {
    let mut args = Vec::new();
    args.try_reserve_exact(instruction.args().len())
        .map_err(|error| {
            DetectionError::invalid_sampler_compilation(format!(
                "unable to reserve {} direct-frame argument slots: {error}",
                instruction.args().len()
            ))
        })?;
    args.extend_from_slice(instruction.args());
    let mut targets = Vec::new();
    targets
        .try_reserve_exact(instruction.targets().len())
        .map_err(|error| {
            DetectionError::invalid_sampler_compilation(format!(
                "unable to reserve {} direct-frame target slots: {error}",
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
