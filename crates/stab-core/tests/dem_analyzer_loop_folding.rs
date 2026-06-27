#![allow(
    clippy::expect_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{Circuit, ErrorAnalyzerOptions, circuit_to_detector_error_model};

fn analyze_folding_loops(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze")
    .to_dem_string()
}

#[test]
fn dem_analyzer_fold_loops_preserves_simple_nested_repeat_like_stim() {
    let dem = analyze_folding_loops(
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
    );

    assert_eq!(
        dem,
        "repeat 3 {\n    repeat 2 {\n        error(0.25) D0\n        shift_detectors 1\n    }\n}\n"
    );
}
