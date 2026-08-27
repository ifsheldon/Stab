#![allow(
    clippy::expect_used,
    reason = "sampled-flow parity tests mirror compact upstream examples"
)]

use std::str::FromStr;

use stab_algebra::Flow;
use stab_engine::{
    RandomPolicy, SampledFlowError, SamplingExecutionError, Seed, ShotCount,
    sample_if_circuit_has_stabilizer_flows,
};
use stab_model::Circuit;

#[test]
fn sampled_flow_checks_signed_unitary_and_measurement_record_flows() {
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

#[test]
fn sampled_flow_checks_record_and_pauli_observables() {
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

#[test]
fn sampled_flow_checks_inverted_observable_targets() {
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

#[test]
fn sampled_flow_rejects_malformed_measurement_references() {
    let error = sample_if_circuit_has_stabilizer_flows(
        &circuit("M 0\n"),
        &[flow("Z -> rec[-2]")],
        ShotCount::new(8),
        RandomPolicy::Seeded(Seed::new(19)),
    )
    .expect_err("reject out-of-range sampled flow record");

    assert!(matches!(error, SampledFlowError::InvalidFlow { .. }));
    assert!(
        error.to_string().contains("outside sampled flow circuit"),
        "{error}"
    );
}

#[test]
fn sampled_flow_preserves_sampling_storage_admission() {
    let circuit = circuit("H 10000\nM 10000\n");
    assert!(matches!(
        sample_if_circuit_has_stabilizer_flows(
            &circuit,
            &[flow("1 -> 1")],
            ShotCount::new(0),
            RandomPolicy::Seeded(Seed::new(29)),
        ),
        Err(SampledFlowError::SamplingExecution(
            SamplingExecutionError::SessionStorageLimit { .. }
        ))
    ));
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn flow(text: &str) -> Flow {
    Flow::from_str(text).expect("parse flow")
}
