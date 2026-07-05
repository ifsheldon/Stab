#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests use direct assertions for compact diagnostics"
)]

use stab_core::{
    CircuitResult, CodeDistance, DemInstructionKind, DemItem, DemTarget, DetectorErrorModel,
    ErrorAnalyzerOptions, Probability, RepetitionCodeParams, RepetitionCodeTask, RoundCount,
    SurfaceCodeParams, SurfaceCodeTask, circuit_to_detector_error_model,
    find_undetectable_logical_error, generate_repetition_code_circuit,
    generate_surface_code_circuit, likeliest_error_sat_problem, shortest_error_sat_problem,
    shortest_graphlike_undetectable_logical_error,
};

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

    let sat_error = shortest_error_sat_problem(&hostile)
        .expect_err("SAT problem generation should reject hostile repeat expansion")
        .to_string();
    assert!(
        sat_error
            .contains("DEM SAT problem generation currently supports repeat counts up to 100000"),
        "{sat_error}"
    );
}

#[test]
fn pf6_generated_qec_graphlike_search_matches_upstream_instruction_counts() {
    let surface = generated_rotated_surface_code_dem().unwrap();
    assert_search_result_shape(
        &shortest_graphlike_undetectable_logical_error(&surface, false).unwrap(),
        5,
    );
    assert_search_result_shape(
        &shortest_graphlike_undetectable_logical_error(&surface, true).unwrap(),
        5,
    );

    let repetition = generated_repetition_code_dem().unwrap();
    assert_search_result_shape(
        &shortest_graphlike_undetectable_logical_error(&repetition, false).unwrap(),
        7,
    );

    let ungraphlike_surface = generated_rotated_surface_code_ungraphlike_dem().unwrap();
    assert_search_result_shape(
        &shortest_graphlike_undetectable_logical_error(&ungraphlike_surface, true).unwrap(),
        5,
    );
    let error = shortest_graphlike_undetectable_logical_error(&ungraphlike_surface, false)
        .expect_err("ungraphlike generated DEM should be rejected without ignore flag")
        .to_string();
    assert!(error.contains("non-graphlike error mechanism"), "{error}");
}

#[test]
fn pf6_generated_qec_hypergraph_search_matches_upstream_instruction_counts() {
    let surface = generated_rotated_surface_code_dem().unwrap();
    assert_search_result_shape(
        &find_undetectable_logical_error(&surface, 4, 4, true).unwrap(),
        5,
    );

    let repetition = generated_repetition_code_dem().unwrap();
    assert_search_result_shape(
        &find_undetectable_logical_error(&repetition, 4, 4, false).unwrap(),
        7,
    );

    let ungraphlike_surface = generated_rotated_surface_code_ungraphlike_dem().unwrap();
    assert_search_result_shape(
        &find_undetectable_logical_error(&ungraphlike_surface, 4, 4, true).unwrap(),
        5,
    );
}

#[test]
fn pf6_generated_sat_wcnf_qec_encoding_is_structural() {
    let surface = generated_small_rotated_surface_code_dem(true).unwrap();
    assert_unweighted_wcnf_shape(&shortest_error_sat_problem(&surface).unwrap());
    assert_weighted_wcnf_shape(&likeliest_error_sat_problem(&surface, 100).unwrap());

    let repetition = generated_small_repetition_code_dem().unwrap();
    assert_unweighted_wcnf_shape(&shortest_error_sat_problem(&repetition).unwrap());
    assert_weighted_wcnf_shape(&likeliest_error_sat_problem(&repetition, 100).unwrap());

    let ungraphlike_surface = generated_small_rotated_surface_code_dem(false).unwrap();
    assert_unweighted_wcnf_shape(&shortest_error_sat_problem(&ungraphlike_surface).unwrap());
}

fn generated_rotated_surface_code_dem() -> CircuitResult<DetectorErrorModel> {
    generated_rotated_surface_code_dem_with_options(ErrorAnalyzerOptions {
        decompose_errors: true,
        ..ErrorAnalyzerOptions::default()
    })
}

fn generated_rotated_surface_code_ungraphlike_dem() -> CircuitResult<DetectorErrorModel> {
    generated_rotated_surface_code_dem_with_options(ErrorAnalyzerOptions::default())
}

fn generated_rotated_surface_code_dem_with_options(
    options: ErrorAnalyzerOptions,
) -> CircuitResult<DetectorErrorModel> {
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

fn generated_repetition_code_dem() -> CircuitResult<DetectorErrorModel> {
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
            decompose_errors: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
}

fn generated_small_rotated_surface_code_dem(
    decompose_errors: bool,
) -> CircuitResult<DetectorErrorModel> {
    let probability = Probability::try_new(0.001)?;
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(3)?,
        CodeDistance::try_new(3)?,
        SurfaceCodeTask::RotatedMemoryX,
    )?
    .with_after_clifford_depolarization(probability)
    .with_before_measure_flip_probability(probability)
    .with_after_reset_flip_probability(probability)
    .with_before_round_data_depolarization(probability);
    let generated = generate_surface_code_circuit(&params)?;
    circuit_to_detector_error_model(
        generated.circuit(),
        ErrorAnalyzerOptions {
            decompose_errors,
            ..ErrorAnalyzerOptions::default()
        },
    )
}

fn generated_small_repetition_code_dem() -> CircuitResult<DetectorErrorModel> {
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(4)?,
        CodeDistance::try_new(5)?,
        RepetitionCodeTask::Memory,
    )?
    .with_before_round_data_depolarization(Probability::try_new(0.01)?);
    let generated = generate_repetition_code_circuit(&params)?;
    circuit_to_detector_error_model(
        generated.circuit(),
        ErrorAnalyzerOptions {
            decompose_errors: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
}

fn assert_search_result_shape(model: &DetectorErrorModel, expected_error_count: usize) {
    assert_eq!(model.items().len(), expected_error_count);
    assert!(
        model.items().iter().all(|item| matches!(
            item,
            DemItem::Instruction(instruction)
                if instruction.kind() == DemInstructionKind::Error
        )),
        "search output should contain only error instructions: {}",
        model.to_dem_string()
    );
    assert!(
        model.items().iter().any(|item| matches!(
            item,
            DemItem::Instruction(instruction)
                if instruction.targets().iter().any(|target| matches!(target, DemTarget::LogicalObservable(_)))
        )),
        "search output should include a logical observable: {}",
        model.to_dem_string()
    );
}

fn assert_unweighted_wcnf_shape(wcnf: &str) {
    let header = parse_wcnf_header(wcnf);
    assert!(header.variables > 1, "{wcnf}");
    assert!(header.clauses > header.variables, "{wcnf}");
    assert_eq!(header.top_weight, header.clauses + 1, "{wcnf}");
    assert_eq!(wcnf.lines().skip(1).count(), header.clauses, "{wcnf}");
    assert!(wcnf.lines().skip(1).any(|line| line.starts_with("1 -")));
    assert!(
        wcnf.lines()
            .skip(1)
            .any(|line| line.starts_with(&format!("{} ", header.top_weight))),
        "{wcnf}"
    );
}

fn assert_weighted_wcnf_shape(wcnf: &str) {
    let header = parse_wcnf_header(wcnf);
    assert!(header.variables > 1, "{wcnf}");
    assert!(header.clauses > header.variables, "{wcnf}");
    assert!(header.top_weight > header.clauses, "{wcnf}");
    assert_eq!(wcnf.lines().skip(1).count(), header.clauses, "{wcnf}");
    assert!(
        wcnf.lines()
            .skip(1)
            .filter_map(first_wcnf_weight)
            .any(|weight| weight == header.top_weight),
        "{wcnf}"
    );
    assert!(
        wcnf.lines()
            .skip(1)
            .filter_map(first_wcnf_weight)
            .any(|weight| weight > 0 && weight < header.top_weight),
        "{wcnf}"
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WcnfHeader {
    variables: usize,
    clauses: usize,
    top_weight: usize,
}

fn parse_wcnf_header(wcnf: &str) -> WcnfHeader {
    let header = wcnf.lines().next().expect("WCNF header");
    let fields = header.split_whitespace().collect::<Vec<_>>();
    assert_eq!(fields.first(), Some(&"p"), "{header}");
    assert_eq!(fields.get(1), Some(&"wcnf"), "{header}");
    WcnfHeader {
        variables: fields
            .get(2)
            .expect("variable count")
            .parse()
            .expect("numeric variable count"),
        clauses: fields
            .get(3)
            .expect("clause count")
            .parse()
            .expect("numeric clause count"),
        top_weight: fields
            .get(4)
            .expect("top weight")
            .parse()
            .expect("numeric top weight"),
    }
}

fn first_wcnf_weight(line: &str) -> Option<usize> {
    line.split_whitespace().next()?.parse().ok()
}
