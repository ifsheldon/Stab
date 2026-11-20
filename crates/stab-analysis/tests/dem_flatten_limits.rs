#![allow(
    clippy::expect_used,
    clippy::panic_in_result_fn,
    reason = "flattening policy tests use compact fixture construction and exact diagnostics"
)]

use stab_analysis::{
    AnalysisOperation, AnalysisResult, DemFlattenLimits, ResourceKind,
    flattened_detector_error_model, flattened_detector_error_model_with_limits,
};
use stab_model::{
    DemInstruction, DemItem, DemRepeatBlock, DemRepeatCount, DemTarget, DetectorErrorModel,
    MemoryBudget,
};

fn budget(bytes: u64) -> DemFlattenLimits {
    DemFlattenLimits::default().with_memory_budget(MemoryBudget::from_bytes(bytes))
}

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
        DemInstruction::error(
            stab_model::Probability::try_new(0.125).expect("probability"),
            vec![DemTarget::relative_detector(0).expect("D0")],
            None,
        )
        .expect("build error instruction"),
    );
    model
}

#[test]
fn materialized_bytes_admit_the_exact_budget_and_reject_the_first_excess_before_allocation() {
    let model = nested_repeat_model();
    let exact_bytes = 6
        * (std::mem::size_of::<DemItem>()
            + std::mem::size_of::<DemTarget>()
            + std::mem::size_of::<f64>()) as u64;
    let flattened = flattened_detector_error_model_with_limits(&model, budget(exact_bytes))
        .expect("the exact budget materializes");
    assert_eq!(flattened.items().len(), 6);

    let mut rejected = None;
    let measured = allocation_counter::measure(|| {
        rejected = Some(flattened_detector_error_model_with_limits(
            &model,
            budget(exact_bytes - 1),
        ));
    });
    let error = rejected
        .expect("measured call ran")
        .expect_err("one byte below the requirement is refused");
    assert_eq!(measured.bytes_total, 0, "{measured:?}");
    let resource = error.resource_limit_error().expect("typed budget refusal");
    assert_eq!(
        resource.operation(),
        AnalysisOperation::DetectorErrorModelFlatten
    );
    assert_eq!(resource.resource(), ResourceKind::MaterializedBytes);
    assert_eq!(
        (resource.actual(), resource.limit()),
        (exact_bytes, exact_bytes - 1)
    );
    assert!(error.to_string().contains("memory budget"), "{error}");
}

#[test]
fn repeat_counts_above_the_retired_caps_flatten_within_the_budget() {
    let model = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n\
             error(0.125) D0\n\
         }\n",
    )
    .expect("parse repeat above the retired per-block cap");
    assert_eq!(
        flattened_detector_error_model(&model)
            .expect("expansion within the default budget")
            .items()
            .len(),
        100_001
    );

    let hostile = DetectorErrorModel::from_dem_str(
        "repeat 1000000000000 {\n\
             error(0.125) D0\n\
         }\n",
    )
    .expect("parse hostile compact repeat");
    let error = flattened_detector_error_model(&hostile)
        .expect_err("a compact repeat far beyond the budget is refused before allocation");
    assert_eq!(
        error
            .resource_limit_error()
            .map(|resource| resource.resource()),
        Some(ResourceKind::MaterializedBytes)
    );
}

#[test]
fn caller_raised_budget_cannot_exceed_platform_materialization_capacity() {
    let mut model = DetectorErrorModel::new();
    model.push_repeat_block(DemRepeatBlock::new(
        DemRepeatCount::new(u64::MAX),
        single_error_model(),
        None,
    ));
    let error = flattened_detector_error_model_with_limits(&model, budget(u64::MAX))
        .expect_err("platform vector capacity must reject before allocation");
    let resource = error
        .resource_limit_error()
        .expect("platform materialization rejection should remain typed");
    assert_eq!(
        resource.operation(),
        AnalysisOperation::DetectorErrorModelFlatten
    );
    assert_eq!(resource.resource(), ResourceKind::MaterializedUnits);
    assert_eq!(resource.actual(), u64::MAX);
    assert!(resource.limit() < resource.actual());
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

    let error = flattened_detector_error_model_with_limits(&model, budget(u64::MAX))
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

    let error = flattened_detector_error_model_with_limits(&model, budget(u64::MAX))
        .expect_err("resource policy must not relax repeat nesting");

    assert_eq!(
        error.to_string(),
        "invalid detector error model: DEM flattened repeat nesting exceeds current limit 256"
    );
}

#[test]
fn default_entry_points_are_equivalent() -> AnalysisResult<()> {
    let model = nested_repeat_model();
    let free = flattened_detector_error_model(&model)?;
    let explicit_defaults =
        flattened_detector_error_model_with_limits(&model, DemFlattenLimits::default())?;

    assert_eq!(free, explicit_defaults);
    assert_eq!(
        DemFlattenLimits::default().memory_budget(),
        MemoryBudget::DEFAULT
    );
    assert_eq!(
        model,
        nested_repeat_model(),
        "flattening must not mutate its source"
    );
    Ok(())
}
