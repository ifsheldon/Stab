use std::collections::{BTreeSet, HashMap};
use std::ops::ControlFlow;

use stab_model::advanced::{
    DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemItem, FoldedDemTraversal,
    FoldedDemVisitor,
};
use stab_model::{DemDetectorId, DemInstruction, DemInstructionKind, DemRepeatBlock, DemTarget};

use super::budget::LogicalErrorSearchLimits;
use crate::resources::LogicalErrorSearchResource;
use crate::{AnalysisError, AnalysisResult, ResourceLimitError};

pub(in crate::dem) fn search_graph_nonzero_error_targets(
    traversal: &FoldedDemTraversal<'_>,
    context: &'static str,
    policy: SearchGraphTargetPolicy,
    limits: LogicalErrorSearchLimits,
) -> AnalysisResult<BTreeSet<DemDetectorId>> {
    let mut counts = DemErrorTargetCounts::new(limits.max_effective_detector_nodes());
    visit_search_graph_errors_with_limits(
        traversal,
        context,
        limits,
        |instruction, detector_offset| {
            policy.include_error_targets(
                instruction.targets(),
                detector_offset,
                context,
                &mut counts,
            )
        },
    )?;
    Ok(counts.detectors)
}

pub(in crate::dem) fn visit_search_graph_errors_with_limits<F>(
    traversal: &FoldedDemTraversal<'_>,
    context: &'static str,
    limits: LogicalErrorSearchLimits,
    visit_error: F,
) -> AnalysisResult<()>
where
    F: FnMut(&DemInstruction, u64) -> AnalysisResult<()>,
{
    traversal.validate_repeat_depth(context)?;
    let block_policies = SearchBlockPolicies::new(traversal.root());
    let mut visitor = SearchErrorVisitor {
        context,
        limits,
        visited_error_mechanisms: 0,
        visited_target_occurrences: 0,
        block_policies,
        visit_error,
    };
    let _ = traversal.try_visit(&mut visitor)?;
    Ok(())
}

struct SearchErrorVisitor<F> {
    context: &'static str,
    limits: LogicalErrorSearchLimits,
    visited_error_mechanisms: u64,
    visited_target_occurrences: usize,
    block_policies: SearchBlockPolicies,
    visit_error: F,
}

impl<F> FoldedDemVisitor for SearchErrorVisitor<F>
where
    F: FnMut(&DemInstruction, u64) -> AnalysisResult<()>,
{
    type Error = AnalysisError;

    fn visit_instruction(
        &mut self,
        instruction: &DemInstruction,
        state: &DemTraversalState,
    ) -> AnalysisResult<ControlFlow<()>> {
        if instruction.kind() == DemInstructionKind::Error
            && instruction.args().first().copied().unwrap_or(0.0) != 0.0
        {
            let visited_error_mechanisms = self
                .visited_error_mechanisms
                .checked_add(1)
                .ok_or_else(|| traversal_error(self.context, "error mechanism count overflowed"))?;
            let mechanism_limit = self.limits.max_expanded_error_mechanisms();
            if visited_error_mechanisms > mechanism_limit {
                return Err(ResourceLimitError::logical_error_search(
                    self.context,
                    LogicalErrorSearchResource::ExpandedErrorMechanisms,
                    visited_error_mechanisms,
                    mechanism_limit,
                )
                .into());
            }
            let target_occurrences = instruction.targets().len();
            let per_mechanism_limit = self.limits.max_error_target_occurrences_per_mechanism();
            if target_occurrences > per_mechanism_limit {
                return Err(ResourceLimitError::logical_error_search(
                    self.context,
                    LogicalErrorSearchResource::ErrorTargetOccurrencesPerMechanism,
                    target_occurrences as u64,
                    per_mechanism_limit as u64,
                )
                .into());
            }
            let visited_target_occurrences = self
                .visited_target_occurrences
                .checked_add(target_occurrences)
                .ok_or_else(|| {
                    traversal_error(self.context, "total target occurrence count overflowed")
                })?;
            let total_target_limit = self.limits.max_total_error_target_occurrences();
            if visited_target_occurrences > total_target_limit {
                return Err(ResourceLimitError::logical_error_search(
                    self.context,
                    LogicalErrorSearchResource::TotalErrorTargetOccurrences,
                    visited_target_occurrences as u64,
                    total_target_limit as u64,
                )
                .into());
            }
            self.visited_error_mechanisms = visited_error_mechanisms;
            self.visited_target_occurrences = visited_target_occurrences;
            (self.visit_error)(instruction, state.detector_offset())?;
        }
        Ok(ControlFlow::Continue(()))
    }

    fn enter_repeat(
        &mut self,
        repeat: &DemRepeatBlock,
        body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> AnalysisResult<DemRepeatSelection> {
        let policy = self.block_policies.policy_for(body)?;
        if !policy.has_nonzero_probability_error {
            return Ok(DemRepeatSelection::Skip);
        }
        if policy.compact_error_count.clone()?.is_some() {
            return Ok(DemRepeatSelection::FoldOnce);
        }
        let repeat_count = repeat.repeat_count().get();
        let repeat_limit = self.limits.max_repeat_unroll();
        if repeat_count > repeat_limit {
            return Err(ResourceLimitError::logical_error_search(
                self.context,
                LogicalErrorSearchResource::RepeatCount,
                repeat_count,
                repeat_limit,
            )
            .into());
        }
        Ok(DemRepeatSelection::Expand {
            max_total_iterations: self.limits.max_repeat_iterations(),
            context: self.context,
        })
    }

    fn repeat_expansion_limit_error(
        &mut self,
        context: &'static str,
        actual: u64,
        limit: u64,
    ) -> AnalysisError {
        ResourceLimitError::logical_error_traversal_repeat_iterations(context, actual, limit).into()
    }
}

#[derive(Clone, Debug)]
struct SearchBlockPolicy {
    has_nonzero_probability_error: bool,
    compact_error_count: AnalysisResult<Option<u64>>,
}

#[derive(Debug)]
struct SearchBlockPolicies {
    by_block: HashMap<usize, SearchBlockPolicy>,
}

impl SearchBlockPolicies {
    fn new(root: &FoldedDemBlock<'_>) -> Self {
        let mut by_block = HashMap::new();
        summarize_search_block_policy(root, &mut by_block);
        Self { by_block }
    }

    fn policy_for(&self, block: &FoldedDemBlock<'_>) -> AnalysisResult<&SearchBlockPolicy> {
        self.by_block.get(&block.compact_id()).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "DEM search compact policy is missing a folded block",
            )
        })
    }
}

fn summarize_search_block_policy(
    block: &FoldedDemBlock<'_>,
    by_block: &mut HashMap<usize, SearchBlockPolicy>,
) -> SearchBlockPolicy {
    let mut has_nonzero_probability_error = false;
    let mut compact_error_count = Ok(Some(0_u64));

    for item in block.items() {
        match item {
            FoldedDemItem::Instruction(instruction) => {
                if instruction.kind() == DemInstructionKind::Error
                    && instruction.args().first().copied().unwrap_or(0.0) != 0.0
                {
                    has_nonzero_probability_error = true;
                }
                let Some(count) = active_compact_count(&compact_error_count) else {
                    continue;
                };
                compact_error_count = update_search_instruction_count(count, instruction);
            }
            FoldedDemItem::Repeat { repeat, body } => {
                let child = summarize_search_block_policy(body, by_block);
                if repeat.repeat_count().get() > 0 {
                    has_nonzero_probability_error |= child.has_nonzero_probability_error;
                }
                let Some(count) = active_compact_count(&compact_error_count) else {
                    continue;
                };
                if repeat.repeat_count().get() == 0 {
                    continue;
                }
                compact_error_count = child.compact_error_count.clone().and_then(|child_count| {
                    let Some(child_count) = child_count else {
                        return Ok(None);
                    };
                    if body.summary().detector_shift()? != 0 {
                        return Ok(None);
                    }
                    count.checked_add(child_count).map(Some).ok_or_else(|| {
                        AnalysisError::invalid_detector_error_model(
                            "DEM search compact-repeat error count overflowed",
                        )
                    })
                });
            }
        }
    }

    let policy = SearchBlockPolicy {
        has_nonzero_probability_error,
        compact_error_count,
    };
    by_block.insert(block.compact_id(), policy.clone());
    policy
}

fn active_compact_count(count: &AnalysisResult<Option<u64>>) -> Option<u64> {
    match count {
        Ok(Some(count)) => Some(*count),
        Ok(None) | Err(_) => None,
    }
}

fn update_search_instruction_count(
    count: u64,
    instruction: &DemInstruction,
) -> AnalysisResult<Option<u64>> {
    match instruction.kind() {
        DemInstructionKind::Error => {
            if instruction.args().first().copied().unwrap_or(0.0) == 0.0 {
                return Ok(Some(count));
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
            if !has_search_target {
                return Ok(Some(count));
            }
            count.checked_add(1).map(Some).ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "DEM search compact-repeat error count overflowed",
                )
            })
        }
        DemInstructionKind::ShiftDetectors
            if stab_model::advanced::dem_instruction_detector_shift(instruction)? == 0 =>
        {
            Ok(Some(count))
        }
        DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => Ok(Some(count)),
        DemInstructionKind::ShiftDetectors => Ok(None),
    }
}

fn traversal_error(context: &'static str, message: &'static str) -> AnalysisError {
    AnalysisError::invalid_detector_error_model(format!("DEM {context} {message}"))
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
    ) -> AnalysisResult<()> {
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
) -> AnalysisResult<()> {
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
                AnalysisError::invalid_detector_error_model(
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
        AnalysisError::invalid_detector_error_model("graphlike target component range is invalid")
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
) -> AnalysisResult<()> {
    let mut detectors = Vec::new();
    for target in targets {
        if let DemTarget::RelativeDetector(detector) = *target {
            if detectors.len() == 2 {
                if ignore_ungraphlike_errors {
                    return Ok(());
                }
                return Err(AnalysisError::invalid_detector_error_model(
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
) -> AnalysisResult<()> {
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
                return Err(AnalysisError::invalid_detector_error_model(
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

fn shifted_detector(
    detector: DemDetectorId,
    detector_offset: u64,
) -> AnalysisResult<DemDetectorId> {
    let detector_id = detector_offset.checked_add(detector.get()).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model("DEM nonzero-error detector target overflowed")
    })?;
    DemDetectorId::try_new(detector_id).map_err(Into::into)
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
    ) -> AnalysisResult<()> {
        if self.detectors.contains(&detector) {
            return Ok(());
        }
        let next = self.detectors.len().checked_add(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "{context} effective detector node count overflowed"
            ))
        })?;
        if next > self.max_detector_nodes {
            return Err(self.too_many_detectors_error(context, next));
        }
        self.detectors.insert(detector);
        Ok(())
    }

    fn too_many_detectors_error(&self, context: &'static str, actual: usize) -> AnalysisError {
        ResourceLimitError::logical_error_search(
            context,
            LogicalErrorSearchResource::EffectiveDetectorNodes,
            actual as u64,
            self.max_detector_nodes as u64,
        )
        .into()
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use fixed valid DEM fixtures for traversal diagnostics"
    )]

    use super::*;
    use stab_model::DetectorErrorModel;
    use std::cell::Cell;

    #[test]
    fn search_traversal_counter_overflow_rejects_before_forwarding() {
        let model = DetectorErrorModel::from_dem_str("error(0.1) D0\n").unwrap();
        let traversal = FoldedDemTraversal::new(&model).unwrap();
        let instruction = match model.items().first().expect("fixture has one item") {
            stab_model::DemItem::Instruction(instruction) => instruction,
            stab_model::DemItem::RepeatBlock(_) => {
                unreachable!("fixture contains one instruction")
            }
        };
        let forwarded = Cell::new(0);
        let mut mechanism_visitor = SearchErrorVisitor {
            context: "test search",
            limits: LogicalErrorSearchLimits::default()
                .with_max_expanded_error_mechanisms(u64::MAX),
            visited_error_mechanisms: u64::MAX,
            visited_target_occurrences: 0,
            block_policies: SearchBlockPolicies::new(traversal.root()),
            visit_error: |_: &DemInstruction, _: u64| -> AnalysisResult<()> {
                forwarded.set(forwarded.get() + 1);
                Ok(())
            },
        };
        let error = mechanism_visitor
            .visit_instruction(instruction, &DemTraversalState::default())
            .expect_err("the mechanism counter must reject integer overflow");
        assert!(
            error
                .to_string()
                .contains("error mechanism count overflowed"),
            "unexpected error: {error}"
        );
        assert_eq!(forwarded.get(), 0);

        let mut target_visitor = SearchErrorVisitor {
            context: "test search",
            limits: LogicalErrorSearchLimits::default()
                .with_max_total_error_target_occurrences(usize::MAX),
            visited_error_mechanisms: 0,
            visited_target_occurrences: usize::MAX,
            block_policies: SearchBlockPolicies::new(traversal.root()),
            visit_error: |_: &DemInstruction, _: u64| -> AnalysisResult<()> {
                forwarded.set(forwarded.get() + 1);
                Ok(())
            },
        };
        let error = target_visitor
            .visit_instruction(instruction, &DemTraversalState::default())
            .expect_err("the aggregate target counter must reject integer overflow");
        assert!(
            error
                .to_string()
                .contains("total target occurrence count overflowed"),
            "unexpected error: {error}"
        );
        assert_eq!(forwarded.get(), 0);
    }

    #[test]
    fn search_policy_cache_scales_with_compact_nested_repeats() {
        let model = DetectorErrorModel::from_dem_str(
            "repeat 1000000000 {\n    repeat 1000000000 {\n        error(0.1) D0 L0\n        shift_detectors 0\n    }\n}\n",
        )
        .unwrap();
        let traversal = FoldedDemTraversal::new(&model).unwrap();
        let policies = SearchBlockPolicies::new(traversal.root());
        assert_eq!(policies.by_block.len(), 3);

        let mut visited = 0;
        visit_search_graph_errors_with_limits(
            &traversal,
            "test search",
            LogicalErrorSearchLimits::default(),
            |_, _| {
                visited += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(visited, 1);
    }

    #[test]
    fn rejected_error_payload_is_not_forwarded_to_search_graph_work() {
        let model = DetectorErrorModel::from_dem_str("error(0.1) D0 D1 L0\n").unwrap();
        let traversal = FoldedDemTraversal::new(&model).unwrap();
        let limits =
            LogicalErrorSearchLimits::default().with_max_error_target_occurrences_per_mechanism(2);
        let mut visited = 0;
        let error =
            visit_search_graph_errors_with_limits(&traversal, "test search", limits, |_, _| {
                visited += 1;
                Ok(())
            })
            .expect_err("per-mechanism target limit");
        assert!(
            error
                .to_string()
                .contains("at most 2 target occurrences per nonzero error mechanism, got 3")
        );
        assert_eq!(visited, 0);
    }
}
