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

#[test]
fn dem_analyzer_mpp_noise_and_result_flip_match_upstream_subset() {
    let dem = analyze(
        "
        RX 0
        Z_ERROR(0.125) 0
        MPP X0*Z1
        DETECTOR rec[-1]
        ",
    );
    assert_eq!(dem, "error(0.125) D0\n");

    let dem = analyze(
        "
        MPP(0.25) Z0*Z1
        DETECTOR rec[-1]
        ",
    );
    assert_eq!(dem, "error(0.25) D0\n");
}

#[test]
fn dem_analyzer_mpp_ordering_deterministic_cases_match_upstream_subset() {
    for circuit in [
        "
        MPP X0*X1 X0
        TICK
        MPP X0
        DETECTOR rec[-1] rec[-2]
        ",
        "
        MPP X0*X1 X0 X0
        DETECTOR rec[-1] rec[-2]
        ",
        "
        MPP X2*X1 X0
        TICK
        MPP X0
        DETECTOR rec[-1] rec[-2]
        ",
    ] {
        assert_eq!(analyze(circuit), "detector D0\n");
    }
}

#[test]
fn dem_analyzer_mpp_ordering_rejects_non_deterministic_upstream_subset() {
    for circuit in [
        "
        MPP X0 X0*X1
        TICK
        MPP X0
        DETECTOR rec[-1] rec[-2]
        ",
        "
        MPP X0 X2*X1
        TICK
        MPP X0
        DETECTOR rec[-1] rec[-2]
        ",
    ] {
        let parsed = Circuit::from_stim_str(circuit).expect("circuit");
        let error = circuit_to_detector_error_model(&parsed, ErrorAnalyzerOptions::default())
            .unwrap_err()
            .to_string();
        assert!(error.contains("non-deterministic detectors"));
        assert!(error.contains("D0"));
    }
}

#[test]
fn dem_analyzer_keeps_spp_explicitly_rejected_until_state_support_lands() {
    for gate_name in ["SPP", "SPP_DAG"] {
        let parsed =
            Circuit::from_stim_str(&format!("{gate_name} X0 X1*Y2*Z3\n")).expect("parse SPP");
        let error = circuit_to_detector_error_model(&parsed, ErrorAnalyzerOptions::default())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("analyze_errors does not yet support"),
            "{gate_name}: {error}"
        );
    }
}
