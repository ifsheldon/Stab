#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests use direct assertions for compact diagnostics"
)]

use stab_analysis::{
    ErrorAnalyzerOptions, ResourceKind, ResourceOperation, circuit_to_detector_error_model,
};
use stab_model::Circuit;

#[test]
fn pf4_dem_analyzer_repeat_resource_policy_is_source_owned() {
    let allowed = Circuit::from_stim_str(
        "
        REPEAT 2 {
            X_ERROR(0.125) 0
            M 0
            DETECTOR rec[-1]
        }
        ",
    )
    .unwrap();
    let dem = circuit_to_detector_error_model(&allowed, ErrorAnalyzerOptions::default()).unwrap();
    assert_eq!(dem.to_dem_string(), "error(0.125) D0 D1\nerror(0.125) D1\n");

    let above_retired_per_block_cap =
        Circuit::from_stim_str("REPEAT 100001 {\n    TICK\n}\n").unwrap();
    let empty = circuit_to_detector_error_model(
        &above_retired_per_block_cap,
        ErrorAnalyzerOptions::default(),
    )
    .expect("aggregate work below the limit remains admitted");
    assert!(empty.is_empty());

    let too_large = Circuit::from_stim_str("REPEAT 1000001 {\n    TICK\n}\n").unwrap();
    let error = circuit_to_detector_error_model(&too_large, ErrorAnalyzerOptions::default())
        .expect_err("reject excessive aggregate repeat work");
    let resource = error.resource_limit_error().expect("typed resource limit");
    assert_eq!(
        resource.operation(),
        ResourceOperation::CircuitToDetectorErrorModel
    );
    assert_eq!(resource.resource(), ResourceKind::RepeatIterations);
    assert_eq!(resource.actual(), 1_000_001);
    assert_eq!(resource.limit(), 1_000_000);

    let nested = Circuit::from_stim_str(
        "
        REPEAT 100000 {
            REPEAT 100000 {
                M 0
                DETECTOR rec[-1]
            }
        }
        ",
    )
    .unwrap();
    let error = circuit_to_detector_error_model(&nested, ErrorAnalyzerOptions::default())
        .expect_err("reject nested expansion");
    let resource = error.resource_limit_error().expect("typed resource limit");
    assert_eq!(resource.resource(), ResourceKind::RepeatIterations);
    assert!(resource.actual() > resource.limit());
}
