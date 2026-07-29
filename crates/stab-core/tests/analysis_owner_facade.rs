#![allow(
    clippy::expect_used,
    reason = "facade tests extract required failures directly"
)]

use stab_core::{
    Circuit, CircuitFlattenLimits, ResourceKind, ResourceOperation,
    analysis::flattened_circuit_with_limits,
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
