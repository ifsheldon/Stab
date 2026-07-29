use std::collections::{BTreeMap, VecDeque, btree_map::Entry};

use stab_model::{DemItem, DetectorErrorModel};

use super::{Graph, ObservableMask, SearchState};
use crate::dem::search::budget::{LogicalErrorSearchLimits, SearchBudget};
use crate::{AnalysisError, AnalysisResult};

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

pub fn shortest_graphlike_undetectable_logical_error_with_limits(
    model: &DetectorErrorModel,
    ignore_ungraphlike_errors: bool,
    limits: LogicalErrorSearchLimits,
) -> AnalysisResult<DetectorErrorModel> {
    let graph = Graph::from_dem_with_limits(model, ignore_ungraphlike_errors, limits)?;
    let empty = SearchState::new(None, None, ObservableMask::new());
    if !graph.distance_1_error_mask.is_empty() {
        let mut out = DetectorErrorModel::new();
        SearchState::new(None, None, graph.distance_1_error_mask)
            .append_transition_as_error_instruction_to(&empty, &mut out)?;
        return Ok(out);
    }

    let mut queue = VecDeque::new();
    let mut back_map = BTreeMap::new();
    let mut budget = SearchBudget::new("graphlike search", limits);
    budget.admit_state(0, 0, false)?;
    back_map.insert(empty.clone(), empty.clone());

    for (source_index, node) in graph.nodes.iter().enumerate() {
        let source = graph.detector_for_node_index(source_index)?;
        for edge in &node.edges {
            if !edge.observables.is_empty() && edge.detector.is_none_or(|target| source < target) {
                let start_terms = edge
                    .observables
                    .len()
                    .checked_add(1)
                    .and_then(|count| count.checked_add(usize::from(edge.detector.is_some())))
                    .ok_or_else(|| {
                        AnalysisError::invalid_detector_error_model(
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
        let Some(active) = current.detector_active else {
            return Err(AnalysisError::invalid_detector_error_model(
                "graphlike search reached a state without an active detector",
            ));
        };
        let active_index = graph.node_index_for_detector(active)?;
        let Some(node) = graph.nodes.get(active_index) else {
            return Err(AnalysisError::invalid_detector_error_model(
                "graphlike active detector is outside the graph",
            ));
        };
        let current_terms = current.term_count()?;
        for edge in &node.edges {
            budget.record_transition()?;
            let next_terms = edge
                .observables
                .symmetric_difference_len(&current.observables)
                .checked_add(usize::from(edge.detector.is_some()))
                .and_then(|count| count.checked_add(usize::from(current.detector_held.is_some())))
                .ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(
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
                    return backtrack_path(&back_map, &inserted);
                }
                if inserted.detector_active.is_none() {
                    std::mem::swap(&mut inserted.detector_active, &mut inserted.detector_held);
                }
                queue.push_back(inserted);
            }
        }
    }

    Err(AnalysisError::invalid_detector_error_model(
        no_graphlike_logical_error_message(model, &graph)?,
    ))
}

fn backtrack_path(
    back_map: &BTreeMap<SearchState, SearchState>,
    final_state: &SearchState,
) -> AnalysisResult<DetectorErrorModel> {
    let mut out = DetectorErrorModel::new();
    let mut current = final_state.clone();
    loop {
        let Some(previous) = back_map.get(&current) else {
            return Err(AnalysisError::invalid_detector_error_model(
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
            return Err(AnalysisError::invalid_detector_error_model(
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

fn no_graphlike_logical_error_message(
    model: &DetectorErrorModel,
    graph: &Graph,
) -> AnalysisResult<String> {
    let mut message = String::from("Failed to find any graphlike logical errors.");
    if graph.num_observables == 0 {
        message.push_str(
            "\n    WARNING: NO OBSERVABLES. The circuit or detector error model didn't define any observables, making it vacuously impossible to find a logical error.",
        );
    }
    if !graph.has_declared_detectors {
        message.push_str(
            "\n    WARNING: NO DETECTORS. The circuit or detector error model didn't define any detectors.",
        );
    }
    if model.count_errors()? == 0 {
        message.push_str(
            "\n    WARNING: NO ERRORS. The circuit or detector error model didn't include any errors, making it vacuously impossible to find a logical error.",
        );
    } else if !graph_has_edges(graph) {
        message.push_str(
            "\n    WARNING: NO GRAPHLIKE ERRORS. Although the circuit or detector error model does define some errors, none of them are graphlike (i.e. have at most two detection events), making it vacuously impossible to find a graphlike logical error.",
        );
    }
    Ok(message)
}

fn graph_has_edges(graph: &Graph) -> bool {
    graph.nodes.iter().any(|node| !node.edges.is_empty())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact diagnostics"
    )]

    use super::*;

    #[test]
    fn default_policy_preserves_graphlike_search_result() {
        let model = DetectorErrorModel::from_dem_str("error(0.1) D0\nerror(0.1) D0 L0\n")
            .expect("valid search model");
        let legacy = shortest_graphlike_undetectable_logical_error(&model, false)
            .expect("default search succeeds");
        let explicit = shortest_graphlike_undetectable_logical_error_with_limits(
            &model,
            false,
            LogicalErrorSearchLimits::default(),
        )
        .expect("explicit default search succeeds");
        assert_eq!(legacy, explicit);
    }

    #[test]
    fn graphlike_search_state_boundary_is_inclusive() {
        let model = DetectorErrorModel::from_dem_str(
            "error(0.1) D0 L0\nerror(0.1) D0 L1\nerror(0.1) D0 L2\n",
        )
        .expect("valid search model");
        let exact = LogicalErrorSearchLimits::default().with_max_search_states(5);
        shortest_graphlike_undetectable_logical_error_with_limits(&model, false, exact)
            .expect("the first derived logical state reaches the exact boundary");

        let error = shortest_graphlike_undetectable_logical_error_with_limits(
            &model,
            false,
            exact.with_max_search_states(4),
        )
        .expect_err("first state past the limit");
        assert!(
            error
                .to_string()
                .contains("at most 4 search states, got at least 5")
        );
    }

    #[test]
    fn graphlike_search_rejects_excessive_search_states() {
        let mut text = String::new();
        for observable in 0..64 {
            text.push_str(&format!("error(0.1) D0 L{observable}\n"));
        }
        let model = DetectorErrorModel::from_dem_str(&text).expect("valid search model");
        let limits = LogicalErrorSearchLimits::default().with_max_search_states(64);
        let error =
            shortest_graphlike_undetectable_logical_error_with_limits(&model, false, limits)
                .expect_err("search state cap");
        assert!(error.to_string().contains("at most 64 search states"));
    }

    #[test]
    fn graphlike_search_bounds_variable_state_payloads() {
        let per_state = variable_payload_model(64, 2);
        let limits = LogicalErrorSearchLimits::default()
            .with_max_search_state_terms(64)
            .with_max_stored_search_state_terms(256);
        let error =
            shortest_graphlike_undetectable_logical_error_with_limits(&per_state, false, limits)
                .expect_err("per-state payload cap");
        assert!(
            error
                .to_string()
                .contains("at most 64 detector and observable terms per search state")
        );

        let aggregate = variable_payload_model(60, 4);
        let error =
            shortest_graphlike_undetectable_logical_error_with_limits(&aggregate, false, limits)
                .expect_err("aggregate payload cap");
        assert!(
            error
                .to_string()
                .contains("at most 256 stored detector and observable search-state terms")
        );
    }

    fn variable_payload_model(observables: usize, hops: usize) -> DetectorErrorModel {
        let mut text = String::from("error(0.1) D0 D1");
        for observable in 0..observables {
            text.push_str(&format!(" L{observable}"));
        }
        text.push_str("\nerror(0.1) D0 D2\n");
        for detector in 2..=hops {
            text.push_str(&format!("error(0.1) D{detector} D{}\n", detector + 1));
        }
        text.push_str(&format!("error(0.1) D{}\nerror(0.1) D1\n", hops + 1));
        DetectorErrorModel::from_dem_str(&text).expect("valid variable-payload model")
    }

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
    fn graphlike_algo_no_error_matches_upstream() {
        assert!(shortest("").is_err());
        assert!(shortest("error(0.1) D0 L0\n").is_err());
        assert!(shortest("error(0.1) D0\nerror(0.1) D0 D1\nerror(0.1) D1\n").is_err());
    }

    #[test]
    fn graphlike_algo_distance_1_matches_upstream() {
        assert_eq!(shortest("error(0.1) L0\n").unwrap(), "error(1) L0\n");
    }

    #[test]
    fn graphlike_algo_distance_2_matches_upstream() {
        assert_eq!(
            shortest("error(0.1) D0\nerror(0.1) D0 L0\n").unwrap(),
            "error(1) D0\nerror(1) D0 L0\n"
        );

        assert_eq!(
            shortest("error(0.1) D0 L0\nerror(0.1) D0 L1\n").unwrap(),
            "error(1) D0 L0\nerror(1) D0 L1\n"
        );

        assert_eq!(
            shortest("error(0.1) D0 D1 L0\nerror(0.1) D0 D1 L1\n").unwrap(),
            "error(1) D0 D1 L0\nerror(1) D0 D1 L1\n"
        );

        assert_eq!(
            shortest("error(0.1) D0 D1 L1\nerror(0.1) D0 D1 L0\n").unwrap(),
            "error(1) D0 D1 L0\nerror(1) D0 D1 L1\n"
        );
    }

    #[test]
    fn graphlike_algo_distance_3_matches_upstream() {
        assert_eq!(
            shortest("error(0.1) D0\nerror(0.1) D0 D1 L0\nerror(0.1) D1\n").unwrap(),
            "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n"
        );

        assert_eq!(
            shortest("error(0.1) D1\nerror(0.1) D1 D0 L0\nerror(0.1) D0\n").unwrap(),
            "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n"
        );
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
