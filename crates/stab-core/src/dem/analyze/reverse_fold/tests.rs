#![allow(
    clippy::expect_used,
    reason = "focused analyzer regressions use direct failure diagnostics"
)]

use crate::{Circuit, ErrorAnalyzerOptions};

use super::{MAX_LOOP_CYCLE_STEPS, try_analyze};

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
