#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "CQ2 compatibility tests use direct fixture assertions for precise failures"
)]

use stab_core::{Circuit, CircuitInstruction, Gate, QubitId, Target};

#[test]
fn cq2_circuit_api_instruction_value_contract_matches_stim() {
    let x_error = CircuitInstruction::new(
        Gate::from_name("X_ERROR").unwrap(),
        vec![0.5],
        vec![q(5)],
        None,
    )
    .expect("construct X_ERROR");
    assert_eq!(x_error.gate().canonical_name(), "X_ERROR");
    assert_eq!(x_error.targets(), &[q(5)]);
    assert_eq!(x_error.args(), &[0.5]);
    assert_eq!(x_error, x_error.clone());
    assert_ne!(
        x_error,
        CircuitInstruction::new(
            Gate::from_name("Z_ERROR").unwrap(),
            vec![0.5],
            vec![q(5)],
            None,
        )
        .unwrap()
    );
    assert_ne!(
        x_error,
        CircuitInstruction::new(
            Gate::from_name("X_ERROR").unwrap(),
            vec![0.25],
            vec![q(5)],
            None,
        )
        .unwrap()
    );
    assert_ne!(
        x_error,
        CircuitInstruction::new(
            Gate::from_name("X_ERROR").unwrap(),
            vec![0.5],
            vec![q(5), q(6)],
            None,
        )
        .unwrap()
    );
}

#[test]
fn cq2_circuit_api_instruction_measurement_counts_match_stim() {
    for (text, expected) in [
        ("X 1 2 3\n", 0),
        ("MXX 1 2\n", 1),
        ("M 1 2\n", 2),
        ("MPAD 0 1 0\n", 3),
    ] {
        assert_eq!(
            Circuit::from_stim_str(text)
                .expect("parse measurement-count fixture")
                .count_measurements()
                .expect("count measurements"),
            expected,
            "{text:?}"
        );
    }
}

fn q(id: u32) -> Target {
    Target::qubit(QubitId::new(id).unwrap(), false)
}
