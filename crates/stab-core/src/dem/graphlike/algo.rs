use std::collections::{BTreeMap, VecDeque};

use super::{Graph, ObservableMask, SearchState};
use crate::{CircuitError, CircuitResult, DemItem, DetectorErrorModel};

pub(in crate::dem) fn shortest_graphlike_undetectable_logical_error(
    model: &DetectorErrorModel,
    ignore_ungraphlike_errors: bool,
) -> CircuitResult<DetectorErrorModel> {
    let graph = Graph::from_dem(model, ignore_ungraphlike_errors)?;
    let empty = SearchState::new(None, None, ObservableMask::new());
    if !graph.distance_1_error_mask.is_empty() {
        let mut out = DetectorErrorModel::new();
        SearchState::new(None, None, graph.distance_1_error_mask)
            .append_transition_as_error_instruction_to(&empty, &mut out)?;
        return Ok(out);
    }

    let mut queue = VecDeque::new();
    let mut back_map = BTreeMap::new();
    back_map.insert(empty.clone(), empty.clone());

    for (source_index, node) in graph.nodes.iter().enumerate() {
        let source = graph.detector_for_node_index(source_index)?;
        for edge in &node.edges {
            if !edge.observables.is_empty() && edge.detector.is_none_or(|target| source < target) {
                let start = SearchState::new(Some(source), edge.detector, edge.observables.clone());
                if back_map.insert(start.clone(), empty.clone()).is_none() {
                    queue.push_back(start);
                }
            }
        }
    }

    while let Some(current) = queue.pop_front() {
        let Some(active) = current.detector_active else {
            return Err(CircuitError::invalid_detector_error_model(
                "graphlike search reached a state without an active detector",
            ));
        };
        let active_index = graph.node_index_for_detector(active)?;
        let Some(node) = graph.nodes.get(active_index) else {
            return Err(CircuitError::invalid_detector_error_model(
                "graphlike active detector is outside the graph",
            ));
        };
        for edge in &node.edges {
            let mut next = SearchState::new(
                edge.detector,
                current.detector_held,
                edge.observables.symmetric_difference(&current.observables),
            );
            if back_map.contains_key(&next) {
                continue;
            }
            back_map.insert(next.clone(), current.clone());
            if next.is_undetected() {
                return backtrack_path(&back_map, &next);
            }
            if next.detector_active.is_none() {
                std::mem::swap(&mut next.detector_active, &mut next.detector_held);
            }
            queue.push_back(next);
        }
    }

    Err(CircuitError::invalid_detector_error_model(
        no_graphlike_logical_error_message(model, &graph)?,
    ))
}

fn backtrack_path(
    back_map: &BTreeMap<SearchState, SearchState>,
    final_state: &SearchState,
) -> CircuitResult<DetectorErrorModel> {
    let mut out = DetectorErrorModel::new();
    let mut current = final_state.clone();
    loop {
        let Some(previous) = back_map.get(&current) else {
            return Err(CircuitError::invalid_detector_error_model(
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

fn sorted_error_model(model: DetectorErrorModel) -> CircuitResult<DetectorErrorModel> {
    let mut instructions = Vec::new();
    for item in model.items() {
        let DemItem::Instruction(instruction) = item else {
            return Err(CircuitError::invalid_detector_error_model(
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
) -> CircuitResult<String> {
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
    if count_nonzero_error_instructions(model)? == 0 {
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

fn count_nonzero_error_instructions(model: &DetectorErrorModel) -> CircuitResult<usize> {
    let mut total = 0usize;
    for item in model.items() {
        let count = match item {
            DemItem::Instruction(instruction) => usize::from(
                instruction.kind() == crate::DemInstructionKind::Error
                    && instruction.args().first().copied().unwrap_or(0.0) != 0.0,
            ),
            DemItem::RepeatBlock(repeat) => {
                let repeat_count = usize::try_from(repeat.repeat_count().get()).map_err(|_| {
                    CircuitError::invalid_detector_error_model(
                        "repeat count does not fit usize while counting graphlike errors",
                    )
                })?;
                repeat_count
                    .checked_mul(count_nonzero_error_instructions(repeat.body())?)
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "graphlike error count overflowed",
                        )
                    })?
            }
        };
        total = total.checked_add(count).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("graphlike error count overflowed")
        })?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact diagnostics"
    )]

    use super::*;

    fn shortest(dem: &str) -> CircuitResult<String> {
        let model = DetectorErrorModel::from_dem_str(dem)?;
        shortest_graphlike_undetectable_logical_error(&model, false)
            .map(|error| error.to_dem_string())
    }

    fn shortest_ignoring_ungraphlike(dem: &str) -> CircuitResult<String> {
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
