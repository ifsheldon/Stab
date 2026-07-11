#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "integration parity tests use direct failure diagnostics for fixed upstream cases"
)]

use std::collections::BTreeSet;

use stab_core::{
    Circuit, CircuitError, CodeDistance, DemInstructionKind, DemItem, DemTarget,
    DetectorErrorModel, ErrorAnalyzerOptions, Probability, RepetitionCodeParams,
    RepetitionCodeTask, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    circuit_to_detector_error_model, find_undetectable_logical_error,
    generate_repetition_code_circuit, generate_surface_code_circuit, likeliest_error_sat_problem,
    shortest_error_sat_problem, shortest_graphlike_undetectable_logical_error,
};

const UNSAT_WDIMACS: &str = "p wcnf 1 2 3\n3 -1 0\n3 1 0\n";
const TWO_ERROR_UNWEIGHTED_WDIMACS: &str = "\
p wcnf 3 8 9
1 -1 0
9 1 2 -3 0
9 1 -2 3 0
9 -1 2 3 0
9 -1 -2 -3 0
1 -2 0
9 -3 0
9 1 0
";

fn dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).expect("valid test DEM")
}

fn assert_graphlike_exact(input: &str, expected: &str) {
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&dem(input), false)
            .expect("graphlike logical error")
            .to_dem_string(),
        expected
    );
}

fn assert_hypergraph_exact(input: &str, expected: &str) {
    assert_eq!(
        find_undetectable_logical_error(&dem(input), usize::MAX, usize::MAX, false)
            .expect("hypergraph logical error")
            .to_dem_string(),
        expected
    );
}

fn assert_no_logical_error(
    result: Result<DetectorErrorModel, CircuitError>,
    expected_prefix: &str,
) {
    let error = result.expect_err("model should not contain an undetectable logical error");
    let CircuitError::InvalidDetectorErrorModel { message } = error else {
        panic!("expected invalid detector error model, got {error:?}");
    };
    assert!(
        message.starts_with(expected_prefix),
        "unexpected logical-error rejection: {message}"
    );
}

fn assert_search_signature(
    source: &DetectorErrorModel,
    result: &DetectorErrorModel,
    source_shape: SourceMechanismShape,
    expected_error_count: usize,
    max_detectors_per_error: usize,
    expected_observable: u64,
) {
    let source_signatures = source_error_signatures(source, source_shape);
    let mut detector_parity = BTreeSet::new();
    let mut observable_parity = BTreeSet::new();
    let mut target_sets = BTreeSet::new();
    let mut error_count = 0;

    for item in result.items() {
        let DemItem::Instruction(instruction) = item else {
            panic!(
                "search output should not contain repeat blocks: {}",
                result.to_dem_string()
            );
        };
        assert_eq!(instruction.kind(), DemInstructionKind::Error);
        assert_eq!(instruction.args(), &[1.0]);

        let mut detectors = BTreeSet::new();
        let mut observables = BTreeSet::new();
        for target in instruction.targets() {
            match *target {
                DemTarget::RelativeDetector(detector) => {
                    assert!(detectors.insert(detector.get()));
                    toggle_parity(&mut detector_parity, detector.get());
                }
                DemTarget::LogicalObservable(observable) => {
                    assert!(observables.insert(observable.get()));
                    toggle_parity(&mut observable_parity, observable.get());
                }
                DemTarget::Separator | DemTarget::Numeric(_) => panic!(
                    "search output contains a non-canonical target: {}",
                    result.to_dem_string()
                ),
            }
        }
        assert!(detectors.len() <= max_detectors_per_error);
        assert!(
            source_signatures.contains(&(detectors.clone(), observables.clone())),
            "search result mechanism was not present in the source DEM: {:?}\nsource:\n{}\nresult:\n{}",
            (&detectors, &observables),
            source.to_dem_string(),
            result.to_dem_string()
        );
        assert!(target_sets.insert((detectors, observables)));
        error_count += 1;
    }

    assert_eq!(error_count, expected_error_count);
    assert!(detector_parity.is_empty());
    assert_eq!(observable_parity, BTreeSet::from([expected_observable]));
    assert_eq!(target_sets.len(), expected_error_count);
}

#[derive(Clone, Copy)]
enum SourceMechanismShape {
    GraphlikeComponents,
    Hypergraph,
}

fn source_error_signatures(
    source: &DetectorErrorModel,
    shape: SourceMechanismShape,
) -> BTreeSet<(BTreeSet<u64>, BTreeSet<u64>)> {
    let mut signatures = BTreeSet::new();
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
                    signatures.insert(error_signature(component));
                }
            }
            SourceMechanismShape::Hypergraph => {
                signatures.insert(error_signature(instruction.targets()));
            }
        }
    }
    signatures
}

fn error_signature(targets: &[DemTarget]) -> (BTreeSet<u64>, BTreeSet<u64>) {
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
    (detectors, observables)
}

fn toggle_parity(values: &mut BTreeSet<u64>, value: u64) {
    if !values.insert(value) {
        values.remove(&value);
    }
}

#[test]
fn pfm_b5_graphlike_no_error() {
    for input in [
        "",
        "error(0.1) D0 L0\n",
        "error(0.1) D0\nerror(0.1) D0 D1\nerror(0.1) D1\n",
    ] {
        assert_no_logical_error(
            shortest_graphlike_undetectable_logical_error(&dem(input), false),
            "Failed to find any graphlike logical errors.",
        );
    }
}

#[test]
fn pfm_b5_graphlike_distance_one() {
    assert_graphlike_exact("error(0.1) L0\n", "error(1) L0\n");
}

#[test]
fn pfm_b5_graphlike_distance_two() {
    for (input, expected) in [
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
    ] {
        assert_graphlike_exact(input, expected);
    }
}

#[test]
fn pfm_b5_graphlike_distance_three() {
    let expected = "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n";
    for input in [
        "error(0.1) D0\nerror(0.1) D0 D1 L0\nerror(0.1) D1\n",
        "error(0.1) D1\nerror(0.1) D1 D0 L0\nerror(0.1) D0\n",
    ] {
        assert_graphlike_exact(input, expected);
    }
}

#[test]
fn pfm_b5_graphlike_surface_code() {
    let (graphlike, ungraphlike) = generated_surface_models();
    let strict = shortest_graphlike_undetectable_logical_error(&graphlike, false)
        .expect("graphlike surface-code error");
    assert_search_signature(
        &graphlike,
        &strict,
        SourceMechanismShape::GraphlikeComponents,
        5,
        2,
        0,
    );
    let ignored = shortest_graphlike_undetectable_logical_error(&graphlike, true)
        .expect("graphlike surface-code error with ignored decomposition");
    assert_search_signature(
        &graphlike,
        &ignored,
        SourceMechanismShape::GraphlikeComponents,
        5,
        2,
        0,
    );
    let ungraphlike_ignored = shortest_graphlike_undetectable_logical_error(&ungraphlike, true)
        .expect("ungraphlike surface-code error with ignored decomposition");
    assert_search_signature(
        &ungraphlike,
        &ungraphlike_ignored,
        SourceMechanismShape::GraphlikeComponents,
        5,
        2,
        0,
    );
    let error = shortest_graphlike_undetectable_logical_error(&ungraphlike, false)
        .expect_err("undecomposed surface model should be rejected");
    assert!(matches!(
        error,
        CircuitError::InvalidDetectorErrorModel { .. }
    ));
}

#[test]
fn pfm_b5_graphlike_repetition_code() {
    let source = generated_repetition_model();
    let result = shortest_graphlike_undetectable_logical_error(&source, false)
        .expect("repetition-code logical error");
    assert_search_signature(
        &source,
        &result,
        SourceMechanismShape::GraphlikeComponents,
        7,
        2,
        0,
    );
}

#[test]
fn pfm_b5_graphlike_many_observables() {
    let source = many_observables_model();
    let result = shortest_graphlike_undetectable_logical_error(&source, false)
        .expect("many-observable graphlike error");
    assert_search_signature(
        &source,
        &result,
        SourceMechanismShape::GraphlikeComponents,
        5,
        2,
        1200,
    );
}

#[test]
fn pfm_b5_hypergraph_no_error() {
    for input in [
        "",
        "error(0.1) D0 L0\n",
        "error(0.1) D0\nerror(0.1) D0 D1\nerror(0.1) D1\n",
    ] {
        assert_no_logical_error(
            find_undetectable_logical_error(&dem(input), usize::MAX, usize::MAX, false),
            "Failed to find any logical errors.",
        );
    }
}

#[test]
fn pfm_b5_hypergraph_distance_one() {
    assert_hypergraph_exact("error(0.1) L0\n", "error(1) L0\n");
}

#[test]
fn pfm_b5_hypergraph_distance_two() {
    for (input, expected) in [
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
    ] {
        assert_hypergraph_exact(input, expected);
    }
}

#[test]
fn pfm_b5_hypergraph_distance_three() {
    let expected = "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n";
    for input in [
        "error(0.1) D0\nerror(0.1) D0 D1 L0\nerror(0.1) D1\n",
        "error(0.1) D1\nerror(0.1) D1 D0 L0\nerror(0.1) D0\n",
    ] {
        assert_hypergraph_exact(input, expected);
    }
}

#[test]
fn pfm_b5_hypergraph_hyper_error() {
    let input = "\
error(0.1) D0 D1
error(0.1) D0 D1 D2 D3
error(0.1) D2 D3 D4 D5 L0
error(0.1) D4 D5 D6 D7
error(0.1) D6 D7 D8 D9
error(0.1) D8
error(0.1) D9
";
    let expected = input.replace("error(0.1)", "error(1)");
    assert_eq!(
        find_undetectable_logical_error(&dem(input), 4, 4, true)
            .expect("hypergraph logical error")
            .to_dem_string(),
        expected
    );
}

#[test]
fn pfm_b5_hypergraph_surface_code() {
    let (graphlike, ungraphlike) = generated_surface_models();
    let graphlike_result = find_undetectable_logical_error(&graphlike, 4, 4, true)
        .expect("surface-code hypergraph error");
    assert_search_signature(
        &graphlike,
        &graphlike_result,
        SourceMechanismShape::Hypergraph,
        5,
        4,
        0,
    );
    let ungraphlike_result = find_undetectable_logical_error(&ungraphlike, 4, 4, true)
        .expect("ungraphlike surface-code hypergraph error");
    assert_search_signature(
        &ungraphlike,
        &ungraphlike_result,
        SourceMechanismShape::Hypergraph,
        5,
        4,
        0,
    );
}

#[test]
fn pfm_b5_hypergraph_repetition_code() {
    let source = generated_repetition_model();
    let result = find_undetectable_logical_error(&source, 4, 4, false)
        .expect("repetition-code hypergraph error");
    assert_search_signature(&source, &result, SourceMechanismShape::Hypergraph, 7, 4, 0);
}

#[test]
fn pfm_b5_hypergraph_many_observables() {
    let source = many_observables_model();
    let result = find_undetectable_logical_error(&source, 4, 4, false)
        .expect("many-observable hypergraph error");
    assert_search_signature(
        &source,
        &result,
        SourceMechanismShape::Hypergraph,
        5,
        4,
        1200,
    );
}

#[test]
fn pfm_b5_graphlike_zero_probability_diagnostics_match_stim() {
    let model = dem("error(0) L0\n");
    let graphlike = shortest_graphlike_undetectable_logical_error(&model, false)
        .expect_err("zero-probability graphlike model has no logical error")
        .to_string();
    assert_eq!(
        graphlike,
        "invalid detector error model: Failed to find any graphlike logical errors.\n    WARNING: NO DETECTORS. The circuit or detector error model didn't define any detectors.\n    WARNING: NO GRAPHLIKE ERRORS. Although the circuit or detector error model does define some errors, none of them are graphlike (i.e. have at most two detection events), making it vacuously impossible to find a graphlike logical error."
    );
}

#[test]
fn pfm_b5_hypergraph_zero_probability_diagnostics_match_stim() {
    let model = dem("error(0) L0\n");
    let hypergraph = find_undetectable_logical_error(&model, usize::MAX, usize::MAX, false)
        .expect_err("zero-probability hypergraph model has no logical error")
        .to_string();
    assert_eq!(
        hypergraph,
        "invalid detector error model: Failed to find any logical errors.\n    WARNING: NO DETECTORS. The circuit or detector error model didn't define any detectors."
    );
}

#[test]
fn pfm_b5_wcnf_shortest_no_error() {
    assert_eq!(
        shortest_error_sat_problem(&DetectorErrorModel::new()).expect("WCNF"),
        UNSAT_WDIMACS
    );
}

#[test]
fn pfm_b5_wcnf_shortest_observable_only() {
    assert_eq!(
        shortest_error_sat_problem(&dem("error(0.1) L0\n")).expect("WCNF"),
        "p wcnf 1 2 3\n1 -1 0\n3 1 0\n"
    );
}

#[test]
fn pfm_b5_wcnf_shortest_detector_only() {
    assert_eq!(
        shortest_error_sat_problem(&dem("error(0.1) D0\n")).expect("WCNF"),
        UNSAT_WDIMACS
    );
}

#[test]
fn pfm_b5_wcnf_shortest_large_detector_only_is_unsat() {
    let model = dem("repeat 100001 {\n    error(0.1) D0\n    shift_detectors 1\n}\n");
    assert_eq!(
        shortest_error_sat_problem(&model).expect("trivial detector-only WCNF"),
        UNSAT_WDIMACS
    );
}

#[test]
fn pfm_b5_wcnf_shortest_no_targets() {
    assert_eq!(
        shortest_error_sat_problem(&dem("error(0.1)\n")).expect("WCNF"),
        UNSAT_WDIMACS
    );
}

#[test]
fn pfm_b5_wcnf_shortest_empty_model() {
    assert_eq!(
        shortest_error_sat_problem(&dem("")).expect("WCNF"),
        UNSAT_WDIMACS
    );
}

#[test]
fn pfm_b5_wcnf_shortest_detector_observable() {
    for input in [
        "error(0.1) D0 L0\nerror(0.1) D0\n",
        "error(1.0) D0 L0\nerror(0) D0\n",
        "error(0.5) D0 L0\nerror(0.999) D0\n",
        "error(0.001) D0 L0\nerror(0.999) D0\n",
        "error(0) D0 L0\nerror(0) D0\n",
        "error(0.5) D0 L0\nerror(0.5) D0\n",
    ] {
        assert_eq!(
            shortest_error_sat_problem(&dem(input)).expect("WCNF"),
            TWO_ERROR_UNWEIGHTED_WDIMACS
        );
    }
}

#[test]
fn pfm_b5_wcnf_likeliest_no_error() {
    assert_eq!(
        likeliest_error_sat_problem(&DetectorErrorModel::new(), 10).expect("WCNF"),
        UNSAT_WDIMACS
    );
}

#[test]
fn pfm_b5_wcnf_likeliest_detector_observable() {
    assert_eq!(
        likeliest_error_sat_problem(&dem("error(0.1) D0 L0\nerror(0.1) D0\n"), 10,).expect("WCNF"),
        "\
p wcnf 3 8 81
10 -1 0
81 1 2 -3 0
81 1 -2 3 0
81 -1 2 3 0
81 -1 -2 -3 0
10 -2 0
81 -3 0
81 1 0
"
    );
}

#[test]
fn pfm_b5_wcnf_likeliest_large_probability() {
    assert_eq!(
        likeliest_error_sat_problem(&dem("error(0.1) D0 L0\nerror(0.9) D0\n"), 10,).expect("WCNF"),
        "\
p wcnf 3 8 81
10 -1 0
81 1 2 -3 0
81 1 -2 3 0
81 -1 2 3 0
81 -1 -2 -3 0
10 2 0
81 -3 0
81 1 0
"
    );
}

#[test]
fn pfm_b5_wcnf_likeliest_half_probability() {
    assert_eq!(
        likeliest_error_sat_problem(&dem("error(0.1) D0 L0\nerror(0.5) D0\n"), 10,).expect("WCNF"),
        "\
p wcnf 3 7 71
10 -1 0
71 1 2 -3 0
71 1 -2 3 0
71 -1 2 3 0
71 -1 -2 -3 0
71 -3 0
71 1 0
"
    );
}

#[test]
fn pfm_b5_wcnf_likeliest_low_quantization_header_matches_stim() {
    assert_eq!(
        likeliest_error_sat_problem(&dem("error(0.1) D0 L0\nerror(0.49) D0\n"), 1)
            .expect("low-quantization WCNF"),
        "p wcnf 3 8 9\n1 -1 0\n9 1 2 -3 0\n9 1 -2 3 0\n9 -1 2 3 0\n9 -1 -2 -3 0\n9 -3 0\n9 1 0\n"
    );
}

fn generated_surface_models() -> (DetectorErrorModel, DetectorErrorModel) {
    let probability = Probability::try_new(0.001).expect("probability");
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(5).expect("rounds"),
        CodeDistance::try_new(5).expect("distance"),
        SurfaceCodeTask::RotatedMemoryX,
    )
    .expect("surface-code params")
    .with_after_clifford_depolarization(probability)
    .with_before_measure_flip_probability(probability)
    .with_after_reset_flip_probability(probability)
    .with_before_round_data_depolarization(probability);
    let generated = generate_surface_code_circuit(&params).expect("surface-code circuit");
    let graphlike = circuit_to_detector_error_model(
        generated.circuit(),
        ErrorAnalyzerOptions {
            fold_loops: true,
            decompose_errors: true,
            block_decomposition_from_introducing_remnant_edges: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("graphlike surface-code DEM");
    let ungraphlike = circuit_to_detector_error_model(
        generated.circuit(),
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("ungraphlike surface-code DEM");
    (graphlike, ungraphlike)
}

fn generated_repetition_model() -> DetectorErrorModel {
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(10).expect("rounds"),
        CodeDistance::try_new(7).expect("distance"),
        RepetitionCodeTask::Memory,
    )
    .expect("repetition-code params")
    .with_before_round_data_depolarization(Probability::try_new(0.01).expect("probability"));
    let generated = generate_repetition_code_circuit(&params).expect("repetition-code circuit");
    circuit_to_detector_error_model(
        generated.circuit(),
        ErrorAnalyzerOptions {
            fold_loops: true,
            decompose_errors: true,
            block_decomposition_from_introducing_remnant_edges: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("repetition-code DEM")
}

fn many_observables_model() -> DetectorErrorModel {
    let circuit = Circuit::from_stim_str(
        "\
MPP Z0*Z1 Z1*Z2 Z2*Z3 Z3*Z4
X_ERROR(0.1) 0 1 2 3 4
MPP Z0*Z1 Z1*Z2 Z2*Z3 Z3*Z4
DETECTOR rec[-1] rec[-5]
DETECTOR rec[-2] rec[-6]
DETECTOR rec[-3] rec[-7]
DETECTOR rec[-4] rec[-8]
M 4
OBSERVABLE_INCLUDE(1200) rec[-1]
",
    )
    .expect("many-observable circuit");
    circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            decompose_errors: true,
            block_decomposition_from_introducing_remnant_edges: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("many-observable DEM")
}
