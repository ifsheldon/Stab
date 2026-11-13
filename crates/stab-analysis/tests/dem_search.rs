#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "integration tests use direct assertions for compact diagnostics"
)]

use std::collections::BTreeSet;

use stab_analysis::{
    AnalysisResult, CodeDistance, ErrorAnalyzerOptions, RepetitionCodeParams, RepetitionCodeTask,
    RoundCount, SurfaceCodeParams, SurfaceCodeTask, circuit_to_detector_error_model,
    find_undetectable_logical_error, generate_repetition_code_circuit,
    generate_surface_code_circuit, likeliest_error_sat_problem, shortest_error_sat_problem,
    shortest_graphlike_undetectable_logical_error,
};
use stab_model::{
    Circuit, DemInstruction, DemInstructionKind, DemItem, DemTarget, DetectorErrorModel,
    Probability,
};

#[path = "support/sat_wcnf.rs"]
mod sat_wcnf;

#[test]
fn pf4_dem_search_and_sat_repeat_resource_policy_is_source_owned() {
    let allowed = DetectorErrorModel::from_dem_str(
        "error(0.1) D0\nrepeat 2 {\n    error(0.1) D0 D1\n    shift_detectors 1\n}\nerror(0.1) D0 L0\n",
    )
    .unwrap();
    let expected = "error(1) D0\nerror(1) D0 D1\nerror(1) D1 D2\nerror(1) D2 L0\n";

    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&allowed, false)
            .unwrap()
            .to_dem_string(),
        expected
    );
    assert_eq!(
        find_undetectable_logical_error(&allowed, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        expected
    );
    let sat_problem = shortest_error_sat_problem(&allowed).unwrap();
    assert_eq!(
        sat_problem
            .lines()
            .filter(|line| line.starts_with("1 -"))
            .count(),
        4,
        "SAT problem should include one soft clause per expanded shifted-repeat error"
    );

    let hostile = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1) D0\n    shift_detectors 1\n}\nerror(0.1) D0 L0\n",
    )
    .unwrap();

    let graphlike_error = shortest_graphlike_undetectable_logical_error(&hostile, true)
        .expect_err("graphlike search should reject hostile repeat expansion")
        .to_string();
    assert!(
        graphlike_error
            .contains("DEM graphlike search currently supports repeat counts up to 100000"),
        "{graphlike_error}"
    );

    let hyper_error = find_undetectable_logical_error(&hostile, usize::MAX, usize::MAX, false)
        .expect_err("hypergraph search should reject hostile repeat expansion")
        .to_string();
    assert!(
        hyper_error.contains("DEM hypergraph search currently supports repeat counts up to 100000"),
        "{hyper_error}"
    );
}

/// Repeat bodies that leave the detector frame unchanged (zero-probability, annotation-only,
/// no-target, zero-shift, nested, and mixed bodies) must fold to the compact single-iteration
/// model for graphlike search, hypergraph search, and both SAT encodings.
#[test]
fn dem_search_and_sat_fold_zero_shift_repeat_bodies_like_compact_models() {
    const ACTIVE: &str = "error(0.1) D0\nerror(0.1) D0 L0\n";
    // The last column marks bodies whose SAT encoding is also compact: SAT problem generation
    // drops annotation-only bodies but, like Stim, materializes every repeated error mechanism.
    let cases: &[(&str, &str, &str, bool)] = &[
        (
            "zero-probability body",
            "repeat 100001 {\n    error(0) D1000000 L1000\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
            ACTIVE,
            false,
        ),
        (
            "nested annotation-only body",
            "repeat 100001 {\n    detector(2, 3) D0\n    logical_observable L1\n    shift_detectors(5, 7) 0\n    repeat 100001 {\n        detector(11) D1\n        logical_observable L2\n        shift_detectors 0\n    }\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
            ACTIVE,
            true,
        ),
        (
            "high-id nested annotation-only body",
            "repeat 100001 {\n    detector(2, 3) D1000001\n    logical_observable L1000001\n    shift_detectors(5, 7) 0\n    repeat 100001 {\n        detector(11) D1000002\n        logical_observable L1000002\n        shift_detectors 0\n    }\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
            ACTIVE,
            true,
        ),
        (
            "flat detector-touching body",
            "repeat 100001 {\n    error(0.1) D0 L0\n    error(0.2) D0\n}\n",
            "error(0.1) D0 L0\nerror(0.2) D0\n",
            false,
        ),
        (
            "flat logical-only body",
            "repeat 100001 {\n    error(0.1) L0\n}\n",
            "error(0.1) L0\n",
            false,
        ),
        (
            "flat logical-only pair body",
            "repeat 100001 {\n    error(0.1) L0\n    error(0.2) L1\n}\n",
            "error(0.1) L0\nerror(0.2) L1\n",
            false,
        ),
        (
            "no-target body",
            "repeat 100001 {\n    error(0.1)\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
            ACTIVE,
            false,
        ),
        (
            "no-target and logical body",
            "repeat 100001 {\n    error(0.1)\n    error(0.2) L0\n}\n",
            "error(0.1)\nerror(0.2) L0\n",
            false,
        ),
        (
            "zero-shift no-target body",
            "repeat 100001 {\n    shift_detectors 0\n    error(0.1)\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
            ACTIVE,
            false,
        ),
        (
            "zero-shift no-target and logical body",
            "repeat 100001 {\n    error(0.1)\n    shift_detectors 0\n    error(0.2) L0\n}\n",
            "error(0.1)\nshift_detectors 0\nerror(0.2) L0\n",
            false,
        ),
        (
            "zero-shift detector-touching body",
            "repeat 100001 {\n    error(0.1) D0\n    shift_detectors(4, 5) 0\n    error(0.2) D0 L0\n}\n",
            "error(0.1) D0\nshift_detectors(4, 5) 0\nerror(0.2) D0 L0\n",
            false,
        ),
        (
            "annotated detector-touching body",
            "repeat 100001 {\n    detector(1, 2) D0\n    logical_observable L2\n    error(0.1) D0\n    error(0.2) D0 L0\n}\n",
            "detector(1, 2) D0\nlogical_observable L2\nerror(0.1) D0\nerror(0.2) D0 L0\n",
            false,
        ),
        (
            "annotated logical-only body",
            "repeat 100001 {\n    detector(3, 4) D0\n    logical_observable L3\n    error(0.1) L0\n}\n",
            "detector(3, 4) D0\nlogical_observable L3\nerror(0.1) L0\n",
            false,
        ),
        (
            "nested zero-shift detector-touching body",
            "repeat 100001 {\n    detector(1, 2) D0\n    repeat 100001 {\n        error(0.1) D0\n        shift_detectors 0\n        error(0.2) D0 L0\n    }\n}\n",
            "detector(1, 2) D0\nerror(0.1) D0\nshift_detectors 0\nerror(0.2) D0 L0\n",
            false,
        ),
        (
            "nested zero-shift logical-only body",
            "repeat 100001 {\n    logical_observable L2\n    repeat 100001 {\n        error(0.1) L0\n        shift_detectors 0\n        error(0.2) L1\n    }\n}\n",
            "logical_observable L2\nerror(0.1) L0\nshift_detectors 0\nerror(0.2) L1\n",
            false,
        ),
        (
            "nested zero-shift no-target body",
            "repeat 100001 {\n    repeat 100001 {\n        error(0.1)\n        shift_detectors 0\n    }\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n",
            ACTIVE,
            false,
        ),
        (
            "mixed zero-probability detector-touching body",
            "repeat 100001 {\n    error(0) D1000000 L1000\n    error(0.1) D0\n    shift_detectors 0\n    error(0.1) D0 L0\n}\n",
            ACTIVE,
            false,
        ),
        (
            "mixed zero-probability logical-only body",
            "repeat 100001 {\n    detector D0\n    error(0) D1000000\n    error(0.1) L0\n}\n",
            "detector D0\nerror(0.1) L0\n",
            false,
        ),
        (
            "nested mixed zero-probability body",
            "repeat 100001 {\n    detector D0\n    repeat 100001 {\n        error(0) D1000000 L1000\n        error(0.1) D0\n        shift_detectors 0\n        error(0.2) D0 L0\n    }\n    shift_detectors 0\n}\n",
            "detector D0\nerror(0.1) D0\nerror(0.2) D0 L0\n",
            false,
        ),
    ];
    for &(name, repeated, compact, sat_folds) in cases {
        assert_search_and_sat_match_compact(name, &dem(repeated), &dem(compact), sat_folds);
    }

    // The folded zero-probability body must not perturb the pinned search output.
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&dem(ACTIVE), false)
            .unwrap()
            .to_dem_string(),
        "error(1) D0\nerror(1) D0 L0\n"
    );
    // Graphlike and hypergraph search break the logical-only tie differently, like Stim.
    let logical_pair = dem("error(0.1) L0\nerror(0.2) L1\n");
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&logical_pair, false)
            .unwrap()
            .to_dem_string(),
        "error(1) L0\n"
    );
    assert_eq!(
        find_undetectable_logical_error(&logical_pair, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        "error(1) L1\n"
    );
}

#[test]
fn dem_search_rejects_nonzero_shift_repeat_bodies_beyond_expansion_limit() {
    for shifted in [
        "repeat 100001 {\n    detector D0\n    logical_observable L1\n    error(0.1)\n    shift_detectors 1\n    error(0.2) D0 L0\n}\n",
        "repeat 100001 {\n    repeat 100001 {\n        error(0.1) D0\n        shift_detectors 1\n        error(0.2) D0 L0\n    }\n}\n",
        "repeat 100001 {\n    error(0) D1000000\n    shift_detectors 1\n    error(0.1) D0 L0\n}\n",
    ] {
        let shifted = dem(shifted);
        let graphlike_error = shortest_graphlike_undetectable_logical_error(&shifted, false)
            .expect_err("nonzero detector shifts should remain outside the graphlike fold")
            .to_string();
        assert!(
            graphlike_error.contains("supports repeat counts"),
            "{graphlike_error}"
        );
        let hyper_error = find_undetectable_logical_error(&shifted, usize::MAX, usize::MAX, false)
            .expect_err("nonzero detector shifts should remain outside the hypergraph fold")
            .to_string();
        assert!(
            hyper_error.contains("supports repeat counts"),
            "{hyper_error}"
        );
    }

    let error = DemInstruction::error(
        Probability::try_new(0.1).unwrap(),
        vec![DemTarget::numeric(5)],
        None,
    )
    .expect_err("numeric error targets should stay rejected at the typed boundary")
    .to_string();
    assert!(error.contains("raw numbers"), "{error}");
}

fn dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).unwrap()
}

fn assert_search_and_sat_match_compact(
    name: &str,
    model: &DetectorErrorModel,
    compact: &DetectorErrorModel,
    sat_folds: bool,
) {
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(model, false)
            .unwrap()
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(compact, false)
            .unwrap()
            .to_dem_string(),
        "{name}: graphlike search"
    );
    assert_eq!(
        find_undetectable_logical_error(model, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        find_undetectable_logical_error(compact, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        "{name}: hypergraph search"
    );
    if !sat_folds {
        return;
    }
    assert_eq!(
        sat_wcnf::optimum(&shortest_error_sat_problem(model).unwrap()),
        sat_wcnf::optimum(&shortest_error_sat_problem(compact).unwrap()),
        "{name}: shortest-error WCNF optimum"
    );
    assert_eq!(
        sat_wcnf::optimum(&likeliest_error_sat_problem(model, 10).unwrap()),
        sat_wcnf::optimum(&likeliest_error_sat_problem(compact, 10).unwrap()),
        "{name}: likeliest-error WCNF optimum"
    );
}

#[test]
fn dem_search_zero_probability_diagnostics_match_stim() {
    let model = dem("error(0) L0\n");
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&model, false)
            .expect_err("zero-probability graphlike model has no logical error")
            .to_string(),
        "invalid detector error model: Failed to find any graphlike logical errors.\n    WARNING: NO DETECTORS. The circuit or detector error model didn't define any detectors.\n    WARNING: NO GRAPHLIKE ERRORS. Although the circuit or detector error model does define some errors, none of them are graphlike (i.e. have at most two detection events), making it vacuously impossible to find a graphlike logical error."
    );
    assert_eq!(
        find_undetectable_logical_error(&model, usize::MAX, usize::MAX, false)
            .expect_err("zero-probability hypergraph model has no logical error")
            .to_string(),
        "invalid detector error model: Failed to find any logical errors.\n    WARNING: NO DETECTORS. The circuit or detector error model didn't define any detectors."
    );
}

#[test]
fn wcnf_shortest_large_detector_only_is_unsat() {
    let model = dem("repeat 100001 {\n    error(0.1) D0\n    shift_detectors 1\n}\n");
    assert_eq!(
        shortest_error_sat_problem(&model).expect("trivial detector-only WCNF"),
        "p wcnf 1 2 3\n3 -1 0\n3 1 0\n"
    );
}

#[test]
fn pf6_direct_dem_graphlike_search_matches_upstream_distance_cases() {
    for dem_text in [
        "",
        "error(0.1) D0 L0\n",
        "error(0.1) D0\nerror(0.1) D0 D1\nerror(0.1) D1\n",
    ] {
        let error = shortest_graphlike_undetectable_logical_error(&dem(dem_text), false)
            .expect_err("model has no undetectable graphlike logical error")
            .to_string();
        assert!(
            error.starts_with(
                "invalid detector error model: Failed to find any graphlike logical errors."
            ),
            "{dem_text:?}: {error}"
        );
    }

    let cases = [
        ("error(0.1) L0\n", "error(1) L0\n"),
        (
            "error(0.1) D0\nerror(0.1) D0 L0\n",
            "error(1) D0\nerror(1) D0 L0\n",
        ),
        (
            "error(0.1) D0 L0\nerror(0.1) D0 L1\n",
            "error(1) D0 L0\nerror(1) D0 L1\n",
        ),
        (
            "error(0.1) D0 D1 L0\nerror(0.1) D0 D1 L1\n",
            "error(1) D0 D1 L0\nerror(1) D0 D1 L1\n",
        ),
        (
            "error(0.1) D0 D1 L1\nerror(0.1) D0 D1 L0\n",
            "error(1) D0 D1 L0\nerror(1) D0 D1 L1\n",
        ),
        (
            "error(0.1) D0\nerror(0.1) D0 D1 L0\nerror(0.1) D1\n",
            "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n",
        ),
        (
            "error(0.1) D1\nerror(0.1) D1 D0 L0\nerror(0.1) D0\n",
            "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n",
        ),
    ];
    for (dem_text, expected) in cases {
        let model = DetectorErrorModel::from_dem_str(dem_text).unwrap();
        assert_eq!(
            shortest_graphlike_undetectable_logical_error(&model, false)
                .unwrap()
                .to_dem_string(),
            expected,
            "direct graphlike search mismatch for {dem_text:?}"
        );
    }
}

#[test]
fn pf6_direct_dem_hypergraph_search_matches_upstream_distance_cases() {
    for dem_text in [
        "",
        "error(0.1) D0 L0\n",
        "error(0.1) D0\nerror(0.1) D0 D1\nerror(0.1) D1\n",
    ] {
        let error = find_undetectable_logical_error(&dem(dem_text), usize::MAX, usize::MAX, false)
            .expect_err("model has no undetectable logical error")
            .to_string();
        assert!(
            error.starts_with("invalid detector error model: Failed to find any logical errors."),
            "{dem_text:?}: {error}"
        );
    }

    let cases = [
        (
            "error(0.1) L0\n",
            "error(1) L0\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D0\nerror(0.1) D0 L0\n",
            "error(1) D0\nerror(1) D0 L0\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D0 L0\nerror(0.1) D0 L1\n",
            "error(1) D0 L0\nerror(1) D0 L1\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D0 D1 L0\nerror(0.1) D0 D1 L1\n",
            "error(1) D0 D1 L0\nerror(1) D0 D1 L1\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D0 D1 L1\nerror(0.1) D0 D1 L0\n",
            "error(1) D0 D1 L0\nerror(1) D0 D1 L1\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D0\nerror(0.1) D0 D1 L0\nerror(0.1) D1\n",
            "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D1\nerror(0.1) D1 D0 L0\nerror(0.1) D0\n",
            "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n",
            usize::MAX,
            usize::MAX,
            false,
        ),
        (
            "error(0.1) D0 D1\nerror(0.1) D0 D1 D2 D3\nerror(0.1) D2 D3 D4 D5 L0\nerror(0.1) D4 D5 D6 D7\nerror(0.1) D6 D7 D8 D9\nerror(0.1) D8\nerror(0.1) D9\n",
            "error(1) D0 D1\nerror(1) D0 D1 D2 D3\nerror(1) D2 D3 D4 D5 L0\nerror(1) D4 D5 D6 D7\nerror(1) D6 D7 D8 D9\nerror(1) D8\nerror(1) D9\n",
            4,
            4,
            true,
        ),
    ];
    for (dem_text, expected, max_event_count, max_edge_degree, allow_increasing_error_degree) in
        cases
    {
        let model = DetectorErrorModel::from_dem_str(dem_text).unwrap();
        assert_eq!(
            find_undetectable_logical_error(
                &model,
                max_event_count,
                max_edge_degree,
                allow_increasing_error_degree
            )
            .unwrap()
            .to_dem_string(),
            expected,
            "direct hypergraph search mismatch for {dem_text:?}"
        );
    }
}

#[test]
fn pf6_generated_qec_graphlike_search_has_structural_signature() {
    let surface = generated_rotated_surface_code_dem().unwrap();
    for ignore_ungraphlike_errors in [false, true] {
        assert_graphlike_search_signature(
            &surface,
            &shortest_graphlike_undetectable_logical_error(&surface, ignore_ungraphlike_errors)
                .unwrap(),
            5,
        );
    }

    let repetition = generated_repetition_code_dem().unwrap();
    assert_graphlike_search_signature(
        &repetition,
        &shortest_graphlike_undetectable_logical_error(&repetition, false).unwrap(),
        7,
    );

    let ungraphlike_surface = generated_rotated_surface_code_ungraphlike_dem().unwrap();
    assert_graphlike_search_signature(
        &ungraphlike_surface,
        &shortest_graphlike_undetectable_logical_error(&ungraphlike_surface, true).unwrap(),
        5,
    );
    let error = shortest_graphlike_undetectable_logical_error(&ungraphlike_surface, false)
        .expect_err("ungraphlike generated DEM should be rejected without ignore flag")
        .to_string();
    assert!(error.contains("non-graphlike error mechanism"), "{error}");
}

#[test]
fn pf6_generated_qec_hypergraph_search_has_structural_signature() {
    let surface = generated_rotated_surface_code_dem().unwrap();
    assert_hypergraph_search_signature(
        &surface,
        &find_undetectable_logical_error(&surface, 4, 4, true).unwrap(),
        5,
    );

    let repetition = generated_repetition_code_dem().unwrap();
    assert_hypergraph_search_signature(
        &repetition,
        &find_undetectable_logical_error(&repetition, 4, 4, false).unwrap(),
        7,
    );

    let ungraphlike_surface = generated_rotated_surface_code_ungraphlike_dem().unwrap();
    assert_hypergraph_search_signature(
        &ungraphlike_surface,
        &find_undetectable_logical_error(&ungraphlike_surface, 4, 4, true).unwrap(),
        5,
    );
}

#[test]
fn pf6_graphlike_search_many_observables_preserves_high_observable_id() {
    let model = many_observables_dem().unwrap();
    assert_search_result_signature_for_observable(
        &model,
        &shortest_graphlike_undetectable_logical_error(&model, false).unwrap(),
        SourceMechanismShape::GraphlikeComponents,
        5,
        2,
        1200,
    );
}

#[test]
fn pf6_hypergraph_search_many_observables_preserves_high_observable_id() {
    let model = many_observables_dem().unwrap();
    assert_search_result_signature_for_observable(
        &model,
        &find_undetectable_logical_error(&model, 4, 4, false).unwrap(),
        SourceMechanismShape::Hypergraph,
        5,
        4,
        1200,
    );
}

// The generated QEC models are loop-folded so search must expand shifted repeat blocks.
fn generated_rotated_surface_code_dem() -> AnalysisResult<DetectorErrorModel> {
    generated_rotated_surface_code_dem_with_options(ErrorAnalyzerOptions {
        fold_loops: true,
        decompose_errors: true,
        ..ErrorAnalyzerOptions::default()
    })
}

fn generated_rotated_surface_code_ungraphlike_dem() -> AnalysisResult<DetectorErrorModel> {
    generated_rotated_surface_code_dem_with_options(ErrorAnalyzerOptions {
        fold_loops: true,
        ..ErrorAnalyzerOptions::default()
    })
}

fn generated_rotated_surface_code_dem_with_options(
    options: ErrorAnalyzerOptions,
) -> AnalysisResult<DetectorErrorModel> {
    let probability = Probability::try_new(0.001)?;
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(5)?,
        CodeDistance::try_new(5)?,
        SurfaceCodeTask::RotatedMemoryX,
    )?
    .with_after_clifford_depolarization(probability)
    .with_before_measure_flip_probability(probability)
    .with_after_reset_flip_probability(probability)
    .with_before_round_data_depolarization(probability);
    let generated = generate_surface_code_circuit(&params)?;
    circuit_to_detector_error_model(generated.circuit(), options)
}

fn generated_repetition_code_dem() -> AnalysisResult<DetectorErrorModel> {
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(10)?,
        CodeDistance::try_new(7)?,
        RepetitionCodeTask::Memory,
    )?
    .with_before_round_data_depolarization(Probability::try_new(0.01)?);
    let generated = generate_repetition_code_circuit(&params)?;
    circuit_to_detector_error_model(
        generated.circuit(),
        ErrorAnalyzerOptions {
            fold_loops: true,
            decompose_errors: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
}

fn many_observables_dem() -> AnalysisResult<DetectorErrorModel> {
    let circuit = Circuit::from_stim_str(concat!(
        "MPP Z0*Z1 Z1*Z2 Z2*Z3 Z3*Z4\n",
        "X_ERROR(0.1) 0 1 2 3 4\n",
        "MPP Z0*Z1 Z1*Z2 Z2*Z3 Z3*Z4\n",
        "DETECTOR rec[-1] rec[-5]\n",
        "DETECTOR rec[-2] rec[-6]\n",
        "DETECTOR rec[-3] rec[-7]\n",
        "DETECTOR rec[-4] rec[-8]\n",
        "M 4\n",
        "OBSERVABLE_INCLUDE(1200) rec[-1]\n",
    ))?;
    circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            decompose_errors: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
}

fn assert_graphlike_search_signature(
    source: &DetectorErrorModel,
    result: &DetectorErrorModel,
    expected_error_count: usize,
) {
    assert_search_result_signature_for_observable(
        source,
        result,
        SourceMechanismShape::GraphlikeComponents,
        expected_error_count,
        2,
        0,
    );
}

fn assert_hypergraph_search_signature(
    source: &DetectorErrorModel,
    result: &DetectorErrorModel,
    expected_error_count: usize,
) {
    assert_search_result_signature_for_observable(
        source,
        result,
        SourceMechanismShape::Hypergraph,
        expected_error_count,
        4,
        0,
    );
}

/// Checks the pinned Stim error count and observable plus the structural invariants of a search
/// result: every reported mechanism exists in the source DEM (as a whole error for hypergraph
/// search, or as one decomposition component for graphlike search), no target set repeats, and
/// the combined detector parity cancels.
fn assert_search_result_signature_for_observable(
    source: &DetectorErrorModel,
    model: &DetectorErrorModel,
    source_shape: SourceMechanismShape,
    expected_error_count: usize,
    max_detectors_per_error: usize,
    expected_observable: u64,
) {
    let signature = SearchResultSignature::from_model(model, max_detectors_per_error);
    let source_mechanisms = source_mechanisms(source, source_shape);
    for targets in &signature.target_sets {
        assert!(
            source_mechanisms.contains(targets),
            "search result mechanism {targets:?} is not present in the source DEM: {}",
            model.to_dem_string()
        );
    }
    assert_eq!(
        signature.error_count,
        expected_error_count,
        "search output should have the pinned Stim v1.16.0 error count: {}",
        model.to_dem_string()
    );
    assert!(
        signature.detector_parity.is_empty(),
        "search output should be undetected after target-set parity: {:?}\n{}",
        signature.detector_parity,
        model.to_dem_string()
    );
    assert_eq!(
        signature.observable_parity,
        BTreeSet::from([expected_observable]),
        "search output should toggle exactly logical observable L{expected_observable}: {}",
        model.to_dem_string()
    );
    assert_eq!(
        signature.target_sets.len(),
        expected_error_count,
        "search output should not repeat canonical target sets: {}",
        model.to_dem_string()
    );
}

#[derive(Clone, Copy)]
enum SourceMechanismShape {
    GraphlikeComponents,
    Hypergraph,
}

fn source_mechanisms(
    source: &DetectorErrorModel,
    shape: SourceMechanismShape,
) -> BTreeSet<CanonicalSearchTargets> {
    let mut mechanisms = BTreeSet::new();
    for instruction in source.iter_flattened_instructions() {
        let instruction = instruction.expect("generated source DEM flattens");
        if instruction.kind() != DemInstructionKind::Error || instruction.args() == [0.0] {
            continue;
        }
        match shape {
            SourceMechanismShape::GraphlikeComponents => {
                for component in instruction
                    .targets()
                    .split(|target| matches!(target, DemTarget::Separator))
                {
                    mechanisms.insert(CanonicalSearchTargets::from_source_targets(component));
                }
            }
            SourceMechanismShape::Hypergraph => {
                mechanisms.insert(CanonicalSearchTargets::from_source_targets(
                    instruction.targets(),
                ));
            }
        }
    }
    mechanisms
}

#[derive(Debug, Eq, PartialEq)]
struct SearchResultSignature {
    error_count: usize,
    detector_parity: BTreeSet<u64>,
    observable_parity: BTreeSet<u64>,
    target_sets: BTreeSet<CanonicalSearchTargets>,
}

impl SearchResultSignature {
    fn from_model(model: &DetectorErrorModel, max_detectors_per_error: usize) -> Self {
        let mut signature = Self {
            error_count: 0,
            detector_parity: BTreeSet::new(),
            observable_parity: BTreeSet::new(),
            target_sets: BTreeSet::new(),
        };

        for item in model.items() {
            assert!(
                matches!(item, DemItem::Instruction(_)),
                "search output should not contain repeat blocks: {}",
                model.to_dem_string()
            );
            let DemItem::Instruction(instruction) = item else {
                continue;
            };
            assert_eq!(
                instruction.kind(),
                DemInstructionKind::Error,
                "search output should contain only error instructions: {}",
                model.to_dem_string()
            );
            assert_eq!(
                instruction.args(),
                &[1.0],
                "search output should use deterministic error instructions: {}",
                model.to_dem_string()
            );

            let targets = CanonicalSearchTargets::from_targets(instruction.targets());
            assert!(
                targets.detectors.len() <= max_detectors_per_error,
                "search output exceeded the per-error detector weight: {}",
                model.to_dem_string()
            );
            assert!(
                signature.target_sets.insert(targets.clone()),
                "search output repeated a canonical target set: {}",
                model.to_dem_string()
            );
            for detector in &targets.detectors {
                toggle_parity(&mut signature.detector_parity, *detector);
            }
            for observable in &targets.observables {
                toggle_parity(&mut signature.observable_parity, *observable);
            }
            signature.error_count += 1;
        }
        signature
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CanonicalSearchTargets {
    detectors: Vec<u64>,
    observables: Vec<u64>,
}

impl CanonicalSearchTargets {
    fn from_targets(targets: &[DemTarget]) -> Self {
        let mut detectors = Vec::new();
        let mut observables = Vec::new();
        for target in targets {
            assert!(
                !matches!(target, DemTarget::Separator | DemTarget::Numeric(_)),
                "search output should use only detector and observable targets"
            );
            match target {
                DemTarget::RelativeDetector(detector) => detectors.push(detector.get()),
                DemTarget::LogicalObservable(observable) => observables.push(observable.get()),
                DemTarget::Separator | DemTarget::Numeric(_) => {}
            }
        }
        detectors.sort_unstable();
        observables.sort_unstable();
        assert_strictly_increasing(&detectors, "detector");
        assert_strictly_increasing(&observables, "observable");
        Self {
            detectors,
            observables,
        }
    }

    /// Canonicalizes a source error mechanism, XOR-cancelling repeated targets as the DEM does.
    fn from_source_targets(targets: &[DemTarget]) -> Self {
        let mut detectors = BTreeSet::new();
        let mut observables = BTreeSet::new();
        for target in targets {
            match *target {
                DemTarget::RelativeDetector(detector) => {
                    toggle_parity(&mut detectors, detector.get());
                }
                DemTarget::LogicalObservable(observable) => {
                    toggle_parity(&mut observables, observable.get());
                }
                DemTarget::Separator => {}
                DemTarget::Numeric(_) => panic!("error mechanism contains a numeric target"),
            }
        }
        Self {
            detectors: detectors.into_iter().collect(),
            observables: observables.into_iter().collect(),
        }
    }
}

fn toggle_parity(set: &mut BTreeSet<u64>, value: u64) {
    if !set.insert(value) {
        set.remove(&value);
    }
}

fn assert_strictly_increasing(values: &[u64], label: &str) {
    assert!(
        values
            .windows(2)
            .all(|window| matches!(window, [left, right] if left < right)),
        "search output should not repeat {label} targets inside one error row: {values:?}"
    );
}
