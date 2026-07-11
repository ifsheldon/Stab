#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "unit tests use direct assertions for compact diagnostics"
)]

use super::{
    DemTarget, DetectorErrorModel, Graph, MAX_HYPERGRAPH_EDGE_DEGREE,
    find_undetectable_logical_error,
};

#[test]
fn hyper_graph_rejects_excessive_edge_degree_before_adjacency_allocation() {
    let mut graph = Graph::new(MAX_HYPERGRAPH_EDGE_DEGREE + 1, 0);
    let targets = (0..=MAX_HYPERGRAPH_EDGE_DEGREE)
        .map(|detector| DemTarget::relative_detector(detector as u64).unwrap())
        .collect::<Vec<_>>();

    let error = graph
        .add_edge_from_dem_targets(&targets, usize::MAX)
        .expect_err("hard edge-degree cap");
    assert!(
        error
            .to_string()
            .contains("edges with at most 64 detectors")
    );
    assert!(graph.edges.is_empty());
    assert_eq!(graph.edge_incidences, 0);
}

#[test]
fn hyper_graph_rejects_excessive_edge_incidences_before_allocation() {
    let mut graph = Graph::new(MAX_HYPERGRAPH_EDGE_DEGREE, 5);
    let detector_targets = (0..MAX_HYPERGRAPH_EDGE_DEGREE)
        .map(|detector| DemTarget::relative_detector(detector as u64).unwrap())
        .collect::<Vec<_>>();
    for observable in 0..4 {
        let mut targets = detector_targets.clone();
        targets.push(DemTarget::logical_observable(observable).unwrap());
        graph
            .add_edge_from_dem_targets(&targets, usize::MAX)
            .unwrap();
    }

    let mut rejected = detector_targets;
    rejected.push(DemTarget::logical_observable(4).unwrap());
    let error = graph
        .add_edge_from_dem_targets(&rejected, usize::MAX)
        .expect_err("hard edge-incidence cap");
    assert!(error.to_string().contains("at most 256 edge incidences"));
    assert_eq!(graph.edges.len(), 4);
    assert_eq!(graph.edge_incidences, 256);
}

#[test]
fn hypergraph_search_bounds_variable_state_payloads() {
    let per_state = variable_payload_model(64, 2);
    let error = find_undetectable_logical_error(&per_state, usize::MAX, usize::MAX, false)
        .expect_err("per-state payload cap");
    assert!(
        error
            .to_string()
            .contains("at most 64 detector and observable terms per search state")
    );

    let aggregate = variable_payload_model(60, 4);
    let error = find_undetectable_logical_error(&aggregate, usize::MAX, usize::MAX, false)
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
