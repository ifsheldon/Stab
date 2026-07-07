use std::collections::BTreeSet;

use crate::dem::{
    MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS, MAX_DEM_FLATTEN_REPEAT_ITERATIONS,
    MAX_DEM_FLATTEN_REPEAT_UNROLL, MAX_DEM_REPEAT_NESTING,
};
use crate::{
    CircuitError, CircuitResult, DemInstructionKind, DemItem, DemTarget, DetectorErrorModel,
};

pub(super) fn error_keys_from_dem(
    model: &DetectorErrorModel,
) -> CircuitResult<Vec<Vec<DemTarget>>> {
    validate_error_matcher_filter_budget(model)?;
    let mut keys = Vec::new();
    collect_error_keys_from_dem(model, 0, &mut keys)?;
    Ok(keys)
}

#[derive(Debug, Default)]
struct ErrorMatcherFilterBudget {
    expanded_instructions: u64,
    repeat_iterations: u64,
}

impl ErrorMatcherFilterBudget {
    fn add_expanded_instructions(&mut self, count: u64) -> CircuitResult<()> {
        self.expanded_instructions =
            self.expanded_instructions
                .checked_add(count)
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "DEM ErrorMatcher filter expanded instruction count overflowed",
                    )
                })?;
        if self.expanded_instructions > MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM ErrorMatcher filter currently supports at most {MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS} expanded instructions, got at least {}",
                self.expanded_instructions
            )));
        }
        Ok(())
    }

    fn add_repeat_iterations(&mut self, count: u64) -> CircuitResult<()> {
        self.repeat_iterations = self.repeat_iterations.checked_add(count).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "DEM ErrorMatcher filter repeat iteration count overflowed",
            )
        })?;
        if self.repeat_iterations > MAX_DEM_FLATTEN_REPEAT_ITERATIONS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM ErrorMatcher filter currently supports at most {MAX_DEM_FLATTEN_REPEAT_ITERATIONS} expanded repeat iterations, got at least {}",
                self.repeat_iterations
            )));
        }
        Ok(())
    }
}

fn validate_error_matcher_filter_budget(model: &DetectorErrorModel) -> CircuitResult<()> {
    let mut budget = ErrorMatcherFilterBudget::default();
    validate_error_matcher_filter_budget_items(model, 1, 0, &mut budget)
}

fn validate_error_matcher_filter_budget_items(
    model: &DetectorErrorModel,
    multiplier: u64,
    depth: usize,
    budget: &mut ErrorMatcherFilterBudget,
) -> CircuitResult<()> {
    if depth > MAX_DEM_REPEAT_NESTING {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "DEM ErrorMatcher filter repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
        )));
    }
    for item in model.items() {
        match item {
            DemItem::Instruction(_) => budget.add_expanded_instructions(multiplier)?,
            DemItem::RepeatBlock(repeat) => {
                if let Some(error_count) =
                    selected_filter_compact_repeat_error_count(repeat.body())?
                {
                    let folded_count = multiplier.checked_mul(error_count).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "DEM ErrorMatcher filter folded repeat error count overflowed",
                        )
                    })?;
                    budget.add_expanded_instructions(folded_count)?;
                    continue;
                }
                let repeat_count = repeat.repeat_count().get();
                if repeat_count > MAX_DEM_FLATTEN_REPEAT_UNROLL {
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "DEM ErrorMatcher filter currently supports repeat counts up to {MAX_DEM_FLATTEN_REPEAT_UNROLL}, got {repeat_count}"
                    )));
                }
                let repeated_multiplier =
                    multiplier.checked_mul(repeat_count).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "DEM ErrorMatcher filter repeat expansion count overflowed",
                        )
                    })?;
                budget.add_repeat_iterations(repeated_multiplier)?;
                validate_error_matcher_filter_budget_items(
                    repeat.body(),
                    repeated_multiplier,
                    depth + 1,
                    budget,
                )?;
            }
        }
    }
    Ok(())
}

fn selected_filter_compact_repeat_error_count(
    model: &DetectorErrorModel,
) -> CircuitResult<Option<u64>> {
    selected_filter_compact_repeat_error_count_inner(model, 0)
}

fn selected_filter_compact_repeat_error_count_inner(
    model: &DetectorErrorModel,
    depth: usize,
) -> CircuitResult<Option<u64>> {
    if depth > MAX_DEM_REPEAT_NESTING {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "DEM ErrorMatcher filter compact-repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
        )));
    }
    if model.items().is_empty() {
        return Ok(None);
    }
    let mut count = 0_u64;
    for item in model.items() {
        match item {
            DemItem::Instruction(instruction) => match instruction.kind() {
                DemInstructionKind::Error => {
                    let mut has_filter_key_target = false;
                    for target in instruction.targets() {
                        match target {
                            DemTarget::RelativeDetector(_) | DemTarget::LogicalObservable(_) => {
                                has_filter_key_target = true;
                            }
                            DemTarget::Numeric(_) => return Ok(None),
                            DemTarget::Separator => {}
                        }
                    }
                    if !has_filter_key_target {
                        return Ok(None);
                    }
                    count = count.checked_add(1).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "DEM ErrorMatcher filter compact-repeat error count overflowed",
                        )
                    })?;
                }
                DemInstructionKind::ShiftDetectors if instruction.detector_shift()? == 0 => {}
                DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => {}
                DemInstructionKind::ShiftDetectors => return Ok(None),
            },
            DemItem::RepeatBlock(repeat) => {
                if repeat.body().total_detector_shift()? != 0 {
                    return Ok(None);
                }
                let Some(child_count) =
                    selected_filter_compact_repeat_error_count_inner(repeat.body(), depth + 1)?
                else {
                    return Ok(None);
                };
                count = count.checked_add(child_count).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "DEM ErrorMatcher filter compact-repeat error count overflowed",
                    )
                })?;
            }
        }
    }
    if count == 0 {
        Ok(None)
    } else {
        Ok(Some(count))
    }
}

fn collect_error_keys_from_dem(
    model: &DetectorErrorModel,
    detector_offset: u64,
    keys: &mut Vec<Vec<DemTarget>>,
) -> CircuitResult<()> {
    let mut current_detector_offset = detector_offset;
    for item in model.items() {
        match item {
            DemItem::Instruction(instruction) => match instruction.kind() {
                DemInstructionKind::Error => {
                    keys.push(canonical_error_key(
                        instruction.targets(),
                        current_detector_offset,
                    )?);
                }
                DemInstructionKind::ShiftDetectors => {
                    current_detector_offset = current_detector_offset
                        .checked_add(instruction.detector_shift()?)
                        .ok_or_else(|| {
                            CircuitError::invalid_detector_error_model("detector shift overflowed")
                        })?;
                }
                DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => {}
            },
            DemItem::RepeatBlock(repeat) => {
                if selected_filter_compact_repeat_error_count(repeat.body())?.is_some() {
                    collect_error_keys_from_dem(repeat.body(), current_detector_offset, keys)?;
                    continue;
                }
                let body_shift = repeat.body().total_detector_shift()?;
                for _ in 0..repeat.repeat_count().get() {
                    collect_error_keys_from_dem(repeat.body(), current_detector_offset, keys)?;
                    current_detector_offset = current_detector_offset
                        .checked_add(body_shift)
                        .ok_or_else(|| {
                            CircuitError::invalid_detector_error_model(
                                "repeat detector shift overflowed",
                            )
                        })?;
                }
            }
        }
    }
    Ok(())
}

fn canonical_error_key(
    targets: &[DemTarget],
    detector_offset: u64,
) -> CircuitResult<Vec<DemTarget>> {
    let mut toggled = BTreeSet::new();
    for target in targets {
        let shifted = match *target {
            DemTarget::RelativeDetector(detector) => DemTarget::relative_detector(
                detector.get().checked_add(detector_offset).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("detector id overflowed")
                })?,
            )?,
            DemTarget::LogicalObservable(_) => *target,
            DemTarget::Separator | DemTarget::Numeric(_) => continue,
        };
        if !toggled.insert(shifted) {
            toggled.remove(&shifted);
        }
    }
    Ok(toggled.into_iter().collect())
}
