#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests use direct assertions for compact diagnostics"
)]

use stab_core::{
    Circuit, DetectorErrorModel, ErrorAnalyzerOptions, circuit_to_detector_error_model,
    explain_errors_from_circuit,
};

#[test]
fn pf4_dem_analyzer_repeat_resource_policy_is_source_owned() {
    let allowed = Circuit::from_stim_str(
        "
        REPEAT 2 {
            X_ERROR(0.125) 0
            M 0
            DETECTOR rec[-1]
        }
        ",
    )
    .unwrap();
    let dem = circuit_to_detector_error_model(&allowed, ErrorAnalyzerOptions::default()).unwrap();
    assert_eq!(dem.to_dem_string(), "error(0.125) D0 D1\nerror(0.125) D1\n");

    let too_large = Circuit::from_stim_str(
        "
        REPEAT 100001 {
            M 0
            DETECTOR rec[-1]
        }
        ",
    )
    .unwrap();
    let error = circuit_to_detector_error_model(&too_large, ErrorAnalyzerOptions::default())
        .expect_err("reject excessive repeat count")
        .to_string();
    assert!(
        error.contains("analyze_errors currently supports repeat counts up to 100000"),
        "{error}"
    );

    let nested = Circuit::from_stim_str(
        "
        REPEAT 100000 {
            REPEAT 100000 {
                M 0
                DETECTOR rec[-1]
            }
        }
        ",
    )
    .unwrap();
    let error = circuit_to_detector_error_model(&nested, ErrorAnalyzerOptions::default())
        .expect_err("reject nested expansion")
        .to_string();
    assert!(error.contains("expanded repeat iterations"), "{error}");
}

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
