#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "budget fixtures use direct assertions and report an unexpected error by value"
)]

//! Operations that used to admit against the default budget refuse under a smaller caller budget,
//! naming the operation and that budget, and admit the same request at its exact requirement.

use stab_algebra::{Flow, PauliString};
use stab_analysis::{
    AnalysisError, FlowGeneratorOptions, InverseQecOptions, MissingDetectorOptions,
    PauliConjugationOptions, ResourceKind, ResourceOperation, TimeReversedForFlowsOptions,
    UnsignedFlowCheckOptions, all_detecting_region_targets_with_budget,
    all_detecting_region_ticks_with_budget, check_if_circuit_has_unsigned_stabilizer_flows,
    check_if_circuit_has_unsigned_stabilizer_flows_with_options,
    check_unsigned_stabilizer_flows_with_diagnostics,
    check_unsigned_stabilizer_flows_with_diagnostics_with_options,
    circuit_flow_generators_with_options, circuit_inverse_qec_with_options,
    circuit_time_reversed_for_flows_with_options, missing_detectors, pauli_after_circuit,
    pauli_after_circuit_with_options, pauli_before_circuit, pauli_before_circuit_with_options,
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

/// Bytes of a Pauli string collected from `qubits` bases: two packed planes plus one byte per
/// collected basis.
fn collected_pauli_bytes(qubits: u64) -> u64 {
    2 * 8 * qubits.div_ceil(64) + qubits
}

#[test]
fn sparse_flow_checking_takes_the_caller_budget() {
    // The measurement keeps the query on the sparse tracker, which extracts each flow's region
    // only as wide as its support: qubits 0 through 129.
    let source = circuit("M 0\nH 129\n");
    let flows = ["X129 -> Z129".parse::<Flow>().expect("flow")];
    let required = collected_pauli_bytes(130);
    let options = |bytes| UnsignedFlowCheckOptions {
        memory_budget: MemoryBudget::from_bytes(bytes),
        ..UnsignedFlowCheckOptions::default()
    };
    let short = MemoryBudget::from_bytes(required - 1);
    assert_refused(
        check_if_circuit_has_unsigned_stabilizer_flows_with_options(
            &source,
            &flows,
            &options(required - 1),
        )
        .expect_err("one byte short"),
        ResourceOperation::FlowCheck,
        ResourceKind::MaterializedBytes,
        required,
        short,
    );
    assert_refused(
        check_unsigned_stabilizer_flows_with_diagnostics_with_options(
            &source,
            &flows,
            &options(required - 1),
        )
        .expect_err("diagnostics one byte short"),
        ResourceOperation::FlowCheck,
        ResourceKind::MaterializedBytes,
        required,
        short,
    );
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows_with_options(
            &source,
            &flows,
            &options(required)
        ),
        Ok(vec![true])
    );
    assert_eq!(
        check_unsigned_stabilizer_flows_with_diagnostics_with_options(
            &source,
            &flows,
            &options(required),
        ),
        Ok(check_unsigned_stabilizer_flows_with_diagnostics(
            &source, &flows
        ))
    );
    // The unbudgeted check keeps failing the flow closed rather than reporting the refusal.
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&source, &flows),
        vec![true]
    );

    // An intermediate Clifford fan-out and fan-in leaves a 2-qubit final region, so only live
    // sensitivity-set metering catches the transient 128-qubit spread.
    let mut fanout_then_cancel = String::from("M 0\nH 1\n");
    for target in 2..=128 {
        fanout_then_cancel.push_str(&format!("CX 1 {target}\n"));
    }
    for target in (2..=128).rev() {
        fanout_then_cancel.push_str(&format!("CX 1 {target}\n"));
    }
    fanout_then_cancel.push_str("H 1\n");
    let transient_wide = circuit(&fanout_then_cancel);
    let flow = ["Z1 -> Z1".parse::<Flow>().expect("flow")];
    let error = check_if_circuit_has_unsigned_stabilizer_flows_with_options(
        &transient_wide,
        &flow,
        &options(1024),
    )
    .expect_err("transient 128-qubit live sensitivity exceeds 1 KiB");
    let resource = error
        .resource_limit_error()
        .expect("typed live-state refusal");
    assert_eq!(resource.operation(), ResourceOperation::FlowCheck);
    assert_eq!(resource.resource(), ResourceKind::LiveStateUnits);
    assert_eq!(resource.limit(), 1024);
    assert!(resource.actual() > 1024);
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows_with_options(
            &transient_wide,
            &flow,
            &options(16_384),
        ),
        Ok(vec![true])
    );
}

#[test]
fn pauli_conjugation_takes_the_caller_budget() {
    // A one-qubit Tableau conjugates a 6400-qubit observable, so the conjugated string is the
    // largest admission.
    let source = circuit("H 0\n");
    let mut text = String::from("X");
    text.push_str(&"_".repeat(6399));
    let pauli: PauliString = text.parse().expect("Pauli fixture");
    let required = collected_pauli_bytes(6400);
    let options = |bytes| PauliConjugationOptions {
        memory_budget: MemoryBudget::from_bytes(bytes),
        ..PauliConjugationOptions::default()
    };
    for (conjugate, unbudgeted) in [
        (
            pauli_after_circuit_with_options as fn(&_, &_, &_) -> _,
            pauli_after_circuit as fn(&_, &_) -> _,
        ),
        (pauli_before_circuit_with_options, pauli_before_circuit),
    ] {
        assert_refused(
            conjugate(&pauli, &source, &options(required - 1)).expect_err("one byte short"),
            ResourceOperation::PauliConjugation,
            ResourceKind::MaterializedBytes,
            required,
            MemoryBudget::from_bytes(required - 1),
        );
        assert_eq!(
            conjugate(&pauli, &source, &options(required)),
            unbudgeted(&pauli, &source)
        );
    }

    // A Tableau the budget refuses is not an error: conjugation walks the instructions instead.
    let wide = circuit("H 0\nCX 0 99\nS 99\n");
    let mut text = String::from("X");
    text.push_str(&"_".repeat(98));
    text.push('Z');
    let pauli: PauliString = text.parse().expect("Pauli fixture");
    let walked = options(1024);
    let after = pauli_after_circuit(&pauli, &wide).expect("conjugate after");
    assert_eq!(
        pauli_after_circuit_with_options(&pauli, &wide, &walked),
        Ok(after.clone())
    );
    assert_eq!(
        pauli_before_circuit_with_options(&after, &wide, &walked),
        Ok(pauli)
    );
}
