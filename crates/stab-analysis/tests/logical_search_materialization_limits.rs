#![allow(
    clippy::expect_used,
    clippy::panic_in_result_fn,
    reason = "resource-contract tests use direct assertions for compact diagnostics"
)]

use stab_analysis::{
    AnalysisResult, LogicalErrorSearchLimits, ResourceKind, ResourceOperation,
    find_undetectable_logical_error, find_undetectable_logical_error_with_limits,
    shortest_graphlike_undetectable_logical_error,
    shortest_graphlike_undetectable_logical_error_with_limits,
};
use stab_model::DetectorErrorModel;

type SearchRunner =
    fn(&DetectorErrorModel, LogicalErrorSearchLimits) -> AnalysisResult<DetectorErrorModel>;

fn graphlike(
    model: &DetectorErrorModel,
    limits: LogicalErrorSearchLimits,
) -> AnalysisResult<DetectorErrorModel> {
    shortest_graphlike_undetectable_logical_error_with_limits(model, false, limits)
}

fn hypergraph(
    model: &DetectorErrorModel,
    limits: LogicalErrorSearchLimits,
) -> AnalysisResult<DetectorErrorModel> {
    find_undetectable_logical_error_with_limits(model, usize::MAX, usize::MAX, false, limits)
}

fn variable_payload_model(observables: usize, hops: usize) -> String {
    let mut text = String::from("error(0.1) D0 D1");
    for observable in 0..observables {
        text.push_str(&format!(" L{observable}"));
    }
    text.push_str("\nerror(0.1) D0 D2\n");
    for detector in 2..=hops {
        text.push_str(&format!("error(0.1) D{detector} D{}\n", detector + 1));
    }
    text.push_str(&format!("error(0.1) D{}\nerror(0.1) D1\n", hops + 1));
    text
}

fn assert_search_limit(
    name: &str,
    run: SearchRunner,
    source: &str,
    limits: LogicalErrorSearchLimits,
    expected_resource: ResourceKind,
) {
    let model = DetectorErrorModel::from_dem_str(source).expect("valid logical-search fixture");
    let original = model.clone();
    let error = run(&model, limits).expect_err("logical-search limit must reject the fixture");
    let resource = error
        .resource_limit_error()
        .expect("logical-search rejection must retain typed resource context");
    assert_eq!(
        resource.operation(),
        ResourceOperation::LogicalErrorSearch,
        "{name}"
    );
    assert_eq!(resource.resource(), expected_resource, "{name}: {error}");
    assert!(resource.actual() > resource.limit(), "{name}: {error}");
    assert_eq!(model, original, "{name} changed its source model");
}

fn assert_search_boundary_admitted(
    name: &str,
    run: SearchRunner,
    source: &str,
    limits: LogicalErrorSearchLimits,
) {
    let model = DetectorErrorModel::from_dem_str(source).expect("valid logical-search fixture");
    let original = model.clone();
    if let Err(error) = run(&model, limits) {
        assert!(
            error.resource_limit_error().is_none(),
            "{name} exact boundary was rejected: {error}"
        );
    }
    assert_eq!(model, original, "{name} changed its source model");
}

#[test]
fn logical_search_limits_return_typed_failures_without_mutating_source() -> AnalysisResult<()> {
    let defaults = LogicalErrorSearchLimits::default();
    let ordinary = DetectorErrorModel::from_dem_str("error(0.1) D0\nerror(0.1) D0 L0\n")?;
    assert_eq!(
        graphlike(&ordinary, defaults)?,
        shortest_graphlike_undetectable_logical_error(&ordinary, false)?
    );
    assert_eq!(
        hypergraph(&ordinary, defaults)?,
        find_undetectable_logical_error(&ordinary, usize::MAX, usize::MAX, false)?
    );

    let annotations = DetectorErrorModel::from_dem_str(
        "repeat 10001 {\ndetector D0\nlogical_observable L0\nshift_detectors 0\n}\nerror(0.1) L0\n",
    )?;
    graphlike(&annotations, defaults.with_max_expanded_error_mechanisms(1))?;

    for (name, run, source, limits) in [
        (
            "graphlike repeat count",
            graphlike as SearchRunner,
            "repeat 2 {\nerror(0.1) D0 L0\nshift_detectors 1\n}\n",
            defaults.with_max_repeat_unroll(2),
        ),
        (
            "hypergraph repeat iterations",
            hypergraph as SearchRunner,
            "repeat 2 {\nrepeat 2 {\nerror(0.1) D0 L0\nshift_detectors 1\n}\n}\n",
            defaults.with_max_repeat_iterations(6),
        ),
        (
            "hypergraph targets per mechanism",
            hypergraph as SearchRunner,
            "error(0.1) D0 D1 L0\n",
            defaults.with_max_error_target_occurrences_per_mechanism(3),
        ),
        (
            "graphlike total targets",
            graphlike as SearchRunner,
            "error(0.1) D0 L0\nerror(0.1) D1 L1\n",
            defaults.with_max_total_error_target_occurrences(4),
        ),
        (
            "hypergraph detector nodes",
            hypergraph as SearchRunner,
            "error(0.1) D0 L0\nerror(0.1) D1 L1\n",
            defaults.with_max_effective_detector_nodes(2),
        ),
    ] {
        assert_search_boundary_admitted(name, run, source, limits);
    }

    let variable_payload = variable_payload_model(8, 4);
    for (name, run, source, limits, resource) in [
        (
            "graphlike repeat count",
            graphlike as SearchRunner,
            "repeat 2 {\nerror(0.1) D0 L0\nshift_detectors 1\n}\n",
            defaults.with_max_repeat_unroll(1),
            ResourceKind::RepeatCount,
        ),
        (
            "hypergraph repeat iterations",
            hypergraph as SearchRunner,
            "repeat 2 {\nrepeat 2 {\nerror(0.1) D0 L0\nshift_detectors 1\n}\n}\n",
            defaults.with_max_repeat_iterations(5),
            ResourceKind::RepeatIterations,
        ),
        (
            "graphlike mechanisms",
            graphlike as SearchRunner,
            "error(0.1) D0 L0\nerror(0.2) D0\n",
            defaults.with_max_expanded_error_mechanisms(1),
            ResourceKind::ErrorMechanisms,
        ),
        (
            "hypergraph targets per mechanism",
            hypergraph as SearchRunner,
            "error(0.1) D0 D1 L0\n",
            defaults.with_max_error_target_occurrences_per_mechanism(2),
            ResourceKind::ErrorTargetOccurrencesPerMechanism,
        ),
        (
            "graphlike total targets",
            graphlike as SearchRunner,
            "error(0.1) D0 L0\nerror(0.1) D1 L1\n",
            defaults.with_max_total_error_target_occurrences(3),
            ResourceKind::TotalErrorTargetOccurrences,
        ),
        (
            "hypergraph detector nodes",
            hypergraph as SearchRunner,
            "error(0.1) D0 L0\nerror(0.1) D1 L1\n",
            defaults.with_max_effective_detector_nodes(1),
            ResourceKind::EffectiveDetectorNodes,
        ),
        (
            "graphlike unique edges",
            graphlike as SearchRunner,
            "error(0.1) D0 L0\nerror(0.1) D0 L1\n",
            defaults.with_max_unique_graph_edges(1),
            ResourceKind::UniqueGraphEdges,
        ),
        (
            "graphlike stored graph terms",
            graphlike as SearchRunner,
            "error(0.1) D0 L0 L1\n",
            defaults.with_max_stored_graph_terms(2),
            ResourceKind::StoredGraphTerms,
        ),
        (
            "hypergraph edge degree",
            hypergraph as SearchRunner,
            "error(0.1) D0 D1 L0\n",
            defaults.with_max_hyperedge_degree(1),
            ResourceKind::HyperedgeDegree,
        ),
        (
            "hypergraph edge incidences",
            hypergraph as SearchRunner,
            "error(0.1) D0 D1 L0\nerror(0.1) D0 D1 L1\n",
            defaults
                .with_max_hyperedge_degree(2)
                .with_max_hyperedge_incidences(2),
            ResourceKind::HyperedgeIncidences,
        ),
        (
            "graphlike search states",
            graphlike as SearchRunner,
            "error(0.1) D0 L0\nerror(0.1) D0 L1\nerror(0.1) D0 L2\n",
            defaults.with_max_search_states(4),
            ResourceKind::SearchStates,
        ),
        (
            "hypergraph transitions",
            hypergraph as SearchRunner,
            "error(0.1) D0 L0\nerror(0.1) D0 L1\nerror(0.1) D0 L2\n",
            defaults.with_max_search_transitions(0),
            ResourceKind::SearchTransitions,
        ),
        (
            "graphlike state terms",
            graphlike as SearchRunner,
            variable_payload.as_str(),
            defaults.with_max_search_state_terms(8),
            ResourceKind::SearchStateTerms,
        ),
        (
            "hypergraph retained state terms",
            hypergraph as SearchRunner,
            variable_payload.as_str(),
            defaults
                .with_max_search_state_terms(64)
                .with_max_stored_search_state_terms(8),
            ResourceKind::StoredSearchStateTerms,
        ),
    ] {
        assert_search_limit(name, run, source, limits, resource);
    }
    Ok(())
}
