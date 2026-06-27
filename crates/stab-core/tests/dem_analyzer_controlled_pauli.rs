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
