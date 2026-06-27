#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{Circuit, ErrorAnalyzerOptions, circuit_to_detector_error_model};

#[test]
fn dem_analyzer_ignores_sweep_controls_like_upstream() {
    let circuit = Circuit::from_stim_str(
        "
        X_ERROR(0.25) 0
        CNOT sweep[0] 0
        M 0
        DETECTOR rec[-1]
        ",
    )
    .expect("circuit");

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect("analyze")
        .to_dem_string();

    assert_eq!(dem, "error(0.25) D0\n");
}
