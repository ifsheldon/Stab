use std::collections::HashSet;

use stab_model::advanced::dem_instruction_rebase_detectors;
use stab_model::{DemInstruction, DemInstructionView, DemItem, DemTarget, DetectorErrorModel};

use crate::AnalysisOperation;
use crate::{AnalysisError, AnalysisResult};

/// Error symptoms already emitted.
pub(super) type SeenTargets = HashSet<DemTarget>;

/// Restores forward order to a model built in reverse, rebasing detector ids in place.
///
/// The reversed model is consumed: items are reversed and filtered without copying their
/// targets or tags.
pub(super) fn unreverse_model(
    model: &mut DetectorErrorModel,
    base_detector_id: &mut u64,
    seen: &mut SeenTargets,
) -> AnalysisResult<()> {
    model.reverse_items();
    model.try_retain_items_mut(|item| match item {
        DemItem::Instruction(instruction) => {
            unreverse_instruction(instruction, base_detector_id, seen)
        }
        DemItem::RepeatBlock(repeat) => {
            let repeat_count = repeat.repeat_count().get();
            if repeat_count != 0 {
                let old_base = *base_detector_id;
                unreverse_model(repeat.body_mut(), base_detector_id, seen)?;
                let one_body_shift = base_detector_id.checked_sub(old_base).ok_or_else(|| {
                    AnalysisError::internal(
                        AnalysisOperation::CircuitToDetectorErrorModel,
                        "folded analyzer repeat detector shift underflowed",
                    )
                })?;
                let extra_repetitions = repeat_count.checked_sub(1).ok_or_else(|| {
                    AnalysisError::internal(
                        AnalysisOperation::CircuitToDetectorErrorModel,
                        "folded analyzer repeat count underflowed",
                    )
                })?;
                *base_detector_id = base_detector_id
                    .checked_add(one_body_shift.checked_mul(extra_repetitions).ok_or_else(
                        || {
                            AnalysisError::count_overflow(
                                AnalysisOperation::CircuitToDetectorErrorModel,
                                "folded analyzer repeat detector shift overflowed",
                            )
                        },
                    )?)
                    .ok_or_else(|| {
                        AnalysisError::count_overflow(
                            AnalysisOperation::CircuitToDetectorErrorModel,
                            "folded analyzer detector base overflowed",
                        )
                    })?;
            }
            Ok(true)
        }
    })
}

/// Rebases one instruction and reports whether the forward model keeps it.
fn unreverse_instruction(
    instruction: &mut DemInstruction,
    base_detector_id: &mut u64,
    seen: &mut SeenTargets,
) -> AnalysisResult<bool> {
    match instruction.view() {
        DemInstructionView::ShiftDetectors { detector_shift, .. } => {
            *base_detector_id = base_detector_id
                .checked_add(detector_shift)
                .ok_or_else(|| {
                    AnalysisError::count_overflow(
                        AnalysisOperation::CircuitToDetectorErrorModel,
                        "folded analyzer detector base overflowed",
                    )
                })?;
            Ok(true)
        }
        DemInstructionView::Error { targets, .. } => {
            seen.extend(targets.iter().copied());
            rebase(instruction, *base_detector_id)?;
            Ok(true)
        }
        DemInstructionView::Detector { .. } | DemInstructionView::LogicalObservable { .. } => {
            let target_is_seen = instruction
                .targets()
                .first()
                .is_some_and(|target| seen.contains(target));
            let keep = !instruction.args().is_empty()
                || instruction.tag_bytes().is_some()
                || !target_is_seen;
            if keep {
                rebase(instruction, *base_detector_id)?;
            }
            Ok(keep)
        }
    }
}

fn rebase(instruction: &mut DemInstruction, base_detector_id: u64) -> AnalysisResult<()> {
    dem_instruction_rebase_detectors(instruction, base_detector_id).map_err(|detector| {
        AnalysisError::internal(
            AnalysisOperation::CircuitToDetectorErrorModel,
            format!("folded analyzer detector D{detector} precedes base D{base_detector_id}"),
        )
    })
}
