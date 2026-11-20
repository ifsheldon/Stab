use std::collections::{BTreeMap, VecDeque, btree_map::Entry};

use stab_model::{DemItem, DetectorErrorModel};

use super::{Graph, ObservableMask, SearchState};
use crate::AnalysisOperation;
use crate::dem::search::budget::{LogicalErrorSearchLimits, SearchBudget};
use crate::{AnalysisError, AnalysisResult, NoLogicalErrorReport};

pub fn shortest_graphlike_undetectable_logical_error(
    model: &DetectorErrorModel,
    ignore_ungraphlike_errors: bool,
) -> AnalysisResult<DetectorErrorModel> {
    shortest_graphlike_undetectable_logical_error_with_limits(
        model,
        ignore_ungraphlike_errors,
        LogicalErrorSearchLimits::default(),
    )
}

/// Finds a shortest graphlike undetectable logical error under explicit limits.
///
/// # Errors
///
/// Returns [`AnalysisError::Cancelled`] once the limits' cancellation signal is observed, while
/// the graph is built or during the search, and [`AnalysisError::ResourceLimit`] when a limit is
/// exceeded.
pub fn shortest_graphlike_undetectable_logical_error_with_limits(
    model: &DetectorErrorModel,
    ignore_ungraphlike_errors: bool,
    limits: LogicalErrorSearchLimits,
) -> AnalysisResult<DetectorErrorModel> {
    let graph = Graph::from_dem_with_limits(model, ignore_ungraphlike_errors, &limits)?;
    let empty = SearchState::new(None, None, ObservableMask::new());
    if !graph.distance_1_error_mask.is_empty() {
        let mut out = DetectorErrorModel::new();
        SearchState::new(None, None, graph.distance_1_error_mask)
            .append_transition_as_error_instruction_to(&empty, &mut out)?;
        return Ok(out);
    }

    let mut queue = VecDeque::new();
    let mut back_map = BTreeMap::new();
    let mut budget = SearchBudget::new("graphlike search", &limits);
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
                    return backtrack_path(&back_map, &inserted, &mut budget);
                }
                if inserted.detector_active.is_none() {
                    std::mem::swap(&mut inserted.detector_active, &mut inserted.detector_held);
                }
                queue.push_back(inserted);
            }
        }
    }

    Err(no_graphlike_logical_error(model, &graph, &mut budget)?)
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
) -> AnalysisResult<AnalysisError> {
    let no_errors = model.count_errors()? == 0;
    // Pinned Stim inspects the graph's edges only when the model has errors.
    let no_graphlike_errors = !no_errors && !graph_has_edges(graph, budget)?;
    Ok(AnalysisError::NoLogicalErrorFound(
        NoLogicalErrorReport::graphlike(
            graph.num_observables == 0,
            !graph.has_declared_detectors,
            no_errors,
            no_graphlike_errors,
        ),
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

    fn shortest(dem: &str) -> AnalysisResult<String> {
        let model = DetectorErrorModel::from_dem_str(dem)?;
        shortest_graphlike_undetectable_logical_error(&model, false)
            .map(|error| error.to_dem_string())
    }

    fn shortest_ignoring_ungraphlike(dem: &str) -> AnalysisResult<String> {
        let model = DetectorErrorModel::from_dem_str(dem)?;
        shortest_graphlike_undetectable_logical_error(&model, true)
            .map(|error| error.to_dem_string())
    }

    #[test]
    fn graphlike_algo_ignores_ungraphlike_errors_when_requested() {
        let dem = "error(0.1) D0 D1 D2\nerror(0.1) D0\nerror(0.1) D0 L0\n";
        assert!(shortest(dem).is_err());
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
