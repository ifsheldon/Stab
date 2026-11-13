#![allow(
    clippy::expect_used,
    reason = "regression tests use fixed circuits with direct assertions"
)]

//! Pinned Stim keeps a measurement-record sensitivity entry alive after duplicate `DETECTOR` or
//! `OBSERVABLE_INCLUDE` record targets cancel it to empty, and that entry participates in the
//! shifted-copy test that chooses the folded loop period. Dropping cancelled entries made Stab fold
//! all 23 iterations where Stim folds 22 and analyzes the first iteration separately.
//!
//! Expected outputs are `stim analyze_errors --fold_loops` captures from pinned Stim v1.16.0.

use stab_analysis::{ErrorAnalyzerOptions, circuit_to_detector_error_model};
use stab_model::Circuit;

fn analyze_folding_loops(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze")
    .to_dem_string()
}

#[test]
fn cancelled_record_declarations_choose_stim_fold_period() {
    for (name, circuit, expected) in [
        (
            "cancelled detector record",
            "R 0\nM 0\nREPEAT 23 {\n    R 0\n    X_ERROR(0.25) 0\n    M 0\n    DETECTOR rec[-1] rec[-2] rec[-2]\n}\n",
            "repeat 22 {\n    error(0.25) D0\n    shift_detectors 1\n}\nerror(0.25) D0\n",
        ),
        (
            "cancelled observable record",
            "R 0\nM 0\nREPEAT 23 {\n    R 0\n    X_ERROR(0.25) 0\n    M 0\n    DETECTOR rec[-1]\n    OBSERVABLE_INCLUDE(0) rec[-2] rec[-2]\n}\n",
            "repeat 22 {\n    error(0.25) D0\n    logical_observable L0\n    shift_detectors 1\n}\nerror(0.25) D0\nlogical_observable L0\n",
        ),
    ] {
        assert_eq!(analyze_folding_loops(circuit), expected, "{name}");
    }
}
