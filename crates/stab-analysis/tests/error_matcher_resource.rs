#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests use direct assertions for compact diagnostics"
)]

use stab_analysis::explain_errors_from_circuit;
use stab_model::{Circuit, DetectorErrorModel};

#[test]
fn pf4_error_matcher_repeat_resource_policy_is_source_owned() {
    let allowed = Circuit::from_stim_str(
        "
        R 0
        REPEAT 2 {
            TICK
        }
        X_ERROR(0.125) 0
        M 0
        DETECTOR rec[-1]
        ",
    )
    .unwrap();
    let explained = explain_errors_from_circuit(&allowed, None, false).unwrap();
    assert_eq!(explained.len(), 1);
    assert!(
        explained
            .first()
            .unwrap()
            .to_string()
            .contains("(after 2 TICKs)"),
        "bounded repeat traversal should update ErrorMatcher stack timing"
    );

    let repeated_noise = Circuit::from_stim_str(
        "
        REPEAT 2 {
            X_ERROR(0.125) 0
        }
        M 0
        DETECTOR rec[-1]
        ",
    )
    .unwrap();
    let error = explain_errors_from_circuit(&repeated_noise, None, false)
        .expect_err("reject repeat-contained noise until recursive matching exists")
        .to_string();
    assert!(error.contains("repeat-contained noise"), "{error}");

    let nested = Circuit::from_stim_str(
        "
        REPEAT 100000 {
            REPEAT 100000 {
                TICK
            }
        }
        ",
    )
    .unwrap();
    let error = explain_errors_from_circuit(&nested, None, false)
        .expect_err("reject nested expansion")
        .to_string();
    assert!(error.contains("expanded repeat iterations"), "{error}");
}

#[test]
fn pf4_error_matcher_filter_rejects_shifted_repeat() {
    let circuit = Circuit::from_stim_str(
        "
        M 0
        DETECTOR rec[-1]
        ",
    )
    .unwrap();
    let filter = DetectorErrorModel::from_dem_str(
        "
        repeat 100001 {
            error(0.1) D0
            shift_detectors 1
        }
        ",
    )
    .unwrap();
    let error = explain_errors_from_circuit(&circuit, Some(&filter), false)
        .expect_err("reject oversized filter DEM")
        .to_string();
    assert!(
        error.contains("DEM ErrorMatcher filter currently supports repeat counts"),
        "{error}"
    );
}

#[test]
fn pf4_error_matcher_filter_folds_flat_detector_repeat() {
    let circuit = Circuit::from_stim_str(
        "
        M(0.125) 0
        DETECTOR rec[-1]
        ",
    )
    .unwrap();
    let compact_filter = DetectorErrorModel::from_dem_str("error(0.1) D0\n").unwrap();
    let flat_repeat_filter =
        DetectorErrorModel::from_dem_str("repeat 100001 {\n    error(0.1) D0\n}\n").unwrap();

    let expected = explain_errors_from_circuit(&circuit, Some(&compact_filter), false)
        .unwrap()
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    let actual = explain_errors_from_circuit(&circuit, Some(&flat_repeat_filter), false)
        .unwrap()
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn pf4_error_matcher_filter_folds_rich_flat_detector_repeat() {
    let circuit = Circuit::from_stim_str(
        "
        MPAD 0
        DETECTOR rec[-1]
        M(0.125) 0
        M(0.25) 1
        DETECTOR rec[-2]
        DETECTOR rec[-1]
        OBSERVABLE_INCLUDE(0) rec[-1]
        ",
    )
    .unwrap();
    let compact_filter = DetectorErrorModel::from_dem_str(
        "
        shift_detectors 1
        error(0.1) D0
        error(0.1) D0 D0 D1 ^ L0
        ",
    )
    .unwrap();
    let flat_repeat_filter = DetectorErrorModel::from_dem_str(
        "
        shift_detectors 1
        repeat 100001 {
            error(0.1) D0
            error(0.1) D0 D0 D1 ^ L0
        }
        ",
    )
    .unwrap();

    let expected = explain_errors_from_circuit(&circuit, Some(&compact_filter), false)
        .unwrap()
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    let actual = explain_errors_from_circuit(&circuit, Some(&flat_repeat_filter), false)
        .unwrap()
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn pf4_error_matcher_filter_folds_nested_detector_repeat() {
    let circuit = Circuit::from_stim_str(
        "
        MPAD 0
        DETECTOR rec[-1]
        M(0.125) 0
        M(0.25) 1
        DETECTOR rec[-2]
        DETECTOR rec[-1]
        OBSERVABLE_INCLUDE(0) rec[-1]
        ",
    )
    .unwrap();
    let compact_filter = DetectorErrorModel::from_dem_str(
        "
        shift_detectors 1
        error(0.1) D0
        error(0.1) D0 D0 D1 ^ L0
        ",
    )
    .unwrap();
    let nested_repeat_filter = DetectorErrorModel::from_dem_str(
        "
        shift_detectors 1
        repeat 100001 {
            shift_detectors(4, 5) 0
            repeat 17 {
                error(0.1) D0
            }
            repeat 19 {
                error(0.1) D0 D0 D1 ^ L0
                shift_detectors 0
            }
        }
        ",
    )
    .unwrap();

    let expected = explain_errors_from_circuit(&circuit, Some(&compact_filter), false)
        .unwrap()
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    let actual = explain_errors_from_circuit(&circuit, Some(&nested_repeat_filter), false)
        .unwrap()
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn pf4_error_matcher_filter_folds_logical_only_repeat() {
    let circuit = Circuit::from_stim_str(
        "
        M(0.125) 0
        OBSERVABLE_INCLUDE(0) rec[-1]
        M(0.25) 1
        OBSERVABLE_INCLUDE(1) rec[-1]
        ",
    )
    .unwrap();
    let compact_filter = DetectorErrorModel::from_dem_str(
        "
        error(0.1) L0
        error(0.1) L1
        ",
    )
    .unwrap();
    let nested_repeat_filter = DetectorErrorModel::from_dem_str(
        "
        repeat 100001 {
            error(0.1) L0
            repeat 17 {
                shift_detectors 0
                error(0.1) L1
            }
        }
        ",
    )
    .unwrap();

    let expected = explain_errors_from_circuit(&circuit, Some(&compact_filter), false)
        .unwrap()
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        !expected.is_empty(),
        "logical-only filter should select errors"
    );
    let actual = explain_errors_from_circuit(&circuit, Some(&nested_repeat_filter), false)
        .unwrap()
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn pf4_error_matcher_filter_folds_annotation_repeat() {
    let circuit = Circuit::from_stim_str(
        "
        MPAD 0
        DETECTOR rec[-1]
        M(0.125) 0
        M(0.25) 1
        DETECTOR rec[-2]
        DETECTOR rec[-1]
        OBSERVABLE_INCLUDE(0) rec[-1]
        ",
    )
    .unwrap();
    let compact_filter = DetectorErrorModel::from_dem_str(
        "
        shift_detectors 1
        detector(2, 3) D0
        logical_observable L0
        error(0.1) D0
        error(0.1) D0 D0 D1 ^ L0
        ",
    )
    .unwrap();
    let annotation_repeat_filter = DetectorErrorModel::from_dem_str(
        "
        shift_detectors 1
        repeat 100001 {
            detector(2, 3) D0
            logical_observable L0
            error(0.1) D0
            repeat 17 {
                detector(7) D1
                logical_observable L0
                shift_detectors 0
                error(0.1) D0 D0 D1 ^ L0
            }
        }
        ",
    )
    .unwrap();

    let expected = explain_errors_from_circuit(&circuit, Some(&compact_filter), false)
        .unwrap()
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        !expected.is_empty(),
        "annotation-bearing filter should select errors"
    );
    let actual = explain_errors_from_circuit(&circuit, Some(&annotation_repeat_filter), false)
        .unwrap()
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn pf4_error_matcher_filter_skips_annotation_only_repeat() {
    let circuit = Circuit::from_stim_str(
        "
        M 0
        DETECTOR rec[-1]
        ",
    )
    .unwrap();
    let annotation_only_filter = DetectorErrorModel::from_dem_str(
        "
        repeat 100001 {
            detector(2) D0
            logical_observable L0
            shift_detectors 0
        }
        ",
    )
    .unwrap();

    assert_eq!(
        explain_errors_from_circuit(&circuit, Some(&annotation_only_filter), false).unwrap(),
        Vec::new()
    );
}

#[test]
fn pfm_b3_folded_traversal_matcher_filter() {
    let circuit = Circuit::from_stim_str(
        "MPAD 0\n\
         DETECTOR rec[-1]\n\
         M(0.125) 0\n\
         M(0.25) 1\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n\
         OBSERVABLE_INCLUDE(0) rec[-1]\n\
         OBSERVABLE_INCLUDE(1) rec[-2]\n",
    )
    .expect("matcher circuit");
    let compact = DetectorErrorModel::from_dem_str(
        "shift_detectors 1\n\
         error(0.1) D0\n\
         error(0.1) D0 D0 D1 ^ L0\n\
         error(0.1) L1\n",
    )
    .expect("compact filter");
    let repeated = DetectorErrorModel::from_dem_str(
        "shift_detectors 1\n\
         repeat 100001 {\n\
             detector(2, 3) D0\n\
             logical_observable L0\n\
             error(0.1) D0\n\
             repeat 17 {\n\
                 detector(7) D1\n\
                 error(0.1) D0 D0 D1 ^ L0\n\
                 error(0.1) L1\n\
                 shift_detectors 0\n\
             }\n\
         }\n",
    )
    .expect("folded filter");
    let normalize = |filter: &DetectorErrorModel| {
        explain_errors_from_circuit(&circuit, Some(filter), false)
            .expect("matcher filter traversal")
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(normalize(&repeated), normalize(&compact));

    let neutral = DetectorErrorModel::from_dem_str("repeat 100001 {\n}\n").expect("neutral filter");
    assert_eq!(normalize(&neutral), normalize(&DetectorErrorModel::new()));

    let shifted = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1) D0\n    shift_detectors 1\n}\n",
    )
    .expect("shifted filter");
    let error = explain_errors_from_circuit(&circuit, Some(&shifted), false)
        .expect_err("shifted filter repeat exceeds bounded traversal");
    assert!(
        error.to_string().contains("supports repeat counts"),
        "{error}"
    );
}
