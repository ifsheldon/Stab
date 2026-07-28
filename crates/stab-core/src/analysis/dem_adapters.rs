use std::convert::Infallible;

use crate::dem::DemFlattenLimits;
use crate::{
    CircuitResult, DemInstruction, DemInstructionKind, DemItem, DemRepeatBlock, DetectorErrorModel,
};

/// Returns a compact copy of `model` with every instruction and repeat-block tag removed.
///
/// Item order, instruction arguments and targets, repeat counts, and repeat nesting are preserved.
/// The source model is not mutated. This transform walks the compact model directly instead of
/// flattening repeats, so its work and output allocation scale with the folded input structure.
pub fn detector_error_model_without_tags(model: &DetectorErrorModel) -> DetectorErrorModel {
    let transformed: Result<DetectorErrorModel, Infallible> =
        transform_compact_dem(model, false, |instruction| {
            let mut instruction = instruction.clone();
            instruction.clear_tag();
            Ok(instruction)
        });
    match transformed {
        Ok(transformed) => transformed,
        Err(never) => match never {},
    }
}

/// Returns a materialized DEM with repeat blocks expanded and detector shifts applied.
///
/// `shift_detectors` instructions are removed after their detector and coordinate shifts are
/// applied to following instructions. Instruction tags are preserved, while repeat-block tags
/// disappear with the expanded repeat blocks. The transform validates the existing Stab
/// flattening budget before materialization and returns the same overflow or resource-limit errors
/// as [`flattened_detector_error_model`].
pub fn flattened_detector_error_model(
    model: &DetectorErrorModel,
) -> CircuitResult<DetectorErrorModel> {
    flattened_detector_error_model_with_limits(model, DemFlattenLimits::default())
}

/// Returns a materialized DEM under the given repeat-expansion resource policy.
///
/// Resource admission completes before the output model is created or mutated. Repeat nesting
/// remains subject to Stab's fixed model-safety invariant and is not configurable through
/// `limits`.
pub fn flattened_detector_error_model_with_limits(
    model: &DetectorErrorModel,
    limits: DemFlattenLimits,
) -> CircuitResult<DetectorErrorModel> {
    let budget = model.validate_flattening_budget_with_limits("flattened", limits)?;
    let mut flattened = DetectorErrorModel::new();
    flattened.try_reserve_items_exact(budget.materialized_capacity()?)?;
    for instruction in model.iter_flattened_instructions() {
        flattened.push_instruction(instruction?);
    }
    Ok(flattened)
}

/// Returns a compact copy of `model` with error probabilities rounded to `digits` decimal places.
///
/// Only the probability argument of `error` instructions is rounded. Tags, target order,
/// separators, detector and coordinate declarations, shifts, repeat counts, and repeat structure
/// are preserved. Errors rounded to zero remain present, matching Stim v1.16.0 behavior.
pub fn rounded_detector_error_model(
    model: &DetectorErrorModel,
    digits: u8,
) -> CircuitResult<DetectorErrorModel> {
    transform_compact_dem(model, true, |instruction| {
        rounded_instruction(instruction, digits)
    })
}

fn rounded_instruction(instruction: &DemInstruction, digits: u8) -> CircuitResult<DemInstruction> {
    if instruction.kind() != DemInstructionKind::Error {
        return Ok(instruction.clone());
    }
    let args = instruction
        .args()
        .iter()
        .map(|arg| rounded_probability_arg(*arg, digits))
        .collect::<Vec<_>>();
    DemInstruction::new_with_tag_bytes(
        instruction.kind(),
        args,
        instruction.targets().to_vec(),
        instruction.tag_bytes(),
    )
}

struct TransformFrame<'a> {
    source: &'a DetectorErrorModel,
    index: usize,
    output: DetectorErrorModel,
    owning_repeat: Option<&'a DemRepeatBlock>,
}

fn transform_compact_dem<E>(
    model: &DetectorErrorModel,
    preserve_repeat_tags: bool,
    mut transform_instruction: impl FnMut(&DemInstruction) -> Result<DemInstruction, E>,
) -> Result<DetectorErrorModel, E> {
    let mut stack = vec![TransformFrame {
        source: model,
        index: 0,
        output: DetectorErrorModel::new(),
        owning_repeat: None,
    }];
    loop {
        let Some(frame) = stack.last_mut() else {
            unreachable!("the root DEM transform frame remains until completion");
        };
        if let Some(item) = frame.source.items().get(frame.index) {
            frame.index += 1;
            match item {
                DemItem::Instruction(instruction) => {
                    frame
                        .output
                        .push_instruction(transform_instruction(instruction)?);
                }
                DemItem::RepeatBlock(repeat) => stack.push(TransformFrame {
                    source: repeat.body(),
                    index: 0,
                    output: DetectorErrorModel::new(),
                    owning_repeat: Some(repeat),
                }),
            }
            continue;
        }

        let Some(completed) = stack.pop() else {
            unreachable!("a completed DEM transform frame is available");
        };
        let Some(repeat) = completed.owning_repeat else {
            return Ok(completed.output);
        };
        let tag = preserve_repeat_tags.then(|| repeat.tag_bytes()).flatten();
        let transformed_repeat =
            DemRepeatBlock::new_with_tag_bytes(repeat.repeat_count(), completed.output, tag);
        let Some(parent) = stack.last_mut() else {
            unreachable!("a repeated DEM transform body has a parent frame");
        };
        parent.output.push_repeat_block(transformed_repeat);
    }
}

fn rounded_probability_arg(value: f64, digits: u8) -> f64 {
    let mut scale = 1.0;
    for _ in 0..digits {
        scale *= 10.0;
    }
    (value * scale).round() / scale
}
