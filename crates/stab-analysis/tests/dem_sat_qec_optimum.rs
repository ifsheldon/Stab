#![allow(
    clippy::expect_used,
    reason = "generated-code tests use direct fixture assertions"
)]

//! The shortest-error WCNF must encode the logical-error problem, not just a well-formed file:
//! its exact optimum must equal the error count of the hypergraph search on the same DEM, and on
//! codes whose errors are all graphlike also the graphlike search. Optima come from an
//! independent WCNF solver that reads only the emitted text; the branch-and-bound solver is
//! first cross-checked against exhaustive enumeration on an instance small enough to enumerate.

use stab_analysis::{
    CodeCircuitParams, CodeDistance, CodeTask, ErrorAnalyzerOptions, ErrorDecomposition,
    LoopFolding, RepetitionCodeTask, RoundCount, SurfaceCodeTask, circuit_to_detector_error_model,
    find_undetectable_logical_error, likeliest_error_sat_problem, shortest_error_sat_problem,
    shortest_graphlike_undetectable_logical_error,
};
use stab_model::{Circuit, DetectorErrorModel, Probability};

#[path = "support/logical_search.rs"]
mod logical_search;

#[path = "support/sat_wcnf.rs"]
mod sat_wcnf;

fn probability(value: f64) -> Probability {
    Probability::try_new(value).expect("probability")
}

fn repetition_code(distance: u32, rounds: u64) -> Circuit {
    let params = CodeCircuitParams::try_new(
        CodeTask::Repetition(RepetitionCodeTask::Memory),
        CodeDistance::try_new(distance).expect("distance"),
        RoundCount::try_new(rounds).expect("rounds"),
    )
    .expect("repetition params")
    .with_before_round_data_depolarization(probability(0.01))
    .with_before_measure_flip_probability(probability(0.02));
    params
        .generate()
        .expect("repetition circuit")
        .circuit()
        .clone()
}

fn rotated_surface_code(rounds: u64) -> Circuit {
    let noise = probability(0.001);
    let params = CodeCircuitParams::try_new(
        CodeTask::Surface(SurfaceCodeTask::RotatedMemoryX),
        CodeDistance::try_new(3).expect("distance"),
        RoundCount::try_new(rounds).expect("rounds"),
    )
    .expect("surface params")
    .with_after_clifford_depolarization(noise)
    .with_before_measure_flip_probability(noise)
    .with_after_reset_flip_probability(noise)
    .with_before_round_data_depolarization(noise);
    params
        .generate()
        .expect("surface circuit")
        .circuit()
        .clone()
}

fn dem(circuit: &Circuit, decompose_errors: bool) -> DetectorErrorModel {
    circuit_to_detector_error_model(
        circuit,
        &ErrorAnalyzerOptions {
            loops: LoopFolding::Fold,
            decomposition: decompose_errors.then_some(ErrorDecomposition::default()),
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze generated circuit")
}

fn error_count(model: &DetectorErrorModel) -> usize {
    usize::try_from(model.count_errors().expect("error count")).expect("small error count")
}

fn shortest_sat_optimum(model: &DetectorErrorModel) -> usize {
    sat_wcnf::optimum_by_branch_and_bound(
        &shortest_error_sat_problem(model, &Default::default()).expect("shortest-error WCNF"),
    )
    .expect("a logical error exists")
}

fn hypergraph_error_count(model: &DetectorErrorModel) -> usize {
    error_count(
        &find_undetectable_logical_error(model, &logical_search::unbounded_hypergraph())
            .expect("hypergraph search")
            .into_found()
            .expect("the search finds a logical error"),
    )
}

fn graphlike_error_count(model: &DetectorErrorModel) -> usize {
    error_count(
        &shortest_graphlike_undetectable_logical_error(
            model,
            &logical_search::rejecting_ungraphlike(),
        )
        .expect("graphlike search")
        .into_found()
        .expect("the search finds a logical error"),
    )
}

#[test]
fn branch_and_bound_wcnf_oracle_matches_exhaustive_enumeration() {
    let model = dem(&repetition_code(3, 1), false);
    for wcnf in [
        shortest_error_sat_problem(&model, &Default::default()).expect("shortest-error WCNF"),
        likeliest_error_sat_problem(&model, 100, &Default::default())
            .expect("likeliest-error WCNF"),
    ] {
        let exhaustive = sat_wcnf::optimum(&wcnf);
        // A positive optimum means the soft clauses really penalize the activated errors.
        assert!(exhaustive.is_some_and(|cost| cost > 0), "{wcnf}");
        assert_eq!(sat_wcnf::optimum_by_branch_and_bound(&wcnf), exhaustive);
    }
}

#[test]
fn repetition_code_shortest_error_wcnf_optimum_is_the_code_distance() {
    for (distance, rounds) in [(3_u32, 1_u64), (3, 10), (5, 6)] {
        let model = dem(&repetition_code(distance, rounds), false);
        let expected = usize::try_from(distance).expect("small distance");
        assert_eq!(
            shortest_sat_optimum(&model),
            expected,
            "d={distance} r={rounds}"
        );
        assert_eq!(
            hypergraph_error_count(&model),
            expected,
            "d={distance} r={rounds}"
        );
        assert_eq!(
            graphlike_error_count(&dem(&repetition_code(distance, rounds), true)),
            expected,
            "d={distance} r={rounds}"
        );
    }
}

#[test]
fn surface_code_shortest_error_wcnf_optimum_matches_hypergraph_search() {
    let circuit = rotated_surface_code(2);
    let model = dem(&circuit, false);
    let optimum = shortest_sat_optimum(&model);
    assert_eq!(optimum, hypergraph_error_count(&model));
    // Decomposition can only split hyperedges into more graphlike errors.
    assert!(graphlike_error_count(&dem(&circuit, true)) >= optimum);
    assert_eq!(optimum, 3, "distance-3 rotated surface code");
}
