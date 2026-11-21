#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests use direct assertions for compact diagnostics"
)]

use stab_analysis::{
    AnalysisOperation, ErrorAnalyzerOptions, ResourceKind, circuit_to_detector_error_model,
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
    let dem = circuit_to_detector_error_model(&allowed, &ErrorAnalyzerOptions::default()).unwrap();
    assert_eq!(dem.to_dem_string(), "error(0.125) D0 D1\nerror(0.125) D1\n");

    let above_retired_per_block_cap =
        Circuit::from_stim_str("REPEAT 100001 {\n    TICK\n}\n").unwrap();
    let empty = circuit_to_detector_error_model(
        &above_retired_per_block_cap,
        &ErrorAnalyzerOptions::default(),
    )
    .expect("aggregate work below the limit remains admitted");
    assert!(empty.is_empty());

    let above_retired_aggregate_cap =
        Circuit::from_stim_str("REPEAT 1000001 {\n    TICK\n}\n").unwrap();
    assert!(
        circuit_to_detector_error_model(
            &above_retired_aggregate_cap,
            &ErrorAnalyzerOptions::default()
        )
        .expect("aggregate repeat work is not capped")
        .is_empty()
    );

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
    let error = circuit_to_detector_error_model(
        &nested,
        &ErrorAnalyzerOptions {
            memory_budget: stab_model::MemoryBudget::from_bytes(64 * 1024),
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect_err("refuse nested expansion output beyond the budget");
    let resource = error.resource_limit_error().expect("typed resource limit");
    assert_eq!(
        resource.operation(),
        AnalysisOperation::CircuitToDetectorErrorModel
    );
    assert_eq!(resource.resource(), ResourceKind::MaterializedBytes);
    assert!(resource.actual() > resource.limit());
}
