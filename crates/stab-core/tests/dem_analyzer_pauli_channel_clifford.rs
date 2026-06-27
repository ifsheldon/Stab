#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{Circuit, ErrorAnalyzerOptions, Probability, circuit_to_detector_error_model};

#[test]
fn dem_analyzer_pauli_channel1_crosses_two_qubit_cliffords_like_stim() {
    let circuit = Circuit::from_stim_str(include_str!(
        "../../../oracle/fixtures/inputs/analyze_errors_pauli_channel1_two_qubit_clifford.stim"
    ))
    .expect("circuit");
    let dem = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            approximate_disjoint_errors_threshold: Some(Probability::try_new(1.0).unwrap()),
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze")
    .to_dem_string();

    assert_eq!(
        dem,
        "error(0.375) D1\nerror(0.125) D2\nerror(0.375) D2 D3\nerror(0.25) D3\nerror(0.375) D4 D5\ndetector D0\n"
    );
}

#[test]
fn dem_analyzer_pauli_channel1_crosses_controlled_pauli_gates_like_stim() {
    let circuit = Circuit::from_stim_str(include_str!(
        "../../../oracle/fixtures/inputs/analyze_errors_pauli_channel1_controlled_pauli.stim"
    ))
    .expect("circuit");
    let dem = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            approximate_disjoint_errors_threshold: Some(Probability::try_new(1.0).unwrap()),
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze")
    .to_dem_string();

    assert_eq!(
        dem,
        "error(0.375) D0 D1\nerror(0.375) D2 D3\nerror(0.625) D4 D5\n"
    );
}
