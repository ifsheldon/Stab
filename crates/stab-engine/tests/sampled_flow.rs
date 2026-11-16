#![allow(
    clippy::expect_used,
    reason = "sampled-flow parity tests mirror compact upstream examples"
)]

use std::assert_matches;
use std::str::FromStr;

use stab_algebra::{Flow, PauliBasis, PauliSign, PauliString};
use stab_engine::{
    RandomPolicy, SampledFlowError, SampledFlowOptions, SamplingCompileError,
    SamplingExecutionError, Seed, ShotCount, sample_if_circuit_has_stabilizer_flows,
    sample_if_circuit_has_stabilizer_flows_with_options,
};
use stab_model::{Cancellation, Circuit, MemoryBudget};

#[test]
fn sampled_signed_flow_semantics_match_stim() {
    assert_signed_unitary_and_measurement_record_flows();
    assert_record_and_pauli_observables();
    assert_inverted_observable_targets();
    assert_malformed_measurement_references_are_rejected();
}

fn assert_signed_unitary_and_measurement_record_flows() {
    let unitary_circuit = circuit(
        "
        R 2 3
        X 1 3
        ",
    );
    let unitary_flows = [
        flow("Z0 -> Z0"),
        flow("Z1 -> -Z1"),
        flow("1 -> Z2"),
        flow("1 -> -Z3"),
        flow("Z0 -> -Z0"),
        flow("Z1 -> Z1"),
        flow("1 -> -Z2"),
        flow("1 -> Z3"),
    ];
    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(
            &unitary_circuit,
            &unitary_flows,
            ShotCount::new(256),
            RandomPolicy::Seeded(Seed::new(5)),
        )
        .expect("sample signed flows"),
        vec![true, true, true, true, false, false, false, false]
    );

    let measurement_circuit = circuit(
        "
        X 1
        M 0 1 2
        X 2
        ",
    );
    let measurement_flows = [
        flow("Z0 -> Z0"),
        flow("Z1 -> -Z1"),
        flow("Z2 -> -Z2"),
        flow("Z0 -> rec[-3]"),
        flow("-Z1 -> rec[-2]"),
        flow("Z2 -> rec[-1]"),
        flow("1 -> Z0 xor rec[-3]"),
        flow("1 -> Z1 xor rec[-2]"),
        flow("1 -> -Z2 xor rec[-1]"),
        flow("Z0 -> -Z0"),
        flow("Z1 -> Z1"),
        flow("Z2 -> Z2"),
        flow("-Z0 -> rec[-3]"),
        flow("Z1 -> rec[-2]"),
        flow("-Z2 -> rec[-1]"),
        flow("1 -> -Z0 xor rec[-3]"),
        flow("1 -> -Z1 xor rec[-2]"),
        flow("1 -> Z2 xor rec[-1]"),
    ];
    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(
            &measurement_circuit,
            &measurement_flows,
            ShotCount::new(256),
            RandomPolicy::Seeded(Seed::new(7)),
        )
        .expect("sample signed measurement flows"),
        vec![
            true, true, true, true, true, true, true, true, true, false, false, false, false,
            false, false, false, false, false
        ]
    );
}

fn assert_record_and_pauli_observables() {
    let observable_record_circuit = circuit(
        "
        X 1
        M 0 1 2
        X 2
        OBSERVABLE_INCLUDE(0) rec[-3]
        OBSERVABLE_INCLUDE(1) rec[-2]
        OBSERVABLE_INCLUDE(2) rec[-1]
        ",
    );
    let observable_record_flows = [
        flow("Z0 -> obs[0]"),
        flow("-Z1 -> obs[1]"),
        flow("Z2 -> obs[2]"),
        flow("1 -> Z0 xor obs[0]"),
        flow("1 -> Z1 xor obs[1]"),
        flow("1 -> -Z2 xor obs[2]"),
        flow("-Z0 -> obs[0]"),
        flow("Z1 -> obs[1]"),
        flow("-Z2 -> obs[2]"),
    ];
    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(
            &observable_record_circuit,
            &observable_record_flows,
            ShotCount::new(256),
            RandomPolicy::Seeded(Seed::new(11)),
        )
        .expect("sample signed observable record flows"),
        vec![true, true, true, true, true, true, false, false, false]
    );

    let observable_pauli_circuit = circuit(
        "
        OBSERVABLE_INCLUDE(3) X0
        OBSERVABLE_INCLUDE(2) Y0
        OBSERVABLE_INCLUDE(4) Z1
        ",
    );
    let observable_pauli_flows = [
        flow("X0 -> obs[3]"),
        flow("Y0 -> obs[2]"),
        flow("Z1 -> obs[4]"),
        flow("-X0 -> obs[3]"),
        flow("X0 -> obs[2]"),
        flow("Y0 -> obs[3]"),
        flow("-Z1 -> obs[4]"),
    ];
    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(
            &observable_pauli_circuit,
            &observable_pauli_flows,
            ShotCount::new(256),
            RandomPolicy::Seeded(Seed::new(13)),
        )
        .expect("sample signed observable Pauli flows"),
        vec![true, true, true, false, false, false, false]
    );
}

fn assert_inverted_observable_targets() {
    let inverted_pauli_circuit = circuit(
        "
        OBSERVABLE_INCLUDE(3) X0
        OBSERVABLE_INCLUDE(2) !X0
        ",
    );
    let inverted_pauli_flows = [
        flow("X0 -> obs[3]"),
        flow("-X0 -> obs[2]"),
        flow("-X0 -> obs[3]"),
        flow("X0 -> obs[2]"),
    ];
    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(
            &inverted_pauli_circuit,
            &inverted_pauli_flows,
            ShotCount::new(256),
            RandomPolicy::Seeded(Seed::new(17)),
        )
        .expect("sample signed inverted observable Pauli flows"),
        vec![true, true, false, false]
    );

    let inverted_record_circuit = circuit(
        "
        M !0
        OBSERVABLE_INCLUDE(3) rec[-1]
        ",
    );
    let inverted_record_flows = [flow("-Z0 -> obs[3]"), flow("Z0 -> obs[3]")];
    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(
            &inverted_record_circuit,
            &inverted_record_flows,
            ShotCount::new(256),
            RandomPolicy::Seeded(Seed::new(23)),
        )
        .expect("sample signed inverted record-backed observable flows"),
        vec![true, false]
    );
}

fn assert_malformed_measurement_references_are_rejected() {
    let error = sample_if_circuit_has_stabilizer_flows(
        &circuit("M 0\n"),
        &[flow("Z -> rec[-2]")],
        ShotCount::new(8),
        RandomPolicy::Seeded(Seed::new(19)),
    )
    .expect_err("reject out-of-range sampled flow record");

    assert_matches!(error, SampledFlowError::InvalidFlow { .. });
    assert!(
        error.to_string().contains("outside sampled flow circuit"),
        "{error}"
    );
}

#[test]
fn sampled_flow_preserves_sampling_storage_admission() {
    let wide = PauliString::from_bases(PauliSign::Plus, vec![PauliBasis::X; 17_000])
        .expect("wide active Pauli");
    assert_matches!(
        sample_if_circuit_has_stabilizer_flows(
            &Circuit::new(),
            &[Flow::from_paulis(wide.clone(), wide)],
            ShotCount::new(1),
            RandomPolicy::Seeded(Seed::new(29)),
        ),
        Err(SampledFlowError::SamplingExecution(
            SamplingExecutionError::SessionStorageLimit { .. }
        ))
    );
}

#[test]
fn sampled_flow_compacts_sparse_qubits_and_ignores_coordinate_only_width() {
    let mut input = PauliString::identity(1_000_001).expect("sparse high input");
    input.set(1_000_000, PauliBasis::X).expect("sparse high X");
    let mut output = PauliString::identity(1_000_001).expect("sparse high output");
    output.set(1_000_000, PauliBasis::Z).expect("sparse high Z");
    assert_eq!(
        sample_if_circuit_has_stabilizer_flows(
            &circuit("QUBIT_COORDS(1, 2) 16000000\nREPEAT 3 {\nH 1000000\n}\n"),
            &[
                Flow::from_paulis(input.clone(), output),
                Flow::from_paulis(input.clone(), input),
            ],
            ShotCount::new(256),
            RandomPolicy::Seeded(Seed::new(31)),
        )
        .expect("sample sparse high flows"),
        vec![true, false]
    );
}

/// Bytes a typed admission refusal says its stage needs, or `None` for any other failure.
fn refused_bytes(error: &SampledFlowError) -> Option<u64> {
    match error {
        SampledFlowError::SamplingExecution(SamplingExecutionError::SessionStorageLimit {
            estimated_bytes,
            ..
        }) => u64::try_from(*estimated_bytes).ok(),
        SampledFlowError::SamplingCompile(SamplingCompileError::CompiledBytesLimit {
            required,
            ..
        }) => Some(*required),
        _ => None,
    }
}

#[test]
fn sampled_flow_admits_every_stage_against_one_caller_budget() {
    // Many measurements make the sampling session larger than the witness simulator, so the
    // compiled plan's session must be admitted against the caller's budget for the walk below
    // to need more than the simulator's figure.
    let checked = circuit("R 2 3\nX 1 3\nREPEAT 4000 {\n    M 0 1\n}\n");
    let flows = [flow("Z0 -> Z0"), flow("1 -> -Z3"), flow("Z1 -> Z1")];
    let run = |bytes: u64| {
        sample_if_circuit_has_stabilizer_flows_with_options(
            &checked,
            &flows,
            ShotCount::new(256),
            RandomPolicy::Seeded(Seed::new(41)),
            SampledFlowOptions {
                memory_budget: MemoryBudget::from_bytes(bytes),
                ..SampledFlowOptions::default()
            },
        )
    };
    // Each refusal reports what its stage needs; raising the budget to it reaches the next
    // stage, so the walk ends at the smallest budget every stage of every flow admits.
    let mut required = 0_u64;
    let mut refusals = 0_usize;
    let verdicts = loop {
        match run(required) {
            Ok(verdicts) => break verdicts,
            Err(error) => {
                let needed = refused_bytes(&error).expect("a typed admission refusal");
                assert!(needed > required, "{error}");
                required = needed;
                refusals += 1;
            }
        }
    };
    assert_eq!(
        verdicts,
        sample_if_circuit_has_stabilizer_flows(
            &checked,
            &flows,
            ShotCount::new(256),
            RandomPolicy::Seeded(Seed::new(41)),
        )
        .expect("default budget")
    );
    assert_eq!(verdicts, vec![true, true, false]);
    assert!(
        refusals >= 2,
        "the session stage must refuse after the simulator stage admits"
    );
    let short = run(required - 1).expect_err("one byte short of the requirement");
    assert!(
        refused_bytes(&short).is_some(),
        "a budget one byte short must fail admission, got {short}"
    );
}

#[test]
fn sampled_flow_reports_cancellation_instead_of_a_verdict() {
    let cancellation = Cancellation::new();
    cancellation.cancel();
    assert_matches!(
        sample_if_circuit_has_stabilizer_flows_with_options(
            &circuit("H 0\nM 0\n"),
            &[flow("X0 -> rec[-1]")],
            ShotCount::new(256),
            RandomPolicy::Seeded(Seed::new(43)),
            SampledFlowOptions {
                cancellation,
                ..SampledFlowOptions::default()
            },
        ),
        Err(SampledFlowError::Cancelled(_))
    );
}

#[test]
fn sampled_flow_charges_augmented_witness_circuit_without_retaining_dropped_noise() {
    let mut unitary_heavy = "S 0\nZ 0\n".repeat(500);
    unitary_heavy.push_str("M 0\n");
    let tight = SampledFlowOptions {
        memory_budget: MemoryBudget::from_bytes(16_384),
        ..SampledFlowOptions::default()
    };
    let refused = sample_if_circuit_has_stabilizer_flows_with_options(
        &circuit(&unitary_heavy),
        &[flow("Z0 -> rec[-1]")],
        ShotCount::new(256),
        RandomPolicy::Seeded(Seed::new(47)),
        tight.clone(),
    )
    .expect_err("1,000-gate augmented witness circuit exceeds 16 KiB");
    assert_matches!(
        refused,
        SampledFlowError::SamplingCompile(SamplingCompileError::CompiledBytesLimit {
            budget: 16_384,
            ..
        })
    );

    let mut noisy_heavy = "X_ERROR(0.1) 0\nY_ERROR(0.1) 0\n".repeat(2000);
    noisy_heavy.push_str("H 0\nM 0\n");
    assert_eq!(
        sample_if_circuit_has_stabilizer_flows_with_options(
            &circuit(&noisy_heavy),
            &[flow("X0 -> rec[-1]")],
            ShotCount::new(256),
            RandomPolicy::Seeded(Seed::new(47)),
            tight,
        )
        .expect("dropped noise instructions do not inflate the witness circuit"),
        vec![true]
    );
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn flow(text: &str) -> Flow {
    Flow::from_str(text).expect("parse flow")
}
