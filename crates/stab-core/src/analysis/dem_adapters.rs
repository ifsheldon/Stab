use crate::{
    CircuitResult, DemInstruction, DemInstructionKind, DemItem, DemRepeatBlock, DetectorErrorModel,
};

/// Returns a compact copy of `model` with every instruction and repeat-block tag removed.
///
/// Item order, instruction arguments and targets, repeat counts, and repeat nesting are preserved.
/// The source model is not mutated. This transform walks the compact model directly instead of
/// flattening repeats, so its work and output allocation scale with the folded input structure.
pub fn detector_error_model_without_tags(model: &DetectorErrorModel) -> DetectorErrorModel {
    let mut transformed = DetectorErrorModel::new();
    for item in model.iter_items() {
        match item {
            DemItem::Instruction(instruction) => {
                let mut instruction = instruction.clone();
                instruction.clear_tag();
                transformed.push_instruction(instruction);
            }
            DemItem::RepeatBlock(repeat) => {
                transformed.push_repeat_block(DemRepeatBlock::new(
                    repeat.repeat_count(),
                    detector_error_model_without_tags(repeat.body()),
                    None,
                ));
            }
        }
    }
    transformed
}

/// Returns a materialized DEM with repeat blocks expanded and detector shifts applied.
///
/// `shift_detectors` instructions are removed after their detector and coordinate shifts are
/// applied to following instructions. Instruction tags are preserved, while repeat-block tags
/// disappear with the expanded repeat blocks. The transform validates the existing Stab
/// flattening budget before materialization and returns the same overflow or resource-limit errors
/// as [`DetectorErrorModel::flattened`].
pub fn flattened_detector_error_model(
    model: &DetectorErrorModel,
) -> CircuitResult<DetectorErrorModel> {
    model.validate_flattening_budget("flattened")?;
    let mut flattened = DetectorErrorModel::new();
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
    let mut transformed = DetectorErrorModel::new();
    for item in model.iter_items() {
        match item {
            DemItem::Instruction(instruction) => {
                transformed.push_instruction(rounded_instruction(instruction, digits)?);
            }
            DemItem::RepeatBlock(repeat) => {
                transformed.push_repeat_block(DemRepeatBlock::new(
                    repeat.repeat_count(),
                    rounded_detector_error_model(repeat.body(), digits)?,
                    repeat.tag().map(ToOwned::to_owned),
                ));
            }
        }
    }
    Ok(transformed)
}

// Temporary pre-0.2 method adapters. The free analysis functions own these transforms; the
// adapters preserve source compatibility until the model and analysis crates are extracted.
impl DetectorErrorModel {
    /// Returns a compact copy with every instruction and repeat-block tag removed.
    ///
    /// Item order, semantic instruction data, repeat counts, and repeat nesting are preserved, and
    /// the source model is not mutated. This compatibility method delegates to
    /// [`detector_error_model_without_tags`].
    pub fn without_tags(&self) -> Self {
        detector_error_model_without_tags(self)
    }

    /// Returns a materialized DEM with repeat blocks expanded and detector shifts applied.
    ///
    /// Instruction tags are preserved, repeat-block tags are removed with their blocks, and
    /// excessive expansion or arithmetic overflow is rejected before returning a partial model.
    /// This compatibility method delegates to [`flattened_detector_error_model`].
    pub fn flattened(&self) -> CircuitResult<Self> {
        flattened_detector_error_model(self)
    }

    /// Returns a compact copy with error probabilities rounded to `digits` decimal places.
    ///
    /// Non-error arguments, tags, targets, separators, repeat counts, and repeat structure are
    /// preserved. Errors rounded to zero remain in the result. This compatibility method delegates
    /// to [`rounded_detector_error_model`].
    pub fn rounded(&self, digits: u8) -> CircuitResult<Self> {
        rounded_detector_error_model(self, digits)
    }
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
    DemInstruction::new(
        instruction.kind(),
        args,
        instruction.targets().to_vec(),
        instruction.tag().map(ToOwned::to_owned),
    )
}

fn rounded_probability_arg(value: f64, digits: u8) -> f64 {
    let mut scale = 1.0;
    for _ in 0..digits {
        scale *= 10.0;
    }
    (value * scale).round() / scale
}
