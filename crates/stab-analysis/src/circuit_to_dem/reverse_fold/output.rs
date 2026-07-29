use std::collections::BTreeSet;

use stab_model::{DemInstruction, DemInstructionKind, DemItem, DemTarget, DetectorErrorModel};

use crate::{AnalysisError, AnalysisResult};

pub(super) fn unreverse_model(
    reversed: &DetectorErrorModel,
    base_detector_id: &mut u64,
    seen: &mut BTreeSet<DemTarget>,
) -> AnalysisResult<DetectorErrorModel> {
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
                output.push_repeat_block(stab_model::advanced::dem_repeat_block_with_tag_bytes(
                    repeat.repeat_count(),
                    body,
                    repeat.tag_bytes(),
                ));
                let one_body_shift = base_detector_id.checked_sub(old_base).ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(
                        "folded analyzer repeat detector shift underflowed",
                    )
                })?;
                let extra_repetitions =
                    repeat.repeat_count().get().checked_sub(1).ok_or_else(|| {
                        AnalysisError::invalid_detector_error_model(
                            "folded analyzer repeat count underflowed",
                        )
                    })?;
                *base_detector_id = base_detector_id
                    .checked_add(one_body_shift.checked_mul(extra_repetitions).ok_or_else(
                        || {
                            AnalysisError::invalid_detector_error_model(
                                "folded analyzer repeat detector shift overflowed",
                            )
                        },
                    )?)
                    .ok_or_else(|| {
                        AnalysisError::invalid_detector_error_model(
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
) -> AnalysisResult<()> {
    match instruction.kind() {
        DemInstructionKind::ShiftDetectors => {
            let detector_shift = stab_model::advanced::dem_instruction_detector_shift(instruction)?;
            *base_detector_id = base_detector_id
                .checked_add(detector_shift)
                .ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(
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
            if !instruction.args().is_empty()
                || instruction.tag_bytes().is_some()
                || !target_is_seen
            {
                output.push_instruction(rebased_instruction(instruction, *base_detector_id)?);
            }
        }
    }
    Ok(())
}

fn rebased_instruction(
    instruction: &DemInstruction,
    base_detector_id: u64,
) -> AnalysisResult<DemInstruction> {
    let targets = instruction
        .targets()
        .iter()
        .map(|target| match *target {
            DemTarget::RelativeDetector(detector) => {
                let detector = detector
                    .get()
                    .checked_sub(base_detector_id)
                    .ok_or_else(|| {
                        AnalysisError::invalid_detector_error_model(format!(
                            "folded analyzer detector D{} precedes base D{base_detector_id}",
                            detector.get()
                        ))
                    })?;
                DemTarget::relative_detector(detector).map_err(Into::into)
            }
            DemTarget::LogicalObservable(_) | DemTarget::Separator | DemTarget::Numeric(_) => {
                Ok(*target)
            }
        })
        .collect::<AnalysisResult<Vec<_>>>()?;
    stab_model::advanced::dem_instruction_with_tag_bytes(
        instruction.kind(),
        instruction.args().to_vec(),
        targets,
        instruction.tag_bytes(),
    )
    .map_err(Into::into)
}
