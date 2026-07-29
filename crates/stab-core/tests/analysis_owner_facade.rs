#![allow(
    clippy::expect_used,
    reason = "facade tests extract required failures directly"
)]

use stab_core::{
    Circuit, CircuitFlattenLimits, DemFlattenLimits, DetectorErrorModel, ResourceKind,
    ResourceOperation, SatMaterializationLimits,
    analysis::{
        flattened_circuit_with_limits, flattened_detector_error_model,
        flattened_detector_error_model_with_limits,
    },
    shortest_error_sat_problem, shortest_error_sat_problem_with_limits,
};

#[test]
fn analysis_resource_error_conversion_preserves_the_facade_contract() {
    let circuit = Circuit::from_stim_str("REPEAT 4 {\nH 0\n}\n").expect("parse circuit");
    let limits = CircuitFlattenLimits::default().with_max_expanded_operations(3);
    let error = flattened_circuit_with_limits(&circuit, limits)
        .expect_err("the fourth operation exceeds the analysis-owned policy");
    let resource = error
        .resource_limit_error()
        .expect("analysis rejection remains structured");

    assert_eq!(resource.operation(), ResourceOperation::CircuitFlatten);
    assert_eq!(resource.resource(), ResourceKind::ExpandedOperations);
    assert_eq!(resource.actual(), 4);
    assert_eq!(resource.limit(), 3);
    assert_eq!(
        error.to_string(),
        "invalid flattened circuit operation count value 4 exceeds current materialized limit 3"
    );
}

#[test]
fn dem_flatten_resource_conversion_preserves_the_facade_contract() {
    let model = DetectorErrorModel::from_dem_str(
        "repeat 4 {\n\
             error(0.125) D0\n\
         }\n",
    )
    .expect("parse DEM");
    assert_eq!(
        flattened_detector_error_model(&model).expect("flatten through facade"),
        stab_analysis::flattened_detector_error_model(&model).expect("flatten through owner")
    );

    let limits = DemFlattenLimits::default().with_max_repeat_unroll(3);
    let error = flattened_detector_error_model_with_limits(&model, limits)
        .expect_err("the fourth repeat exceeds the analysis-owned policy");
    let resource = error
        .resource_limit_error()
        .expect("analysis rejection remains structured");

    assert_eq!(
        resource.operation(),
        ResourceOperation::DetectorErrorModelFlatten
    );
    assert_eq!(resource.resource(), ResourceKind::RepeatCount);
    assert_eq!(resource.actual(), 4);
    assert_eq!(resource.limit(), 3);
    assert_eq!(
        error.to_string(),
        "invalid detector error model: DEM flattened currently supports repeat counts up to 3, got 4"
    );
}

#[test]
fn sat_resource_conversion_preserves_the_facade_contract() {
    let model =
        DetectorErrorModel::from_dem_str("error(0.125) D0 L0\n").expect("parse SAT fixture");
    assert_eq!(
        shortest_error_sat_problem(&model).expect("encode through facade"),
        stab_analysis::shortest_error_sat_problem(&model).expect("encode through owner")
    );

    let limits = SatMaterializationLimits::default().with_max_target_occurrences(1);
    let error = shortest_error_sat_problem_with_limits(&model, limits)
        .expect_err("the second target exceeds the analysis-owned policy");
    let resource = error
        .resource_limit_error()
        .expect("analysis rejection remains structured");

    assert_eq!(resource.operation(), ResourceOperation::SatMaterialization);
    assert_eq!(resource.resource(), ResourceKind::TargetOccurrences);
    assert_eq!(resource.actual(), 2);
    assert_eq!(resource.limit(), 1);
    assert_eq!(
        error.to_string(),
        "invalid detector error model: SAT problem generation currently supports at most 1 target occurrences, got at least 2"
    );
}
