#![allow(
    clippy::expect_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{Circuit, ErrorAnalyzerOptions, circuit_to_detector_error_model};

#[test]
fn dem_analyzer_two_qubit_cliffords_propagate_pauli_errors_like_stim() {
    let circuit = Circuit::from_stim_str(include_str!(
        "../../../oracle/fixtures/inputs/analyze_errors_two_qubit_cliffords.stim"
    ))
    .expect("circuit");
    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect("analyze")
        .to_dem_string();

    assert_eq!(
        dem,
        "error(0.125) D1\nerror(0.125) D3\nerror(0.125) D4 D5\nerror(0.125) D7\nerror(0.125) D9\nerror(0.125) D10 D11\nerror(0.125) D12 D13\nerror(0.125) D15\nerror(0.125) D17\nerror(0.125) D18 D19\nerror(0.125) D20 D21\ndetector D0\ndetector D2\ndetector D6\ndetector D8\ndetector D14\ndetector D16\n"
    );
}
