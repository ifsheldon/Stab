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
fn dem_analyzer_period3_single_qubit_cliffords_match_upstream() {
    for (circuit, expected) in [
        (
            "
            RY 0 1 2
            X_ERROR(1) 0
            Y_ERROR(1) 1
            Z_ERROR(1) 2
            C_XYZ 0 1 2
            M 0 1 2
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(1) D0\nerror(1) D2\ndetector D1\n",
        ),
        (
            "
            R 0 1 2
            C_XYZ 0 1 2
            X_ERROR(1) 0
            Y_ERROR(1) 1
            Z_ERROR(1) 2
            C_ZYX 0 1 2
            M 0 1 2
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(1) D1\nerror(1) D2\ndetector D0\n",
        ),
        (
            "
            R 0 1 2
            C_ZYX 0 1 2
            X_ERROR(1) 0
            Y_ERROR(1) 1
            Z_ERROR(1) 2
            C_XYZ 0 1 2
            M 0 1 2
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(1) D0\nerror(1) D2\ndetector D1\n",
        ),
    ] {
        assert_eq!(analyze(circuit), expected);
    }
}
