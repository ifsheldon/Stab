use std::collections::BTreeSet;
use std::ops::ControlFlow;

use super::traversal::{
    DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemTraversal, FoldedDemVisitor,
};
use super::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemRepeatBlock, DemTarget,
    MAX_DEM_FLATTEN_REPEAT_ITERATIONS, MAX_DEM_FLATTEN_REPEAT_UNROLL,
};
use crate::{CircuitError, CircuitResult};

#[cfg(not(test))]
const MAX_DEM_SEARCH_ERROR_MECHANISMS: u64 = 5_000_000;
#[cfg(test)]
const MAX_DEM_SEARCH_ERROR_MECHANISMS: u64 = 10_000;

#[cfg(not(test))]
const MAX_DEM_SEARCH_ERROR_TARGET_OCCURRENCES: usize = 65_536;
#[cfg(test)]
const MAX_DEM_SEARCH_ERROR_TARGET_OCCURRENCES: usize = 128;

pub(in crate::dem) fn search_graph_nonzero_error_targets(
    traversal: &FoldedDemTraversal<'_>,
    context: &'static str,
    policy: SearchGraphTargetPolicy,
    max_detector_nodes: usize,
) -> CircuitResult<BTreeSet<DemDetectorId>> {
    let mut counts = DemErrorTargetCounts::new(max_detector_nodes);
    visit_search_graph_errors(traversal, context, |instruction, detector_offset| {
        policy.include_error_targets(instruction.targets(), detector_offset, context, &mut counts)
    })?;
    Ok(counts.detectors)
}

pub(in crate::dem) fn visit_search_graph_errors<F>(
    traversal: &FoldedDemTraversal<'_>,
    context: &'static str,
    visit_error: F,
) -> CircuitResult<()>
where
    F: FnMut(&DemInstruction, u64) -> CircuitResult<()>,
{
    traversal.validate_repeat_depth(context)?;
    let mut visitor = SearchErrorVisitor {
        context,
        visited_error_mechanisms: 0,
        visit_error,
    };
    let _ = traversal.try_visit(&mut visitor)?;
    Ok(())
}

struct SearchErrorVisitor<F> {
    context: &'static str,
    visited_error_mechanisms: u64,
    visit_error: F,
}

impl<F> FoldedDemVisitor for SearchErrorVisitor<F>
where
    F: FnMut(&DemInstruction, u64) -> CircuitResult<()>,
{
    fn visit_instruction(
        &mut self,
        instruction: &DemInstruction,
        state: &DemTraversalState,
    ) -> CircuitResult<ControlFlow<()>> {
        if instruction.kind() == DemInstructionKind::Error
            && instruction.args().first().copied().unwrap_or(0.0) != 0.0
        {
            self.visited_error_mechanisms = self
                .visited_error_mechanisms
                .checked_add(1)
                .ok_or_else(|| traversal_error(self.context, "error mechanism count overflowed"))?;
            if self.visited_error_mechanisms > MAX_DEM_SEARCH_ERROR_MECHANISMS {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DEM {} currently supports at most {MAX_DEM_SEARCH_ERROR_MECHANISMS} expanded nonzero error mechanisms, got at least {}",
                    self.context, self.visited_error_mechanisms
                )));
            }
            let target_occurrences = instruction.targets().len();
            if target_occurrences > MAX_DEM_SEARCH_ERROR_TARGET_OCCURRENCES {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DEM {} currently supports at most {MAX_DEM_SEARCH_ERROR_TARGET_OCCURRENCES} target occurrences per nonzero error mechanism, got {target_occurrences}",
                    self.context
                )));
            }
            (self.visit_error)(instruction, state.detector_offset())?;
        }
        Ok(ControlFlow::Continue(()))
    }

    fn enter_repeat(
        &mut self,
        repeat: &DemRepeatBlock,
        body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> CircuitResult<DemRepeatSelection> {
        if !body.summary().has_nonzero_probability_error() {
            return Ok(DemRepeatSelection::Skip);
        }
        if body.summary().compact_search_error_count()?.is_some() {
            return Ok(DemRepeatSelection::FoldOnce);
        }
        let repeat_count = repeat.repeat_count().get();
        if repeat_count > MAX_DEM_FLATTEN_REPEAT_UNROLL {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM {} currently supports repeat counts up to {MAX_DEM_FLATTEN_REPEAT_UNROLL}, got {repeat_count}",
                self.context
            )));
        }
        Ok(DemRepeatSelection::Expand {
            max_total_iterations: MAX_DEM_FLATTEN_REPEAT_ITERATIONS,
            context: self.context,
        })
    }
}

fn traversal_error(context: &'static str, message: &'static str) -> CircuitError {
    CircuitError::invalid_detector_error_model(format!("DEM {context} {message}"))
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use fixed valid DEM fixtures for traversal diagnostics"
    )]

    use super::*;
    use crate::DetectorErrorModel;

    #[test]
    fn search_traversal_budgets_nonzero_error_mechanisms_not_annotations() {
        let annotations = DetectorErrorModel::from_dem_str(
            "repeat 10001 {\n    detector D0\n    logical_observable L0\n    shift_detectors 0\n}\nerror(0.1) L0\n",
        )
        .unwrap();
        let traversal = FoldedDemTraversal::new(&annotations).unwrap();
        let mut visited = 0;
        visit_search_graph_errors(&traversal, "test search", |_, _| {
            visited += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(visited, 1);
    }

    #[test]
    fn search_traversal_has_a_distinct_error_mechanism_cap() {
        let mechanisms = DetectorErrorModel::from_dem_str(
            "repeat 10001 {\n    error(0.1) D0\n    shift_detectors 1\n}\n",
        )
        .unwrap();
        let traversal = FoldedDemTraversal::new(&mechanisms).unwrap();
        let error = visit_search_graph_errors(&traversal, "test search", |_, _| Ok(()))
            .expect_err("expanded nonzero mechanisms should hit the search-specific cap")
            .to_string();
        assert!(error.contains("at most 10000 expanded nonzero error mechanisms"));
        assert!(!error.contains("expanded instructions"));
    }

    #[test]
    fn search_target_collection_has_a_distinct_effective_detector_cap() {
        let model =
            DetectorErrorModel::from_dem_str("error(0.1) D0\nerror(0.1) D1\nerror(0.1) D2\n")
                .unwrap();
        let traversal = FoldedDemTraversal::new(&model).unwrap();
        let error = search_graph_nonzero_error_targets(
            &traversal,
            "test graphlike search",
            SearchGraphTargetPolicy::Graphlike {
                ignore_ungraphlike_errors: false,
            },
            2,
        )
        .expect_err("three touched detectors should exceed the two-node test cap")
        .to_string();
        assert!(error.contains("at most 2 effective detector nodes, got 3"));
        assert!(!error.contains("expanded nonzero error mechanisms"));
    }

    #[test]
    fn search_traversal_rejects_large_error_target_lists_before_normalization() {
        let mut text = String::from("error(0.1)");
        for observable in 0..=MAX_DEM_SEARCH_ERROR_TARGET_OCCURRENCES {
            text.push_str(&format!(" L{observable}"));
        }
        text.push('\n');
        let model = DetectorErrorModel::from_dem_str(&text).unwrap();
        let traversal = FoldedDemTraversal::new(&model).unwrap();
        let error = visit_search_graph_errors(&traversal, "test search", |_, _| Ok(()))
            .expect_err("target occurrence cap")
            .to_string();
        assert!(error.contains("at most 128 target occurrences per nonzero error mechanism"));
    }
}
