#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{Circuit, ErrorAnalyzerOptions, Probability, circuit_to_detector_error_model};

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
fn dem_analyzer_duplicate_detector_records_match_upstream_parity() {
    let empty = analyze(
        "
        X_ERROR(0.25) 0
        M 0
        DETECTOR
        ",
    );
    let duplicate_even = analyze(
        "
        X_ERROR(0.25) 0
        M 0
        DETECTOR rec[-1] rec[-1]
        ",
    );
    let single = analyze(
        "
        X_ERROR(0.25) 0
        M 0
        DETECTOR rec[-1]
        ",
    );
    let duplicate_odd = analyze(
        "
        X_ERROR(0.25) 0
        M 0
        DETECTOR rec[-1] rec[-1] rec[-1]
        ",
    );

    assert_eq!(empty, duplicate_even);
    assert_eq!(single, duplicate_odd);
    assert_eq!(empty, "detector D0\n");
    assert_eq!(single, "error(0.25) D0\n");
}

#[test]
fn dem_analyzer_pauli_channel1_crosses_product_measurements_like_stim() {
    let dem = analyze_with_options(
        include_str!(
            "../../../oracle/fixtures/inputs/analyze_errors_pauli_channel1_product_measurements.stim"
        ),
        ErrorAnalyzerOptions {
            approximate_disjoint_errors_threshold: Some(Probability::try_new(1.0).unwrap()),
            ..ErrorAnalyzerOptions::default()
        },
    );

    assert_eq!(
        dem,
        "error(0.625) D0\nerror(0.5) D1\nerror(0.375) D2\nerror(0.625) D3\n"
    );
}

#[test]
fn dem_analyzer_rejects_measurement_record_before_beginning_like_upstream() {
    for circuit in [
        "
        DETECTOR rec[-1]
        ",
        "
        OBSERVABLE_INCLUDE(0) rec[-1]
        ",
    ] {
        let parsed = Circuit::from_stim_str(circuit).expect("circuit");
        let error = circuit_to_detector_error_model(&parsed, ErrorAnalyzerOptions::default())
            .unwrap_err()
            .to_string();
        assert!(error.contains("out of range"));
    }
}

#[test]
fn dem_analyzer_mpad_matches_upstream_subset() {
    let dem = analyze(
        "
        M(0.125) 5
        MPAD 0 1
        DETECTOR rec[-1] rec[-2]
        DETECTOR rec[-3]
        ",
    );

    assert_eq!(dem, "error(0.125) D1\ndetector D0\n");
}

#[test]
fn dem_analyzer_pair_measurements_match_upstream_subset() {
    for (circuit, expected) in [
        (
            "
            RX 0 1
            MXX(0.125) 0 1
            DETECTOR rec[-1]
            ",
            "error(0.125) D0\n",
        ),
        (
            "
            RX 0 1 2 3
            X_ERROR(0.125) 0
            Y_ERROR(0.25) 1
            Z_ERROR(0.375) 2
            MXX 0 1 !2 !3
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.25) D0\nerror(0.375) D1\n",
        ),
        (
            "
            RY 0 1
            MYY(0.125) 0 1
            DETECTOR rec[-1]
            ",
            "error(0.125) D0\n",
        ),
        (
            "
            RY 0 1 2 3
            Y_ERROR(0.125) 0
            X_ERROR(0.25) 1
            Z_ERROR(0.375) 2
            MYY 0 1 !2 !3
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.25) D0\nerror(0.375) D1\n",
        ),
        (
            "
            RZ 0 1
            MZZ(0.125) 0 1
            DETECTOR rec[-1]
            ",
            "error(0.125) D0\n",
        ),
        (
            "
            RZ 0 1 2 3
            Z_ERROR(0.125) 0
            Y_ERROR(0.25) 1
            X_ERROR(0.375) 2
            MZZ 0 1 !2 !3
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.25) D0\nerror(0.375) D1\n",
        ),
    ] {
        assert_eq!(analyze(circuit), expected);
    }
}

#[test]
fn dem_analyzer_noisy_basis_measurements_match_upstream_subset() {
    for (circuit, expected) in [
        (
            "
            RX 0
            MX(0.125) 0
            MX 0
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.125) D0\ndetector D1\n",
        ),
        (
            "
            RX 0 1
            Y_ERROR(1) 0 1
            MX(0.125) 0 1
            MX 0 1
            DETECTOR rec[-4]
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.125) D0\nerror(1) D0 D2\nerror(0.125) D1\nerror(1) D1 D3\n",
        ),
        (
            "
            RY 0
            MY(0.125) 0
            MY 0
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.125) D0\ndetector D1\n",
        ),
        (
            "
            RY 0 1
            Z_ERROR(1) 0 1
            MY(0.125) 0 1
            MY 0 1
            DETECTOR rec[-4]
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.125) D0\nerror(1) D0 D2\nerror(0.125) D1\nerror(1) D1 D3\n",
        ),
        (
            "
            RZ 0
            MZ(0.125) 0
            MZ 0
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.125) D0\ndetector D1\n",
        ),
        (
            "
            RZ 0 1
            X_ERROR(1) 0 1
            MZ(0.125) 0 1
            MZ 0 1
            DETECTOR rec[-4]
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.125) D0\nerror(1) D0 D2\nerror(0.125) D1\nerror(1) D1 D3\n",
        ),
        (
            "
            RX 0
            MRX(0.125) 0
            MRX 0
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.125) D0\ndetector D1\n",
        ),
        (
            "
            RX 0 1
            Z_ERROR(1) 0 1
            MRX(0.125) 0 1
            MRX 0 1
            DETECTOR rec[-4]
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.875) D0\nerror(0.875) D1\ndetector D2\ndetector D3\n",
        ),
        (
            "
            RY 0
            MRY(0.125) 0
            MRY 0
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.125) D0\ndetector D1\n",
        ),
        (
            "
            RY 0 1
            X_ERROR(1) 0 1
            MRY(0.125) 0 1
            MRY 0 1
            DETECTOR rec[-4]
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.875) D0\nerror(0.875) D1\ndetector D2\ndetector D3\n",
        ),
        (
            "
            RZ 0
            MRZ(0.125) 0
            MRZ 0
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.125) D0\ndetector D1\n",
        ),
        (
            "
            RZ 0 1
            X_ERROR(1) 0 1
            MRZ(0.125) 0 1
            MRZ 0 1
            DETECTOR rec[-4]
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.875) D0\nerror(0.875) D1\ndetector D2\ndetector D3\n",
        ),
    ] {
        assert_eq!(analyze(circuit), expected);
    }
}

#[test]
fn dem_analyzer_measure_reset_basis_matches_upstream_subset() {
    for (circuit, expected) in [
        (
            "
            RZ 0 1 2
            X_ERROR(0.25) 0
            Y_ERROR(0.25) 1
            Z_ERROR(0.25) 2
            MZ 0 1 2
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.25) D0\nerror(0.25) D1\ndetector D2\n",
        ),
        (
            "
            RX 0 1 2
            X_ERROR(0.25) 0
            Y_ERROR(0.25) 1
            Z_ERROR(0.25) 2
            MX 0 1 2
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.25) D1\nerror(0.25) D2\ndetector D0\n",
        ),
        (
            "
            RY 0 1 2
            X_ERROR(0.25) 0
            Y_ERROR(0.25) 1
            Z_ERROR(0.25) 2
            MY 0 1 2
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.25) D0\nerror(0.25) D2\ndetector D1\n",
        ),
        (
            "
            MRZ 0 1 2
            X_ERROR(0.25) 0
            Y_ERROR(0.25) 1
            Z_ERROR(0.25) 2
            MRZ 0 1 2
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.25) D0\nerror(0.25) D1\ndetector D2\n",
        ),
        (
            "
            MRX 0 1 2
            X_ERROR(0.25) 0
            Y_ERROR(0.25) 1
            Z_ERROR(0.25) 2
            MRX 0 1 2
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.25) D1\nerror(0.25) D2\ndetector D0\n",
        ),
        (
            "
            MRY 0 1 2
            X_ERROR(0.25) 0
            Y_ERROR(0.25) 1
            Z_ERROR(0.25) 2
            MRY 0 1 2
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.25) D0\nerror(0.25) D2\ndetector D1\n",
        ),
    ] {
        assert_eq!(analyze(circuit), expected);
    }
}

#[test]
fn dem_analyzer_repeated_measure_reset_matches_upstream_subset() {
    for (circuit, expected) in [
        (
            "
            MRZ 0 0
            X_ERROR(0.25) 0
            MRZ 0 0
            DETECTOR rec[-4]
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.25) D2\ndetector D0\ndetector D1\ndetector D3\n",
        ),
        (
            "
            RY 0 0
            MRY 0 0
            X_ERROR(0.25) 0
            MRY 0 0
            DETECTOR rec[-4]
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.25) D2\ndetector D0\ndetector D1\ndetector D3\n",
        ),
        (
            "
            RX 0 0
            MRX 0 0
            Z_ERROR(0.25) 0
            MRX 0 0
            DETECTOR rec[-4]
            DETECTOR rec[-3]
            DETECTOR rec[-2]
            DETECTOR rec[-1]
            ",
            "error(0.25) D2\ndetector D0\ndetector D1\ndetector D3\n",
        ),
    ] {
        assert_eq!(analyze(circuit), expected);
    }
}
