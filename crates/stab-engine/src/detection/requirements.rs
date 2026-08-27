use stab_model::{Circuit, CircuitItem, RepeatNestingLimit, Target};

use super::error::{
    DetectionError, DetectionResourceLimitError as ResourceLimitError, DetectionResult,
};

pub(super) fn circuit_requires_detector_frame(circuit: &Circuit) -> DetectionResult<bool> {
    let mut requires_detector_frame = false;
    let mut stack = vec![(circuit, 0_usize)];
    while let Some((current, depth)) = stack.pop() {
        for item in current.items() {
            match item {
                CircuitItem::Instruction(instruction)
                    if matches!(
                        instruction.gate().canonical_name(),
                        "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1"
                    ) =>
                {
                    requires_detector_frame = true;
                }
                CircuitItem::Instruction(instruction)
                    if instruction.gate().canonical_name() == "OBSERVABLE_INCLUDE"
                        && instruction.targets().iter().any(Target::is_pauli_target) =>
                {
                    requires_detector_frame = true;
                }
                CircuitItem::Instruction(_) => {}
                CircuitItem::RepeatBlock(repeat) => {
                    let next_depth = depth.checked_add(1).ok_or_else(|| {
                        DetectionError::invalid_sampler_compilation(
                            "detection conversion repeat nesting overflowed",
                        )
                    })?;
                    if next_depth > RepeatNestingLimit::HARD_MAX {
                        return Err(ResourceLimitError::detection_repeat_nesting(
                            next_depth,
                            RepeatNestingLimit::HARD_MAX,
                        )
                        .into());
                    }
                    stack.push((repeat.body(), next_depth));
                }
            }
        }
    }
    Ok(requires_detector_frame)
}
