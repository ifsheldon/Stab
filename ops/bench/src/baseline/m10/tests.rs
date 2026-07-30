#![allow(
    clippy::expect_used,
    reason = "focused benchmark witness tests use direct assertions"
)]

use super::{
    ANALYZE_BASIC_EXPECTED, ANALYZE_BASIC_FIXTURE, ANALYZE_FOLD_REPEAT_EXPECTED,
    ANALYZE_FOLD_REPEAT_FIXTURE, ERROR_DECOMP_EXACT_EXPECTED, ERROR_DECOMP_INDEPENDENT_EXPECTED,
    ERROR_DECOMP_NO_SOLUTION_EXPECTED, GRAPHLIKE_SEARCH_EXPECTED, PF7_ANALYZE_DECOMPOSE_EXPECTED,
    PF7_ANALYZE_GENERATED_EXPECTED, analyzer_semantic_witness, benchmark_probability,
    ensure_analyzer_semantic_witness, ensure_exact_text_witness, ensure_probability_triple,
    error_analyzer_surface_code, exact_text_witness, graphlike_search_model,
    preflight_analyze_exact, preflight_analyze_semantic, preflight_error_decomp,
    preflight_graphlike_search,
};
use stab_analysis::ErrorAnalyzerOptions;
use stab_core::Circuit;

#[test]
fn analyzer_witness_rejects_same_width_wrong_content() {
    let actual = analyzer_semantic_witness("pf7-cli-analyze-errors-decompose", b"detector D1\n")
        .expect("derive mutated analyzer witness");
    assert_eq!(actual.bytes, PF7_ANALYZE_DECOMPOSE_EXPECTED.bytes);

    ensure_analyzer_semantic_witness(
        "pf7-cli-analyze-errors-decompose",
        PF7_ANALYZE_DECOMPOSE_EXPECTED,
        &actual,
    )
    .expect_err("same-width output with a different detector must be rejected");
}

#[test]
fn analyzer_witness_normalizes_only_insignificant_probability_printing() {
    let pinned = analyzer_semantic_witness(
        "pinned",
        b"error(0.002529202133333148701244130762688656) D0 D8\n",
    )
    .expect("derive pinned witness");
    let stab = analyzer_semantic_witness(
        "stab",
        b"error(0.002529202133333149134924999756890429) D0 D8\n",
    )
    .expect("derive Stab witness");
    let changed = analyzer_semantic_witness(
        "changed",
        b"error(0.102529202133333149134924999756890429) D0 D8\n",
    )
    .expect("derive materially changed witness");

    assert_eq!(pinned.digest, stab.digest);
    assert_ne!(pinned.digest, changed.digest);
}

#[test]
fn analyzer_model_preflights_match_pinned_stim_witnesses() {
    let basic = Circuit::from_stim_str(ANALYZE_BASIC_FIXTURE).expect("parse basic fixture");
    preflight_analyze_exact(
        "m10-analyze-errors-decompose-cli",
        &basic,
        ErrorAnalyzerOptions {
            decompose_errors: true,
            ..ErrorAnalyzerOptions::default()
        },
        ANALYZE_BASIC_EXPECTED,
    )
    .expect("basic analyzer output matches pinned Stim");

    let folded = Circuit::from_stim_str(ANALYZE_FOLD_REPEAT_FIXTURE).expect("parse folded fixture");
    preflight_analyze_exact(
        "m10-analyze-errors-fold-cli",
        &folded,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
        ANALYZE_FOLD_REPEAT_EXPECTED,
    )
    .expect("folded analyzer output matches pinned Stim");

    let generated =
        error_analyzer_surface_code("m10-error-analyzer").expect("generate analyzer circuit");
    preflight_analyze_semantic(
        "m10-error-analyzer",
        &generated,
        ErrorAnalyzerOptions::default(),
        PF7_ANALYZE_GENERATED_EXPECTED,
    )
    .expect("generated analyzer output matches pinned Stim semantic digest");
}

#[test]
fn analyzer_exact_preflight_rejects_same_width_wrong_dem() {
    let mut mutated = ANALYZE_FOLD_REPEAT_EXPECTED.as_bytes().to_vec();
    let detector = mutated
        .iter()
        .position(|byte| *byte == b'D')
        .expect("fixture contains a detector target");
    *mutated
        .get_mut(detector)
        .expect("located detector target remains in bounds") = b'L';
    assert_eq!(mutated.len(), ANALYZE_FOLD_REPEAT_EXPECTED.len());

    let error = super::require_exact(
        "m10-analyze-errors-fold-cli",
        "analyze_errors canonical DEM",
        mutated.as_slice(),
        ANALYZE_FOLD_REPEAT_EXPECTED.as_bytes(),
    )
    .expect_err("same-width target mutation must fail");
    assert!(error.to_string().contains("wrong content"));
}

#[test]
fn graphlike_preflight_matches_complete_chain_and_rejects_same_size_mutation() {
    let model = graphlike_search_model("m10-graphlike-search").expect("build graphlike model");
    preflight_graphlike_search("m10-graphlike-search", &model)
        .expect("chain result matches fixed complete witness");

    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"error(1) D0\n");
    for detector in 0..super::GRAPHLIKE_SEARCH_DETECTORS - 1 {
        canonical.extend_from_slice(format!("error(1) D{detector} D{}\n", detector + 1).as_bytes());
    }
    canonical.extend_from_slice(b"error(1) D127 L0\n");
    let witness = exact_text_witness(&canonical);
    ensure_exact_text_witness(
        "m10-graphlike-search",
        "graphlike logical-error DEM",
        GRAPHLIKE_SEARCH_EXPECTED,
        &witness,
    )
    .expect("independently constructed chain matches fixed digest");

    let old = b"D126 D127";
    let replacement = b"D125 D127";
    let start = canonical
        .windows(old.len())
        .position(|window| window == old)
        .expect("chain contains final interior edge");
    canonical
        .get_mut(start..start + replacement.len())
        .expect("located edge span remains in bounds")
        .copy_from_slice(replacement);
    let mutated = exact_text_witness(&canonical);
    assert_eq!(mutated.bytes, GRAPHLIKE_SEARCH_EXPECTED.bytes);
    assert_eq!(mutated.records, GRAPHLIKE_SEARCH_EXPECTED.records);
    ensure_exact_text_witness(
        "m10-graphlike-search",
        "graphlike logical-error DEM",
        GRAPHLIKE_SEARCH_EXPECTED,
        &mutated,
    )
    .expect_err("same-size edge mutation must fail");
}

#[test]
fn error_decomp_preflight_matches_all_timed_cases() {
    let probability =
        |value| benchmark_probability("m10-error-decomp", value).expect("valid probability");
    preflight_error_decomp(
        "m10-error-decomp",
        [probability(0.1), probability(0.2), probability(0.3)],
        [probability(0.1), probability(0.2), probability(0.15)],
        [probability(0.1), probability(0.2), probability(0.0)],
        [probability(0.01), probability(0.02), probability(0.0)],
    )
    .expect("all conversion cases match pinned numeric expectations");
}

#[test]
fn error_decomp_preflight_rejects_complete_wrong_numeric_results() {
    ensure_probability_triple(
        "m10-error-decomp",
        "independent-to-disjoint XYZ conversion",
        ERROR_DECOMP_INDEPENDENT_EXPECTED,
        Some([0.15, 0.11, 0.23]),
    )
    .expect_err("same-size permuted triple must fail");
    ensure_probability_triple(
        "m10-error-decomp",
        "exact disjoint-to-independent XYZ conversion",
        ERROR_DECOMP_EXACT_EXPECTED,
        Some([0.0, 0.0, 0.0]),
    )
    .expect_err("wrong complete triple must fail");
    ensure_probability_triple(
        "m10-error-decomp",
        "p10 disjoint-to-independent XYZ no-solution result",
        ERROR_DECOMP_NO_SOLUTION_EXPECTED,
        Some([0.1, 0.2, 0.0]),
    )
    .expect_err("unexpected approximate result must fail");
}
