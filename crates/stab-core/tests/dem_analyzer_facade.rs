#![allow(
    clippy::expect_used,
    reason = "facade tests extract required results directly"
)]

use stab_core::{Circuit, CircuitError, ErrorAnalyzerOptions, circuit_to_detector_error_model};

#[test]
fn dem_analyzer_facade_converts_options_and_preserves_exact_output() {
    let circuit = Circuit::from_stim_str(
        "
        REPEAT 3 {
            REPEAT 2 {
                R 0
                X_ERROR(0.25) 0
                M 0
                DETECTOR rec[-1]
            }
        }
        ",
    )
    .expect("parse circuit through the stab-core facade");

    let dem = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze through the stab-core facade");

    assert_eq!(
        dem.to_dem_string(),
        "repeat 3 {\n    repeat 2 {\n        error(0.25) D0\n        shift_detectors 1\n    }\n}\n"
    );
}

#[test]
fn dem_analyzer_facade_preserves_analysis_error_details() {
    let circuit = Circuit::from_stim_str(
        "
        REPEAT 100001 {
            M 0
            DETECTOR rec[-1]
        }
        ",
    )
    .expect("parse circuit through the stab-core facade");

    let error = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect_err("the canonical analyzer repeat limit must survive the facade");
    let message =
        "analyze_errors currently supports repeat counts up to 100000, got 100001".to_string();

    assert_eq!(
        error,
        CircuitError::InvalidDetectorErrorModel {
            message: message.clone()
        }
    );
    assert_eq!(
        error.to_string(),
        format!("invalid detector error model: {message}")
    );
}
