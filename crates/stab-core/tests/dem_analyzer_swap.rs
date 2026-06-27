#![allow(
    clippy::expect_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{Circuit, ErrorAnalyzerOptions, circuit_to_detector_error_model};

fn analyze(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect("analyze")
        .to_dem_string()
}

#[test]
fn dem_analyzer_swap_propagates_pending_pauli_errors_like_stim() {
    let dem = analyze(
        "
        R 0 1
        X_ERROR(0.25) 0
        SWAP 0 1
        M 0 1
        DETECTOR rec[-1]
        DETECTOR rec[-2]
        ",
    );

    assert_eq!(dem, "error(0.25) D0\ndetector D1\n");
}

#[test]
fn dem_analyzer_swap_propagates_pending_pauli_channels_like_stim() {
    let dem = analyze(
        "
        R 0 1
        PAULI_CHANNEL_1(0.125, 0, 0) 0
        SWAP 0 1
        M 0 1
        DETECTOR rec[-1]
        DETECTOR rec[-2]
        ",
    );

    assert_eq!(
        dem,
        "error(0.125000000000000055511151231257827) D0\ndetector D1\n"
    );
}
