#![allow(
    clippy::expect_used,
    clippy::panic_in_result_fn,
    reason = "resource-contract tests use direct assertions for compact diagnostics"
)]

use stab_analysis::{
    AnalysisResult, ResourceKind, ResourceOperation, SatMaterializationLimits,
    likeliest_error_sat_problem, likeliest_error_sat_problem_with_limits,
    shortest_error_sat_problem, shortest_error_sat_problem_with_limits,
};
use stab_model::DetectorErrorModel;

type SatRunner = fn(&DetectorErrorModel, SatMaterializationLimits) -> AnalysisResult<String>;

fn dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).expect("valid SAT resource fixture")
}

fn shortest(
    model: &DetectorErrorModel,
    limits: SatMaterializationLimits,
) -> AnalysisResult<String> {
    shortest_error_sat_problem_with_limits(model, limits)
}

fn likeliest(
    model: &DetectorErrorModel,
    limits: SatMaterializationLimits,
) -> AnalysisResult<String> {
    likeliest_error_sat_problem_with_limits(model, 100, limits)
}

fn assert_first_excess(
    source: &str,
    accepted: SatMaterializationLimits,
    rejected: SatMaterializationLimits,
    expected_resource: ResourceKind,
) -> AnalysisResult<()> {
    let model = dem(source);
    let original = model.clone();
    for (name, run) in [
        ("shortest", shortest as SatRunner),
        ("likeliest", likeliest as SatRunner),
    ] {
        run(&model, accepted)?;
        let error = run(&model, rejected).expect_err("first excess must be rejected");
        let resource = error
            .resource_limit_error()
            .expect("SAT rejection must retain typed resource context");
        assert_eq!(
            resource.operation(),
            ResourceOperation::SatMaterialization,
            "{name}"
        );
        assert_eq!(resource.resource(), expected_resource, "{name}");
        assert_eq!(resource.actual(), resource.limit() + 1, "{name}: {error}");
        assert_eq!(model, original, "{name} changed its source model");
    }
    Ok(())
}

#[test]
fn sat_materialization_limits_admit_exact_boundaries_and_preserve_source() -> AnalysisResult<()> {
    let defaults = SatMaterializationLimits::default();
    let ordinary = dem("error(0.1) D0 L0\nerror(0.2) D0\n");
    assert_eq!(
        shortest_error_sat_problem_with_limits(&ordinary, defaults)?,
        shortest_error_sat_problem(&ordinary)?
    );
    assert_eq!(
        likeliest_error_sat_problem_with_limits(&ordinary, 100, defaults)?,
        likeliest_error_sat_problem(&ordinary, 100)?
    );

    for (source, accepted, rejected, resource) in [
        (
            "repeat 2 {\nerror(0.1) D0 L0\nshift_detectors 1\n}\n",
            defaults.with_max_repeat_unroll(2),
            defaults.with_max_repeat_unroll(1),
            ResourceKind::RepeatCount,
        ),
        (
            "repeat 2 {\nrepeat 2 {\nerror(0.1) D0 L0\nshift_detectors 1\n}\n}\n",
            defaults.with_max_repeat_iterations(6),
            defaults.with_max_repeat_iterations(5),
            ResourceKind::RepeatIterations,
        ),
        (
            "error(0.1) D0 L0\nshift_detectors 1\n",
            defaults.with_max_expanded_instructions(2),
            defaults.with_max_expanded_instructions(1),
            ResourceKind::ExpandedOperations,
        ),
        (
            "error(0.1) D0 L0\nerror(0.2) D0\n",
            defaults.with_max_error_mechanisms(2),
            defaults.with_max_error_mechanisms(1),
            ResourceKind::ErrorMechanisms,
        ),
        (
            "error(0.1) D0 L0\n",
            defaults.with_max_target_occurrences(2),
            defaults.with_max_target_occurrences(1),
            ResourceKind::TargetOccurrences,
        ),
        (
            "error(0.1) D0 L0\nerror(0.2) D0\n",
            defaults.with_max_variables(3),
            defaults.with_max_variables(2),
            ResourceKind::Variables,
        ),
        (
            "error(0.1) D0 L0\nerror(0.2) D0\n",
            defaults.with_max_clauses(8),
            defaults.with_max_clauses(7),
            ResourceKind::Clauses,
        ),
        (
            "error(0.1) D0 L0\nerror(0.2) D0\n",
            defaults.with_max_clause_literals(16),
            defaults.with_max_clause_literals(15),
            ResourceKind::ClauseLiterals,
        ),
    ] {
        assert_first_excess(source, accepted, rejected, resource)?;
    }

    for (name, run, model) in [
        ("shortest", shortest as SatRunner, dem("")),
        (
            "likeliest",
            likeliest as SatRunner,
            dem("error(0) D1000001 L1000001\n"),
        ),
    ] {
        let output = run(&model, defaults)?;
        assert_eq!(
            run(&model, defaults.with_max_output_bytes(output.len()))?,
            output,
            "{name} exact output boundary"
        );
        let error = run(&model, defaults.with_max_output_bytes(output.len() - 1))
            .expect_err("first output byte beyond the limit must be rejected");
        let resource = error
            .resource_limit_error()
            .expect("output rejection must retain typed resource context");
        assert_eq!(resource.resource(), ResourceKind::OutputBytes, "{name}");
        assert_eq!(resource.actual(), output.len() as u64, "{name}");
        assert_eq!(resource.limit(), (output.len() - 1) as u64, "{name}");
    }

    for source in [
        "repeat 1152921504606846975 {\nrepeat 1152921504606846975 {\nerror(0.1) D0 L0\n}\n}\n",
        "repeat 4 {\nshift_detectors 1152921504606846975\n}\nerror(0.1) D1152921504606846975 L0\n",
    ] {
        let model = dem(source);
        let original = model.clone();
        shortest(&model, defaults).expect_err("overflowing SAT input must be rejected");
        assert_eq!(model, original);
    }
    Ok(())
}
