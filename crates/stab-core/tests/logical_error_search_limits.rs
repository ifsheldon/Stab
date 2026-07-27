#![allow(
    clippy::expect_used,
    reason = "integration tests use deterministic valid DEM fixtures"
)]

use stab_core::{
    CircuitError, DetectorErrorModel, LogicalErrorSearchLimits, ResourceKind, ResourceOperation,
    find_undetectable_logical_error, find_undetectable_logical_error_with_limits,
    shortest_graphlike_undetectable_logical_error,
    shortest_graphlike_undetectable_logical_error_with_limits,
};

fn many_initial_states_model() -> DetectorErrorModel {
    let mut text = String::new();
    for observable in 0..65 {
        text.push_str(&format!("error(0.1) D0 L{observable}\n"));
    }
    DetectorErrorModel::from_dem_str(&text).expect("valid logical-error search fixture")
}

#[test]
fn graphlike_default_entry_point_uses_production_search_limits() {
    let result = shortest_graphlike_undetectable_logical_error(&many_initial_states_model(), false)
        .expect("65 initial states are below the production default");
    assert_eq!(result.count_errors().expect("count result errors"), 2);
}

#[test]
fn hypergraph_default_entry_point_uses_production_search_limits() {
    let result = find_undetectable_logical_error(&many_initial_states_model(), 3, 3, false)
        .expect("65 initial states are below the production default");
    assert_eq!(result.count_errors().expect("count result errors"), 2);
}

#[test]
fn public_search_entry_points_propagate_traversal_graph_and_frontier_limits() {
    let single_error =
        DetectorErrorModel::from_dem_str("error(0.1) D0 L0\n").expect("valid search fixture");
    let frontier = many_initial_states_model();
    let cases = [
        (
            &single_error,
            LogicalErrorSearchLimits::default().with_max_expanded_error_mechanisms(0),
            ResourceKind::ErrorMechanisms,
            1,
            0,
        ),
        (
            &single_error,
            LogicalErrorSearchLimits::default().with_max_effective_detector_nodes(0),
            ResourceKind::EffectiveDetectorNodes,
            1,
            0,
        ),
        (
            &frontier,
            LogicalErrorSearchLimits::default().with_max_search_states(64),
            ResourceKind::SearchStates,
            65,
            64,
        ),
    ];

    for (model, limits, expected_resource, expected_actual, expected_limit) in cases {
        let errors: [CircuitError; 2] = [
            shortest_graphlike_undetectable_logical_error_with_limits(model, false, limits)
                .expect_err("the graphlike public entry point should propagate the policy"),
            find_undetectable_logical_error_with_limits(
                model,
                usize::MAX,
                usize::MAX,
                false,
                limits,
            )
            .expect_err("the hypergraph public entry point should propagate the policy"),
        ];

        for error in errors {
            let resource = error
                .resource_limit_error()
                .expect("logical-error search limits should expose typed context");
            assert_eq!(resource.operation(), ResourceOperation::LogicalErrorSearch);
            assert_eq!(resource.resource(), expected_resource);
            assert_eq!(resource.actual(), expected_actual);
            assert_eq!(resource.limit(), expected_limit);
        }
    }
}

#[test]
fn public_search_policies_reject_before_retaining_the_first_excess_state() {
    let model = many_initial_states_model();
    let limits = LogicalErrorSearchLimits::default().with_max_search_states(64);
    for error in [
        shortest_graphlike_undetectable_logical_error_with_limits(&model, false, limits)
            .expect_err("the graphlike state policy should reject"),
        find_undetectable_logical_error_with_limits(&model, 3, 3, false, limits)
            .expect_err("the hypergraph state policy should reject"),
    ] {
        let resource = error
            .resource_limit_error()
            .expect("logical-error search limits should expose typed context");
        assert_eq!(resource.operation(), ResourceOperation::LogicalErrorSearch);
        assert_eq!(resource.resource(), ResourceKind::SearchStates);
        assert_eq!(resource.actual(), 65);
        assert_eq!(resource.limit(), 64);
    }
}
