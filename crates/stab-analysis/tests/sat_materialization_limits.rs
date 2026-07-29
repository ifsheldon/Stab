#![allow(
    clippy::expect_used,
    clippy::panic_in_result_fn,
    reason = "SAT materialization policy tests use direct fixture assertions"
)]

use stab_analysis::{
    AnalysisResult, ResourceKind, ResourceOperation, SatMaterializationLimits,
    likeliest_error_sat_problem, likeliest_error_sat_problem_with_limits,
    shortest_error_sat_problem, shortest_error_sat_problem_with_limits,
};
use stab_model::DetectorErrorModel;

fn dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).expect("parse DEM")
}

fn assert_rejected(result: AnalysisResult<String>, expected: &str) {
    let error = result.expect_err("SAT materialization limit should reject the model");
    assert!(
        error.to_string().contains(expected),
        "expected {expected:?} in {error}"
    );
}

macro_rules! limits {
    () => {
        SatMaterializationLimits::default()
    };
}

#[test]
fn default_policy_keeps_frozen_literals_and_existing_entry_points() -> AnalysisResult<()> {
    let model = dem("error(0.1) D0 L0\nerror(0.2) D0\n");
    let limits = limits!();

    assert_eq!(
        shortest_error_sat_problem_with_limits(&model, limits)?,
        shortest_error_sat_problem(&model)?
    );
    assert_eq!(
        likeliest_error_sat_problem_with_limits(&model, 100, limits)?,
        likeliest_error_sat_problem(&model, 100)?
    );

    assert_eq!(limits.max_repeat_unroll(), 100_000);
    assert_eq!(limits.max_expanded_instructions(), 1_000_000);
    assert_eq!(limits.max_repeat_iterations(), 1_000_000);
    assert_eq!(limits.max_error_mechanisms(), 250_000);
    assert_eq!(limits.max_target_occurrences(), 500_000);
    assert_eq!(limits.max_variables(), 500_000);
    assert_eq!(limits.max_clauses(), 500_000);
    assert_eq!(limits.max_clause_literals(), 1_500_000);
    assert_eq!(limits.max_output_bytes(), 128 * 1024 * 1024);

    let excessive_repeat = dem("repeat 100001 {\nerror(0.1) D0 L0\nshift_detectors 1\n}\n");
    let legacy_error = shortest_error_sat_problem(&excessive_repeat)
        .expect_err("legacy entry point should enforce the default repeat limit")
        .to_string();
    let limited_error = shortest_error_sat_problem_with_limits(&excessive_repeat, limits)
        .expect_err("explicit default policy should enforce the same repeat limit")
        .to_string();
    assert_eq!(limited_error, legacy_error);
    assert_eq!(
        limited_error,
        "invalid detector error model: DEM SAT problem generation currently supports repeat counts up to 100000, got 100001"
    );
    Ok(())
}

#[test]
fn traversal_limits_accept_exact_maxima_and_reject_first_excesses() -> AnalysisResult<()> {
    let accepted = dem("repeat 3 {\nerror(0.1) D0 L0\nshift_detectors 1\n}\n");
    shortest_error_sat_problem_with_limits(
        &accepted,
        limits!()
            .with_max_repeat_unroll(3)
            .with_max_repeat_iterations(3)
            .with_max_expanded_instructions(6),
    )?;

    assert_rejected(
        shortest_error_sat_problem_with_limits(
            &accepted,
            limits!()
                .with_max_repeat_unroll(2)
                .with_max_repeat_iterations(100)
                .with_max_expanded_instructions(100),
        ),
        "supports repeat counts up to 2, got 3",
    );

    let nested = dem("repeat 2 {\nrepeat 2 {\nerror(0.1) D0 L0\nshift_detectors 1\n}\n}\n");
    let exact = limits!()
        .with_max_repeat_unroll(2)
        .with_max_repeat_iterations(6)
        .with_max_expanded_instructions(8);
    shortest_error_sat_problem_with_limits(&nested, exact)?;

    assert_rejected(
        shortest_error_sat_problem_with_limits(&nested, exact.with_max_repeat_iterations(5)),
        "supports at most 5 expanded repeat iterations, got at least 6",
    );

    let model = dem("error(0.1) D0 L0\nshift_detectors 1\n");
    shortest_error_sat_problem_with_limits(&model, limits!().with_max_expanded_instructions(2))?;
    assert_rejected(
        shortest_error_sat_problem_with_limits(&model, limits!().with_max_expanded_instructions(1)),
        "supports at most 1 expanded instructions, got at least 2",
    );
    Ok(())
}

#[test]
fn flattened_error_and_target_limits_are_admitted_before_collection() -> AnalysisResult<()> {
    let two_errors = dem("error(0.1) D0 L0\nerror(0.2) D0\n");
    shortest_error_sat_problem_with_limits(&two_errors, limits!().with_max_error_mechanisms(2))?;
    assert_rejected(
        shortest_error_sat_problem_with_limits(&two_errors, limits!().with_max_error_mechanisms(1)),
        "supports at most 1 error mechanisms, got at least 2",
    );
    let error =
        shortest_error_sat_problem_with_limits(&two_errors, limits!().with_max_error_mechanisms(1))
            .expect_err("error mechanism admission should be structured");
    let resource = error
        .resource_limit_error()
        .expect("SAT limit should expose typed context");
    assert_eq!(resource.operation(), ResourceOperation::SatMaterialization);
    assert_eq!(resource.resource(), ResourceKind::ErrorMechanisms);
    assert_eq!(resource.actual(), 2);
    assert_eq!(resource.limit(), 1);

    let two_targets = dem("error(0.1) D0 L0\n");
    shortest_error_sat_problem_with_limits(&two_targets, limits!().with_max_target_occurrences(2))?;
    assert_rejected(
        shortest_error_sat_problem_with_limits(
            &two_targets,
            limits!().with_max_target_occurrences(1),
        ),
        "supports at most 1 target occurrences, got at least 2",
    );
    Ok(())
}

#[test]
fn cnf_shape_limits_accept_exact_maxima_and_reject_first_excess() -> AnalysisResult<()> {
    let model = dem("error(0.1) D0 L0\nerror(0.2) D0\n");

    shortest_error_sat_problem_with_limits(&model, limits!().with_max_variables(3))?;
    assert_rejected(
        shortest_error_sat_problem_with_limits(&model, limits!().with_max_variables(2)),
        "supports at most 2 variables, got at least 3",
    );

    shortest_error_sat_problem_with_limits(&model, limits!().with_max_clauses(8))?;
    assert_rejected(
        shortest_error_sat_problem_with_limits(&model, limits!().with_max_clauses(7)),
        "supports at most 7 clauses, got at least 8",
    );

    shortest_error_sat_problem_with_limits(&model, limits!().with_max_clause_literals(16))?;
    assert_rejected(
        shortest_error_sat_problem_with_limits(&model, limits!().with_max_clause_literals(15)),
        "supports at most 15 clause literals, got at least 16",
    );
    Ok(())
}

#[test]
fn every_early_unsat_path_obeys_the_output_byte_limit() -> AnalysisResult<()> {
    const UNSAT_WDIMACS: &str = "p wcnf 1 2 3\n3 -1 0\n3 1 0\n";

    let no_model_content = dem("");
    let filtered_weighted_error = dem("error(0) D0 L0\n");
    let no_logical_error_target = dem("logical_observable L0\nerror(0.1) D0\n");
    let exact = limits!().with_max_output_bytes(UNSAT_WDIMACS.len());

    for output in [
        shortest_error_sat_problem_with_limits(&no_model_content, exact)?,
        likeliest_error_sat_problem_with_limits(&filtered_weighted_error, 100, exact)?,
        shortest_error_sat_problem_with_limits(&no_logical_error_target, exact)?,
    ] {
        assert_eq!(output, UNSAT_WDIMACS);
    }

    let rejected = exact.with_max_output_bytes(UNSAT_WDIMACS.len() - 1);
    for result in [
        shortest_error_sat_problem_with_limits(&no_model_content, rejected),
        likeliest_error_sat_problem_with_limits(&filtered_weighted_error, 100, rejected),
        shortest_error_sat_problem_with_limits(&no_logical_error_target, rejected),
    ] {
        let error = result.expect_err("the first byte above the UNSAT output limit should fail");
        let resource = error
            .resource_limit_error()
            .expect("early UNSAT output rejection should expose typed context");
        assert_eq!(resource.operation(), ResourceOperation::SatMaterialization);
        assert_eq!(resource.resource(), ResourceKind::OutputBytes);
        assert_eq!(resource.actual(), UNSAT_WDIMACS.len() as u64);
        assert_eq!(resource.limit(), (UNSAT_WDIMACS.len() - 1) as u64);
    }

    let ordinary = dem("error(0.1) D0 L0\nerror(0.2) D0\n");
    let ordinary_output = shortest_error_sat_problem(&ordinary)?;
    let exact_ordinary = limits!().with_max_output_bytes(ordinary_output.len());
    assert_eq!(
        shortest_error_sat_problem_with_limits(&ordinary, exact_ordinary)?,
        ordinary_output
    );
    let first_excess = ordinary_output.len() - 1;
    let error = shortest_error_sat_problem_with_limits(
        &ordinary,
        exact_ordinary.with_max_output_bytes(first_excess),
    )
    .expect_err("ordinary WCNF must reject the first byte above its exact output size");
    let resource = error
        .resource_limit_error()
        .expect("ordinary WCNF output rejection should expose typed context");
    assert_eq!(resource.operation(), ResourceOperation::SatMaterialization);
    assert_eq!(resource.resource(), ResourceKind::OutputBytes);
    assert_eq!(resource.actual(), ordinary_output.len() as u64);
    assert_eq!(resource.limit(), first_excess as u64);
    Ok(())
}

#[test]
fn shifted_detector_targets_are_validated_during_sat_admission() {
    const MAX_TEXT_INTEGER: u64 = (1_u64 << 60) - 1;
    let model = dem(&format!(
        "repeat 4 {{\nshift_detectors {MAX_TEXT_INTEGER}\n}}\n\
         error(0.1) D{MAX_TEXT_INTEGER} L0\n"
    ));
    let original = model.clone();

    let error = shortest_error_sat_problem_with_limits(&model, limits!())
        .expect_err("shifted detector overflow must fail during the admission traversal");
    assert!(
        error.to_string().contains("detector"),
        "unexpected shifted-target error: {error}"
    );
    assert_eq!(model, original);
}

#[test]
fn arithmetic_overflow_is_rejected_without_mutating_the_source() {
    let model =
        dem("repeat 1152921504606846975 {\nrepeat 1152921504606846975 {\nerror(0.1) D0 L0\n}\n}\n");
    let original = model.clone();

    assert_rejected(
        shortest_error_sat_problem_with_limits(&model, limits!()),
        "repeat error count overflowed",
    );
    assert_eq!(model, original);
}

#[test]
fn rejection_leaves_the_source_model_unchanged() {
    let model = dem("error(0.1) D0 L0\nerror(0.2) D0\n");
    let original = model.clone();

    assert_rejected(
        shortest_error_sat_problem_with_limits(&model, limits!().with_max_error_mechanisms(1)),
        "supports at most 1 error mechanisms",
    );
    assert_eq!(model, original);
}
