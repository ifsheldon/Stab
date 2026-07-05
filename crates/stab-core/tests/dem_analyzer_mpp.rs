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
        .expect_err("analyze should fail")
        .to_string()
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
fn dem_analyzer_spp_matches_explicit_phase_product_expansions() {
    for (direct, explicit) in [
        (
            "SPP Z0\nS_DAG 0\nX_ERROR(0.125) 0\nM 0\nDETECTOR rec[-1]\n",
            "S 0\nS_DAG 0\nX_ERROR(0.125) 0\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "SPP_DAG Z0\nS 0\nX_ERROR(0.125) 0\nM 0\nDETECTOR rec[-1]\n",
            "S_DAG 0\nS 0\nX_ERROR(0.125) 0\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "SPP !Z0\nS 0\nX_ERROR(0.125) 0\nM 0\nDETECTOR rec[-1]\n",
            "S_DAG 0\nS 0\nX_ERROR(0.125) 0\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "SPP X0\nH 0\nS_DAG 0\nH 0\nX_ERROR(0.125) 0\nM 0\nDETECTOR rec[-1]\n",
            "H 0\nS 0\nH 0\nH 0\nS_DAG 0\nH 0\nX_ERROR(0.125) 0\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "Z_ERROR(0.125) 0\nSPP X0\nH 0\nS_DAG 0\nH 0\nM 0\nDETECTOR rec[-1]\n",
            "Z_ERROR(0.125) 0\nH 0\nS 0\nH 0\nH 0\nS_DAG 0\nH 0\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "SPP X0*X1\nH 0\nH 1\nCX 1 0\nS_DAG 0\nCX 1 0\nH 1\nH 0\nX_ERROR(0.125) 0\nM 0\nDETECTOR rec[-1]\n",
            "H 0\nH 1\nCX 1 0\nS 0\nCX 1 0\nH 1\nH 0\nH 0\nH 1\nCX 1 0\nS_DAG 0\nCX 1 0\nH 1\nH 0\nX_ERROR(0.125) 0\nM 0\nDETECTOR rec[-1]\n",
        ),
    ] {
        assert_eq!(analyze(direct), analyze(explicit), "{direct}");
    }
}

#[test]
fn dem_analyzer_spp_nondeterministic_detector_matches_explicit_expansion() {
    let direct = analyze_error("SPP X0\nM 0\nDETECTOR rec[-1]\n");
    let explicit = analyze_error("H 0\nS 0\nH 0\nM 0\nDETECTOR rec[-1]\n");
    for error in [direct, explicit] {
        assert!(error.contains("non-deterministic detectors"), "{error}");
        assert!(error.contains("D0"), "{error}");
    }
}

#[test]
fn dem_analyzer_spp_nondeterministic_observable_matches_explicit_expansion() {
    let direct = analyze_error(
        "OBSERVABLE_INCLUDE(0) Z0\nSPP X0\nX_ERROR(0.125) 0\nOBSERVABLE_INCLUDE(1) Z0\n",
    );
    let explicit = analyze_error(
        "OBSERVABLE_INCLUDE(0) Z0\nH 0\nS 0\nH 0\nX_ERROR(0.125) 0\nOBSERVABLE_INCLUDE(1) Z0\n",
    );
    for error in [direct, explicit] {
        assert!(error.contains("non-deterministic observables"), "{error}");
        assert!(error.contains("L1"), "{error}");
    }
}

#[test]
fn dem_analyzer_rejects_anti_hermitian_spp_products() {
    for gate_name in ["SPP", "SPP_DAG"] {
        let error = analyze_error(&format!("{gate_name} X0*Z0\nM 0\nDETECTOR rec[-1]\n"));
        assert!(error.contains("anti-Hermitian"), "{gate_name}: {error}");
    }
}
