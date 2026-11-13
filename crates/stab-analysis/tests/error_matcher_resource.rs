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
        REPEAT 1001 {
            REPEAT 1000 {
                TICK
            }
        }
        ",
    )
    .unwrap();
    // Expanded traversal above the retired one-million caps is not capped.
    assert!(
        explain_errors_from_circuit(&nested, None, false)
            .expect("uncapped nested traversal")
            .is_empty()
    );
}

#[test]
fn pf4_error_matcher_filter_refuses_shifted_repeat_keys_beyond_the_memory_budget() {
    let circuit = Circuit::from_stim_str("M 0\nDETECTOR rec[-1]\n").unwrap();
    let filter = DetectorErrorModel::from_dem_str(
        "repeat 1000000000000 {\n    error(0.1) D0\n    shift_detectors 1\n}\n",
    )
    .unwrap();
    let error = explain_errors_from_circuit(&circuit, Some(&filter), false)
        .expect_err("reject oversized filter DEM")
        .to_string();
    assert!(error.contains("memory budget"), "{error}");
}

/// A filter DEM whose repeat bodies leave the detector frame unchanged must select exactly the
/// errors its compact single-iteration form selects, without expanding the repeat counts.
#[test]
fn error_matcher_filter_folds_zero_shift_repeats_like_compact_filters() {
    const SINGLE_DETECTOR: &str = "M(0.125) 0\nDETECTOR rec[-1]\n";
    const PADDED_DETECTORS: &str = "\
MPAD 0
DETECTOR rec[-1]
M(0.125) 0
M(0.25) 1
DETECTOR rec[-2]
DETECTOR rec[-1]
OBSERVABLE_INCLUDE(0) rec[-1]
";
    const PADDED_COMPACT: &str = "shift_detectors 1\nerror(0.1) D0\nerror(0.1) D0 D0 D1 ^ L0\n";
    // (name, circuit, compact filter, repeated filter, compact filter selects some errors)
    let cases: &[(&str, &str, &str, &str, bool)] = &[
        (
            "flat detector repeat",
            SINGLE_DETECTOR,
            "error(0.1) D0\n",
            "repeat 100001 {\n    error(0.1) D0\n}\n",
            true,
        ),
        (
            "flat repeat with decomposed observable error",
            PADDED_DETECTORS,
            PADDED_COMPACT,
            "shift_detectors 1\nrepeat 100001 {\n    error(0.1) D0\n    error(0.1) D0 D0 D1 ^ L0\n}\n",
            true,
        ),
        (
            "nested detector repeats",
            PADDED_DETECTORS,
            PADDED_COMPACT,
            "shift_detectors 1\nrepeat 100001 {\n    shift_detectors(4, 5) 0\n    repeat 17 {\n        error(0.1) D0\n    }\n    repeat 19 {\n        error(0.1) D0 D0 D1 ^ L0\n        shift_detectors 0\n    }\n}\n",
            true,
        ),
        (
            "nested logical-only repeats",
            "M(0.125) 0\nOBSERVABLE_INCLUDE(0) rec[-1]\nM(0.25) 1\nOBSERVABLE_INCLUDE(1) rec[-1]\n",
            "error(0.1) L0\nerror(0.1) L1\n",
            "repeat 100001 {\n    error(0.1) L0\n    repeat 17 {\n        shift_detectors 0\n        error(0.1) L1\n    }\n}\n",
            true,
        ),
        (
            "annotated nested repeats",
            PADDED_DETECTORS,
            "shift_detectors 1\ndetector(2, 3) D0\nlogical_observable L0\nerror(0.1) D0\nerror(0.1) D0 D0 D1 ^ L0\n",
            "shift_detectors 1\nrepeat 100001 {\n    detector(2, 3) D0\n    logical_observable L0\n    error(0.1) D0\n    repeat 17 {\n        detector(7) D1\n        logical_observable L0\n        shift_detectors 0\n        error(0.1) D0 D0 D1 ^ L0\n    }\n}\n",
            true,
        ),
        (
            "annotated nested repeats with a second observable",
            "MPAD 0\nDETECTOR rec[-1]\nM(0.125) 0\nM(0.25) 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(1) rec[-2]\n",
            "shift_detectors 1\nerror(0.1) D0\nerror(0.1) D0 D0 D1 ^ L0\nerror(0.1) L1\n",
            "shift_detectors 1\nrepeat 100001 {\n    detector(2, 3) D0\n    logical_observable L0\n    error(0.1) D0\n    repeat 17 {\n        detector(7) D1\n        error(0.1) D0 D0 D1 ^ L0\n        error(0.1) L1\n        shift_detectors 0\n    }\n}\n",
            true,
        ),
        (
            "annotation-only repeat",
            "M 0\nDETECTOR rec[-1]\n",
            "",
            "repeat 100001 {\n    detector(2) D0\n    logical_observable L0\n    shift_detectors 0\n}\n",
            false,
        ),
        (
            "empty repeat",
            PADDED_DETECTORS,
            "",
            "repeat 100001 {\n}\n",
            false,
        ),
    ];
    for &(name, circuit, compact, repeated, selects_errors) in cases {
        let circuit = Circuit::from_stim_str(circuit).unwrap();
        let explain = |filter: &str| {
            let filter = DetectorErrorModel::from_dem_str(filter).unwrap();
            explain_errors_from_circuit(&circuit, Some(&filter), false)
                .unwrap()
                .into_iter()
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
        };
        let expected = explain(compact);
        assert_eq!(!expected.is_empty(), selects_errors, "{name}: {expected:?}");
        assert_eq!(explain(repeated), expected, "{name}");
    }
}
