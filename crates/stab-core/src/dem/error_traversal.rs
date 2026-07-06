use super::{
    DemFlatteningBudget, DemInstructionKind, DemItem, DemTarget, DetectorErrorModel,
    MAX_DEM_FLATTEN_REPEAT_UNROLL, MAX_DEM_REPEAT_NESTING,
};
use crate::{CircuitError, CircuitResult};

impl DetectorErrorModel {
    pub(crate) fn validate_search_graph_error_traversal_budget(
        &self,
        context: &'static str,
    ) -> CircuitResult<()> {
        let mut budget = DemFlatteningBudget::default();
        self.validate_search_graph_error_traversal_budget_items(1, 0, context, &mut budget)
    }

    pub(crate) fn search_graph_nonzero_error_target_counts(
        &self,
        context: &'static str,
    ) -> CircuitResult<(u64, u64)> {
        let mut counts = DemErrorTargetCounts::default();
        self.collect_search_graph_nonzero_error_target_counts_from(0, 0, context, &mut counts)?;
        Ok((counts.detector_count, counts.observable_count))
    }

    pub(crate) fn selected_search_graph_flat_repeat_error_count(
        &self,
    ) -> CircuitResult<Option<u64>> {
        if self.items.is_empty() {
            return Ok(None);
        }
        let mut count = 0_u64;
        for item in &self.items {
            let DemItem::Instruction(instruction) = item else {
                return Ok(None);
            };
            match instruction.kind() {
                DemInstructionKind::Error => {
                    let probability = instruction.args().first().copied().unwrap_or(0.0);
                    if probability == 0.0 {
                        return Ok(None);
                    }
                    let mut has_any_target = false;
                    let mut has_search_target = false;
                    for target in instruction.targets() {
                        has_any_target = true;
                        match target {
                            DemTarget::RelativeDetector(_) | DemTarget::LogicalObservable(_) => {
                                has_search_target = true;
                            }
                            DemTarget::Numeric(_) => return Ok(None),
                            DemTarget::Separator => {}
                        }
                    }
                    if !has_search_target && has_any_target {
                        return Ok(None);
                    }
                    if has_search_target {
                        count = count.checked_add(1).ok_or_else(|| {
                            CircuitError::invalid_detector_error_model(
                                "DEM search flat-repeat error count overflowed",
                            )
                        })?;
                    }
                }
                DemInstructionKind::ShiftDetectors if instruction.detector_shift()? == 0 => {}
                DemInstructionKind::ShiftDetectors
                | DemInstructionKind::Detector
                | DemInstructionKind::LogicalObservable => {
                    return Ok(None);
                }
            }
        }
        Ok(Some(count))
    }

    pub(crate) fn has_nonzero_probability_error(
        &self,
        context: &'static str,
    ) -> CircuitResult<bool> {
        self.has_nonzero_probability_error_inner(0, context)
    }

    fn validate_search_graph_error_traversal_budget_items(
        &self,
        multiplier: u64,
        depth: usize,
        context: &'static str,
        budget: &mut DemFlatteningBudget,
    ) -> CircuitResult<()> {
        if depth > MAX_DEM_REPEAT_NESTING {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM {context} repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
            )));
        }
        for item in &self.items {
            match item {
                DemItem::Instruction(_) => {
                    budget.add_expanded_instructions(multiplier, context)?;
                }
                DemItem::RepeatBlock(repeat) => {
                    if let Some(error_count) = repeat
                        .body()
                        .selected_search_graph_flat_repeat_error_count()?
                    {
                        let folded_count =
                            multiplier.checked_mul(error_count).ok_or_else(|| {
                                CircuitError::invalid_detector_error_model(format!(
                                    "DEM {context} folded repeat error count overflowed"
                                ))
                            })?;
                        budget.add_expanded_instructions(folded_count, context)?;
                        continue;
                    }
                    if !repeat
                        .body()
                        .has_nonzero_probability_error_inner(depth + 1, context)?
                    {
                        continue;
                    }
                    let repeat_count = repeat.repeat_count().get();
                    if repeat_count > MAX_DEM_FLATTEN_REPEAT_UNROLL {
                        return Err(CircuitError::invalid_detector_error_model(format!(
                            "DEM {context} currently supports repeat counts up to {MAX_DEM_FLATTEN_REPEAT_UNROLL}, got {repeat_count}"
                        )));
                    }
                    let repeated_multiplier =
                        multiplier.checked_mul(repeat_count).ok_or_else(|| {
                            CircuitError::invalid_detector_error_model(format!(
                                "DEM {context} repeat expansion count overflowed"
                            ))
                        })?;
                    budget.add_repeat_iterations(repeated_multiplier, context)?;
                    repeat
                        .body()
                        .validate_search_graph_error_traversal_budget_items(
                            repeated_multiplier,
                            depth + 1,
                            context,
                            budget,
                        )?;
                }
            }
        }
        Ok(())
    }

    fn has_nonzero_probability_error_inner(
        &self,
        depth: usize,
        context: &'static str,
    ) -> CircuitResult<bool> {
        if depth > MAX_DEM_REPEAT_NESTING {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM {context} repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
            )));
        }
        for item in &self.items {
            match item {
                DemItem::Instruction(instruction)
                    if instruction.kind() == DemInstructionKind::Error
                        && instruction.args().first().copied().unwrap_or(0.0) != 0.0 =>
                {
                    return Ok(true);
                }
                DemItem::RepeatBlock(repeat) => {
                    if repeat
                        .body()
                        .has_nonzero_probability_error_inner(depth + 1, context)?
                    {
                        return Ok(true);
                    }
                }
                DemItem::Instruction(_) => {}
            }
        }
        Ok(false)
    }

    fn collect_search_graph_nonzero_error_target_counts_from(
        &self,
        mut detector_offset: u64,
        depth: usize,
        context: &'static str,
        counts: &mut DemErrorTargetCounts,
    ) -> CircuitResult<u64> {
        if depth > MAX_DEM_REPEAT_NESTING {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM {context} repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
            )));
        }
        for item in &self.items {
            match item {
                DemItem::Instruction(instruction) => match instruction.kind() {
                    DemInstructionKind::Error => {
                        if instruction.args().first().copied().unwrap_or(0.0) != 0.0 {
                            counts.include_error_targets(instruction.targets(), detector_offset)?;
                        }
                    }
                    DemInstructionKind::ShiftDetectors => {
                        detector_offset = detector_offset
                            .checked_add(instruction.detector_shift()?)
                            .ok_or_else(|| {
                                CircuitError::invalid_detector_error_model(format!(
                                    "DEM {context} detector offset overflowed"
                                ))
                            })?;
                    }
                    DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => {}
                },
                DemItem::RepeatBlock(repeat) => {
                    if repeat
                        .body()
                        .selected_search_graph_flat_repeat_error_count()?
                        .is_some()
                    {
                        detector_offset = repeat
                            .body()
                            .collect_search_graph_nonzero_error_target_counts_from(
                                detector_offset,
                                depth + 1,
                                context,
                                counts,
                            )?;
                        continue;
                    }
                    let body_shift = repeat.body().total_detector_shift()?;
                    let repeat_count = repeat.repeat_count().get();
                    if !repeat
                        .body()
                        .has_nonzero_probability_error_inner(depth + 1, context)?
                    {
                        detector_offset = body_shift
                            .checked_mul(repeat_count)
                            .and_then(|shift| detector_offset.checked_add(shift))
                            .ok_or_else(|| {
                                CircuitError::invalid_detector_error_model(format!(
                                    "DEM {context} repeat detector offset overflowed"
                                ))
                            })?;
                        continue;
                    }
                    if repeat_count > MAX_DEM_FLATTEN_REPEAT_UNROLL {
                        return Err(CircuitError::invalid_detector_error_model(format!(
                            "DEM {context} currently supports repeat counts up to {MAX_DEM_FLATTEN_REPEAT_UNROLL}, got {repeat_count}"
                        )));
                    }
                    for _ in 0..repeat_count {
                        detector_offset = repeat
                            .body()
                            .collect_search_graph_nonzero_error_target_counts_from(
                                detector_offset,
                                depth + 1,
                                context,
                                counts,
                            )?;
                    }
                }
            }
        }
        Ok(detector_offset)
    }
}

#[derive(Clone, Debug, Default)]
struct DemErrorTargetCounts {
    detector_count: u64,
    observable_count: u64,
}

impl DemErrorTargetCounts {
    fn include_error_targets(
        &mut self,
        targets: &[DemTarget],
        detector_offset: u64,
    ) -> CircuitResult<()> {
        for target in targets {
            match *target {
                DemTarget::RelativeDetector(id) => {
                    let detector_id = detector_offset.checked_add(id.get()).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "DEM nonzero-error detector target overflowed",
                        )
                    })?;
                    self.detector_count =
                        self.detector_count
                            .max(detector_id.checked_add(1).ok_or_else(|| {
                                CircuitError::invalid_detector_error_model(
                                    "DEM nonzero-error detector count overflowed",
                                )
                            })?);
                }
                DemTarget::LogicalObservable(id) => {
                    self.observable_count =
                        self.observable_count
                            .max(id.get().checked_add(1).ok_or_else(|| {
                                CircuitError::invalid_detector_error_model(
                                    "DEM nonzero-error observable count overflowed",
                                )
                            })?);
                }
                DemTarget::Separator | DemTarget::Numeric(_) => {}
            }
        }
        Ok(())
    }
}
