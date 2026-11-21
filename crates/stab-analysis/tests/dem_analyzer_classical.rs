#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_analysis::{
    AnalysisError, ErrorAnalyzerOptions, LoopFolding, circuit_to_detector_error_model,
};
use stab_model::Circuit;

fn analyze(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(&circuit, &ErrorAnalyzerOptions::default())
        .expect("analyze")
        .to_dem_string()
}

fn analyze_error(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(&circuit, &ErrorAnalyzerOptions::default())
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

/// Deliberate divergence `analysis.circuit-to-dem-feedback-record-lookback`: pinned Stim v1.16.0
/// `analyze_errors` silently accepts a classically controlled Pauli whose `rec[...]` control looks
/// back before the first measurement, while its `sample`, `detect`, and `m2d` abort on the same
/// circuit. Stab rejects the lookback in every fold mode, and each row's boundary twin, whose
/// lookback reaches an earlier measurement, still analyzes to pinned Stim's model.
#[test]
fn dem_analyzer_rejects_feedback_record_lookback_before_first_measurement() {
    let in_range_loop_folded =
        "repeat 2 {\n    detector D0\n    shift_detectors 1\n}\ndetector D0\n";
    for (rejected, offset, accepted, unfolded, folded) in [
        (
            "CX sweep[0] 1 rec[-1] 2\n",
            "rec[-1]",
            "M 0\nCX sweep[0] 1 rec[-1] 2\nM 2\nDETECTOR rec[-1]\n",
            "detector D0\n",
            "detector D0\n",
        ),
        (
            "CZ rec[-1] 0\n",
            "rec[-1]",
            "X_ERROR(0.25) 0\nM 0\nRX 1\nCZ rec[-1] 1\nMX 1\nDETECTOR rec[-1]\n",
            "error(0.25) D0\n",
            "error(0.25) D0\n",
        ),
        (
            "M 0\nCX rec[-2] 1\n",
            "rec[-2]",
            "X_ERROR(0.25) 0\nM 0\nCX rec[-1] 1\nM 1\nDETECTOR rec[-1]\n",
            "error(0.25) D0\n",
            "error(0.25) D0\n",
        ),
        (
            // Only the first iteration looks back before the first measurement.
            "REPEAT 3 {\n    X_ERROR(0.1) 0\n    CX rec[-1] 1\n    M 0 1\n    DETECTOR rec[-1]\n}\n",
            "rec[-1]",
            "X_ERROR(0.25) 1\nM 1\nREPEAT 3 {\n    X_ERROR(0.1) 0\n    CX rec[-1] 1\n    M 0 1\n    DETECTOR rec[-1]\n}\n",
            "detector D0\ndetector D1\ndetector D2\n",
            in_range_loop_folded,
        ),
    ] {
        for (fold_loops, expected) in [(false, unfolded), (true, folded)] {
            let options = ErrorAnalyzerOptions {
                loops: if fold_loops {
                    LoopFolding::Fold
                } else {
                    LoopFolding::Unroll
                },
                ..ErrorAnalyzerOptions::default()
            };
            let circuit = Circuit::from_stim_str(rejected).expect("rejected circuit");
            let error = circuit_to_detector_error_model(&circuit, &options)
                .expect_err("lookback before the first measurement must be rejected");
            assert!(
                matches!(error, AnalysisError::MeasurementRecordOutOfRange { .. }),
                "{rejected:?} (fold_loops={fold_loops}): {error:?}"
            );
            assert_eq!(
                error.to_string(),
                format!(
                    "invalid detector error model: measurement record offset {offset} is out of range"
                ),
                "{rejected:?} (fold_loops={fold_loops})"
            );
            let circuit = Circuit::from_stim_str(accepted).expect("accepted circuit");
            let model = circuit_to_detector_error_model(&circuit, &options)
                .expect("in-range feedback lookback must be accepted");
            assert_eq!(
                model.to_dem_string(),
                expected,
                "{accepted:?} (fold_loops={fold_loops})"
            );
        }
    }
}
