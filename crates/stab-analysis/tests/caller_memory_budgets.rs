#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "budget fixtures use direct assertions and report an unexpected error by value"
)]

//! Operations that used to admit against the default budget refuse under a smaller caller budget,
//! naming the operation and that budget, and admit the same request at its exact requirement.

use stab_algebra::Flow;
use stab_analysis::{
    AnalysisError, FlowGeneratorOptions, InverseQecOptions, MissingDetectorOptions, ResourceKind,
    ResourceOperation, TimeReversedForFlowsOptions, all_detecting_region_targets_with_budget,
    all_detecting_region_ticks_with_budget, circuit_flow_generators_with_options,
    circuit_inverse_qec_with_options, circuit_time_reversed_for_flows_with_options,
    missing_detectors,
};
use stab_model::{Circuit, CircuitItem, CircuitTick, DemTarget, MemoryBudget, Target};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("fixture circuit")
}

fn assert_refused(
    error: AnalysisError,
    operation: ResourceOperation,
    resource: ResourceKind,
    required: u64,
    budget: MemoryBudget,
) {
    let limit = error
        .resource_limit_error()
        .unwrap_or_else(|| panic!("expected a typed budget refusal, got {error}"));
    assert_eq!(limit.operation(), operation);
    assert_eq!(limit.resource(), resource);
    assert_eq!(limit.actual(), required);
    assert_eq!(limit.limit(), budget.bytes());
    assert!(error.to_string().contains("memory budget"), "{error}");
}

/// The documented projection of owned flow rows: `2n + m` rows, each a `Flow` plus two `n`-qubit
/// Pauli bytes and one `i32` record per measurement.
fn flow_row_bytes(qubits: u64, measurements: u64) -> u64 {
    (2 * qubits + measurements) * (size_of::<Flow>() as u64 + 2 * qubits + 4 * measurements)
}

#[test]
fn measurement_rich_time_reversal_takes_the_caller_budget() {
    let repeated = circuit("REPEAT 1000 {\n    M 0\n}\n");
    // Three retained copies of each expanded item and its targets.
    let required = 3 * 1000 * (size_of::<CircuitItem>() + size_of::<Target>()) as u64;
    let short = MemoryBudget::from_bytes(required - 1);
    let exact = MemoryBudget::from_bytes(required);
    let reverse = |memory_budget| {
        circuit_time_reversed_for_flows_with_options(
            &repeated,
            &[],
            TimeReversedForFlowsOptions {
                memory_budget,
                ..TimeReversedForFlowsOptions::default()
            },
        )
    };
    let error = reverse(short).expect_err("one byte short");
    assert_refused(
        error,
        ResourceOperation::TimeReversal,
        ResourceKind::MaterializedBytes,
        required,
        short,
    );
    let (inverse, _) = reverse(exact).expect("exact budget");
    assert_eq!(inverse.count_measurements().expect("count"), 1000);

    let inverse_qec = |memory_budget| {
        circuit_inverse_qec_with_options(
            &repeated,
            InverseQecOptions {
                memory_budget,
                ..InverseQecOptions::default()
            },
        )
    };
    assert_refused(
        inverse_qec(short).expect_err("QEC inverse one byte short"),
        ResourceOperation::TimeReversal,
        ResourceKind::MaterializedBytes,
        required,
        short,
    );
    inverse_qec(exact).expect("QEC inverse at the exact budget");
}

#[test]
fn flow_generation_takes_the_caller_budget() {
    let generate = |text: &str, memory_budget| {
        circuit_flow_generators_with_options(
            &circuit(text),
            &FlowGeneratorOptions {
                memory_budget,
                ..FlowGeneratorOptions::default()
            },
        )
    };
    for (text, qubits, measurements) in [
        ("M 0 1 2\n", 3, 3),
        ("R 0\nM 0 1\nCX 0 1\nM 1\n", 2, 3),
        ("QUBIT_COORDS(0) 4\n", 5, 0),
    ] {
        let required = flow_row_bytes(qubits, measurements);
        let short = MemoryBudget::from_bytes(required - 1);
        assert_refused(
            generate(text, short).expect_err("one byte short"),
            ResourceOperation::FlowGeneration,
            ResourceKind::ProjectedPayloadBytes,
            required,
            short,
        );
        assert_eq!(
            generate(text, MemoryBudget::from_bytes(required)).expect("exact budget"),
            generate(text, MemoryBudget::DEFAULT).expect("default budget"),
            "{text}"
        );
    }
}

#[test]
fn missing_detector_discovery_takes_the_caller_budget() {
    let source = circuit("R 0 1\nM 0 1\nDETECTOR rec[-1]\n");
    let required = flow_row_bytes(2, 2);
    let short = MemoryBudget::from_bytes(required - 1);
    let discover = |memory_budget| {
        missing_detectors(
            &source,
            MissingDetectorOptions {
                memory_budget,
                ..MissingDetectorOptions::default()
            },
        )
    };
    assert_refused(
        discover(short).expect_err("one byte short"),
        ResourceOperation::MissingDetectorDiscovery,
        ResourceKind::ProjectedPayloadBytes,
        required,
        short,
    );
    let found = discover(MemoryBudget::from_bytes(required)).expect("exact budget");
    assert_eq!(found, discover(MemoryBudget::DEFAULT).expect("default"));
    assert_eq!(found.count_detectors().expect("count"), 1);
}

#[test]
fn detecting_region_helpers_take_the_caller_budget() {
    let source =
        circuit("R 0\nTICK\nTICK\nTICK\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n");
    let tick_bytes = 3 * size_of::<CircuitTick>() as u64;
    let target_bytes = 2 * size_of::<DemTarget>() as u64;
    for (required, refused) in [
        (
            tick_bytes,
            all_detecting_region_ticks_with_budget(
                &source,
                MemoryBudget::from_bytes(tick_bytes - 1),
            )
            .map(|ticks| ticks.len()),
        ),
        (
            target_bytes,
            all_detecting_region_targets_with_budget(
                &source,
                MemoryBudget::from_bytes(target_bytes - 1),
            )
            .map(|targets| targets.len()),
        ),
    ] {
        assert_refused(
            refused.expect_err("one byte short"),
            ResourceOperation::DetectingRegions,
            ResourceKind::MaterializedBytes,
            required,
            MemoryBudget::from_bytes(required - 1),
        );
    }
    assert_eq!(
        all_detecting_region_ticks_with_budget(&source, MemoryBudget::from_bytes(tick_bytes))
            .expect("exact tick budget")
            .len(),
        3
    );
    assert_eq!(
        all_detecting_region_targets_with_budget(&source, MemoryBudget::from_bytes(target_bytes))
            .expect("exact target budget")
            .len(),
        2
    );
}
