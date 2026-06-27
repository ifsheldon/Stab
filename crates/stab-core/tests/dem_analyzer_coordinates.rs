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
fn dem_analyzer_preserves_shift_coords_as_dem_shift_declarations() {
    let dem = analyze(
        "
        DETECTOR(1, 2)
        SHIFT_COORDS(10, 20)
        DETECTOR(100, 200)
        ",
    );

    assert_eq!(
        dem,
        "detector(1, 2) D0\n\
         shift_detectors(10, 20) 0\n\
         detector(100, 200) D1\n"
    );
}
