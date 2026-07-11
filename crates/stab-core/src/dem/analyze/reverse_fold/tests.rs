#![allow(
    clippy::expect_used,
    reason = "focused analyzer regressions use direct failure diagnostics"
)]

use crate::{Circuit, ErrorAnalyzerOptions, sparse_rev_frame_tracker::SparseReverseFrameTracker};

use super::{AnalyzerProbeBudget, MAX_LOOP_CYCLE_STEPS, try_analyze};

#[test]
fn pfm_b5_nested_analyzer_probe_work_is_bounded() {
    let circuit = Circuit::from_stim_str(
        "\
R 0
M 0
DETECTOR rec[-1]
REPEAT 2 {
    REPEAT 1000000000 {
        X_ERROR(0.125) 0
        M 0
        DETECTOR rec[-1] rec[-2]
    }
}
M 0
DETECTOR rec[-1]
",
    )
    .expect("valid nested no-recurrence circuit");

    let error = try_analyze(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect_err("nested recurrence probing must stop at its cumulative work limit");
    assert!(
        error.to_string().contains(&format!(
            "at most {MAX_LOOP_CYCLE_STEPS} work units across nested circuits and instructions"
        )),
        "{error}"
    );
}

#[test]
fn pfm_b5_supported_unitary_nested_analyzer_probe_work_is_bounded() {
    let nested_unitary = Circuit::from_stim_str("REPEAT 1000000000 {\n    H 0\n}\n")
        .expect("valid supported-unitary repeat");
    let mut tracker = SparseReverseFrameTracker::new(1, 0, 0, false);
    let mut budget = AnalyzerProbeBudget::new(2);
    let error = tracker
        .undo_circuit_for_analyzer_probe(&nested_unitary, &mut budget)
        .expect_err("supported-unitary nested probes must share the instruction budget");
    assert!(
        error
            .to_string()
            .contains("at most 2 work units across nested circuits and instructions"),
        "{error}"
    );
}
