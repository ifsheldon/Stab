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
