use std::collections::BTreeSet;

use super::{
    DemDetectorId, DemFlatteningBudget, DemInstructionKind, DemItem, DemTarget, DetectorErrorModel,
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

    pub(in crate::dem) fn search_graph_nonzero_error_targets(
        &self,
        context: &'static str,
        policy: SearchGraphTargetPolicy,
        max_detector_nodes: usize,
    ) -> CircuitResult<BTreeSet<DemDetectorId>> {
        let mut counts = DemErrorTargetCounts::new(max_detector_nodes);
        self.collect_search_graph_nonzero_error_target_counts_from(
            0,
            0,
            context,
            policy,
            &mut counts,
        )?;
        Ok(counts.detectors)
    }

    pub(crate) fn selected_search_graph_compact_repeat_error_count(
        &self,
    ) -> CircuitResult<Option<u64>> {
        self.selected_search_graph_compact_repeat_error_count_inner(0)
    }

    fn selected_search_graph_compact_repeat_error_count_inner(
        &self,
        depth: usize,
    ) -> CircuitResult<Option<u64>> {
        if depth > MAX_DEM_REPEAT_NESTING {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM search compact repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
            )));
        }
        if self.items.is_empty() {
            return Ok(None);
        }
        let mut count = 0_u64;
        for item in &self.items {
            match item {
                DemItem::Instruction(instruction) => match instruction.kind() {
                    DemInstructionKind::Error => {
                        let probability = instruction.args().first().copied().unwrap_or(0.0);
                        if probability == 0.0 {
                            continue;
                        }
                        let mut has_any_target = false;
                        let mut has_search_target = false;
                        for target in instruction.targets() {
                            has_any_target = true;
                            match target {
                                DemTarget::RelativeDetector(_)
                                | DemTarget::LogicalObservable(_) => {
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
                                    "DEM search compact-repeat error count overflowed",
                                )
                            })?;
                        }
                    }
                    DemInstructionKind::ShiftDetectors if instruction.detector_shift()? == 0 => {}
                    DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => {}
                    DemInstructionKind::ShiftDetectors => {
                        return Ok(None);
                    }
                },
                DemItem::RepeatBlock(repeat) => {
                    let Some(child_count) = repeat
                        .body()
                        .selected_search_graph_compact_repeat_error_count_inner(depth + 1)?
                    else {
                        return Ok(None);
                    };
                    if repeat.body().total_detector_shift()? != 0 {
                        return Ok(None);
                    }
                    count = count.checked_add(child_count).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "DEM search compact-repeat error count overflowed",
                        )
                    })?;
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
                        .selected_search_graph_compact_repeat_error_count()?
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
        policy: SearchGraphTargetPolicy,
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
                            policy.include_error_targets(
                                instruction.targets(),
                                detector_offset,
                                context,
                                counts,
                            )?;
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
                        .selected_search_graph_compact_repeat_error_count()?
                        .is_some()
                    {
                        detector_offset = repeat
                            .body()
                            .collect_search_graph_nonzero_error_target_counts_from(
                                detector_offset,
                                depth + 1,
                                context,
                                policy,
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
                                policy,
                                counts,
                            )?;
                    }
                }
            }
        }
        Ok(detector_offset)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::dem) enum SearchGraphTargetPolicy {
    Graphlike { ignore_ungraphlike_errors: bool },
    Hypergraph { max_weight: usize },
}

impl SearchGraphTargetPolicy {
    fn include_error_targets(
        self,
        targets: &[DemTarget],
        detector_offset: u64,
        context: &'static str,
        counts: &mut DemErrorTargetCounts,
    ) -> CircuitResult<()> {
        match self {
            SearchGraphTargetPolicy::Graphlike {
                ignore_ungraphlike_errors,
            } => include_graphlike_error_targets(
                targets,
                detector_offset,
                ignore_ungraphlike_errors,
                context,
                counts,
            ),
            SearchGraphTargetPolicy::Hypergraph { max_weight } => include_hypergraph_error_targets(
                targets,
                detector_offset,
                max_weight,
                context,
                counts,
            ),
        }
    }
}

fn include_graphlike_error_targets(
    targets: &[DemTarget],
    detector_offset: u64,
    ignore_ungraphlike_errors: bool,
    context: &'static str,
    counts: &mut DemErrorTargetCounts,
) -> CircuitResult<()> {
    if ignore_ungraphlike_errors
        && targets
            .iter()
            .any(|target| matches!(target, DemTarget::Separator))
    {
        return Ok(());
    }

    let mut start = 0;
    for (index, target) in targets.iter().enumerate() {
        if matches!(target, DemTarget::Separator) {
            let component = targets.get(start..index).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "graphlike target component range is invalid",
                )
            })?;
            include_graphlike_target_component(
                component,
                detector_offset,
                ignore_ungraphlike_errors,
                context,
                counts,
            )?;
            start = index + 1;
        }
    }
    let component = targets.get(start..).ok_or_else(|| {
        CircuitError::invalid_detector_error_model("graphlike target component range is invalid")
    })?;
    include_graphlike_target_component(
        component,
        detector_offset,
        ignore_ungraphlike_errors,
        context,
        counts,
    )
}

fn include_graphlike_target_component(
    targets: &[DemTarget],
    detector_offset: u64,
    ignore_ungraphlike_errors: bool,
    context: &'static str,
    counts: &mut DemErrorTargetCounts,
) -> CircuitResult<()> {
    let mut detectors = Vec::new();
    for target in targets {
        if let DemTarget::RelativeDetector(detector) = *target {
            if detectors.len() == 2 {
                if ignore_ungraphlike_errors {
                    return Ok(());
                }
                return Err(CircuitError::invalid_detector_error_model(
                    "The detector error model contained a non-graphlike error mechanism.\nYou can ignore such errors using `ignore_ungraphlike_errors`.\nYou can use `decompose_errors` when converting a circuit into a model to ensure no such errors are present.",
                ));
            }
            detectors.push(detector);
        }
    }

    for detector in detectors {
        counts.include_detector(shifted_detector(detector, detector_offset)?, context)?;
    }
    Ok(())
}

fn include_hypergraph_error_targets(
    targets: &[DemTarget],
    detector_offset: u64,
    max_weight: usize,
    context: &'static str,
    counts: &mut DemErrorTargetCounts,
) -> CircuitResult<()> {
    let mut detectors = BTreeSet::new();
    for target in targets {
        match *target {
            DemTarget::RelativeDetector(detector) => {
                let detector = shifted_detector(detector, detector_offset)?;
                if !detectors.insert(detector) {
                    detectors.remove(&detector);
                }
            }
            DemTarget::LogicalObservable(_) | DemTarget::Separator => {}
            DemTarget::Numeric(_) => {
                return Err(CircuitError::invalid_detector_error_model(
                    "hypergraph error targets cannot include numeric targets",
                ));
            }
        }
    }

    if detectors.len() > max_weight {
        return Ok(());
    }
    for detector in detectors {
        counts.include_detector(detector, context)?;
    }
    Ok(())
}

fn shifted_detector(detector: DemDetectorId, detector_offset: u64) -> CircuitResult<DemDetectorId> {
    let detector_id = detector_offset.checked_add(detector.get()).ok_or_else(|| {
        CircuitError::invalid_detector_error_model("DEM nonzero-error detector target overflowed")
    })?;
    DemDetectorId::try_new(detector_id)
}

#[derive(Clone, Debug)]
struct DemErrorTargetCounts {
    detectors: BTreeSet<DemDetectorId>,
    max_detector_nodes: usize,
}

impl DemErrorTargetCounts {
    fn new(max_detector_nodes: usize) -> Self {
        Self {
            detectors: BTreeSet::new(),
            max_detector_nodes,
        }
    }

    fn include_detector(
        &mut self,
        detector: DemDetectorId,
        context: &'static str,
    ) -> CircuitResult<()> {
        self.detectors.insert(detector);
        if self.detectors.len() > self.max_detector_nodes {
            return Err(self.too_many_detectors_error(context));
        }
        Ok(())
    }

    fn too_many_detectors_error(&self, context: &'static str) -> CircuitError {
        CircuitError::invalid_detector_error_model(format!(
            "{context} currently supports at most {} effective detector nodes, got {}",
            self.max_detector_nodes,
            self.detectors.len()
        ))
    }
}
