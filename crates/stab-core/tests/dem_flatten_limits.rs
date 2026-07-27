#![allow(
    clippy::expect_used,
    clippy::panic_in_result_fn,
    reason = "flattening policy tests use compact fixture construction and exact diagnostics"
)]

use stab_core::{
    CircuitResult, DemFlattenLimits, DemInstruction, DemInstructionKind, DemItem, DemRepeatBlock,
    DemRepeatCount, DemTarget, DetectorErrorModel, ResourceKind, ResourceOperation,
    analysis::{flattened_detector_error_model, flattened_detector_error_model_with_limits},
};

fn nested_repeat_model() -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(
        "repeat 2 {\n\
             repeat 3 {\n\
                 error(0.125) D0\n\
             }\n\
         }\n",
    )
    .expect("parse nested repeat DEM")
}

fn single_error_model() -> DetectorErrorModel {
    let mut model = DetectorErrorModel::new();
    model.push_instruction(
        DemInstruction::new(
            DemInstructionKind::Error,
            vec![0.125],
            vec![DemTarget::relative_detector(0).expect("D0")],
            None,
        )
        .expect("build error instruction"),
    );
    model
}

#[test]
fn policy_builders_and_getters_keep_independent_values() {
    let defaults = DemFlattenLimits::default();
    assert_eq!(defaults.max_repeat_unroll(), 100_000);
    assert_eq!(defaults.max_expanded_instructions(), 1_000_000);
    assert_eq!(defaults.max_repeat_iterations(), 1_000_000);
    assert_eq!(defaults.max_target_occurrences(), 32_000_000);
    assert_eq!(defaults.max_argument_values(), 16_000_000);
    assert_eq!(defaults.max_materialized_bytes(), 512 * 1024 * 1024);

    let customized = defaults
        .with_max_repeat_unroll(3)
        .with_max_expanded_instructions(6)
        .with_max_repeat_iterations(8)
        .with_max_target_occurrences(9)
        .with_max_argument_values(10)
        .with_max_materialized_bytes(11);
    assert_eq!(customized.max_repeat_unroll(), 3);
    assert_eq!(customized.max_expanded_instructions(), 6);
    assert_eq!(customized.max_repeat_iterations(), 8);
    assert_eq!(customized.max_target_occurrences(), 9);
    assert_eq!(customized.max_argument_values(), 10);
    assert_eq!(customized.max_materialized_bytes(), 11);
}

#[test]
fn retained_payload_dimensions_have_exact_boundaries() {
    let model = nested_repeat_model();
    let exact_bytes = 6
        * (std::mem::size_of::<DemItem>()
            + std::mem::size_of::<DemTarget>()
            + std::mem::size_of::<f64>()) as u64;
    let exact = DemFlattenLimits::default()
        .with_max_repeat_unroll(3)
        .with_max_expanded_instructions(6)
        .with_max_repeat_iterations(8)
        .with_max_target_occurrences(6)
        .with_max_argument_values(6)
        .with_max_materialized_bytes(exact_bytes);
    flattened_detector_error_model_with_limits(&model, exact)
        .expect("every exact retained payload boundary should materialize");

    for (limits, expected_resource, actual, limit) in [
        (
            exact.with_max_target_occurrences(5),
            ResourceKind::TargetOccurrences,
            6,
            5,
        ),
        (
            exact.with_max_argument_values(5),
            ResourceKind::ArgumentValues,
            6,
            5,
        ),
        (
            exact.with_max_materialized_bytes(exact_bytes - 1),
            ResourceKind::MaterializedBytes,
            exact_bytes,
            exact_bytes - 1,
        ),
    ] {
        let error = flattened_detector_error_model_with_limits(&model, limits)
            .expect_err("the first retained payload above its boundary must reject");
        let resource = error
            .resource_limit_error()
            .expect("payload rejection should remain typed");
        assert_eq!(
            resource.operation(),
            ResourceOperation::DetectorErrorModelFlatten
        );
        assert_eq!(resource.resource(), expected_resource);
        assert_eq!(resource.actual(), actual);
        assert_eq!(resource.limit(), limit);
    }
}

#[test]
fn caller_raised_limit_cannot_exceed_platform_materialization_capacity() {
    let mut model = DetectorErrorModel::new();
    model.push_repeat_block(DemRepeatBlock::new(
        DemRepeatCount::new(u64::MAX),
        single_error_model(),
        None,
    ));
    let limits = DemFlattenLimits::default()
        .with_max_repeat_unroll(u64::MAX)
        .with_max_expanded_instructions(u64::MAX)
        .with_max_repeat_iterations(u64::MAX)
        .with_max_target_occurrences(u64::MAX)
        .with_max_argument_values(u64::MAX)
        .with_max_materialized_bytes(u64::MAX);
    let error = flattened_detector_error_model_with_limits(&model, limits)
        .expect_err("platform vector capacity must reject before allocation");
    let resource = error
        .resource_limit_error()
        .expect("platform materialization rejection should remain typed");
    assert_eq!(
        resource.operation(),
        ResourceOperation::DetectorErrorModelFlatten
    );
    assert_eq!(resource.resource(), ResourceKind::MaterializedUnits);
    assert_eq!(resource.actual(), u64::MAX);
    assert!(resource.limit() < resource.actual());
}

#[test]
fn each_limit_accepts_its_exact_boundary_and_rejects_the_first_excess() {
    let model = nested_repeat_model();

    let flattened = flattened_detector_error_model_with_limits(
        &model,
        DemFlattenLimits::default()
            .with_max_repeat_unroll(3)
            .with_max_expanded_instructions(6)
            .with_max_repeat_iterations(8),
    )
    .expect("accept every exact resource boundary");
    assert_eq!(flattened.items().len(), 6);

    let error = flattened_detector_error_model_with_limits(
        &model,
        DemFlattenLimits::default()
            .with_max_repeat_unroll(2)
            .with_max_expanded_instructions(u64::MAX)
            .with_max_repeat_iterations(u64::MAX),
    )
    .expect_err("reject first repeat-count excess");
    assert_eq!(
        error.to_string(),
        "invalid detector error model: DEM flattened currently supports repeat counts up to 2, got 3"
    );
    let resource = error
        .resource_limit_error()
        .expect("DEM flatten rejection should expose typed context");
    assert_eq!(
        resource.operation(),
        ResourceOperation::DetectorErrorModelFlatten
    );
    assert_eq!(resource.resource(), ResourceKind::RepeatCount);
    assert_eq!(resource.actual(), 3);
    assert_eq!(resource.limit(), 2);

    let error = flattened_detector_error_model_with_limits(
        &model,
        DemFlattenLimits::default()
            .with_max_repeat_unroll(u64::MAX)
            .with_max_expanded_instructions(5)
            .with_max_repeat_iterations(u64::MAX),
    )
    .expect_err("reject first expanded-instruction excess");
    assert_eq!(
        error.to_string(),
        "invalid detector error model: DEM flattened currently supports at most 5 expanded instructions, got at least 6"
    );

    let error = flattened_detector_error_model_with_limits(
        &model,
        DemFlattenLimits::default()
            .with_max_repeat_unroll(u64::MAX)
            .with_max_expanded_instructions(u64::MAX)
            .with_max_repeat_iterations(7),
    )
    .expect_err("reject first aggregate repeat-iteration excess");
    assert_eq!(
        error.to_string(),
        "invalid detector error model: DEM flattened currently supports at most 7 expanded repeat iterations, got at least 8"
    );
}

#[test]
fn practical_default_repeat_boundaries_are_executed_exactly() {
    let empty = DetectorErrorModel::new();
    let mut exact_unroll = DetectorErrorModel::new();
    exact_unroll.push_repeat_block(DemRepeatBlock::new(
        DemRepeatCount::new(DemFlattenLimits::DEFAULT_MAX_REPEAT_UNROLL),
        empty.clone(),
        None,
    ));
    assert!(
        flattened_detector_error_model_with_limits(&exact_unroll, DemFlattenLimits::default())
            .expect("the exact default repeat maximum should be accepted")
            .items()
            .is_empty()
    );

    let mut inner = DetectorErrorModel::new();
    inner.push_repeat_block(DemRepeatBlock::new(DemRepeatCount::new(999), empty, None));
    let mut exact_iterations = DetectorErrorModel::new();
    exact_iterations.push_repeat_block(DemRepeatBlock::new(
        DemRepeatCount::new(1_000),
        inner,
        None,
    ));
    assert!(
        flattened_detector_error_model_with_limits(&exact_iterations, DemFlattenLimits::default(),)
            .expect("one million aggregate repeat iterations should be accepted")
            .items()
            .is_empty()
    );

    exact_iterations.push_repeat_block(DemRepeatBlock::new(
        DemRepeatCount::new(1),
        DetectorErrorModel::new(),
        None,
    ));
    let error =
        flattened_detector_error_model_with_limits(&exact_iterations, DemFlattenLimits::default())
            .expect_err("the first repeat iteration above the default must reject");
    let resource = error
        .resource_limit_error()
        .expect("repeat-iteration rejection should remain typed");
    assert_eq!(resource.resource(), ResourceKind::RepeatIterations);
    assert_eq!(resource.actual(), 1_000_001);
    assert_eq!(resource.limit(), 1_000_000);
}

#[test]
fn nested_repeat_work_is_aggregated_across_repeat_levels() {
    let model = nested_repeat_model();

    flattened_detector_error_model_with_limits(
        &model,
        DemFlattenLimits::default()
            .with_max_repeat_unroll(3)
            .with_max_expanded_instructions(6)
            .with_max_repeat_iterations(8),
    )
    .expect("outer two plus inner six iterations fit");

    let error = flattened_detector_error_model_with_limits(
        &model,
        DemFlattenLimits::default()
            .with_max_repeat_unroll(3)
            .with_max_expanded_instructions(6)
            .with_max_repeat_iterations(6),
    )
    .expect_err("aggregate work must include both repeat levels");
    assert!(
        error
            .to_string()
            .contains("at most 6 expanded repeat iterations, got at least 8"),
        "{error}"
    );
}

#[test]
fn repeat_multiplier_overflow_is_rejected_before_materialization() {
    let leaf = single_error_model();
    let mut inner = DetectorErrorModel::new();
    inner.push_repeat_block(DemRepeatBlock::new(
        DemRepeatCount::new(u64::MAX),
        leaf,
        None,
    ));
    let mut model = DetectorErrorModel::new();
    model.push_repeat_block(DemRepeatBlock::new(DemRepeatCount::new(2), inner, None));
    let original = model.clone();

    let error = flattened_detector_error_model_with_limits(
        &model,
        DemFlattenLimits::default()
            .with_max_repeat_unroll(u64::MAX)
            .with_max_expanded_instructions(u64::MAX)
            .with_max_repeat_iterations(u64::MAX),
    )
    .expect_err("reject repeat multiplier overflow");

    assert_eq!(
        error.to_string(),
        "invalid detector error model: DEM flattened repeat expansion count overflowed"
    );
    assert_eq!(
        model, original,
        "failed admission must not mutate the source"
    );
}

#[test]
fn repeat_nesting_remains_a_fixed_non_configurable_invariant() {
    let mut model = single_error_model();
    for _ in 0..=256 {
        let mut outer = DetectorErrorModel::new();
        outer.push_repeat_block(DemRepeatBlock::new(DemRepeatCount::new(1), model, None));
        model = outer;
    }

    let error = flattened_detector_error_model_with_limits(
        &model,
        DemFlattenLimits::default()
            .with_max_repeat_unroll(u64::MAX)
            .with_max_expanded_instructions(u64::MAX)
            .with_max_repeat_iterations(u64::MAX),
    )
    .expect_err("resource policy must not relax repeat nesting");

    assert_eq!(
        error.to_string(),
        "invalid detector error model: DEM flattened repeat nesting exceeds current limit 256"
    );
}

#[test]
fn default_entry_points_are_equivalent_and_keep_default_error_text() -> CircuitResult<()> {
    let model = nested_repeat_model();
    let free = flattened_detector_error_model(&model)?;
    let compatibility = model.flattened()?;
    let explicit_defaults =
        flattened_detector_error_model_with_limits(&model, DemFlattenLimits::default())?;

    assert_eq!(free, compatibility);
    assert_eq!(free, explicit_defaults);
    assert_eq!(
        model,
        nested_repeat_model(),
        "flattening must not mutate its source"
    );

    let excessive = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n\
             error(0.125) D0\n\
         }\n",
    )?;
    let error = excessive
        .flattened()
        .expect_err("default compatibility entry point rejects excessive repeat");
    let explicit_error =
        flattened_detector_error_model_with_limits(&excessive, DemFlattenLimits::default())
            .expect_err("explicit default policy rejects excessive repeat");
    assert_eq!(explicit_error, error);
    assert_eq!(
        error.to_string(),
        "invalid detector error model: DEM flattened currently supports repeat counts up to 100000, got 100001"
    );

    Ok(())
}
