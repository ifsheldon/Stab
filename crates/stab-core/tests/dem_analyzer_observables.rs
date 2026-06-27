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
fn dem_analyzer_obs_include_pauli_depolarizing_boundaries_match_upstream_subset() {
    for (noise, expected) in [
        (
            "X_ERROR(0.25) 0",
            "error(0.25) L1 L2\nlogical_observable L0\nlogical_observable L0\n",
        ),
        (
            "Y_ERROR(0.25) 0",
            "error(0.25) L0 L2\nlogical_observable L1\nlogical_observable L1\n",
        ),
        (
            "Z_ERROR(0.25) 0",
            "error(0.25) L0 L1\nlogical_observable L2\nlogical_observable L2\n",
        ),
    ] {
        let circuit = format!(
            "DEPOLARIZE1(0.125) 0\n\
             OBSERVABLE_INCLUDE(0) X0\n\
             OBSERVABLE_INCLUDE(1) Y0\n\
             OBSERVABLE_INCLUDE(2) Z0\n\
             {noise}\n\
             OBSERVABLE_INCLUDE(0) X0\n\
             OBSERVABLE_INCLUDE(1) Y0\n\
             OBSERVABLE_INCLUDE(2) Z0\n\
             DEPOLARIZE1(0.125) 0\n"
        );
        assert_eq!(analyze(&circuit), expected);
    }
}

#[test]
fn dem_analyzer_obs_include_pauli_commuting_and_propagated_cases_match_upstream_subset() {
    let commuting = analyze(
        "
        OBSERVABLE_INCLUDE(0) X0
        OBSERVABLE_INCLUDE(1) Y0
        OBSERVABLE_INCLUDE(2) Z0
        X_ERROR(0.125) 0
        OBSERVABLE_INCLUDE(0) X0
        OBSERVABLE_INCLUDE(1) Y0
        OBSERVABLE_INCLUDE(2) Z0
        ",
    );
    assert_eq!(
        commuting,
        "error(0.125) L1 L2\nlogical_observable L0\nlogical_observable L0\n"
    );

    let propagated = analyze(
        "
        OBSERVABLE_INCLUDE(0) X0
        H 0
        Z_ERROR(0.125) 0
        H 0
        OBSERVABLE_INCLUDE(0) X0
        ",
    );
    assert_eq!(propagated, "logical_observable L0\nlogical_observable L0\n");
}
