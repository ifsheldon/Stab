#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{Circuit, ErrorAnalyzerOptions, circuit_to_detector_error_model};

fn analyze(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect("analyze")
        .to_dem_string()
}

fn analyze_error(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect_err("reject invalid analyzer circuit")
        .to_string()
}

#[test]
fn dem_analyzer_ignores_sweep_controls_like_upstream() {
    for circuit in [
        "
            X_ERROR(0.25) 0
            CNOT sweep[0] 0
            M 0
            DETECTOR rec[-1]
        ",
        "
            X_ERROR(0.25) 0
            CY sweep[0] 0
            M 0
            DETECTOR rec[-1]
        ",
        "
            X_ERROR(0.25) 0
            CZ sweep[0] 0
            M 0
            DETECTOR rec[-1]
        ",
        "
            X_ERROR(0.25) 0
            CZ 0 sweep[0]
            M 0
            DETECTOR rec[-1]
        ",
        "
            X_ERROR(0.25) 0
            CZ sweep[0] sweep[1]
            M 0
            DETECTOR rec[-1]
        ",
        "
            X_ERROR(0.25) 0
            M 1
            CZ rec[-1] sweep[0]
            M 0
            DETECTOR rec[-1]
        ",
        "
            X_ERROR(0.25) 0
            M 1
            CZ sweep[0] rec[-1]
            M 0
            DETECTOR rec[-1]
        ",
        "
            X_ERROR(0.25) 0
            M 1 2
            CZ rec[-1] rec[-2]
            M 0
            DETECTOR rec[-1]
        ",
        "
            X_ERROR(0.25) 0
            XCZ 0 sweep[0]
            M 0
            DETECTOR rec[-1]
        ",
        "
            X_ERROR(0.25) 0
            YCZ 0 sweep[0]
            M 0
            DETECTOR rec[-1]
        ",
    ] {
        assert_eq!(analyze(circuit), "error(0.25) D0\n");
    }
}

#[test]
fn dem_analyzer_ignores_maximum_sweep_id_without_dense_state() {
    assert_eq!(
        analyze(
            "
            X_ERROR(0.25) 0
            CNOT sweep[16777215] 0
            M 0
            DETECTOR rec[-1]
            ",
        ),
        "error(0.25) D0\n"
    );
}

#[test]
fn dem_analyzer_rejects_invalid_sweep_target_positions() {
    for (circuit, expected) in [
        (
            "
            X_ERROR(0.25) 0
            CX 0 sweep[0]
            M 0
            DETECTOR rec[-1]
            ",
            "CX target sweep[0] is not a qubit",
        ),
        (
            "
            X_ERROR(0.25) 0
            CX sweep[0] sweep[1]
            M 0
            DETECTOR rec[-1]
            ",
            "CX target sweep[1] is not a qubit",
        ),
        (
            "
            X_ERROR(0.25) 0
            M 0 1
            CX rec[-1] rec[-2]
            M 0
            DETECTOR rec[-1]
            ",
            "CX target rec[-2] is not a qubit",
        ),
        (
            "
            X_ERROR(0.25) 0
            M 0 1
            CY rec[-1] rec[-2]
            M 0
            DETECTOR rec[-1]
            ",
            "CY target rec[-2] is not a qubit",
        ),
        (
            "
            X_ERROR(0.25) 0
            M 0 1
            XCZ rec[-1] rec[-2]
            M 0
            DETECTOR rec[-1]
            ",
            "XCZ target rec[-1] is not a qubit",
        ),
        (
            "
            X_ERROR(0.25) 0
            M 0 1
            YCZ rec[-1] rec[-2]
            M 0
            DETECTOR rec[-1]
            ",
            "YCZ target rec[-1] is not a qubit",
        ),
        (
            "
            X_ERROR(0.25) 0
            M 0
            CY rec[-1] sweep[0]
            M 0
            DETECTOR rec[-1]
            ",
            "CY target sweep[0] is not a qubit",
        ),
        (
            "
            X_ERROR(0.25) 0
            CY 0 sweep[0]
            M 0
            DETECTOR rec[-1]
            ",
            "CY target sweep[0] is not a qubit",
        ),
        (
            "
            X_ERROR(0.25) 0
            XCZ sweep[0] sweep[1]
            M 0
            DETECTOR rec[-1]
            ",
            "XCZ target sweep[0] is not a qubit",
        ),
        (
            "
            X_ERROR(0.25) 0
            XCZ sweep[0] 0
            M 0
            DETECTOR rec[-1]
            ",
            "XCZ target sweep[0] is not a qubit",
        ),
        (
            "
            X_ERROR(0.25) 0
            YCZ sweep[0] 0
            M 0
            DETECTOR rec[-1]
            ",
            "YCZ target sweep[0] is not a qubit",
        ),
    ] {
        let error = analyze_error(circuit);
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn dem_analyzer_measurement_record_feedback_matches_upstream_subset() {
    for circuit in [
        "
        X_ERROR(0.125) 0
        M 0
        CNOT rec[-1] 1
        M 1
        DETECTOR rec[-1]
        ",
        "
        X_ERROR(0.125) 0
        M 0
        H 1
        CZ rec[-1] 1
        H 1
        M 1
        DETECTOR rec[-1]
        ",
        "
        X_ERROR(0.125) 0
        M 0
        H 1
        CZ 1 rec[-1]
        H 1
        M 1
        DETECTOR rec[-1]
        ",
        "
        X_ERROR(0.125) 0
        M 0
        CY rec[-1] 1
        M 1
        DETECTOR rec[-1]
        ",
        "
        X_ERROR(0.125) 0
        M 0
        XCZ 1 rec[-1]
        M 1
        DETECTOR rec[-1]
        ",
        "
        X_ERROR(0.125) 0
        M 0
        YCZ 1 rec[-1]
        M 1
        DETECTOR rec[-1]
        ",
    ] {
        assert_eq!(analyze(circuit), "error(0.125) D0\n");
    }
}
