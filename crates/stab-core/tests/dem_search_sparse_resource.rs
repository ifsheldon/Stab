#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests use exact DEM strings for compact resource diagnostics"
)]

use stab_core::{
    DetectorErrorModel, find_undetectable_logical_error, likeliest_error_sat_problem,
    shortest_error_sat_problem, shortest_graphlike_undetectable_logical_error,
};

const TWO_ERROR_UNWEIGHTED_WDIMACS: &str = "\
p wcnf 3 8 9
1 -1 0
9 1 2 -3 0
9 1 -2 3 0
9 -1 2 3 0
9 -1 -2 -3 0
1 -2 0
9 -3 0
9 1 0
";

const TWO_ERROR_WEIGHTED_WDIMACS: &str = "\
p wcnf 3 8 81
10 -1 0
81 1 2 -3 0
81 1 -2 3 0
81 -1 2 3 0
81 -1 -2 -3 0
10 -2 0
81 -3 0
81 1 0
";

fn sparse_high_detector_model() -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(
        "\
repeat 1000001 {
    error(0) D0
    shift_detectors 1
}
error(0.1) D0
error(0.1) D0 L0
",
    )
    .unwrap()
}

fn sparse_high_detector_path_model() -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(
        "\
repeat 1000001 {
    error(0) D0
    shift_detectors 1
}
error(0.1) D0
error(0.1) D0 D1 L0
error(0.1) D1
",
    )
    .unwrap()
}

fn sparse_high_detector_no_logical_model() -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(
        "\
repeat 1000001 {
    error(0) D0
    shift_detectors 1
}
logical_observable L0
error(0.1) D0
",
    )
    .unwrap()
}

fn sparse_high_detector_hypergraph_duplicate_model() -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(
        "\
repeat 1000001 {
    error(0) D0
    shift_detectors 1
}
error(0.1) D0
error(0.1) D0 D1 D2 D2 L0
error(0.1) D1
",
    )
    .unwrap()
}

fn sparse_high_observable_model() -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(
        "\
error(0.1) D0 L1000001
error(0.1) D0
",
    )
    .unwrap()
}

fn sparse_sat_high_detector_model() -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(
        "\
error(0.1) D1000001 L0
error(0.1) D1000001
",
    )
    .unwrap()
}

#[test]
fn pf6_search_sparse_high_detectors_graphlike_preserves_original_ids() {
    let distance_2 =
        shortest_graphlike_undetectable_logical_error(&sparse_high_detector_model(), false)
            .unwrap();
    assert_eq!(
        distance_2.to_dem_string(),
        "error(1) D1000001\nerror(1) D1000001 L0\n"
    );

    let distance_3 =
        shortest_graphlike_undetectable_logical_error(&sparse_high_detector_path_model(), false)
            .unwrap();
    assert_eq!(
        distance_3.to_dem_string(),
        "error(1) D1000001\nerror(1) D1000001 D1000002 L0\nerror(1) D1000002\n"
    );
}

#[test]
fn pf6_search_sparse_high_detectors_hypergraph_preserves_original_ids() {
    let distance_2 = find_undetectable_logical_error(
        &sparse_high_detector_model(),
        usize::MAX,
        usize::MAX,
        false,
    )
    .unwrap();
    assert_eq!(
        distance_2.to_dem_string(),
        "error(1) D1000001\nerror(1) D1000001 L0\n"
    );

    let distance_3 = find_undetectable_logical_error(
        &sparse_high_detector_path_model(),
        usize::MAX,
        usize::MAX,
        false,
    )
    .unwrap();
    assert_eq!(
        distance_3.to_dem_string(),
        "error(1) D1000001\nerror(1) D1000001 D1000002 L0\nerror(1) D1000002\n"
    );
}

#[test]
fn pf6_search_sparse_high_detectors_hypergraph_uses_toggled_degree() {
    let distance_3 = find_undetectable_logical_error(
        &sparse_high_detector_hypergraph_duplicate_model(),
        3,
        2,
        false,
    )
    .unwrap();
    assert_eq!(
        distance_3.to_dem_string(),
        "error(1) D1000001\nerror(1) D1000001 D1000002 L0\nerror(1) D1000002\n"
    );
}

#[test]
fn pf6_search_sparse_high_targets_sat_compresses_ids() {
    for model in [
        sparse_sat_high_detector_model(),
        sparse_high_observable_model(),
    ] {
        assert_eq!(
            shortest_error_sat_problem(&model).expect("shortest WCNF"),
            TWO_ERROR_UNWEIGHTED_WDIMACS
        );
        assert_eq!(
            likeliest_error_sat_problem(&model, 10).expect("weighted WCNF"),
            TWO_ERROR_WEIGHTED_WDIMACS
        );
    }
}

#[test]
fn pf6_search_sparse_high_detectors_keep_declared_count_diagnostics() {
    let graphlike_error = shortest_graphlike_undetectable_logical_error(
        &sparse_high_detector_no_logical_model(),
        false,
    )
    .expect_err("model has declared observables but no logical error path")
    .to_string();
    assert!(!graphlike_error.contains("WARNING: NO OBSERVABLES"));
    assert!(!graphlike_error.contains("WARNING: NO DETECTORS"));

    let hypergraph_error = find_undetectable_logical_error(
        &sparse_high_detector_no_logical_model(),
        usize::MAX,
        usize::MAX,
        false,
    )
    .expect_err("model has declared observables but no logical error path")
    .to_string();
    assert!(!hypergraph_error.contains("WARNING: NO OBSERVABLES"));
    assert!(!hypergraph_error.contains("WARNING: NO DETECTORS"));
}
