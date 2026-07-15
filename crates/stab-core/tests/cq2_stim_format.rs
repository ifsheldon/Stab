#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "CQ2 compatibility tests use direct fixture assertions for precise failures"
)]

use std::str::FromStr;

use stab_core::{
    Circuit, CircuitInstruction, CircuitItem, Gate, GateArgumentRule, GateTargetGroupKind,
    GateTargetRule, MeasureRecordOffset, Pauli, QubitId, Target,
};

#[test]
fn cq2_stim_format_from_text_contract_matches_stim() {
    for (input, canonical) in [
        ("# not an operation", ""),
        ("H 0", "H 0\n"),
        ("h 0", "H 0\n"),
        ("H 0     ", "H 0\n"),
        ("     H 0     ", "H 0\n"),
        ("\tH 0\t\t", "H 0\n"),
        ("H 0  # comment", "H 0\n"),
        ("H 23", "H 23\n"),
        (
            "DEPOLARIZE1(0.125) 4 5  # comment",
            "DEPOLARIZE1(0.125) 4 5\n",
        ),
        ("  \t Cnot 5 6  # comment   ", "CX 5 6\n"),
        ("", ""),
        ("# Comment\n\n\n# More", ""),
        ("H 0 \n H 1", "H 0 1\n"),
        ("# EPR\nH 0\nCNOT 0 1", "H 0\nCX 0 1\n"),
        ("M 0 !0 1 !1", "M 0 !0 1 !1\n"),
        (
            "H 0\nM 0\nM 1\nM 2\nSWAP 0 1\nM 0\nM 10\n",
            "H 0\nM 0 1 2\nSWAP 0 1\nM 0 10\n",
        ),
        ("DETECTOR rec[-5]", "DETECTOR rec[-5]\n"),
        ("DETECTOR rec[-6]", "DETECTOR rec[-6]\n"),
        ("DETECTOR rec[-0]", "DETECTOR rec[-0]\n"),
        (
            "CORRELATED_ERROR(0.125) X90 Y91 Z92 X93",
            "E(0.125) X90 Y91 Z92 X93\n",
        ),
    ] {
        assert_eq!(
            Circuit::from_stim_str(input)
                .unwrap_or_else(|error| panic!("parse {input:?}: {error}"))
                .to_stim_string(),
            canonical,
            "{input:?}"
        );
    }

    for rejected in [
        "H a",
        "H(1)",
        "X_ERROR 1",
        "H 9999999999999999999999999999999999999999999",
        "H -1",
        "CNOT 0 a",
        "CNOT 0 99999999999999999999999999999999",
        "CNOT 0 -1",
        "DETECTOR 1 2",
        "CX 1 1",
        "SWAP 1 1",
        "DEPOLARIZE2(1) 1 1",
        "DETEstdCTOR rec[-1]",
        "DETECTOR rec[0]",
        "DETECTOR rec[1]",
        "DETECTOR rec[-999999999999]",
        "OBSERVABLE_INCLUDE rec[-1]",
        "OBSERVABLE_INCLUDE(-1) rec[-1]",
        "CORRELATED_ERROR(1) B1",
        "CORRELATED_ERROR(1) X 1",
        "CORRELATED_ERROR(1) X\n",
        "CORRELATED_ERROR(1) 1",
        "ELSE_CORRELATED_ERROR(1) 1 2",
        "CORRELATED_ERROR(1) 1 2",
        "CORRELATED_ERROR(1) A",
        "CNOT 0\nCNOT 1",
    ] {
        assert!(
            Circuit::from_stim_str(rejected).is_err(),
            "expected rejection: {rejected:?}"
        );
    }

    let repeated = Circuit::from_stim_str("X 0\nREPEAT 2 {\nY 1\nY 2 #####\n} #####\n")
        .expect("parse repeat with comments");
    assert_eq!(repeated.to_stim_string(), "X 0\nREPEAT 2 {\n    Y 1 2\n}\n");

    let nested = Circuit::from_stim_str("M 0\nREPEAT 5 {\nM 1 2\nM 3\n}\n")
        .expect("parse fused repeat body");
    let CircuitItem::RepeatBlock(repeat) = &nested.items()[1] else {
        panic!("expected a repeat block")
    };
    assert_eq!(repeat.body().items().len(), 1);
    let CircuitItem::Instruction(measurement) = &repeat.body().items()[0] else {
        panic!("expected a fused measurement")
    };
    assert_eq!(measurement.targets(), &[q(1), q(2), q(3)]);
}

#[test]
fn cq2_stim_format_canonical_printer_contract_matches_stim() {
    let circuit = Circuit::from_stim_str(
        "tick\nCNOT 2 3\nCNOT rec[-5] 3\nCY sweep[6] 4\nM 1 3 2\nDETECTOR rec[-7]\nOBSERVABLE_INCLUDE(17) rec[-11] rec[-1]\nX_ERROR(0.5) 19\nCORRELATED_ERROR(0.25) X23 Z27 Y29\n",
    )
    .expect("parse canonical-printer fixture");
    assert_eq!(
        circuit.to_stim_string(),
        "TICK\nCX 2 3 rec[-5] 3\nCY sweep[6] 4\nM 1 3 2\nDETECTOR rec[-7]\nOBSERVABLE_INCLUDE(17) rec[-11] rec[-1]\nX_ERROR(0.5) 19\nE(0.25) X23 Z27 Y29\n"
    );

    for (input, expected) in [
        ("REPEAT 2 {\n}\n", "REPEAT 2 {\n\n}\n"),
        ("REPEAT[empty] 3 {\n}\n", "REPEAT[empty] 3 {\n\n}\n"),
        (
            "REPEAT 2 {\nREPEAT 3 {\n}\n}\n",
            "REPEAT 2 {\n    REPEAT 3 {\n\n    }\n}\n",
        ),
    ] {
        assert_eq!(
            Circuit::from_stim_str(input)
                .expect("parse empty-repeat canonical fixture")
                .to_stim_string(),
            expected
        );
    }

    let qualification_cycle = [
        "H 0\n",
        "S 1\n",
        "CX 0 1\n",
        "M 0\n",
        "DETECTOR rec[-1]\n",
        "TICK\n",
    ]
    .iter()
    .cycle()
    .take(64)
    .copied()
    .collect::<String>();
    let qualification_circuit =
        Circuit::from_stim_str(&qualification_cycle).expect("parse qualification print cycle");
    assert_eq!(qualification_circuit.to_stim_string(), qualification_cycle);
}

#[test]
fn cq2_stim_format_sweep_target_roles_match_stim() {
    assert!(Circuit::from_stim_str("H sweep[0]\n").is_err());
    assert!(Circuit::from_stim_str("X sweep[0]\n").is_err());
    let circuit = Circuit::from_stim_str("CNOT sweep[2] 5\n").expect("parse sweep control");
    let CircuitItem::Instruction(instruction) = &circuit.items()[0] else {
        panic!("expected one instruction")
    };
    assert_eq!(instruction.args(), &[]);
    assert_eq!(instruction.targets(), &[Target::sweep_bit(2), q(5)]);
}

#[test]
fn cq2_stim_format_target_text_round_trip_matches_stim() {
    let cases = [
        (Target::qubit(QubitId::new(2).unwrap(), false), "2"),
        (Target::qubit(QubitId::new(3).unwrap(), true), "!3"),
        (Target::sweep_bit(5), "sweep[5]"),
        (
            Target::measurement_record(MeasureRecordOffset::try_new(-7).unwrap()),
            "rec[-7]",
        ),
        (
            Target::pauli(Pauli::X, QubitId::new(11).unwrap(), false),
            "X11",
        ),
        (
            Target::pauli(Pauli::X, QubitId::new(13).unwrap(), true),
            "!X13",
        ),
        (
            Target::pauli(Pauli::Y, QubitId::new(17).unwrap(), false),
            "Y17",
        ),
        (
            Target::pauli(Pauli::Y, QubitId::new(19).unwrap(), true),
            "!Y19",
        ),
        (
            Target::pauli(Pauli::Z, QubitId::new(23).unwrap(), false),
            "Z23",
        ),
        (
            Target::pauli(Pauli::Z, QubitId::new(29).unwrap(), true),
            "!Z29",
        ),
        (Target::combiner(), "*"),
    ];
    for (target, text) in cases {
        assert_eq!(target.to_string(), text);
        assert_eq!(Target::from_str(text).unwrap(), target);
    }

    assert_eq!(Target::from_str("5").unwrap(), q(5));
    assert_eq!(Target::from_str("rec[-3]").unwrap(), record(-3));
    let parsed_zero = Target::from_str("rec[-0]").unwrap();
    assert_eq!(parsed_zero.to_string(), "rec[-0]");
    let parsed_zero_offset = parsed_zero
        .measurement_record_offset()
        .expect("negative-zero record offset");
    assert_eq!(parsed_zero_offset.get(), 0);
    assert_eq!(format!("{parsed_zero_offset:?}"), "MeasureRecordOffset(0)");
    assert!(parsed_zero_offset > MeasureRecordOffset::try_new(-1).unwrap());
    assert_eq!(
        Target::from_str("y17").unwrap(),
        Target::pauli(Pauli::Y, QubitId::new(17).unwrap(), false)
    );
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
fn cq2_stim_format_target_accessors_match_stim() {
    let qubit = q(5);
    assert_eq!(qubit.qubit_id(), QubitId::new(5).ok());
    assert!(!qubit.is_inverted_result_target());
    assert!(qubit.is_qubit_target());

    let inverted_qubit = Target::qubit(QubitId::new(7).unwrap(), true);
    assert_eq!(inverted_qubit.qubit_id(), QubitId::new(7).ok());
    assert!(inverted_qubit.is_inverted_result_target());
    assert!(inverted_qubit.is_qubit_target());

    let record_target = record(-5);
    assert_eq!(
        record_target.measurement_record_offset(),
        MeasureRecordOffset::try_new(-5).ok()
    );
    assert!(record_target.is_measurement_record_target());
    assert_eq!(record_target.qubit_id(), None);
    assert_eq!(record_target.sweep_bit_id(), None);

    let sweep = Target::sweep_bit(11);
    assert_eq!(sweep.sweep_bit_id(), Some(11));
    assert!(sweep.is_sweep_bit_target());
    assert_eq!(sweep.qubit_id(), None);
    assert_eq!(sweep.measurement_record_offset(), None);

    for (pauli_type, id, inverted) in [
        (Pauli::X, 13, false),
        (Pauli::X, 17, true),
        (Pauli::Y, 19, false),
        (Pauli::Y, 23, true),
        (Pauli::Z, 29, false),
        (Pauli::Z, 31, true),
    ] {
        let target = Target::pauli(pauli_type, QubitId::new(id).unwrap(), inverted);
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

    for target in [
        qubit,
        inverted_qubit,
        record_target,
        sweep,
        Target::combiner(),
    ] {
        assert_eq!(target.pauli_type(), None);
    }
    assert!(QubitId::new((1 << 24) - 1).is_ok());
    assert!(QubitId::new(1 << 24).is_err());
    assert!(MeasureRecordOffset::try_new(1).is_err());
    assert!(MeasureRecordOffset::try_new(0).is_err());
    assert!(MeasureRecordOffset::try_new(-(1 << 24)).is_err());
}

#[test]
fn cq2_stim_format_target_inversion_matches_stim() {
    let invertible = [
        q(5),
        pauli(Pauli::X, 5),
        pauli(Pauli::Y, 7),
        pauli(Pauli::Z, 9),
    ];
    for target in invertible {
        let inverted = target.try_inverted().unwrap();
        assert!(inverted.is_inverted_result_target());
        assert_eq!(inverted.try_inverted().unwrap(), target);
    }
    for target in [Target::combiner(), record(-3), Target::sweep_bit(6)] {
        assert!(target.try_inverted().is_err());
    }
}

#[test]
fn cq2_stim_format_target_classification_matches_stim() {
    let paulis = [
        pauli(Pauli::X, 11),
        Target::pauli(Pauli::X, QubitId::new(13).unwrap(), true),
        pauli(Pauli::Y, 17),
        Target::pauli(Pauli::Y, QubitId::new(19).unwrap(), true),
        pauli(Pauli::Z, 23),
        Target::pauli(Pauli::Z, QubitId::new(29).unwrap(), true),
    ];
    for target in paulis {
        assert!(target.is_pauli_target());
        assert!(!target.is_classical_bit_target());
    }

    for target in [q(2), inverted_q(3), Target::combiner()] {
        assert!(!target.is_pauli_target());
        assert!(!target.is_classical_bit_target());
    }
    assert!(record(-7).is_classical_bit_target());
    assert!(Target::sweep_bit(5).is_classical_bit_target());
    assert!(Target::combiner().is_combiner());
}

#[test]
fn cq2_stim_format_tag_escape_and_repeat_contract_matches_stim() {
    let simple =
        Circuit::from_stim_str("H[test] 3 5\nH[] 3 5\nH 3 5\n").expect("parse simple tags");
    let tags = simple
        .items()
        .iter()
        .map(|item| match item {
            CircuitItem::Instruction(instruction) => instruction.tag(),
            CircuitItem::RepeatBlock(_) => panic!("simple tag fixture has no repeat"),
        })
        .collect::<Vec<_>>();
    assert_eq!(tags, vec![Some("test"), None]);
    assert_eq!(simple.to_stim_string(), "H[test] 3 5\nH 3 5 3 5\n");

    let escaped = Circuit::from_stim_str(r"H[test \B\C\r\n] 3 5").expect("parse escaped tag");
    let CircuitItem::Instruction(instruction) = &escaped.items()[0] else {
        panic!("expected tagged instruction")
    };
    assert_eq!(instruction.tag(), Some("test \\]\r\n"));
    assert_eq!(escaped.to_stim_string(), "H[test \\B\\C\\r\\n] 3 5\n");

    let nested =
        Circuit::from_stim_str("H 0\nM 0 1\nREPEAT 2 {\nX 1\nREPEAT 3 {\nY 2\nM 2\nX 0\n}\n}\n")
            .expect("parse nested repeat structure");
    assert_eq!(nested.items().len(), 3);
    let CircuitItem::RepeatBlock(outer) = &nested.items()[2] else {
        panic!("expected outer repeat")
    };
    assert_eq!(outer.body().items().len(), 2);
    let CircuitItem::RepeatBlock(inner) = &outer.body().items()[1] else {
        panic!("expected inner repeat")
    };
    assert_eq!(inner.body().items().len(), 3);
}

#[test]
fn cq2_stim_format_validation_edges_match_stim() {
    for accepted in [
        "TICK\n",
        "X_ERROR(0) 1\n",
        "X_ERROR(0.1) 1\n",
        "X_ERROR(1) 1\n",
        "MPAD 0 1\n",
    ] {
        Circuit::from_stim_str(accepted).unwrap_or_else(|error| {
            panic!("expected Stim syntax to be accepted: {accepted:?}: {error}")
        });
    }

    for rejected in [
        "TICK 1\n",
        "X_ERROR 1\n",
        "X_ERROR(-0.1) 1\n",
        "X_ERROR(1.1) 1\n",
        "X_ERROR(0.1, 0.1) 1\n",
        "MPAD 2\n",
        "MPAD sweep[0]\n",
        "CX 0 1 2\n",
        "M 0\nXCX rec[-1] 1\n",
    ] {
        assert!(
            Circuit::from_stim_str(rejected).is_err(),
            "expected Stim syntax to be rejected: {rejected:?}"
        );
    }

    let classical =
        Circuit::from_stim_str("ZCX 0 1\nZCX rec[-1] 1\nZCY rec[-2] 1\nZCZ rec[-4] 1\n")
            .expect("parse valid classical controls");
    assert_eq!(
        classical.to_stim_string(),
        "CX 0 1 rec[-1] 1\nCY rec[-2] 1\nCZ rec[-4] 1\n"
    );

    assert!(
        CircuitInstruction::new(Gate::from_name("TICK").unwrap(), vec![], vec![q(1)], None)
            .is_err()
    );
    assert!(
        CircuitInstruction::new(
            Gate::from_name("DETECTOR").unwrap(),
            vec![],
            vec![q(1)],
            None,
        )
        .is_err()
    );
}

#[test]
fn cq2_stim_format_repeat_validation_matches_stim() {
    for rejected in [
        "REPEAT 100 {",
        "REPEAT 100 {{\n}",
        "REPEAT {\n}",
        "H {",
        "H {\n}",
        "REPEAT 0 {\nTICK\n}\n",
    ] {
        assert!(
            Circuit::from_stim_str(rejected).is_err(),
            "expected repeat grammar rejection: {rejected:?}"
        );
        let mut existing = Circuit::from_stim_str("H 0\n").unwrap();
        let before = existing.clone();
        assert!(existing.append_from_stim_text(rejected).is_err());
        assert_eq!(existing, before, "failed append mutated the circuit");
    }
}

#[test]
fn cq2_stim_format_instruction_validation_matches_stim() {
    let cx = Gate::from_name("CX").unwrap();
    assert!(CircuitInstruction::new(cx, vec![], vec![q(0)], None).is_err());
    assert!(CircuitInstruction::new(cx, vec![], vec![q(0), q(0)], None).is_err());
    assert!(CircuitInstruction::new(cx, vec![], vec![q(0), q(1)], None).is_ok());

    let x = Gate::from_name("X").unwrap();
    assert!(CircuitInstruction::new(x, vec![], vec![pauli(Pauli::X, 0)], None).is_err());
    assert!(CircuitInstruction::new(x, vec![], vec![inverted_q(0)], None).is_err());
    assert!(CircuitInstruction::new(x, vec![0.5], vec![q(0)], None).is_err());

    let measurement = Gate::from_name("M").unwrap();
    assert!(CircuitInstruction::new(measurement, vec![], vec![pauli(Pauli::X, 0)], None,).is_err());
    assert!(CircuitInstruction::new(measurement, vec![], vec![inverted_q(0)], None).is_ok());
    assert!(CircuitInstruction::new(measurement, vec![0.125], vec![inverted_q(0)], None).is_ok());
    assert!(CircuitInstruction::new(measurement, vec![1.5], vec![q(0)], None).is_err());
    assert!(CircuitInstruction::new(measurement, vec![-1.5], vec![q(0)], None).is_err());
    assert!(CircuitInstruction::new(measurement, vec![0.125, 0.25], vec![q(0)], None).is_err());

    let correlated = Gate::from_name("CORRELATED_ERROR").unwrap();
    assert!(
        CircuitInstruction::new(correlated, vec![0.1], vec![pauli(Pauli::X, 0)], None,).is_ok()
    );
    assert!(
        CircuitInstruction::new(correlated, vec![0.1], vec![pauli(Pauli::Z, 0)], None,).is_ok()
    );
    assert!(CircuitInstruction::new(correlated, vec![], vec![pauli(Pauli::X, 0)], None).is_err());
    assert!(
        CircuitInstruction::new(correlated, vec![0.1, 0.2], vec![pauli(Pauli::X, 0)], None,)
            .is_err()
    );
    assert!(
        CircuitInstruction::new(
            correlated,
            vec![0.1],
            vec![Target::pauli(Pauli::X, QubitId::new(0).unwrap(), true)],
            None,
        )
        .is_err()
    );

    assert!(
        CircuitInstruction::new(
            Gate::from_name("X_ERROR").unwrap(),
            vec![0.5],
            vec![q(0)],
            None,
        )
        .is_ok()
    );
}

#[test]
fn cq2_stim_format_probability_list_validation_matches_stim() {
    for accepted in [
        "PAULI_CHANNEL_1(0,0,0) 1\n",
        "PAULI_CHANNEL_1(0.1,0.2,0.6) 1\n",
        "PAULI_CHANNEL_1(1,0,0) 1\n",
        "PAULI_CHANNEL_1(0.33333333334,0.33333333334,0.33333333334) 1\n",
        "PAULI_CHANNEL_2(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 1 2\n",
        "PAULI_CHANNEL_2(0.1,0,0,0,0,0,0,0,0,0,0.1,0,0,0,0.1) 1 2\n",
    ] {
        assert!(Circuit::from_stim_str(accepted).is_ok(), "{accepted}");
    }

    for rejected in [
        "PAULI_CHANNEL_1 1\n",
        "PAULI_CHANNEL_1(0.1) 1\n",
        "PAULI_CHANNEL_1(0.1,0.1) 1\n",
        "PAULI_CHANNEL_1(0.1,0.1,0.1,0.1) 1\n",
        "PAULI_CHANNEL_1(-1,0,0) 1\n",
        "PAULI_CHANNEL_1(0.1,0.5,0.6) 1\n",
        "PAULI_CHANNEL_2(0.1,0,0,0,0,0,0,0,0,0,0.1,0,0,0,0.1) 1\n",
        "PAULI_CHANNEL_2(0.4,0,0,0,0,0.4,0,0,0,0,0,0,0,0,0.4) 1 2\n",
        "PAULI_CHANNEL_2(0,0,0,0,0,0,0,0,0,0,0,0,0,0) 1 2\n",
        "PAULI_CHANNEL_2(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 1 2\n",
    ] {
        assert!(Circuit::from_stim_str(rejected).is_err(), "{rejected}");
    }

    assert!(
        CircuitInstruction::new(
            Gate::from_name("X_ERROR").unwrap(),
            vec![f64::NAN],
            vec![q(0)],
            None,
        )
        .is_err()
    );
}

#[test]
fn cq2_stim_format_instruction_and_concatenation_fusion_match_stim() {
    for (input, expected) in [
        ("H 1\nH 2 3\n", "H 1 2 3\n"),
        ("R 0\nR\n", "R 0\n"),
        ("M 0 1\nM 2 3\n", "M 0 1 2 3\n"),
        ("TICK\nTICK\n", "TICK\nTICK\n"),
        (
            "DETECTOR rec[-2] rec[-2]\nDETECTOR rec[-1] rec[-1]\n",
            "DETECTOR rec[-2] rec[-2]\nDETECTOR rec[-1] rec[-1]\n",
        ),
    ] {
        assert_eq!(
            Circuit::from_stim_str(input).unwrap().to_stim_string(),
            expected
        );
    }

    assert!(Circuit::from_stim_str("CNOT 0\n").is_err());
    assert!(Circuit::from_stim_str("X(0.5) 0\n").is_err());

    let left = Circuit::from_stim_str("H 0\n").unwrap();
    let right = Circuit::from_stim_str("H 1\n").unwrap();
    assert_eq!(left.concatenated(&right).to_stim_string(), "H 0 1\n");
}

#[test]
fn cq2_stim_format_repeat_coordinate_and_newline_edges_match_stim() {
    const REPETITIONS: u64 = 1_234_567_890_123_456_789;
    let repeated = Circuit::from_stim_str("REPEAT 1234567890123456789 {\n    M 1\n}\n")
        .expect("parse large repeat count");
    assert_eq!(
        repeated.to_stim_string(),
        "REPEAT 1234567890123456789 {\n    M 1\n}\n"
    );
    assert_eq!(repeated.count_measurements().unwrap(), REPETITIONS);

    let coordinates = Circuit::from_stim_str(
        "SHIFT_COORDS(-1, -2, -3)\nQUBIT_COORDS(1, -2) 1\nQUBIT_COORDS(-3.5) 1\n",
    )
    .expect("parse negative coordinates");
    let instructions = coordinates
        .items()
        .iter()
        .map(|item| match item {
            CircuitItem::Instruction(instruction) => instruction,
            CircuitItem::RepeatBlock(_) => panic!("coordinate fixture contains no repeat block"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        instructions[0].coordinate_arguments(),
        Some(&[-1.0, -2.0, -3.0][..])
    );
    assert_eq!(instructions[2].coordinate_arguments(), Some(&[-3.5][..]));
    assert!(Circuit::from_stim_str("M(-0.1) 0\n").is_err());
    assert!(Circuit::from_stim_str("QUBIT_COORDS(1e10000) 0\n").is_err());
    for exponent in ["1e20", "1E+20"] {
        let parsed = Circuit::from_stim_str(&format!("QUBIT_COORDS({exponent}) 0\n"))
            .expect("parse finite exponent");
        let CircuitItem::Instruction(instruction) = &parsed.items()[0] else {
            panic!("coordinate fixture contains one instruction")
        };
        assert_eq!(instruction.coordinate_arguments(), Some(&[1e20][..]));
    }

    assert_eq!(
        Circuit::from_stim_str("H 0\r\nCX 0 1\r\n").expect("parse CRLF circuit"),
        Circuit::from_stim_str("H 0\nCX 0 1\n").expect("parse LF circuit")
    );
}

#[test]
fn cq2_stim_format_target_groups_cover_stim_gate_shapes() {
    let circuit = Circuit::from_stim_str(
        "X\nCX\nS 1\nH 0 2\nTICK\nCX 0 1 2 3\nCY 3 5\nSPP\nMPP X0*X1*Z2 Z7 X5*X9\nSPP Z5\nMPAD 0 1 0\nQUBIT_COORDS 1 2\n",
    )
    .expect("parse representative target groups");
    let groups = circuit
        .items()
        .iter()
        .map(instruction_groups)
        .collect::<Vec<_>>();
    assert_eq!(groups[0], Vec::<Vec<Target>>::new());
    assert_eq!(groups[1], Vec::<Vec<Target>>::new());
    assert_eq!(groups[2], vec![vec![q(1)]]);
    assert_eq!(groups[3], vec![vec![q(0)], vec![q(2)]]);
    assert_eq!(groups[4], Vec::<Vec<Target>>::new());
    assert_eq!(groups[5], vec![vec![q(0), q(1)], vec![q(2), q(3)]]);
    assert_eq!(groups[6], vec![vec![q(3), q(5)]]);
    assert_eq!(groups[7], Vec::<Vec<Target>>::new());
    assert_eq!(
        groups[8],
        vec![
            vec![
                pauli(Pauli::X, 0),
                Target::combiner(),
                pauli(Pauli::X, 1),
                Target::combiner(),
                pauli(Pauli::Z, 2)
            ],
            vec![pauli(Pauli::Z, 7)],
            vec![pauli(Pauli::X, 5), Target::combiner(), pauli(Pauli::X, 9)],
        ]
    );
    assert_eq!(groups[9], vec![vec![pauli(Pauli::Z, 5)]]);
    assert_eq!(groups[10], vec![vec![q(0)], vec![q(1)], vec![q(0)]]);
    assert_eq!(groups[11], vec![vec![q(1)], vec![q(2)]]);

    let mut grouped_target_count = 0usize;
    for gate in Gate::all().filter(|gate| gate.canonical_name() != "REPEAT") {
        let targets = representative_targets(gate.target_rule());
        let instruction = CircuitInstruction::new(
            gate,
            representative_arguments(gate.argument_rule()),
            targets.clone(),
            None,
        )
        .unwrap_or_else(|error| panic!("construct {}: {error}", gate.canonical_name()));
        let groups = instruction.target_groups();
        assert_eq!(
            groups
                .iter()
                .flat_map(|group| group.iter())
                .collect::<Vec<_>>(),
            targets.iter().collect::<Vec<_>>(),
            "{}",
            gate.canonical_name()
        );
        assert!(
            groups.iter().all(|group| !group.is_empty()),
            "{} returned an empty target group",
            gate.canonical_name()
        );
        match gate.target_group_kind() {
            GateTargetGroupKind::None => assert!(groups.is_empty()),
            GateTargetGroupKind::Singles => assert!(groups.iter().all(|group| group.len() == 1)),
            GateTargetGroupKind::Pairs => assert!(groups.iter().all(|group| group.len() == 2)),
            GateTargetGroupKind::PauliProducts | GateTargetGroupKind::AllTargets => {}
        }
        grouped_target_count += groups.iter().map(|group| group.len()).sum::<usize>();
    }
    assert!(grouped_target_count > 0);
}

#[test]
fn cq2_stim_format_instruction_print_and_eager_validation_match_stim() {
    let instruction = CircuitInstruction::new(
        Gate::from_name("X_ERROR").unwrap(),
        vec![0.5],
        vec![q(5)],
        None,
    )
    .expect("construct X_ERROR");
    let mut circuit = Circuit::new();
    circuit.append_instruction(instruction);
    assert_eq!(circuit.to_stim_string(), "X_ERROR(0.5) 5\n");
    assert!(
        CircuitInstruction::new(
            Gate::from_name("CX").unwrap(),
            vec![],
            vec![q(0), q(1), q(2)],
            None,
        )
        .is_err()
    );
}

fn instruction_groups(item: &CircuitItem) -> Vec<Vec<Target>> {
    let CircuitItem::Instruction(instruction) = item else {
        panic!("target-group fixture contains no repeat blocks")
    };
    instruction
        .target_groups()
        .into_iter()
        .map(<[Target]>::to_vec)
        .collect()
}

fn representative_arguments(rule: GateArgumentRule) -> Vec<f64> {
    match rule {
        GateArgumentRule::Exact(count) | GateArgumentRule::ProbabilityList(count) => {
            vec![0.0; count]
        }
        GateArgumentRule::Any | GateArgumentRule::AnyProbabilityList => Vec::new(),
        GateArgumentRule::OptionalProbability => vec![0.125],
        GateArgumentRule::UnsignedInteger => vec![0.0],
    }
}

fn representative_targets(rule: GateTargetRule) -> Vec<Target> {
    match rule {
        GateTargetRule::None => Vec::new(),
        GateTargetRule::AnySingleQubit
        | GateTargetRule::MeasurementQubits
        | GateTargetRule::MeasurementPads
        | GateTargetRule::QubitCoords => vec![q(0)],
        GateTargetRule::PlainPairs
        | GateTargetRule::ClassicalControlPairs
        | GateTargetRule::MeasurementPairs => vec![q(0), q(1)],
        GateTargetRule::RecOnly | GateTargetRule::RecOrPauli => vec![record(-1)],
        GateTargetRule::PauliProducts | GateTargetRule::PauliList => {
            vec![pauli(Pauli::X, 0)]
        }
    }
}

fn q(id: u32) -> Target {
    Target::qubit(QubitId::new(id).unwrap(), false)
}

fn inverted_q(id: u32) -> Target {
    Target::qubit(QubitId::new(id).unwrap(), true)
}

fn pauli(value: Pauli, id: u32) -> Target {
    Target::pauli(value, QubitId::new(id).unwrap(), false)
}

fn record(offset: i32) -> Target {
    Target::measurement_record(MeasureRecordOffset::try_new(offset).unwrap())
}
