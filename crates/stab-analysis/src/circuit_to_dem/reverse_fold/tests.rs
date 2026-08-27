#![allow(
    clippy::expect_used,
    reason = "focused analyzer regressions use direct failure diagnostics"
)]

use stab_model::Circuit;

use crate::ErrorAnalyzerOptions;
use crate::sparse_rev_frame_tracker::SparseReverseFrameTracker;

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
    let resource = error.resource_limit_error().expect("typed resource limit");
    assert_eq!(resource.resource(), crate::ResourceKind::ExpandedOperations);
    assert_eq!(resource.actual(), MAX_LOOP_CYCLE_STEPS + 1);
    assert_eq!(resource.limit(), MAX_LOOP_CYCLE_STEPS);
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
    let resource = error.resource_limit_error().expect("typed resource limit");
    assert_eq!(resource.resource(), crate::ResourceKind::ExpandedOperations);
    assert_eq!(resource.actual(), 3);
    assert_eq!(resource.limit(), 2);
}
