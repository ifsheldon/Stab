#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests use direct assertions for compact diagnostics"
)]

use stab_core::{
    DetectorErrorModel, find_undetectable_logical_error,
    shortest_graphlike_undetectable_logical_error,
};

#[test]
fn pf4_dem_search_mixed_zero_probability_repeat_folds_by_compact_model() {
    let (detector_touching_repeat, detector_touching_compact) =
        mixed_zero_probability_detector_touching_models();
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&detector_touching_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&detector_touching_compact, false)
            .unwrap()
            .to_dem_string()
    );

    let (logical_only_repeat, logical_only_compact) = mixed_zero_probability_logical_only_models();
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&logical_only_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&logical_only_compact, false)
            .unwrap()
            .to_dem_string()
    );

    let (nested_repeat, nested_compact) = nested_mixed_zero_probability_models();
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&nested_repeat, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&nested_compact, false)
            .unwrap()
            .to_dem_string()
    );

    assert_nonzero_shift_search_repeat_rejected();
}

#[test]
fn pf4_hypergraph_mixed_zero_probability_repeat_folds_by_compact_model() {
    let (detector_touching_repeat, detector_touching_compact) =
        mixed_zero_probability_detector_touching_models();
    assert_eq!(
        find_undetectable_logical_error(&detector_touching_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&detector_touching_compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string()
    );

    let (logical_only_repeat, logical_only_compact) = mixed_zero_probability_logical_only_models();
    assert_eq!(
        find_undetectable_logical_error(&logical_only_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&logical_only_compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string()
    );

    let (nested_repeat, nested_compact) = nested_mixed_zero_probability_models();
    assert_eq!(
        find_undetectable_logical_error(&nested_repeat, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(&nested_compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string()
    );

    assert_nonzero_shift_search_repeat_rejected();
}

fn mixed_zero_probability_detector_touching_models() -> (DetectorErrorModel, DetectorErrorModel) {
    let repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0) D1000000 L1000\n    error(0.1) D0\n    shift_detectors 0\n    error(0.1) D0 L0\n}\n",
    )
    .unwrap();
    let compact = DetectorErrorModel::from_dem_str("error(0.1) D0\nerror(0.1) D0 L0\n").unwrap();
    (repeat, compact)
}

fn mixed_zero_probability_logical_only_models() -> (DetectorErrorModel, DetectorErrorModel) {
    let repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    detector D0\n    error(0) D1000000\n    error(0.1) L0\n}\n",
    )
    .unwrap();
    let compact = DetectorErrorModel::from_dem_str("detector D0\nerror(0.1) L0\n").unwrap();
    (repeat, compact)
}

fn nested_mixed_zero_probability_models() -> (DetectorErrorModel, DetectorErrorModel) {
    let repeat = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    detector D0\n    repeat 100001 {\n        error(0) D1000000 L1000\n        error(0.1) D0\n        shift_detectors 0\n        error(0.2) D0 L0\n    }\n    shift_detectors 0\n}\n",
    )
    .unwrap();
    let compact =
        DetectorErrorModel::from_dem_str("detector D0\nerror(0.1) D0\nerror(0.2) D0 L0\n").unwrap();
    (repeat, compact)
}

fn assert_nonzero_shift_search_repeat_rejected() {
    let shifted = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0) D1000000\n    shift_detectors 1\n    error(0.1) D0 L0\n}\n",
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
