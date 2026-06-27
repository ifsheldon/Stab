#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{Circuit, ErrorAnalyzerOptions, Probability, circuit_to_detector_error_model};

fn approximate_options() -> ErrorAnalyzerOptions {
    ErrorAnalyzerOptions {
        approximate_disjoint_errors_threshold: Some(Probability::try_new(1.0).unwrap()),
        ..ErrorAnalyzerOptions::default()
    }
}

fn analyze(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(&circuit, approximate_options())
        .expect("analyze")
        .to_dem_string()
}

fn conditional_erase_circuit(herald: bool, x: bool, z: bool) -> String {
    let herald_detector = if herald {
        "DETECTOR rec[-3]\n"
    } else {
        "DETECTOR\n"
    };
    let x_detector = if x {
        "DETECTOR rec[-2] rec[-5]\n"
    } else {
        "DETECTOR\n"
    };
    let z_detector = if z {
        "DETECTOR rec[-1] rec[-4]\n"
    } else {
        "DETECTOR\n"
    };
    format!(
        "MPP X0*X1 Z0*Z1\n\
         HERALDED_ERASE(1.0) 0\n\
         MPP X0*X1 Z0*Z1\n\
         {herald_detector}\
         {x_detector}\
         {z_detector}"
    )
}

#[test]
fn dem_analyzer_heralded_erase_conditional_division_matches_upstream() {
    for (herald, x, z, expected) in [
        (
            false,
            false,
            false,
            "detector D0\ndetector D1\ndetector D2\n",
        ),
        (
            false,
            false,
            true,
            "error(0.5) D2\ndetector D0\ndetector D1\n",
        ),
        (
            false,
            true,
            false,
            "error(0.5) D1\ndetector D0\ndetector D2\n",
        ),
        (
            false,
            true,
            true,
            "error(0.25) D1\nerror(0.25) D1 D2\nerror(0.25) D2\ndetector D0\n",
        ),
        (
            true,
            false,
            false,
            "error(1) D0\ndetector D1\ndetector D2\n",
        ),
        (
            true,
            false,
            true,
            "error(0.5) D0\nerror(0.5) D0 D2\ndetector D1\n",
        ),
        (
            true,
            true,
            false,
            "error(0.5) D0\nerror(0.5) D0 D1\ndetector D2\n",
        ),
        (
            true,
            true,
            true,
            "error(0.25) D0\nerror(0.25) D0 D1\nerror(0.25) D0 D1 D2\nerror(0.25) D0 D2\n",
        ),
    ] {
        assert_eq!(analyze(&conditional_erase_circuit(herald, x, z)), expected);
    }
}
