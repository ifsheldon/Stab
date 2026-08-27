use stab_model::{Circuit, CircuitItem, RepeatNestingLimit, Target};

use arrayvec::ArrayVec;

use super::error::{DetectionResourceLimitError as ResourceLimitError, DetectionResult};

pub(super) fn circuit_requires_detector_frame(circuit: &Circuit) -> DetectionResult<bool> {
    let mut stack =
        ArrayVec::<std::slice::Iter<'_, CircuitItem>, { RepeatNestingLimit::HARD_MAX + 1 }>::new();
    stack.push(circuit.items().iter());
    loop {
        match stack.last_mut().and_then(Iterator::next) {
            Some(CircuitItem::Instruction(instruction))
                if instruction.gate().canonical_name() == "OBSERVABLE_INCLUDE"
                    && instruction.targets().iter().any(Target::is_pauli_target) =>
            {
                return Ok(true);
            }
            Some(CircuitItem::Instruction(_)) => {}
            Some(CircuitItem::RepeatBlock(repeat)) => {
                let next_depth = stack.len();
                if next_depth > RepeatNestingLimit::HARD_MAX {
                    return Err(ResourceLimitError::detection_repeat_nesting(
                        next_depth,
                        RepeatNestingLimit::HARD_MAX,
                    )
                    .into());
                }
                stack.try_push(repeat.body().items().iter()).map_err(|_| {
                    ResourceLimitError::detection_repeat_nesting(
                        next_depth,
                        RepeatNestingLimit::HARD_MAX,
                    )
                })?;
            }
            None => {
                stack.pop();
                if stack.is_empty() {
                    return Ok(false);
                }
            }
        }
    }
}
