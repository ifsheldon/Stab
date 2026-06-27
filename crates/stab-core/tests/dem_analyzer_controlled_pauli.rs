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

fn analyze_with_options(text: &str, options: ErrorAnalyzerOptions) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(&circuit, options)
        .expect("analyze")
        .to_dem_string()
}

#[test]
fn dem_analyzer_quantum_controlled_pauli_gates_propagate_upstream_subset() {
    for circuit in [
        "
        RX 0
        R 1
        Z_ERROR(0.125) 0
        XCX 0 1
        M 1
        DETECTOR rec[-1]
        ",
        "
        RX 0
        R 1
        Z_ERROR(0.125) 0
        XCY 0 1
        M 1
        DETECTOR rec[-1]
        ",
        "
        RY 0
        R 1
        X_ERROR(0.125) 0
        YCX 0 1
        M 1
        DETECTOR rec[-1]
        ",
        "
        RY 0
        R 1
        X_ERROR(0.125) 0
        YCY 0 1
        M 1
        DETECTOR rec[-1]
        ",
        "
        R 0 1
        X_ERROR(0.125) 0
        CY 0 1
        M 1
        DETECTOR rec[-1]
        ",
        "
        R 0
        RX 1
        X_ERROR(0.125) 0
        CZ 0 1
        MX 1
        DETECTOR rec[-1]
        ",
        "
        RX 0 1
        Z_ERROR(0.125) 0
        XCZ 0 1
        MX 1
        DETECTOR rec[-1]
        ",
        "
        RY 0
        RX 1
        X_ERROR(0.125) 0
        YCZ 0 1
        MX 1
        DETECTOR rec[-1]
        ",
    ] {
        assert_eq!(analyze(circuit), "error(0.125) D0\n");
    }
}

#[test]
fn dem_analyzer_composite_controlled_pauli_case_matches_upstream_subset() {
    let dem = analyze_with_options(
        "
        XCX 0 1 0 3 0 4
        MR 0
        XCZ 0 1 0 2 0 4 0 5
        MR 0
        XCX 0 2 0 5 0 6
        MR 0
        XCZ 0 3 0 4 0 7
        MR 0
        XCX 0 4 0 5 0 7 0 8
        MR 0
        XCZ 0 5 0 6 0 7
        MR 0
        DEPOLARIZE1(0.01) 4
        XCX 0 1 0 3 0 4
        MR 0
        XCZ 0 1 0 2 0 4 0 5
        MR 0
        XCX 0 2 0 5 0 6
        MR 0
        XCZ 0 3 0 4 0 7
        MR 0
        XCX 0 4 0 5 0 7 0 8
        MR 0
        XCZ 0 5 0 6 0 7
        MR 0
        DETECTOR rec[-6] rec[-12]
        DETECTOR rec[-5] rec[-11]
        DETECTOR rec[-4] rec[-10]
        DETECTOR rec[-3] rec[-9]
        DETECTOR rec[-2] rec[-8]
        DETECTOR rec[-1] rec[-7]
        ",
        ErrorAnalyzerOptions {
            decompose_errors: true,
            ..ErrorAnalyzerOptions::default()
        },
    );

    assert_eq!(
        dem,
        "error(0.003344519141621982161183268544846214) D0 D4\nerror(0.003344519141621982161183268544846214) D0 D4 ^ D1 D3\nerror(0.003344519141621982161183268544846214) D1 D3\ndetector D2\ndetector D5\n"
    );
}
