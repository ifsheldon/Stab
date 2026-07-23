use std::collections::BTreeSet;

use crate::{
    CircuitError, CircuitResult, DemInstruction, DemInstructionKind, DemItem, DemRepeatBlock,
    DemTarget, DetectorErrorModel,
};

pub(super) fn unreverse_model(
    reversed: &DetectorErrorModel,
    base_detector_id: &mut u64,
    seen: &mut BTreeSet<DemTarget>,
) -> CircuitResult<DetectorErrorModel> {
    let mut output = DetectorErrorModel::new();
    for item in reversed.items().iter().rev() {
        match item {
            DemItem::Instruction(instruction) => {
                unreverse_instruction(instruction, base_detector_id, seen, &mut output)?;
            }
            DemItem::RepeatBlock(repeat) => {
                if repeat.repeat_count().get() == 0 {
                    output.push_repeat_block(repeat.clone());
                    continue;
                }
                let old_base = *base_detector_id;
                let body = unreverse_model(repeat.body(), base_detector_id, seen)?;
                output.push_repeat_block(DemRepeatBlock::new(
                    repeat.repeat_count(),
                    body,
                    repeat.tag().map(ToOwned::to_owned),
                ));
                let one_body_shift = base_detector_id.checked_sub(old_base).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "folded analyzer repeat detector shift underflowed",
                    )
                })?;
                let extra_repetitions =
                    repeat.repeat_count().get().checked_sub(1).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "folded analyzer repeat count underflowed",
                        )
                    })?;
                *base_detector_id = base_detector_id
                    .checked_add(one_body_shift.checked_mul(extra_repetitions).ok_or_else(
                        || {
                            CircuitError::invalid_detector_error_model(
                                "folded analyzer repeat detector shift overflowed",
                            )
                        },
                    )?)
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "folded analyzer detector base overflowed",
                        )
                    })?;
            }
        }
    }
    Ok(output)
}

fn unreverse_instruction(
    instruction: &DemInstruction,
    base_detector_id: &mut u64,
    seen: &mut BTreeSet<DemTarget>,
    output: &mut DetectorErrorModel,
) -> CircuitResult<()> {
    match instruction.kind() {
        DemInstructionKind::ShiftDetectors => {
            let detector_shift = instruction.detector_shift()?;
            *base_detector_id = base_detector_id
                .checked_add(detector_shift)
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "folded analyzer detector base overflowed",
                    )
                })?;
            output.push_instruction(instruction.clone());
        }
        DemInstructionKind::Error => {
            seen.extend(instruction.targets().iter().copied());
            output.push_instruction(rebased_instruction(instruction, *base_detector_id)?);
        }
        DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => {
            let target_is_seen = instruction
                .targets()
                .first()
                .is_some_and(|target| seen.contains(target));
            if !instruction.args().is_empty() || instruction.tag().is_some() || !target_is_seen {
                output.push_instruction(rebased_instruction(instruction, *base_detector_id)?);
            }
        }
    }
    Ok(())
}

fn rebased_instruction(
    instruction: &DemInstruction,
    base_detector_id: u64,
) -> CircuitResult<DemInstruction> {
    let targets = instruction
        .targets()
        .iter()
        .map(|target| match *target {
            DemTarget::RelativeDetector(detector) => {
                let detector = detector
                    .get()
                    .checked_sub(base_detector_id)
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(format!(
                            "folded analyzer detector D{} precedes base D{base_detector_id}",
                            detector.get()
                        ))
                    })?;
                DemTarget::relative_detector(detector)
            }
            DemTarget::LogicalObservable(_) | DemTarget::Separator | DemTarget::Numeric(_) => {
                Ok(*target)
            }
        })
        .collect::<CircuitResult<Vec<_>>>()?;
    DemInstruction::new(
        instruction.kind(),
        instruction.args().to_vec(),
        targets,
        instruction.tag().map(ToOwned::to_owned),
    )
}
