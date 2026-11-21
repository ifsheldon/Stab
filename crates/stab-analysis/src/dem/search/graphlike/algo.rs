use std::collections::{BTreeMap, VecDeque, btree_map::Entry};

use stab_model::{DemItem, DetectorErrorModel};

use super::{Graph, ObservableMask, SearchState};
use crate::AnalysisOperation;
use crate::dem::search::budget::{LogicalErrorSearchLimits, SearchBudget};
use crate::dem::search::outcome::{
    GraphlikeSearchOptions, LogicalErrorSearch, NoLogicalErrorReport, UngraphlikeErrors,
};
use crate::{AnalysisError, AnalysisResult};

/// Finds a shortest undetectable logical error made of graphlike error components.
///
/// # Errors
///
/// Returns [`AnalysisError::NonGraphlikeError`] when the model has an ungraphlike component and
/// the options reject them, [`AnalysisError::Cancelled`] once the limits' cancellation signal is
/// observed while the graph is built or during the search, and [`AnalysisError::ResourceLimit`]
/// when a limit is exceeded; a search stopped by a limit does not report
/// [`LogicalErrorSearch::NotFound`].
pub fn shortest_graphlike_undetectable_logical_error(
    model: &DetectorErrorModel,
    options: &GraphlikeSearchOptions,
) -> AnalysisResult<LogicalErrorSearch> {
    shortest_graphlike_search(model, options.ungraphlike_errors, &options.limits)
}

pub(in crate::dem::search) fn shortest_graphlike_search(
    model: &DetectorErrorModel,
    ungraphlike_errors: UngraphlikeErrors,
    limits: &LogicalErrorSearchLimits,
) -> AnalysisResult<LogicalErrorSearch> {
    let graph = Graph::from_dem_with_limits(model, ungraphlike_errors.ignored(), limits)?;
    let empty = SearchState::new(None, None, ObservableMask::new());
    if !graph.distance_1_error_mask.is_empty() {
        let mut out = DetectorErrorModel::new();
        SearchState::new(None, None, graph.distance_1_error_mask)
            .append_transition_as_error_instruction_to(&empty, &mut out)?;
        return Ok(LogicalErrorSearch::Found(out));
    }

    let mut queue = VecDeque::new();
    let mut back_map = BTreeMap::new();
    let mut budget = SearchBudget::new("graphlike search", limits);
    budget.admit_state(0, 0, false)?;
    back_map.insert(empty.clone(), empty.clone());

    for (source_index, node) in graph.nodes.iter().enumerate() {
        budget.poll_cancellation()?;
        let source = graph.detector_for_node_index(source_index)?;
        for edge in &node.edges {
            budget.poll_cancellation_steps(edge.observables.len().saturating_add(1))?;
            if !edge.observables.is_empty() && edge.detector.is_none_or(|target| source < target) {
                let start_terms = edge
                    .observables
                    .len()
                    .checked_add(1)
                    .and_then(|count| count.checked_add(usize::from(edge.detector.is_some())))
                    .ok_or_else(|| {
                        AnalysisError::count_overflow(
                            AnalysisOperation::LogicalErrorSearch,
                            "graphlike initial search state term count overflowed",
                        )
                    })?;
                budget.preflight_state_terms(start_terms)?;
                let start = SearchState::new(Some(source), edge.detector, edge.observables.clone());
                if let Entry::Vacant(entry) = back_map.entry(start) {
                    budget.admit_state(start_terms, 0, true)?;
                    queue.push_back(entry.key().clone());
                    entry.insert(empty.clone());
                }
            }
        }
    }

    while let Some(current) = queue.pop_front() {
        budget.poll_cancellation()?;
        let Some(active) = current.detector_active else {
            return Err(AnalysisError::internal(
                AnalysisOperation::LogicalErrorSearch,
                "graphlike search reached a state without an active detector",
            ));
        };
        let active_index = graph.node_index_for_detector(active)?;
        let Some(node) = graph.nodes.get(active_index) else {
            return Err(AnalysisError::internal(
                AnalysisOperation::LogicalErrorSearch,
                "graphlike active detector is outside the graph",
            ));
        };
        let current_terms = current.term_count()?;
        for edge in &node.edges {
            budget.record_transition()?;
            budget.poll_cancellation_steps(current_terms.saturating_add(edge.observables.len()))?;
            let next_terms = edge
                .observables
                .symmetric_difference_len(&current.observables)
                .checked_add(usize::from(edge.detector.is_some()))
                .and_then(|count| count.checked_add(usize::from(current.detector_held.is_some())))
                .ok_or_else(|| {
                    AnalysisError::count_overflow(
                        AnalysisOperation::LogicalErrorSearch,
                        "graphlike next search state term count overflowed",
                    )
                })?;
            budget.preflight_state_terms(next_terms)?;
            let next = SearchState::new(
                edge.detector,
                current.detector_held,
                edge.observables.symmetric_difference(&current.observables),
            );
            let undetected = next.is_undetected();
            if let Entry::Vacant(entry) = back_map.entry(next) {
                budget.admit_state(next_terms, current_terms, !undetected)?;
                let mut inserted = entry.key().clone();
                entry.insert(current.clone());
                if undetected {
                    return backtrack_path(&back_map, &inserted, &mut budget)
                        .map(LogicalErrorSearch::Found);
                }
                if inserted.detector_active.is_none() {
                    std::mem::swap(&mut inserted.detector_active, &mut inserted.detector_held);
                }
                queue.push_back(inserted);
            }
        }
    }

    no_graphlike_logical_error(model, &graph, &mut budget).map(LogicalErrorSearch::NotFound)
}

fn backtrack_path(
    back_map: &BTreeMap<SearchState, SearchState>,
    final_state: &SearchState,
    budget: &mut SearchBudget,
) -> AnalysisResult<DetectorErrorModel> {
    let mut out = DetectorErrorModel::new();
    let mut current = final_state.clone();
    loop {
        budget.poll_cancellation()?;
        let Some(previous) = back_map.get(&current) else {
            return Err(AnalysisError::internal(
                AnalysisOperation::LogicalErrorSearch,
                "graphlike search backtracking reached an unknown state",
            ));
        };
        current.append_transition_as_error_instruction_to(previous, &mut out)?;
        if previous.is_undetected() {
            break;
        }
        current = previous.clone();
    }
    sorted_error_model(out)
}

fn sorted_error_model(model: DetectorErrorModel) -> AnalysisResult<DetectorErrorModel> {
    let mut instructions = Vec::new();
    for item in model.items() {
        let DemItem::Instruction(instruction) = item else {
            return Err(AnalysisError::internal(
                AnalysisOperation::LogicalErrorSearch,
                "graphlike search produced a repeat block unexpectedly",
            ));
        };
        instructions.push(instruction.clone());
    }
    instructions.sort_by(|left, right| left.targets().cmp(right.targets()));

    let mut sorted = DetectorErrorModel::new();
    for instruction in instructions {
        sorted.push_instruction(instruction);
    }
    Ok(sorted)
}

fn no_graphlike_logical_error(
    model: &DetectorErrorModel,
    graph: &Graph,
    budget: &mut SearchBudget,
) -> AnalysisResult<NoLogicalErrorReport> {
    let no_errors = model.count_errors()? == 0;
    // Pinned Stim inspects the graph's edges only when the model has errors.
    let no_graphlike_errors = !no_errors && !graph_has_edges(graph, budget)?;
    Ok(NoLogicalErrorReport::graphlike(
        graph.num_observables == 0,
        !graph.has_declared_detectors,
        no_errors,
        no_graphlike_errors,
    ))
}

fn graph_has_edges(graph: &Graph, budget: &mut SearchBudget) -> AnalysisResult<bool> {
    for node in &graph.nodes {
        budget.poll_cancellation()?;
        if !node.edges.is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact diagnostics"
    )]

    use super::*;

    fn search(dem: &str, ungraphlike_errors: UngraphlikeErrors) -> AnalysisResult<String> {
        let model = DetectorErrorModel::from_dem_str(dem)?;
        let options = GraphlikeSearchOptions {
            ungraphlike_errors,
            ..GraphlikeSearchOptions::default()
        };
        let found = shortest_graphlike_undetectable_logical_error(&model, &options)?
            .into_found()
            .expect("the model has a graphlike logical error");
        Ok(found.to_dem_string())
    }

    fn shortest(dem: &str) -> AnalysisResult<String> {
        search(dem, UngraphlikeErrors::Reject)
    }

    fn shortest_ignoring_ungraphlike(dem: &str) -> AnalysisResult<String> {
        search(dem, UngraphlikeErrors::Ignore)
    }

    #[test]
    fn graphlike_algo_ignores_ungraphlike_errors_when_requested() {
        let dem = "error(0.1) D0 D1 D2\nerror(0.1) D0\nerror(0.1) D0 L0\n";
        assert!(matches!(
            shortest(dem),
            Err(AnalysisError::NonGraphlikeError)
        ));
        assert_eq!(
            shortest_ignoring_ungraphlike(dem).unwrap(),
            "error(1) D0\nerror(1) D0 L0\n"
        );
    }

    #[test]
    fn graphlike_algo_many_observables_direct_dem_case() {
        let dem = "error(0.1) D0\n\
                   error(0.1) D0 D1\n\
                   error(0.1) D1 D2\n\
                   error(0.1) D2 D3\n\
                   error(0.1) D3 L1200\n";
        assert_eq!(
            shortest(dem).unwrap(),
            "error(1) D0\n\
             error(1) D0 D1\n\
             error(1) D1 D2\n\
             error(1) D2 D3\n\
             error(1) D3 L1200\n"
        );
    }
}
