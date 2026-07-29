#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests use direct assertions for compact diagnostics"
)]

use stab_core::{Circuit, ErrorAnalyzerOptions, circuit_to_detector_error_model};

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
