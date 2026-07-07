#![allow(
    clippy::expect_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{Circuit, ErrorAnalyzerOptions, Probability, circuit_to_detector_error_model};

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

#[test]
fn pf6_dem_analyzer_fallback_does_not_mask_prefixed_repeat_errors() {
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

    let error = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect_err("preserve unsupported prefixed-repeat folded error");

    assert!(
        error
            .to_string()
            .contains("supports prefixed repeats only when the first iteration ends"),
        "{error}"
    );
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
fn pf6_dem_analyzer_fallback_preserves_repeat_count_cap() {
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
    .expect_err("reject unsupported mixed top-level expansion beyond cap");

    assert!(
        error
            .to_string()
            .contains("analyze_errors currently supports repeat counts up to 100000"),
        "{error}"
    );
}

#[test]
fn pf6_dem_analyzer_fallback_preserves_repeat_count_cap_for_delayed_rec_dependency() {
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
    .expect_err("reject unsupported delayed dependency beyond repeat cap");

    assert!(
        error
            .to_string()
            .contains("analyze_errors currently supports repeat counts up to 100000"),
        "{error}"
    );
}

#[test]
fn pf6_dem_analyzer_fallback_preserves_repeat_iteration_cap() {
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

    let error = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect_err("reject aggregate repeat iterations beyond cap");

    assert!(
        error.to_string().contains("expanded repeat iterations"),
        "{error}"
    );
}

#[test]
fn pf6_dem_analyzer_fallback_preserves_expanded_instruction_cap() {
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

    let error = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect_err("reject aggregate expanded instructions beyond cap");

    assert!(
        error.to_string().contains("expanded instructions"),
        "{error}"
    );
}
