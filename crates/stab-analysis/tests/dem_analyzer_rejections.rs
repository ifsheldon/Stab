#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "rejection tables use fixed circuits with direct assertion diagnostics"
)]

//! User-reachable analyzer rejections, each paired with an accepted boundary twin so the test
//! pins the rejected parameter rather than an incidental failure. Every row runs in both fold
//! modes. Accept/reject for each row agrees with pinned Stim v1.16.0 `stim analyze_errors`
//! (with and without `--fold_loops`); error wording is Stab's own and is not asserted.

use stab_analysis::{AnalysisError, ErrorAnalyzerOptions, circuit_to_detector_error_model};
use stab_model::{Circuit, Probability};

struct Row {
    name: &'static str,
    rejected: &'static str,
    accepted: &'static str,
    options: ErrorAnalyzerOptions,
}

fn approximate_disjoint(threshold: f64) -> ErrorAnalyzerOptions {
    ErrorAnalyzerOptions {
        approximate_disjoint_errors_threshold: Some(
            Probability::try_new(threshold).expect("threshold"),
        ),
        ..ErrorAnalyzerOptions::default()
    }
}

fn decomposing() -> ErrorAnalyzerOptions {
    ErrorAnalyzerOptions {
        decompose_errors: true,
        ignore_decomposition_failures: true,
        ..ErrorAnalyzerOptions::default()
    }
}

fn rows() -> Vec<Row> {
    vec![
        Row {
            // The leading E(0.2) is under the threshold; the chained component is 0.5 * 0.8.
            name: "ELSE_CORRELATED_ERROR chain component above approximate-disjoint threshold",
            rejected: "E(0.2) X0\nELSE_CORRELATED_ERROR(0.5) X1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
            accepted: "E(0.2) X0\nELSE_CORRELATED_ERROR(0.3) X1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
            options: approximate_disjoint(0.3),
        },
        Row {
            name: "ELSE_CORRELATED_ERROR chain component above threshold inside a folded loop",
            rejected: "REPEAT 100 {\n    R 0 1\n    E(0.2) X0\n    ELSE_CORRELATED_ERROR(0.5) X1\n    M 0 1\n    DETECTOR rec[-2]\n    DETECTOR rec[-1]\n}\n",
            accepted: "REPEAT 100 {\n    R 0 1\n    E(0.2) X0\n    ELSE_CORRELATED_ERROR(0.3) X1\n    M 0 1\n    DETECTOR rec[-2]\n    DETECTOR rec[-1]\n}\n",
            options: approximate_disjoint(0.3),
        },
        Row {
            // A single non-zero component is an independent error and needs no approximation.
            name: "PAULI_CHANNEL_2 with two non-zero components without approximate_disjoint_errors",
            rejected: "PAULI_CHANNEL_2(0.1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0.05) 0 1\nM 0 1\nDETECTOR rec[-1]\n",
            accepted: "PAULI_CHANNEL_2(0.6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0) 0 1\nM 0 1\nDETECTOR rec[-1]\n",
            options: ErrorAnalyzerOptions::default(),
        },
        Row {
            // The threshold applies only once two components make the channel disjoint.
            name: "PAULI_CHANNEL_2 component above approximate-disjoint threshold",
            rejected: "PAULI_CHANNEL_2(0.1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0.05) 0 1\nM 0 1\nDETECTOR rec[-1]\n",
            accepted: "PAULI_CHANNEL_2(0.6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0) 0 1\nM 0 1\nDETECTOR rec[-1]\n",
            options: approximate_disjoint(0.08),
        },
        Row {
            // Pinned Stim checks the arguments before it looks at the (absent) targets.
            name: "target-less PAULI_CHANNEL_2 with two non-zero components",
            rejected: "PAULI_CHANNEL_2(0.1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0.05)\nM 0\nDETECTOR rec[-1]\n",
            accepted: "PAULI_CHANNEL_2(0.1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)\nM 0\nDETECTOR rec[-1]\n",
            options: ErrorAnalyzerOptions::default(),
        },
        Row {
            name: "over-mixing DEPOLARIZE1 above 3/4",
            rejected: "DEPOLARIZE1(0.76) 0\nM 0\nDETECTOR rec[-1]\n",
            accepted: "DEPOLARIZE1(0.75) 0\nM 0\nDETECTOR rec[-1]\n",
            options: ErrorAnalyzerOptions::default(),
        },
        Row {
            name: "over-mixing DEPOLARIZE2 above 15/16",
            rejected: "DEPOLARIZE2(0.95) 0 1\nM 0\nDETECTOR rec[-1]\n",
            accepted: "DEPOLARIZE2(0.9375) 0 1\nM 0\nDETECTOR rec[-1]\n",
            options: ErrorAnalyzerOptions::default(),
        },
        Row {
            name: "over-mixing DEPOLARIZE2 inside a folded loop",
            rejected: "REPEAT 100 {\n    R 0 1\n    DEPOLARIZE2(0.95) 0 1\n    M 0\n    DETECTOR rec[-1]\n}\n",
            accepted: "REPEAT 100 {\n    R 0 1\n    DEPOLARIZE2(0.9375) 0 1\n    M 0\n    DETECTOR rec[-1]\n}\n",
            options: ErrorAnalyzerOptions::default(),
        },
        Row {
            // Only the first iteration looks back past the start; later iterations are in range.
            name: "record lookback before the first measurement on a loop's first iteration",
            rejected: "MR 0\nREPEAT 1000 {\n    X_ERROR(0.1) 0\n    MR 0\n    DETECTOR rec[-3]\n}\n",
            accepted: "MR 0 0\nREPEAT 1000 {\n    X_ERROR(0.1) 0\n    MR 0\n    DETECTOR rec[-3]\n}\n",
            options: ErrorAnalyzerOptions::default(),
        },
        Row {
            name: "record lookback before the first measurement across a nested loop",
            rejected: "REPEAT 1000 {\n    REPEAT 2 {\n        X_ERROR(0.1) 0\n        MR 0\n    }\n    DETECTOR rec[-5]\n}\n",
            accepted: "REPEAT 1000 {\n    REPEAT 2 {\n        X_ERROR(0.1) 0\n        MR 0\n    }\n    DETECTOR rec[-2]\n}\n",
            options: ErrorAnalyzerOptions::default(),
        },
        Row {
            name: "observable lookback before the first measurement after a folded loop",
            rejected: "REPEAT 1000 {\n    R 0\n    X_ERROR(0.1) 0\n    M 0\n    DETECTOR rec[-1]\n}\nOBSERVABLE_INCLUDE(0) rec[-1001]\n",
            accepted: "REPEAT 1000 {\n    R 0\n    X_ERROR(0.1) 0\n    M 0\n    DETECTOR rec[-1]\n}\nOBSERVABLE_INCLUDE(0) rec[-1000]\n",
            options: ErrorAnalyzerOptions::default(),
        },
        Row {
            // Decomposition failures are ignored, so only the observable-mask limit can reject.
            name: "decomposing a hyperedge that flips observable 64",
            rejected: "E(0.1) X0 X1\nX_ERROR(0.1) 2\nE(0.1) X0 X1 X2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(64) rec[-1]\n",
            accepted: "E(0.1) X0 X1\nX_ERROR(0.1) 2\nE(0.1) X0 X1 X2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(63) rec[-1]\n",
            options: decomposing(),
        },
        Row {
            name: "decomposing hyperedges inside a folded loop that feed observable 64",
            rejected: "REPEAT 1000 {\n    R 0 1 2\n    E(0.1) X0 X1\n    X_ERROR(0.1) 2\n    E(0.1) X0 X1 X2\n    M 0 1 2\n    DETECTOR rec[-3]\n    DETECTOR rec[-2]\n    DETECTOR rec[-1]\n}\nOBSERVABLE_INCLUDE(64) rec[-1]\n",
            accepted: "REPEAT 1000 {\n    R 0 1 2\n    E(0.1) X0 X1\n    X_ERROR(0.1) 2\n    E(0.1) X0 X1 X2\n    M 0 1 2\n    DETECTOR rec[-3]\n    DETECTOR rec[-2]\n    DETECTOR rec[-1]\n}\nOBSERVABLE_INCLUDE(63) rec[-1]\n",
            options: decomposing(),
        },
    ]
}

#[test]
fn analyzer_rejections_match_stim_in_both_fold_modes() {
    for row in rows() {
        for fold_loops in [false, true] {
            let options = ErrorAnalyzerOptions {
                fold_loops,
                ..row.options.clone()
            };
            let rejected = Circuit::from_stim_str(row.rejected).expect("rejected circuit parses");
            match circuit_to_detector_error_model(&rejected, options.clone()) {
                Err(AnalysisError::InvalidDetectorErrorModel { .. }) => {}
                other => panic!(
                    "{} (fold_loops={fold_loops}): expected an invalid-DEM rejection, got {other:?}",
                    row.name
                ),
            }
            let accepted = Circuit::from_stim_str(row.accepted).expect("accepted circuit parses");
            if let Err(error) = circuit_to_detector_error_model(&accepted, options) {
                panic!(
                    "{} (fold_loops={fold_loops}): boundary twin was rejected: {error}",
                    row.name
                );
            }
        }
    }
}
