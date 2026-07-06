#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests use direct assertions for compact diagnostics"
)]

use std::collections::BTreeSet;

use stab_core::{
    Circuit, CircuitResult, CodeDistance, DemInstruction, DemInstructionKind, DemItem, DemTarget,
    DetectorErrorModel, ErrorAnalyzerOptions, Probability, RepetitionCodeParams,
    RepetitionCodeTask, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    circuit_to_detector_error_model, find_undetectable_logical_error,
    generate_repetition_code_circuit, generate_surface_code_circuit, likeliest_error_sat_problem,
    shortest_error_sat_problem, shortest_graphlike_undetectable_logical_error,
};

#[test]
fn pf4_dem_search_and_sat_repeat_resource_policy_is_source_owned() {
    let allowed = DetectorErrorModel::from_dem_str(
        "error(0.1) D0\nrepeat 2 {\n    error(0.1) D0 D1\n    shift_detectors 1\n}\nerror(0.1) D0 L0\n",
    )
    .unwrap();
    let expected = "error(1) D0\nerror(1) D0 D1\nerror(1) D1 D2\nerror(1) D2 L0\n";

    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&allowed, false)
            .unwrap()
            .to_dem_string(),
        expected
    );
    assert_eq!(
        find_undetectable_logical_error(&allowed, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        expected
    );
    let sat_problem = shortest_error_sat_problem(&allowed).unwrap();
    assert_eq!(
        sat_problem
            .lines()
            .filter(|line| line.starts_with("1 -"))
            .count(),
        4,
        "SAT problem should include one soft clause per expanded shifted-repeat error"
    );

    let hostile = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1) D0\n    shift_detectors 1\n}\nerror(0.1) D0 L0\n",
    )
    .unwrap();

    let graphlike_error = shortest_graphlike_undetectable_logical_error(&hostile, true)
        .expect_err("graphlike search should reject hostile repeat expansion")
        .to_string();
    assert!(
        graphlike_error
            .contains("DEM graphlike search currently supports repeat counts up to 100000"),
        "{graphlike_error}"
    );

    let hyper_error = find_undetectable_logical_error(&hostile, usize::MAX, usize::MAX, false)
        .expect_err("hypergraph search should reject hostile repeat expansion")
        .to_string();
    assert!(
        hyper_error.contains("DEM hypergraph search currently supports repeat counts up to 100000"),
        "{hyper_error}"
    );

    let sat_error = shortest_error_sat_problem(&hostile)
        .expect_err("SAT problem generation should reject hostile repeat expansion")
        .to_string();
    assert!(
        sat_error
            .contains("DEM SAT problem generation currently supports repeat counts up to 100000"),
        "{sat_error}"
    );
}

#[test]
fn pf4_dem_search_skips_zero_probability_repeat_bodies() {
    let zero_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0) D1000000 L1000\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
    )
    .unwrap();
    let expected = "error(1) D0\nerror(1) D0 L0\n";

    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&zero_repeat, false)
            .unwrap()
            .to_dem_string(),
        expected
    );
    assert_eq!(
        find_undetectable_logical_error(&zero_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        expected
    );

    let sat_error = shortest_error_sat_problem(&zero_repeat)
        .expect_err("unweighted SAT should keep high-index structural zero-probability caps")
        .to_string();
    assert!(
        sat_error.contains(
            "SAT problem generation currently supports at most 1000000 effective detector nodes"
        ),
        "{sat_error}"
    );
}

#[test]
fn pf4_dem_search_weighted_sat_skips_zero_probability_repeat_bodies() {
    let zero_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0) D1000000 L1000\n    shift_detectors 1\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
    )
    .unwrap();
    let expected = likeliest_error_sat_problem(
        &DetectorErrorModel::from_dem_str("error(0.1) D0\nerror(0.1) D0 L0\n").unwrap(),
        10,
    )
    .unwrap();

    assert_eq!(
        likeliest_error_sat_problem(&zero_repeat, 10).unwrap(),
        expected
    );

    let sat_error = shortest_error_sat_problem(&zero_repeat)
        .expect_err("unweighted SAT should retain structural zero-probability repeat cap")
        .to_string();
    assert!(
        sat_error
            .contains("DEM SAT problem generation currently supports repeat counts up to 100000"),
        "{sat_error}"
    );
}

#[test]
fn pf4_dem_search_sat_folds_flat_nonzero_zero_shift_repeat_bodies() {
    let flat_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1) D0 L0\n    error(0.2) D0\n}\n",
    )
    .unwrap();
    let expected_shortest = shortest_error_sat_problem(
        &DetectorErrorModel::from_dem_str("error(0.1) D0 L0\nerror(0.2) D0\n").unwrap(),
    )
    .unwrap();
    assert_eq!(
        shortest_error_sat_problem(&flat_repeat).unwrap(),
        expected_shortest
    );

    let weighted_flat_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.000001) D0 L0\n    error(0.25) D1 L1\n}\nerror(0.1) D0\nerror(0.1) D0 L0\nerror(0.1) D1 L1\n",
    )
    .unwrap();
    let expected_weighted = likeliest_error_sat_problem(
        &DetectorErrorModel::from_dem_str(
            "error(0.000001) D0 L0\nerror(0.25) D1 L1\nerror(0.1) D0\nerror(0.1) D0 L0\nerror(0.1) D1 L1\n",
        )
        .unwrap(),
        100,
    )
    .unwrap();
    assert_eq!(
        likeliest_error_sat_problem(&weighted_flat_repeat, 100).unwrap(),
        expected_weighted
    );

    let shifted_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1) D0 L0\n    shift_detectors 1\n}\n",
    )
    .unwrap();
    let shifted_error = shortest_error_sat_problem(&shifted_repeat)
        .expect_err("shifted active SAT repeats should retain the cap")
        .to_string();
    assert!(
        shifted_error
            .contains("DEM SAT problem generation currently supports repeat counts up to 100000"),
        "{shifted_error}"
    );
}

#[test]
fn pf4_dem_search_sat_folds_flat_zero_probability_zero_shift_repeat_bodies() {
    let flat_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0) D0 L0\n    error(0) D0\n}\n",
    )
    .unwrap();
    let compact = DetectorErrorModel::from_dem_str("error(0) D0 L0\nerror(0) D0\n").unwrap();
    let expected_shortest = shortest_error_sat_problem(&compact).unwrap();
    assert_eq!(
        shortest_error_sat_problem(&flat_repeat).unwrap(),
        expected_shortest
    );
}

#[test]
fn pf4_dem_search_folds_flat_nonzero_zero_shift_repeat_bodies() {
    let flat_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1) D0 L0\n    error(0.2) D0\n}\n",
    )
    .unwrap();
    let compact = DetectorErrorModel::from_dem_str("error(0.1) D0 L0\nerror(0.2) D0\n").unwrap();

    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&flat_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&compact, false)
            .unwrap()
            .to_dem_string()
    );
    assert_eq!(
        find_undetectable_logical_error(&flat_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string()
    );

    let logical_only_repeat =
        DetectorErrorModel::from_dem_str("repeat 100001 {\n    error(0.1) L0\n}\n").unwrap();
    let logical_only_compact = DetectorErrorModel::from_dem_str("error(0.1) L0\n").unwrap();
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&logical_only_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&logical_only_compact, false)
            .unwrap()
            .to_dem_string()
    );

    let logical_only_pair_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1) L0\n    error(0.2) L1\n}\n",
    )
    .unwrap();
    let logical_only_pair_compact =
        DetectorErrorModel::from_dem_str("error(0.1) L0\nerror(0.2) L1\n").unwrap();
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&logical_only_pair_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&logical_only_pair_compact, false)
            .unwrap()
            .to_dem_string()
    );
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&logical_only_pair_compact, false)
            .unwrap()
            .to_dem_string(),
        "error(1) L0\n"
    );
}

#[test]
fn pf4_hypergraph_logical_only_repeat_folds_by_compact_model() {
    let logical_only_repeat =
        DetectorErrorModel::from_dem_str("repeat 100001 {\n    error(0.1) L0\n}\n").unwrap();
    let logical_only_compact = DetectorErrorModel::from_dem_str("error(0.1) L0\n").unwrap();
    assert_eq!(
        find_undetectable_logical_error(&logical_only_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&logical_only_compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string()
    );

    let logical_only_pair_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1) L0\n    error(0.2) L1\n}\n",
    )
    .unwrap();
    let logical_only_pair_compact =
        DetectorErrorModel::from_dem_str("error(0.1) L0\nerror(0.2) L1\n").unwrap();
    assert_eq!(
        find_undetectable_logical_error(&logical_only_pair_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&logical_only_pair_compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string()
    );
    assert_eq!(
        find_undetectable_logical_error(&logical_only_pair_compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        "error(1) L1\n"
    );
}

#[test]
fn pf4_dem_search_skips_flat_nonzero_no_target_repeat_bodies() {
    let no_target_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1)\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
    )
    .unwrap();
    let active_compact =
        DetectorErrorModel::from_dem_str("error(0.1) D0\nerror(0.1) D0 L0\n").unwrap();
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&no_target_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&active_compact, false)
            .unwrap()
            .to_dem_string()
    );

    let mixed_repeat =
        DetectorErrorModel::from_dem_str("repeat 100001 {\n    error(0.1)\n    error(0.2) L0\n}\n")
            .unwrap();
    let mixed_compact = DetectorErrorModel::from_dem_str("error(0.1)\nerror(0.2) L0\n").unwrap();
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&mixed_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&mixed_compact, false)
            .unwrap()
            .to_dem_string()
    );

    assert_numeric_error_target_rejected();
}

#[test]
fn pf4_hypergraph_no_target_repeat_skips_by_compact_model() {
    let no_target_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1)\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
    )
    .unwrap();
    let active_compact =
        DetectorErrorModel::from_dem_str("error(0.1) D0\nerror(0.1) D0 L0\n").unwrap();
    assert_eq!(
        find_undetectable_logical_error(&no_target_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&active_compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string()
    );

    let mixed_repeat =
        DetectorErrorModel::from_dem_str("repeat 100001 {\n    error(0.1)\n    error(0.2) L0\n}\n")
            .unwrap();
    let mixed_compact = DetectorErrorModel::from_dem_str("error(0.1)\nerror(0.2) L0\n").unwrap();
    assert_eq!(
        find_undetectable_logical_error(&mixed_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&mixed_compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string()
    );

    assert_numeric_error_target_rejected();
}

#[test]
fn pf4_dem_search_folds_flat_zero_detector_shift_repeat_bodies() {
    let no_target_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    shift_detectors 0\n    error(0.1)\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
    )
    .unwrap();
    let active_compact =
        DetectorErrorModel::from_dem_str("error(0.1) D0\nerror(0.1) D0 L0\n").unwrap();
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&no_target_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&active_compact, false)
            .unwrap()
            .to_dem_string()
    );

    let mixed_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1)\n    shift_detectors 0\n    error(0.2) L0\n}\n",
    )
    .unwrap();
    let mixed_compact =
        DetectorErrorModel::from_dem_str("error(0.1)\nshift_detectors 0\nerror(0.2) L0\n").unwrap();
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&mixed_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&mixed_compact, false)
            .unwrap()
            .to_dem_string()
    );

    let (detector_touching_repeat, detector_touching_compact) =
        zero_detector_shift_detector_touching_models();
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&detector_touching_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&detector_touching_compact, false)
            .unwrap()
            .to_dem_string()
    );

    assert_nonzero_shift_search_repeat_rejected();
}

#[test]
fn pf4_hypergraph_zero_detector_shift_repeat_folds_by_compact_model() {
    let no_target_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    shift_detectors 0\n    error(0.1)\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
    )
    .unwrap();
    let active_compact =
        DetectorErrorModel::from_dem_str("error(0.1) D0\nerror(0.1) D0 L0\n").unwrap();
    assert_eq!(
        find_undetectable_logical_error(&no_target_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&active_compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string()
    );

    let mixed_repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1)\n    shift_detectors 0\n    error(0.2) L0\n}\n",
    )
    .unwrap();
    let mixed_compact =
        DetectorErrorModel::from_dem_str("error(0.1)\nshift_detectors 0\nerror(0.2) L0\n").unwrap();
    assert_eq!(
        find_undetectable_logical_error(&mixed_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&mixed_compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string()
    );

    let (detector_touching_repeat, detector_touching_compact) =
        zero_detector_shift_detector_touching_models();
    assert_eq!(
        find_undetectable_logical_error(&detector_touching_repeat, usize::MAX, usize::MAX, false,)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&detector_touching_compact, usize::MAX, usize::MAX, false,)
            .unwrap()
            .to_dem_string()
    );

    assert_nonzero_shift_search_repeat_rejected();
}

fn zero_detector_shift_detector_touching_models() -> (DetectorErrorModel, DetectorErrorModel) {
    let repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1) D0\n    shift_detectors(4, 5) 0\n    error(0.2) D0 L0\n}\n",
    )
    .unwrap();
    let compact = DetectorErrorModel::from_dem_str(
        "error(0.1) D0\nshift_detectors(4, 5) 0\nerror(0.2) D0 L0\n",
    )
    .unwrap();
    (repeat, compact)
}

#[test]
fn pf4_dem_search_folds_flat_annotation_repeat_bodies() {
    let (detector_touching_repeat, detector_touching_compact) = annotation_repeat_models();
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&detector_touching_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&detector_touching_compact, false)
            .unwrap()
            .to_dem_string()
    );

    let (logical_only_repeat, logical_only_compact) = annotation_logical_only_repeat_models();
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&logical_only_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&logical_only_compact, false)
            .unwrap()
            .to_dem_string()
    );

    assert_nonzero_shift_search_repeat_rejected();
}

#[test]
fn pf4_hypergraph_annotation_repeat_folds_by_compact_model() {
    let (detector_touching_repeat, detector_touching_compact) = annotation_repeat_models();
    assert_eq!(
        find_undetectable_logical_error(&detector_touching_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&detector_touching_compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string()
    );

    let (logical_only_repeat, logical_only_compact) = annotation_logical_only_repeat_models();
    assert_eq!(
        find_undetectable_logical_error(&logical_only_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&logical_only_compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string()
    );

    assert_nonzero_shift_search_repeat_rejected();
}

fn annotation_repeat_models() -> (DetectorErrorModel, DetectorErrorModel) {
    let repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    detector(1, 2) D0\n    logical_observable L2\n    error(0.1) D0\n    error(0.2) D0 L0\n}\n",
    )
    .unwrap();
    let compact = DetectorErrorModel::from_dem_str(
        "detector(1, 2) D0\nlogical_observable L2\nerror(0.1) D0\nerror(0.2) D0 L0\n",
    )
    .unwrap();
    (repeat, compact)
}

fn annotation_logical_only_repeat_models() -> (DetectorErrorModel, DetectorErrorModel) {
    let repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    detector(3, 4) D0\n    logical_observable L3\n    error(0.1) L0\n}\n",
    )
    .unwrap();
    let compact = DetectorErrorModel::from_dem_str(
        "detector(3, 4) D0\nlogical_observable L3\nerror(0.1) L0\n",
    )
    .unwrap();
    (repeat, compact)
}

fn assert_numeric_error_target_rejected() {
    let error = DemInstruction::error(
        Probability::try_new(0.1).unwrap(),
        vec![DemTarget::numeric(5)],
        None,
    )
    .expect_err("numeric error targets should stay rejected at the typed boundary")
    .to_string();
    assert!(error.contains("raw numbers"), "{error}");
}

fn assert_nonzero_shift_search_repeat_rejected() {
    let shifted = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    detector D0\n    logical_observable L1\n    error(0.1)\n    shift_detectors 1\n    error(0.2) D0 L0\n}\n",
    )
    .unwrap();
    let graphlike_error = shortest_graphlike_undetectable_logical_error(&shifted, false)
        .expect_err("nonzero detector shifts should remain outside the selected graphlike fold")
        .to_string();
    assert!(
        graphlike_error.contains("supports repeat counts"),
        "{graphlike_error}"
    );
    let hyper_error = find_undetectable_logical_error(&shifted, usize::MAX, usize::MAX, false)
        .expect_err("nonzero detector shifts should remain outside the selected hypergraph fold")
        .to_string();
    assert!(
        hyper_error.contains("supports repeat counts"),
        "{hyper_error}"
    );
}

#[test]
fn pf4_dem_search_weighted_sat_rejects_shifted_zero_probability_repeat_node_explosion() {
    let hostile = DetectorErrorModel::from_dem_str(
        "repeat 1000001 {\n    error(0) D0\n    shift_detectors 1\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
    )
    .unwrap();

    let sat_error = likeliest_error_sat_problem(&hostile, 10)
        .expect_err("weighted SAT should reject huge shifted dense detector allocation")
        .to_string();
    assert!(
        sat_error.contains(
            "SAT problem generation currently supports at most 1000000 effective detector nodes"
        ),
        "{sat_error}"
    );
}

#[test]
fn pf4_dem_search_rejects_shifted_zero_probability_repeat_node_explosion() {
    let hostile = DetectorErrorModel::from_dem_str(
        "repeat 1000001 {\n    error(0) D0\n    shift_detectors 1\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
    )
    .unwrap();

    let graphlike_error = shortest_graphlike_undetectable_logical_error(&hostile, false)
        .expect_err("graphlike search should reject excessive effective detector nodes")
        .to_string();
    assert!(
        graphlike_error.contains(
            "graphlike search currently supports at most 1000000 effective detector nodes"
        ),
        "{graphlike_error}"
    );

    let hyper_error = find_undetectable_logical_error(&hostile, usize::MAX, usize::MAX, false)
        .expect_err("hypergraph search should reject excessive effective detector nodes")
        .to_string();
    assert!(
        hyper_error.contains(
            "hypergraph search currently supports at most 1000000 effective detector nodes"
        ),
        "{hyper_error}"
    );
}

#[test]
fn pf6_direct_dem_graphlike_search_matches_upstream_distance_cases() {
    for dem_text in [
        "",
        "error(0.1) D0 L0\n",
        "error(0.1) D0\nerror(0.1) D0 D1\nerror(0.1) D1\n",
    ] {
        let model = DetectorErrorModel::from_dem_str(dem_text).unwrap();
        assert!(
            shortest_graphlike_undetectable_logical_error(&model, false).is_err(),
            "direct graphlike search should reject model without an undetectable logical error: {dem_text:?}"
        );
    }

    let cases = [
        ("error(0.1) L0\n", "error(1) L0\n"),
        (
            "error(0.1) D0\nerror(0.1) D0 L0\n",
            "error(1) D0\nerror(1) D0 L0\n",
        ),
        (
            "error(0.1) D0 L0\nerror(0.1) D0 L1\n",
            "error(1) D0 L0\nerror(1) D0 L1\n",
        ),
        (
            "error(0.1) D0 D1 L0\nerror(0.1) D0 D1 L1\n",
            "error(1) D0 D1 L0\nerror(1) D0 D1 L1\n",
        ),
        (
            "error(0.1) D0 D1 L1\nerror(0.1) D0 D1 L0\n",
            "error(1) D0 D1 L0\nerror(1) D0 D1 L1\n",
        ),
        (
            "error(0.1) D0\nerror(0.1) D0 D1 L0\nerror(0.1) D1\n",
            "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n",
        ),
        (
            "error(0.1) D1\nerror(0.1) D1 D0 L0\nerror(0.1) D0\n",
            "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n",
        ),
    ];
    for (dem_text, expected) in cases {
        let model = DetectorErrorModel::from_dem_str(dem_text).unwrap();
        assert_eq!(
            shortest_graphlike_undetectable_logical_error(&model, false)
                .unwrap()
                .to_dem_string(),
            expected,
            "direct graphlike search mismatch for {dem_text:?}"
        );
    }
}

#[test]
fn pf6_direct_dem_hypergraph_search_matches_upstream_distance_cases() {
    for dem_text in [
        "",
        "error(0.1) D0 L0\n",
        "error(0.1) D0\nerror(0.1) D0 D1\nerror(0.1) D1\n",
    ] {
        let model = DetectorErrorModel::from_dem_str(dem_text).unwrap();
        assert!(
            find_undetectable_logical_error(&model, usize::MAX, usize::MAX, false).is_err(),
            "direct hypergraph search should reject model without an undetectable logical error: {dem_text:?}"
        );
    }

    let cases = [
        (
            "error(0.1) L0\n",
            "error(1) L0\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D0\nerror(0.1) D0 L0\n",
            "error(1) D0\nerror(1) D0 L0\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D0 L0\nerror(0.1) D0 L1\n",
            "error(1) D0 L0\nerror(1) D0 L1\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D0 D1 L0\nerror(0.1) D0 D1 L1\n",
            "error(1) D0 D1 L0\nerror(1) D0 D1 L1\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D0 D1 L1\nerror(0.1) D0 D1 L0\n",
            "error(1) D0 D1 L0\nerror(1) D0 D1 L1\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D0\nerror(0.1) D0 D1 L0\nerror(0.1) D1\n",
            "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D1\nerror(0.1) D1 D0 L0\nerror(0.1) D0\n",
            "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D0 D1\nerror(0.1) D0 D1 D2 D3\nerror(0.1) D2 D3 D4 D5 L0\nerror(0.1) D4 D5 D6 D7\nerror(0.1) D6 D7 D8 D9\nerror(0.1) D8\nerror(0.1) D9\n",
            "error(1) D0 D1\nerror(1) D0 D1 D2 D3\nerror(1) D2 D3 D4 D5 L0\nerror(1) D4 D5 D6 D7\nerror(1) D6 D7 D8 D9\nerror(1) D8\nerror(1) D9\n",
            4,
            4,
            true,
        ),
    ];
    for (dem_text, expected, max_event_count, max_edge_degree, allow_increasing_error_degree) in
        cases
    {
        let model = DetectorErrorModel::from_dem_str(dem_text).unwrap();
        assert_eq!(
            find_undetectable_logical_error(
                &model,
                max_event_count,
                max_edge_degree,
                allow_increasing_error_degree
            )
            .unwrap()
            .to_dem_string(),
            expected,
            "direct hypergraph search mismatch for {dem_text:?}"
        );
    }
}

#[test]
fn pf6_generated_qec_graphlike_search_has_structural_signature() {
    let surface = generated_rotated_surface_code_dem().unwrap();
    assert_graphlike_search_signature(
        &shortest_graphlike_undetectable_logical_error(&surface, false).unwrap(),
        5,
    );
    assert_graphlike_search_signature(
        &shortest_graphlike_undetectable_logical_error(&surface, true).unwrap(),
        5,
    );

    let repetition = generated_repetition_code_dem().unwrap();
    assert_graphlike_search_signature(
        &shortest_graphlike_undetectable_logical_error(&repetition, false).unwrap(),
        7,
    );

    let ungraphlike_surface = generated_rotated_surface_code_ungraphlike_dem().unwrap();
    assert_graphlike_search_signature(
        &shortest_graphlike_undetectable_logical_error(&ungraphlike_surface, true).unwrap(),
        5,
    );
    let error = shortest_graphlike_undetectable_logical_error(&ungraphlike_surface, false)
        .expect_err("ungraphlike generated DEM should be rejected without ignore flag")
        .to_string();
    assert!(error.contains("non-graphlike error mechanism"), "{error}");
}

#[test]
fn pf6_generated_qec_hypergraph_search_has_structural_signature() {
    let surface = generated_rotated_surface_code_dem().unwrap();
    assert_hypergraph_search_signature(
        &find_undetectable_logical_error(&surface, 4, 4, true).unwrap(),
        5,
    );

    let repetition = generated_repetition_code_dem().unwrap();
    assert_hypergraph_search_signature(
        &find_undetectable_logical_error(&repetition, 4, 4, false).unwrap(),
        7,
    );

    let ungraphlike_surface = generated_rotated_surface_code_ungraphlike_dem().unwrap();
    assert_hypergraph_search_signature(
        &find_undetectable_logical_error(&ungraphlike_surface, 4, 4, true).unwrap(),
        5,
    );
}

#[test]
fn pf6_graphlike_search_many_observables_preserves_high_observable_id() {
    let model = many_observables_dem().unwrap();
    assert_search_result_signature_for_observable(
        &shortest_graphlike_undetectable_logical_error(&model, false).unwrap(),
        5,
        2,
        1200,
    );
}

#[test]
fn pf6_hypergraph_search_many_observables_preserves_high_observable_id() {
    let model = many_observables_dem().unwrap();
    assert_search_result_signature_for_observable(
        &find_undetectable_logical_error(&model, 4, 4, false).unwrap(),
        5,
        4,
        1200,
    );
}

#[test]
fn pf6_generated_sat_wcnf_qec_encoding_is_structural() {
    let surface = generated_small_rotated_surface_code_dem(true).unwrap();
    assert_unweighted_wcnf_shape(&shortest_error_sat_problem(&surface).unwrap());
    assert_weighted_wcnf_shape(&likeliest_error_sat_problem(&surface, 100).unwrap());

    let repetition = generated_small_repetition_code_dem().unwrap();
    assert_unweighted_wcnf_shape(&shortest_error_sat_problem(&repetition).unwrap());
    assert_weighted_wcnf_shape(&likeliest_error_sat_problem(&repetition, 100).unwrap());

    let ungraphlike_surface = generated_small_rotated_surface_code_dem(false).unwrap();
    assert_unweighted_wcnf_shape(&shortest_error_sat_problem(&ungraphlike_surface).unwrap());
}

fn generated_rotated_surface_code_dem() -> CircuitResult<DetectorErrorModel> {
    generated_rotated_surface_code_dem_with_options(ErrorAnalyzerOptions {
        decompose_errors: true,
        ..ErrorAnalyzerOptions::default()
    })
}

fn generated_rotated_surface_code_ungraphlike_dem() -> CircuitResult<DetectorErrorModel> {
    generated_rotated_surface_code_dem_with_options(ErrorAnalyzerOptions::default())
}

fn generated_rotated_surface_code_dem_with_options(
    options: ErrorAnalyzerOptions,
) -> CircuitResult<DetectorErrorModel> {
    let probability = Probability::try_new(0.001)?;
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(5)?,
        CodeDistance::try_new(5)?,
        SurfaceCodeTask::RotatedMemoryX,
    )?
    .with_after_clifford_depolarization(probability)
    .with_before_measure_flip_probability(probability)
    .with_after_reset_flip_probability(probability)
    .with_before_round_data_depolarization(probability);
    let generated = generate_surface_code_circuit(&params)?;
    circuit_to_detector_error_model(generated.circuit(), options)
}

fn generated_repetition_code_dem() -> CircuitResult<DetectorErrorModel> {
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(10)?,
        CodeDistance::try_new(7)?,
        RepetitionCodeTask::Memory,
    )?
    .with_before_round_data_depolarization(Probability::try_new(0.01)?);
    let generated = generate_repetition_code_circuit(&params)?;
    circuit_to_detector_error_model(
        generated.circuit(),
        ErrorAnalyzerOptions {
            decompose_errors: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
}

fn generated_small_rotated_surface_code_dem(
    decompose_errors: bool,
) -> CircuitResult<DetectorErrorModel> {
    let probability = Probability::try_new(0.001)?;
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(3)?,
        CodeDistance::try_new(3)?,
        SurfaceCodeTask::RotatedMemoryX,
    )?
    .with_after_clifford_depolarization(probability)
    .with_before_measure_flip_probability(probability)
    .with_after_reset_flip_probability(probability)
    .with_before_round_data_depolarization(probability);
    let generated = generate_surface_code_circuit(&params)?;
    circuit_to_detector_error_model(
        generated.circuit(),
        ErrorAnalyzerOptions {
            decompose_errors,
            ..ErrorAnalyzerOptions::default()
        },
    )
}

fn generated_small_repetition_code_dem() -> CircuitResult<DetectorErrorModel> {
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(4)?,
        CodeDistance::try_new(5)?,
        RepetitionCodeTask::Memory,
    )?
    .with_before_round_data_depolarization(Probability::try_new(0.01)?);
    let generated = generate_repetition_code_circuit(&params)?;
    circuit_to_detector_error_model(
        generated.circuit(),
        ErrorAnalyzerOptions {
            decompose_errors: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
}

fn many_observables_dem() -> CircuitResult<DetectorErrorModel> {
    let circuit = Circuit::from_stim_str(concat!(
        "MPP Z0*Z1 Z1*Z2 Z2*Z3 Z3*Z4\n",
        "X_ERROR(0.1) 0 1 2 3 4\n",
        "MPP Z0*Z1 Z1*Z2 Z2*Z3 Z3*Z4\n",
        "DETECTOR rec[-1] rec[-5]\n",
        "DETECTOR rec[-2] rec[-6]\n",
        "DETECTOR rec[-3] rec[-7]\n",
        "DETECTOR rec[-4] rec[-8]\n",
        "M 4\n",
        "OBSERVABLE_INCLUDE(1200) rec[-1]\n",
    ))?;
    circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            decompose_errors: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
}

fn assert_graphlike_search_signature(model: &DetectorErrorModel, expected_error_count: usize) {
    assert_search_result_signature_for_observable(model, expected_error_count, 2, 0);
}

fn assert_hypergraph_search_signature(model: &DetectorErrorModel, expected_error_count: usize) {
    assert_search_result_signature_for_observable(model, expected_error_count, 4, 0);
}

fn assert_search_result_signature_for_observable(
    model: &DetectorErrorModel,
    expected_error_count: usize,
    max_detectors_per_error: usize,
    expected_observable: u64,
) {
    let signature = SearchResultSignature::from_model(model, max_detectors_per_error);
    assert_eq!(
        signature.error_count,
        expected_error_count,
        "search output should have the pinned Stim v1.16.0 error count: {}",
        model.to_dem_string()
    );
    assert!(
        signature.detector_parity.is_empty(),
        "search output should be undetected after target-set parity: {:?}\n{}",
        signature.detector_parity,
        model.to_dem_string()
    );
    assert_eq!(
        signature.observable_parity,
        BTreeSet::from([expected_observable]),
        "search output should toggle exactly logical observable L{expected_observable}: {}",
        model.to_dem_string()
    );
    assert_eq!(
        signature.unique_target_sets,
        expected_error_count,
        "search output should not repeat canonical target sets: {}",
        model.to_dem_string()
    );
}

#[derive(Debug, Eq, PartialEq)]
struct SearchResultSignature {
    error_count: usize,
    detector_parity: BTreeSet<u64>,
    observable_parity: BTreeSet<u64>,
    unique_target_sets: usize,
}

impl SearchResultSignature {
    fn from_model(model: &DetectorErrorModel, max_detectors_per_error: usize) -> Self {
        let mut signature = Self {
            error_count: 0,
            detector_parity: BTreeSet::new(),
            observable_parity: BTreeSet::new(),
            unique_target_sets: 0,
        };
        let mut target_sets = BTreeSet::new();

        for item in model.items() {
            assert!(
                matches!(item, DemItem::Instruction(_)),
                "search output should not contain repeat blocks: {}",
                model.to_dem_string()
            );
            let DemItem::Instruction(instruction) = item else {
                continue;
            };
            assert_eq!(
                instruction.kind(),
                DemInstructionKind::Error,
                "search output should contain only error instructions: {}",
                model.to_dem_string()
            );
            assert_eq!(
                instruction.args(),
                &[1.0],
                "search output should use deterministic error instructions: {}",
                model.to_dem_string()
            );

            let targets = CanonicalSearchTargets::from_targets(instruction.targets());
            assert!(
                targets.detectors.len() <= max_detectors_per_error,
                "search output exceeded the per-error detector weight: {}",
                model.to_dem_string()
            );
            assert!(
                target_sets.insert(targets.clone()),
                "search output repeated a canonical target set: {}",
                model.to_dem_string()
            );
            for detector in &targets.detectors {
                toggle_parity(&mut signature.detector_parity, *detector);
            }
            for observable in &targets.observables {
                toggle_parity(&mut signature.observable_parity, *observable);
            }
            signature.error_count += 1;
        }

        signature.unique_target_sets = target_sets.len();
        signature
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CanonicalSearchTargets {
    detectors: Vec<u64>,
    observables: Vec<u64>,
}

impl CanonicalSearchTargets {
    fn from_targets(targets: &[DemTarget]) -> Self {
        let mut detectors = Vec::new();
        let mut observables = Vec::new();
        for target in targets {
            assert!(
                !matches!(target, DemTarget::Separator | DemTarget::Numeric(_)),
                "search output should use only detector and observable targets"
            );
            match target {
                DemTarget::RelativeDetector(detector) => detectors.push(detector.get()),
                DemTarget::LogicalObservable(observable) => observables.push(observable.get()),
                DemTarget::Separator | DemTarget::Numeric(_) => {}
            }
        }
        detectors.sort_unstable();
        observables.sort_unstable();
        assert_strictly_increasing(&detectors, "detector");
        assert_strictly_increasing(&observables, "observable");
        Self {
            detectors,
            observables,
        }
    }
}

fn toggle_parity(set: &mut BTreeSet<u64>, value: u64) {
    if !set.insert(value) {
        set.remove(&value);
    }
}

fn assert_strictly_increasing(values: &[u64], label: &str) {
    assert!(
        values
            .windows(2)
            .all(|window| matches!(window, [left, right] if left < right)),
        "search output should not repeat {label} targets inside one error row: {values:?}"
    );
}

fn assert_unweighted_wcnf_shape(wcnf: &str) {
    let header = parse_wcnf_header(wcnf);
    assert!(header.variables > 1, "{wcnf}");
    assert!(header.clauses > header.variables, "{wcnf}");
    assert_eq!(header.top_weight, header.clauses + 1, "{wcnf}");
    assert_eq!(wcnf.lines().skip(1).count(), header.clauses, "{wcnf}");
    assert!(wcnf.lines().skip(1).any(|line| line.starts_with("1 -")));
    assert!(
        wcnf.lines()
            .skip(1)
            .any(|line| line.starts_with(&format!("{} ", header.top_weight))),
        "{wcnf}"
    );
}

fn assert_weighted_wcnf_shape(wcnf: &str) {
    let header = parse_wcnf_header(wcnf);
    assert!(header.variables > 1, "{wcnf}");
    assert!(header.clauses > header.variables, "{wcnf}");
    assert!(header.top_weight > header.clauses, "{wcnf}");
    assert_eq!(wcnf.lines().skip(1).count(), header.clauses, "{wcnf}");
    assert!(
        wcnf.lines()
            .skip(1)
            .filter_map(first_wcnf_weight)
            .any(|weight| weight == header.top_weight),
        "{wcnf}"
    );
    assert!(
        wcnf.lines()
            .skip(1)
            .filter_map(first_wcnf_weight)
            .any(|weight| weight > 0 && weight < header.top_weight),
        "{wcnf}"
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WcnfHeader {
    variables: usize,
    clauses: usize,
    top_weight: usize,
}

fn parse_wcnf_header(wcnf: &str) -> WcnfHeader {
    let header = wcnf.lines().next().expect("WCNF header");
    let fields = header.split_whitespace().collect::<Vec<_>>();
    assert_eq!(fields.first(), Some(&"p"), "{header}");
    assert_eq!(fields.get(1), Some(&"wcnf"), "{header}");
    WcnfHeader {
        variables: fields
            .get(2)
            .expect("variable count")
            .parse()
            .expect("numeric variable count"),
        clauses: fields
            .get(3)
            .expect("clause count")
            .parse()
            .expect("numeric clause count"),
        top_weight: fields
            .get(4)
            .expect("top weight")
            .parse()
            .expect("numeric top weight"),
    }
}

fn first_wcnf_weight(line: &str) -> Option<usize> {
    line.split_whitespace().next()?.parse().ok()
}
