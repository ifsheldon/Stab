#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "CQ2 exact gate target contracts use direct assertions for compact diagnostics"
)]

use std::str::FromStr;

use stab_core::{MeasureRecordOffset, Pauli, QubitId, Target};

#[test]
fn cq2_gate_target_text_round_trip_matches_stim() {
    let cases = [
        (qubit(5, false), "5"),
        (qubit(7, true), "!7"),
        (Target::sweep_bit(11), "sweep[11]"),
        (record(-3), "rec[-3]"),
        (pauli(Pauli::X, 13, false), "X13"),
        (pauli(Pauli::X, 17, true), "!X17"),
        (pauli(Pauli::Y, 19, false), "Y19"),
        (pauli(Pauli::Y, 23, true), "!Y23"),
        (pauli(Pauli::Z, 29, false), "Z29"),
        (pauli(Pauli::Z, 31, true), "!Z31"),
        (Target::combiner(), "*"),
    ];
    for (target, text) in cases {
        assert_eq!(target.to_string(), text);
        assert_eq!(Target::from_str(text).unwrap(), target);
        assert_eq!(target.clone(), target);
        assert!(!format!("{target:?}").is_empty());
    }
    assert_eq!(Pauli::X.to_string(), "X");
    assert_eq!(Pauli::Y.to_string(), "Y");
    assert_eq!(Pauli::Z.to_string(), "Z");

    for rejected in [
        "",
        "+1",
        "16777216",
        "X16777216",
        "rec[0]",
        "rec[-16777216]",
        "sweep[+1]",
        "sweep[16777216]",
    ] {
        assert!(Target::from_str(rejected).is_err(), "{rejected:?}");
    }
}

#[test]
fn cq2_gate_target_accessors_match_stim() {
    let plain = qubit(5, false);
    assert_eq!(plain.qubit_id(), QubitId::new(5).ok());
    assert!(plain.is_qubit_target());
    assert!(!plain.is_inverted_result_target());

    let inverted = qubit(7, true);
    assert_eq!(inverted.qubit_id(), QubitId::new(7).ok());
    assert!(inverted.is_qubit_target());
    assert!(inverted.is_inverted_result_target());

    let measurement_record = record(-5);
    assert_eq!(
        measurement_record.measurement_record_offset(),
        MeasureRecordOffset::try_new(-5).ok()
    );
    assert!(measurement_record.is_measurement_record_target());
    assert_eq!(measurement_record.qubit_id(), None);

    let sweep = Target::sweep_bit(11);
    assert_eq!(sweep.sweep_bit_id(), Some(11));
    assert!(sweep.is_sweep_bit_target());
    assert_eq!(sweep.measurement_record_offset(), None);

    for (pauli_type, id, inverted) in [
        (Pauli::X, 13, false),
        (Pauli::X, 17, true),
        (Pauli::Y, 19, false),
        (Pauli::Y, 23, true),
        (Pauli::Z, 29, false),
        (Pauli::Z, 31, true),
    ] {
        let target = pauli(pauli_type, id, inverted);
        assert_eq!(target.qubit_id(), QubitId::new(id).ok());
        assert_eq!(target.pauli_type(), Some(pauli_type));
        assert_eq!(target.is_inverted_result_target(), inverted);
        assert_eq!(target.is_x_target(), pauli_type == Pauli::X);
        assert_eq!(target.is_y_target(), pauli_type == Pauli::Y);
        assert_eq!(target.is_z_target(), pauli_type == Pauli::Z);
        assert!(!target.is_qubit_target());
        assert!(!target.is_measurement_record_target());
        assert!(!target.is_sweep_bit_target());
        assert!(!target.is_combiner());
    }

    assert!(Target::combiner().is_combiner());
    assert!(QubitId::new((1 << 24) - 1).is_ok());
    assert!(QubitId::new(1 << 24).is_err());
    assert!(MeasureRecordOffset::try_new(1).is_err());
    assert!(MeasureRecordOffset::try_new(0).is_err());
    assert!(MeasureRecordOffset::try_new(-(1 << 24)).is_err());
}

#[test]
fn cq2_gate_target_inversion_matches_stim() {
    for target in [
        qubit(5, false),
        pauli(Pauli::X, 5, false),
        pauli(Pauli::Y, 7, false),
        pauli(Pauli::Z, 9, false),
    ] {
        let inverted = target.clone().try_inverted().unwrap();
        assert!(inverted.is_inverted_result_target());
        assert_eq!(inverted.try_inverted().unwrap(), target);
    }
    for target in [Target::combiner(), record(-3), Target::sweep_bit(6)] {
        assert!(target.try_inverted().is_err());
    }
}

#[test]
fn cq2_gate_target_classification_matches_stim() {
    for target in [
        pauli(Pauli::X, 11, false),
        pauli(Pauli::X, 13, true),
        pauli(Pauli::Y, 17, false),
        pauli(Pauli::Y, 19, true),
        pauli(Pauli::Z, 23, false),
        pauli(Pauli::Z, 29, true),
    ] {
        assert!(target.is_pauli_target());
        assert!(!target.is_classical_bit_target());
    }
    for target in [qubit(2, false), qubit(3, true), Target::combiner()] {
        assert!(!target.is_pauli_target());
        assert!(!target.is_classical_bit_target());
    }
    assert!(record(-7).is_classical_bit_target());
    assert!(Target::sweep_bit(5).is_classical_bit_target());
}

fn qubit(id: u32, inverted: bool) -> Target {
    Target::qubit(QubitId::new(id).expect("bounded qubit"), inverted)
}

fn pauli(value: Pauli, id: u32, inverted: bool) -> Target {
    Target::pauli(value, QubitId::new(id).expect("bounded qubit"), inverted)
}

fn record(offset: i32) -> Target {
    Target::measurement_record(
        MeasureRecordOffset::try_new(offset).expect("negative bounded record offset"),
    )
}
