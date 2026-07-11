#![allow(
    clippy::expect_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{Circuit, ErrorAnalyzerOptions, Probability, circuit_to_detector_error_model};

#[cfg(feature = "ops-contracts")]
use stab_core::__circuit_to_detector_error_model_with_diagnostics;

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

fn analyze_folding_and_decomposing_errors(
    text: &str,
    block_remnant_edges: bool,
) -> Result<String, String> {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            decompose_errors: true,
            block_decomposition_from_introducing_remnant_edges: block_remnant_edges,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .map(|dem| dem.to_dem_string())
    .map_err(|error| error.to_string())
}

fn analyze_folding_with_decomposition_and_approximation(text: &str) -> Result<String, String> {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            decompose_errors: true,
            approximate_disjoint_errors_threshold: Some(
                Probability::try_new(1.0).expect("probability"),
            ),
            ..ErrorAnalyzerOptions::default()
        },
    )
    .map(|dem| dem.to_dem_string())
    .map_err(|error| error.to_string())
}

fn detector_declarations(first: u64, last: u64) -> String {
    let mut text = String::new();
    for detector in first..=last {
        text.push_str(&format!("detector D{detector}\n"));
    }
    text
}

fn indented_detector_declarations(first: u64, last: u64) -> String {
    let mut text = String::new();
    for detector in first..=last {
        text.push_str(&format!("    detector D{detector}\n"));
    }
    text
}

fn period127_observable_circuit(repeat_count: u64) -> String {
    format!(
        "
        R 0 1 2 3 4 5 6
        REPEAT {repeat_count} {{
            CNOT 0 1 1 2 2 3 3 4 4 5 5 6 6 0
            DETECTOR
        }}
        M 6
        OBSERVABLE_INCLUDE(9) rec[-1]
        R 7
        X_ERROR(1) 7
        M 7
        DETECTOR rec[-1]
        "
    )
}

fn period127_observable_expected(middle_repeat_count: u64) -> String {
    [
        detector_declarations(0, 85),
        format!("repeat {middle_repeat_count} {{\n"),
        indented_detector_declarations(86, 212),
        "    shift_detectors 127\n}\n".to_string(),
        "error(1) D211\n".to_string(),
        detector_declarations(86, 210),
        "logical_observable L9\n".to_string(),
    ]
    .concat()
}

#[test]
fn dem_analyzer_fold_loops_preserves_simple_nested_repeat_like_stim() {
    let dem = analyze_folding_loops(
        "
        REPEAT 3 {
            REPEAT 2 {
                R 0
                X_ERROR(0.25) 0
                M 0
                DETECTOR rec[-1]
            }
        }
        ",
    );

    assert_eq!(
        dem,
        "repeat 3 {\n    repeat 2 {\n        error(0.25) D0\n        shift_detectors 1\n    }\n}\n"
    );
}

#[test]
fn dem_analyzer_rejects_nested_repeat_expansion_budget() {
    let circuit = Circuit::from_stim_str(
        "
        REPEAT 100000 {
            REPEAT 100000 {
                M 0
                DETECTOR rec[-1]
            }
        }
        ",
    )
    .expect("circuit");

    let error = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect_err("reject nested expansion");

    assert!(error.to_string().contains("expanded repeat iterations"));
}

#[test]
fn pf6_dem_analyzer_fold_loops_decomposes_repeat_errors() {
    let dem = analyze_folding_and_decomposing_errors(
        "
        REPEAT 5 {
            R 0 1 2
            X_ERROR(0.125) 0
            X_ERROR(0.25) 1
            X_ERROR(0.375) 2
            M 0 1 2
            DETECTOR rec[-3] rec[-1]
            DETECTOR rec[-2] rec[-1]
            DETECTOR rec[-3] rec[-1]
        }
        ",
        true,
    )
    .expect("analyze");

    assert_eq!(
        dem,
        "repeat 5 {\n    error(0.125) D0 D2\n    error(0.375) D0 D2 ^ D1\n    error(0.25) D1\n    shift_detectors 3\n}\n"
    );
}

#[test]
fn pf6_dem_analyzer_fold_loops_respects_remnant_edge_blocking() {
    let fixture = "
        REPEAT 5 {
            R 0 1
            X_ERROR(0.125) 0
            CORRELATED_ERROR(0.25) X0 X1
            M 0 1
            DETECTOR rec[-1]
            DETECTOR rec[-1]
            DETECTOR rec[-2]
            DETECTOR rec[-2]
        }
        ";

    let error = analyze_folding_and_decomposing_errors(fixture, true).expect_err("block remnant");
    assert!(error.contains("Failed to decompose errors into graphlike components"));
    assert!(error.contains("block_decomposition_from_introducing_remnant_edges"));

    let dem = analyze_folding_and_decomposing_errors(fixture, false).expect("analyze");
    assert_eq!(
        dem,
        "repeat 5 {\n    error(0.125) D2 D3\n    error(0.25) D2 D3 ^ D0 D1\n    shift_detectors 4\n}\n"
    );
}

#[test]
fn pf6_dem_analyzer_prefix_repeat_tail_folds_detector_chain() {
    let dem = analyze_folding_loops(
        "
        R 0
        M 0
        DETECTOR rec[-1]
        REPEAT 2 {
            X_ERROR(0.125) 0
            M 0
            DETECTOR rec[-1] rec[-2]
        }
        M 0
        DETECTOR rec[-1] rec[-2]
        ",
    );

    assert_eq!(
        dem,
        "detector D0\nrepeat 2 {\n    error(0.125) D1\n    shift_detectors 1\n}\ndetector D1\n"
    );
}

#[test]
fn pf6_dem_analyzer_prefix_repeat_tail_folds_tail_error() {
    let dem = analyze_folding_loops(
        "
        R 0
        M 0
        DETECTOR rec[-1]
        REPEAT 2 {
            X_ERROR(0.125) 0
            M 0
            DETECTOR rec[-1] rec[-2]
        }
        X_ERROR(0.25) 0
        M 0
        DETECTOR rec[-1] rec[-2]
        ",
    );

    assert_eq!(
        dem,
        "detector D0\nrepeat 2 {\n    error(0.125) D1\n    shift_detectors 1\n}\nerror(0.25) D1\n"
    );
}

#[test]
fn pf6_dem_analyzer_prefix_repeat_tail_folds_large_detector_chain() {
    let dem = analyze_folding_loops(
        "
        R 0
        M 0
        DETECTOR rec[-1]
        REPEAT 100001 {
            X_ERROR(0.125) 0
            M 0
            DETECTOR rec[-1] rec[-2]
        }
        M 0
        DETECTOR rec[-1] rec[-2]
        ",
    );

    assert_eq!(
        dem,
        "detector D0\nrepeat 100001 {\n    error(0.125) D1\n    shift_detectors 1\n}\ndetector D1\n"
    );
}

#[test]
fn pf6_dem_analyzer_loop_carried_observable_folds_like_upstream() {
    let dem = analyze_folding_loops(
        "
        MR 1
        REPEAT 12345678987654321 {
            X_ERROR(0.25) 0
            CX 0 1
            MR 1
            DETECTOR rec[-2] rec[-1]
        }
        M 0
        OBSERVABLE_INCLUDE(9) rec[-1]
        ",
    );

    assert_eq!(
        dem,
        "error(0.25) D0 L9\nrepeat 6172839493827159 {\n    error(0.25) D1 L9\n    error(0.25) D2 L9\n    shift_detectors 2\n}\nerror(0.25) D1 L9\nerror(0.25) D2 L9\n"
    );
}

#[test]
fn pf6_dem_analyzer_period8_observable_folds_like_upstream() {
    let dem = analyze_folding_loops(
        "
        R 0 1 2 3 4
        REPEAT 12345678987654321 {
            CNOT 0 1 1 2 2 3 3 4
            DETECTOR
        }
        M 4
        OBSERVABLE_INCLUDE(9) rec[-1]
        ",
    );

    assert_eq!(
        dem,
        "detector D0\ndetector D1\ndetector D2\nrepeat 1543209873456789 {\n    detector D3\n    detector D4\n    detector D5\n    detector D6\n    detector D7\n    detector D8\n    detector D9\n    detector D10\n    shift_detectors 8\n}\ndetector D3\ndetector D4\ndetector D5\ndetector D6\ndetector D7\ndetector D8\nlogical_observable L9\n"
    );
}

#[test]
fn pf6_dem_analyzer_period127_observable_folds_like_upstream() {
    let dem = analyze_folding_loops(&period127_observable_circuit(12345678987654321));

    assert_eq!(dem, period127_observable_expected(97210070768930));
}

#[test]
fn pf6_dem_analyzer_period127_observable_folds_minimum_compact_shape_like_upstream() {
    let dem = analyze_folding_loops(&period127_observable_circuit(465));

    assert_eq!(dem, period127_observable_expected(2));
}

#[test]
fn pf6_dem_analyzer_period127_observable_keeps_single_middle_repeat_unfolded() {
    let circuit =
        Circuit::from_stim_str(&period127_observable_circuit(338)).expect("period-127 circuit");
    let expected = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect("non-folded analysis")
        .to_dem_string();
    let actual = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("fold-loop bounded fallback")
    .to_dem_string();

    assert_eq!(actual, expected);
    assert!(
        actual.starts_with("error(1) D338\ndetector D0\n"),
        "{actual}"
    );
    assert!(!actual.contains("repeat 1"), "{actual}");
    assert!(actual.ends_with("logical_observable L9\n"), "{actual}");
}

#[test]
fn pf6_dem_analyzer_period127_observable_folds_adjacent_residue() {
    let circuit =
        Circuit::from_stim_str(&period127_observable_circuit(466)).expect("period-127 circuit");
    let actual = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("generic period-127 fold")
    .to_dem_string();

    assert!(actual.starts_with("detector D0\n"), "{actual}");
    assert!(actual.contains("repeat 2 {"), "{actual}");
    assert!(actual.contains("error(1) D212"), "{actual}");
    assert!(actual.ends_with("logical_observable L9\n"), "{actual}");
}

#[test]
fn pf6_dem_analyzer_period127_observable_folds_huge_adjacent_residue() {
    let circuit = Circuit::from_stim_str(&period127_observable_circuit(1_000_083))
        .expect("period-127 circuit");

    let actual = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("generic huge period-127 fold")
    .to_dem_string();

    assert!(actual.contains("repeat 7873 {"), "{actual}");
    assert!(actual.contains("error(1) D212"), "{actual}");
    assert!(actual.lines().count() < 400, "{actual}");
}

#[test]
fn pf6_dem_analyzer_fallback_uses_bounded_unfolded_for_unsafe_tail_dependency() {
    let circuit = Circuit::from_stim_str(
        "
        R 0
        M 0
        DETECTOR rec[-1]
        REPEAT 2 {
            X_ERROR(0.125) 0
            M 0
            DETECTOR rec[-1] rec[-2]
        }
        M 0
        DETECTOR rec[-1]
        ",
    )
    .expect("circuit");

    let expected = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect("non-folded analysis")
        .to_dem_string();
    let actual = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("fold-loop bounded fallback")
    .to_dem_string();

    assert_eq!(actual, expected);
    assert!(!actual.contains("repeat"), "{actual}");
}

#[test]
fn pf6_dem_analyzer_fallback_preserves_delayed_rec_dependency() {
    let circuit = Circuit::from_stim_str(
        "
        M 0
        DETECTOR rec[-1]
        M 0
        DETECTOR rec[-1]
        M 0
        DETECTOR rec[-1]
        M 0
        DETECTOR rec[-1]
        REPEAT 5 {
            X_ERROR(0.125) 0
            M 0
            R 0
            DETECTOR rec[-1] rec[-5]
        }
        M 1
        DETECTOR rec[-1]
        ",
    )
    .expect("circuit");

    let expected = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect("non-folded analysis")
        .to_dem_string();
    let actual = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("fold-loop bounded fallback")
    .to_dem_string();

    assert_eq!(actual, expected);
    assert!(actual.contains("error(0.125) D4 D8"), "{actual}");
    assert!(!actual.contains("repeat"), "{actual}");
}

#[cfg(feature = "ops-contracts")]
#[test]
fn pf6_dem_analyzer_fallback_reports_unsupported_instruction_path() {
    let circuit = Circuit::from_stim_str(
        "\
REPEAT 2 {
    HERALDED_ERASE(0.125) 0
    DETECTOR rec[-1]
}
",
    )
    .expect("valid heralded fallback circuit");
    let (model, diagnostics) = __circuit_to_detector_error_model_with_diagnostics(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            approximate_disjoint_errors_threshold: Some(
                Probability::try_new(1.0).expect("valid threshold"),
            ),
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("bounded heralded fallback");

    assert!(diagnostics.used_bounded_fallback);
    assert!(!diagnostics.used_reverse_fold);
    assert_eq!(model.to_dem_string(), "error(0.125) D0\nerror(0.125) D1\n");
}

#[test]
fn pf6_dem_analyzer_fallback_enforces_each_expansion_budget() {
    let cases = [
        (
            "REPEAT 100001 {\n    HERALDED_ERASE(0.125) 0\n}\n".to_string(),
            "repeat counts up to 100000",
        ),
        (
            "REPEAT 1001 {\n    REPEAT 1000 {\n        HERALDED_ERASE(0.125) 0\n    }\n}\n"
                .to_string(),
            "at most 1000000 expanded repeat iterations",
        ),
        (
            format!(
                "REPEAT 100000 {{\n{} }}\n",
                "    HERALDED_ERASE(0.125) 0\n    TICK\n".repeat(6)
            ),
            "at most 1000000 expanded instructions",
        ),
    ];

    for (text, expected) in cases {
        let circuit = Circuit::from_stim_str(&text).expect("valid fallback budget circuit");
        let error = circuit_to_detector_error_model(
            &circuit,
            ErrorAnalyzerOptions {
                fold_loops: true,
                ..ErrorAnalyzerOptions::default()
            },
        )
        .expect_err("fallback expansion must be preflighted");
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} for:\n{text}\ngot: {error}"
        );
    }
}

#[test]
fn pf6_dem_analyzer_generic_prefixed_repeat_matches_unfolded() {
    let circuit = Circuit::from_stim_str(
        "
        X_ERROR(0.125) 0
        REPEAT 2 {
            M 0
            DETECTOR rec[-1]
        }
        ",
    )
    .expect("circuit");

    let non_folded = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect("non-folded analysis still succeeds")
        .to_dem_string();
    assert_eq!(non_folded, "error(0.125) D0 D1\n");

    let folded = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("generic prefixed-repeat analysis")
    .to_dem_string();

    assert_eq!(folded, non_folded);
}

#[test]
fn pf6_dem_analyzer_rejects_folded_observables_crossing_iterations() {
    let error = analyze_folding_with_decomposition_and_approximation(
        "
        RX 0 2
        REPEAT 100 {
            R 1
            CX 0 1 2 1
            MRZ 1
            MRX 2
        }
        MX 0
        OBSERVABLE_INCLUDE(0) rec[-1] rec[-2] rec[-4]
        ",
    )
    .expect_err("reject incomplete observable dependency across folded iterations");

    assert!(error.contains("non-deterministic observables"), "{error}");
    assert!(error.contains("L0"), "{error}");
}

#[test]
fn pf6_dem_analyzer_no_recurrence_preserves_repeat_count_cap() {
    let circuit = Circuit::from_stim_str(
        "
        R 0
        M 0
        DETECTOR rec[-1]
        REPEAT 100001 {
            X_ERROR(0.125) 0
            M 0
            DETECTOR rec[-1] rec[-2]
        }
        M 0
        DETECTOR rec[-1]
        ",
    )
    .expect("circuit");

    let error = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect_err("reject unsummarizable repeat beyond cap");

    assert!(
        error
            .to_string()
            .contains("found no loop-state recurrence within 1000000 iterations"),
        "{error}"
    );
}

#[test]
fn pf6_dem_analyzer_no_recurrence_rejects_delayed_dependency_beyond_cap() {
    let circuit = Circuit::from_stim_str(
        "
        M 0
        DETECTOR rec[-1]
        M 0
        DETECTOR rec[-1]
        M 0
        DETECTOR rec[-1]
        M 0
        DETECTOR rec[-1]
        REPEAT 100001 {
            X_ERROR(0.125) 0
            M 0
            R 0
            DETECTOR rec[-1] rec[-5]
        }
        M 1
        DETECTOR rec[-1]
        ",
    )
    .expect("circuit");

    let error = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect_err("reject unsummarizable delayed dependency beyond repeat cap");

    assert!(
        error
            .to_string()
            .contains("found no loop-state recurrence within 1000000 iterations"),
        "{error}"
    );
}

#[test]
fn pf6_dem_analyzer_folds_nested_measurement_only_repeats() {
    let circuit = Circuit::from_stim_str(
        "
        M 0
        REPEAT 10000 {
            REPEAT 101 {
                M 0
            }
        }
        M 0
        ",
    )
    .expect("circuit");

    let actual = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("fold nested measurement-only repeats")
    .to_dem_string();

    assert_eq!(actual, "repeat 10000 {\n    repeat 101 {\n    }\n}\n");
}

#[test]
fn pf6_dem_analyzer_folds_measurement_only_instruction_volume() {
    let circuit = Circuit::from_stim_str(
        "
        M 0
        REPEAT 100000 {
            M 0
            R 0
            M 0
            R 0
            M 0
            R 0
            M 0
            R 0
            M 0
            R 0
            M 0
        }
        M 0
        ",
    )
    .expect("circuit");

    let actual = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("fold measurement-only instruction volume")
    .to_dem_string();

    assert_eq!(actual, "repeat 100000 {\n}\n");
}
