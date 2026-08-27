#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "lean parity contracts use fixed Stim fixtures and exact semantic assertions"
)]

use std::collections::BTreeMap;
use std::ops::Bound;
use std::str::FromStr;

use stab_model::{
    Circuit, CircuitDetectorId, CircuitInstruction, CircuitItem, DemDetectorId, DemInstruction,
    DemInstructionKind, DemItem, DemObservableId, DemRepeatBlock, DemRepeatCount, DemTarget,
    DetectorErrorModel, Gate, MeasureRecordOffset, Pauli, Probability, QubitId, RepeatBlock,
    RepeatCount, Target,
};

#[test]
fn circuit_coordinates_and_repeat_aware_counts_match_stim() {
    let circuit = parse_circuit(
        "QUBIT_COORDS(1, 2) 0\n\
         M 0 1\n\
         MXX 4 5 6 7\n\
         MPAD 0 1 0\n\
         HERALDED_ERASE(0.25) 8 9\n\
         REPEAT 3 {\n\
             TICK\n\
             SHIFT_COORDS(10, 1)\n\
             QUBIT_COORDS(2) 2\n\
             MPP X0*X1 Z2\n\
             DETECTOR(4) rec[-1]\n\
             OBSERVABLE_INCLUDE(5) rec[-1]\n\
             CX sweep[7] 3\n\
         }\n",
    );

    assert_eq!(circuit.count_qubits(), 10);
    assert_eq!(circuit.count_measurements().expect("measurements"), 15);
    assert_eq!(circuit.count_detectors().expect("detectors"), 3);
    assert_eq!(circuit.count_observables().expect("observables"), 6);
    assert_eq!(circuit.count_ticks().expect("ticks"), 3);
    assert_eq!(circuit.count_sweep_bits().expect("sweep bits"), 8);
    assert_eq!(
        circuit.final_coordinate_shift().expect("coordinate shift"),
        [30.0, 3.0]
    );
    assert_eq!(
        circuit
            .final_qubit_coordinates()
            .expect("qubit coordinates"),
        BTreeMap::from([(qubit(0), vec![1.0, 2.0]), (qubit(2), vec![32.0, 3.0])])
    );

    let expected_detector_coordinates = BTreeMap::from([
        (CircuitDetectorId::new(0), vec![14.0]),
        (CircuitDetectorId::new(1), vec![24.0]),
        (CircuitDetectorId::new(2), vec![34.0]),
    ]);
    assert_eq!(
        circuit
            .detector_coordinates()
            .expect("detector coordinates"),
        expected_detector_coordinates
    );
    assert_eq!(
        circuit
            .detector_coordinates_for([CircuitDetectorId::new(2)])
            .expect("selected detector coordinate"),
        BTreeMap::from([(CircuitDetectorId::new(2), vec![34.0])])
    );
    assert!(
        circuit
            .coordinates_of_detector(CircuitDetectorId::new(3))
            .is_err()
    );

    let trillion_scale = parse_circuit(
        "REPEAT 1000000 {\n\
             REPEAT 1000000 {\n\
                 M 0\n\
                 DETECTOR rec[-1]\n\
             }\n\
         }\n",
    );
    assert_eq!(
        trillion_scale.count_measurements().expect("folded count"),
        1_000_000_000_000
    );
    assert_eq!(
        trillion_scale.count_detectors().expect("folded detectors"),
        1_000_000_000_000
    );
}

#[test]
fn circuit_mutation_and_value_lifecycle_preserves_semantics() {
    let mut circuit = parse_circuit("H[head] 0\n");
    let CircuitItem::Instruction(head) = &circuit.items()[0] else {
        panic!("head item must be an instruction")
    };
    assert_eq!(head.gate(), Gate::from_name("H").expect("H gate"));
    assert_eq!(head.args(), []);
    assert_eq!(head.targets(), [Target::qubit(qubit(0), false)]);
    assert_eq!(head.tag(), Some("head"));

    circuit.append_instruction(parse_instruction("H[head] 1\n"));
    circuit.append_instruction(parse_instruction("M[read] !0\n"));
    circuit.append_repeat_block(RepeatBlock::new(
        RepeatCount::try_new(2).expect("repeat count"),
        parse_circuit("X 2\n"),
        Some("loop".into()),
    ));
    assert_eq!(
        circuit.to_stim_string(),
        "H[head] 0 1\nM[read] !0\nREPEAT[loop] 2 {\n    X 2\n}\n"
    );
    assert_eq!(
        circuit
            .iter_flattened_instructions()
            .map(|instruction| instruction.gate().canonical_name())
            .collect::<Vec<_>>(),
        ["H", "M", "X", "X"]
    );
    assert_eq!(
        circuit
            .iter_flattened_instructions_reverse()
            .map(|instruction| instruction.gate().canonical_name())
            .collect::<Vec<_>>(),
        ["X", "X", "M", "H"]
    );
    assert_eq!(
        circuit.item_range(1..).expect("valid item range").count(),
        2
    );
    assert!(circuit.instruction_range(..3).is_err());
    assert!(
        circuit
            .item_range((Bound::Excluded(usize::MAX), Bound::Unbounded))
            .is_err()
    );

    let mut insertion_subject = parse_circuit("H 0\nS 0\n");
    insertion_subject
        .insert_instruction(1, parse_instruction("H 1\n"))
        .expect("insert and fuse instruction");
    assert_eq!(insertion_subject.to_stim_string(), "H 0 1\nS 0\n");
    insertion_subject
        .insert_circuit(1, &parse_circuit("X 2\nS 3\n"))
        .expect("insert and fuse circuit boundaries");
    assert_eq!(insertion_subject.to_stim_string(), "H 0 1\nX 2\nS 3 0\n");
    assert_eq!(
        insertion_subject.pop_item(1).expect("remove inserted X"),
        CircuitItem::Instruction(parse_instruction("X 2\n"))
    );
    assert_eq!(
        insertion_subject.pop_last_item().expect("remove fused S"),
        CircuitItem::Instruction(parse_instruction("S 3 0\n"))
    );

    let suffix = parse_circuit("M 6\n");
    let concatenated = circuit.concatenated(&suffix);
    let mut appended = circuit.clone();
    appended.append_circuit(&suffix);
    assert_eq!(appended, concatenated);
    assert_eq!(suffix.to_stim_string(), "M 6\n");
    assert!(concatenated.to_stim_string().ends_with("M 6\n"));
    assert_eq!(
        suffix
            .repeated(0)
            .expect("zero repetitions")
            .to_stim_string(),
        ""
    );
    assert_eq!(suffix.repeated(1).expect("one repetition"), suffix);
    assert_eq!(
        suffix
            .repeated(2)
            .expect("two repetitions")
            .to_stim_string(),
        "REPEAT 2 {\n    M 6\n}\n"
    );
    let mut repeated_in_place = suffix.clone();
    repeated_in_place
        .repeat_in_place(2)
        .expect("in-place repetition");
    assert_eq!(
        repeated_in_place,
        suffix.repeated(2).expect("copy repetition")
    );
    assert!(repeated_in_place.repeated(u64::MAX).is_err());

    let rec = record(-1);
    let largest_observable_id = 18_446_744_073_709_549_568_u64;
    let observable = CircuitInstruction::new(
        Gate::from_name("OBSERVABLE_INCLUDE").expect("observable gate"),
        vec![f64::from_bits(0x43ef_ffff_ffff_ffff)],
        vec![rec.clone()],
        None,
    )
    .expect("largest exactly represented observable id");
    assert_eq!(
        observable
            .observable_id_argument()
            .expect("valid observable argument")
            .expect("observable id")
            .get(),
        largest_observable_id
    );
    let overflowing_observable = CircuitInstruction::new(
        Gate::from_name("OBSERVABLE_INCLUDE").expect("observable gate"),
        vec![f64::from_bits(0x43f0_0000_0000_0000)],
        vec![rec],
        None,
    )
    .expect("integer-shaped observable argument");
    assert!(overflowing_observable.observable_id_argument().is_err());

    let before_failure = circuit.clone();
    assert!(circuit.append_from_stim_text("UNKNOWN 0\n").is_err());
    assert_eq!(circuit, before_failure);
    assert!(
        circuit
            .insert_item(
                circuit.len() + 1,
                CircuitItem::Instruction(parse_instruction("X 0\n")),
            )
            .is_err()
    );
    assert_eq!(circuit, before_failure);

    let CircuitItem::RepeatBlock(repeat) = circuit.pop_last_item().expect("remove repeat") else {
        panic!("last item must be the repeat block")
    };
    assert_eq!(repeat.repeat_count().get(), 2);
    assert_eq!(repeat.tag(), Some("loop"));
    assert_eq!(repeat.body().to_stim_string(), "X 2\n");

    circuit.clear();
    assert!(circuit.is_empty());
    assert_eq!(circuit.to_stim_string(), "");
}

#[test]
fn circuit_syntax_round_trips_to_canonical_stim_text() {
    let source = concat!(
        "# ignored comment\r\n",
        "h[tag \\B\\C\\r\\n] 0\r\n",
        "CNOT 0 1 # alias\r\n",
        "X_ERROR(.125) 2\r\n",
        "MPP X0*!Y1*Z2\r\n",
        "DETECTOR(1,2) rec[-1]\r\n",
        "REPEAT[outer] 2 {\r\n",
        "  REPEAT[inner] 3 {\r\n",
        "    m 0\r\n",
        "  }\r\n",
        "}\r\n",
    );
    let canonical = concat!(
        "H[tag \\B\\C\\r\\n] 0\n",
        "CX 0 1\n",
        "X_ERROR(0.125) 2\n",
        "MPP X0*!Y1*Z2\n",
        "DETECTOR(1, 2) rec[-1]\n",
        "REPEAT[outer] 2 {\n",
        "    REPEAT[inner] 3 {\n",
        "        M 0\n",
        "    }\n",
        "}\n",
    );

    let parsed = parse_circuit(source);
    assert_eq!(parsed.to_stim_string(), canonical);
    assert_eq!(parsed.to_stim_bytes(), canonical.as_bytes());
    assert_eq!(parse_circuit(canonical), parsed);

    let mut appended = parse_circuit("H[tag] 3\n");
    appended
        .append_from_stim_text("H[tag] 4\nREPEAT[loop] 2 {\nM 0\n}\n")
        .expect("append parsed circuit text");
    appended
        .append_from_stim_program_text("S 5\n")
        .expect("append through Stim API alias");
    assert_eq!(
        appended.to_stim_string(),
        "H[tag] 3 4\nREPEAT[loop] 2 {\n    M 0\n}\nS 5\n"
    );
    let before_rejected_append = appended.clone();
    assert!(appended.append_from_stim_text("H 6\nUNKNOWN 7\n").is_err());
    assert_eq!(appended, before_rejected_append);

    for malformed in [
        "REPEAT 0 {\nM 0\n}\n",
        "REPEAT 2 {\nREPEAT 0 {\nM 0\n}\n}\n",
        "H 0 garbage\n",
        "X_ERROR(1.1) 0\n",
        "REPEAT 2 {\nM 0\n",
    ] {
        assert!(
            Circuit::from_stim_str(malformed).is_err(),
            "accepted malformed Stim text {malformed:?}"
        );
    }
}

#[test]
fn circuit_target_grammar_and_gate_roles_are_enforced() {
    let target_cases = [
        ("0", Target::qubit(qubit(0), false)),
        ("!1", Target::qubit(qubit(1), true)),
        ("rec[-7]", record(-7)),
        ("sweep[5]", Target::sweep_bit(5)),
        ("X2", Target::pauli(Pauli::X, qubit(2), false)),
        ("!Y3", Target::pauli(Pauli::Y, qubit(3), true)),
        ("Z4", Target::pauli(Pauli::Z, qubit(4), false)),
        ("*", Target::combiner()),
    ];
    for (source, expected) in target_cases {
        let parsed = Target::from_str(source).expect("valid target fixture");
        assert_eq!(parsed, expected, "source {source}");
        assert_eq!(parsed.to_string(), source);
    }
    assert!(
        Target::from_str("rec[-0]")
            .expect("negative-zero record target")
            .measurement_record_offset()
            .expect("negative-zero offset")
            .is_negative_zero()
    );

    for malformed in [
        "",
        "+1",
        "16777216",
        "X16777216",
        "rec[0]",
        "rec[-16777216]",
        "sweep[+1]",
        "sweep[16777216]",
    ] {
        assert!(Target::from_str(malformed).is_err(), "target {malformed:?}");
    }
    assert_eq!(
        Target::from_str("!Z9")
            .expect("invertible Pauli")
            .try_inverted()
            .expect("toggle inversion")
            .to_string(),
        "Z9"
    );
    for noninvertible in [record(-1), Target::sweep_bit(0), Target::combiner()] {
        assert!(noninvertible.try_inverted().is_err());
    }

    for accepted in [
        "H 0\n",
        "M !0\n",
        "CX sweep[0] 1\n",
        "CX rec[-1] 1\n",
        "CX rec[-1] rec[-2]\n",
        "XCZ 1 rec[-1]\n",
        "MPP X0*!Y1 Z2\n",
        "MPAD 0 1\n",
        "DETECTOR rec[-1]\n",
    ] {
        Circuit::from_stim_str(accepted)
            .unwrap_or_else(|error| panic!("rejected valid gate targets {accepted:?}: {error}"));
    }
    for rejected in [
        "H !0\n",
        "H rec[-1]\n",
        "H X0\n",
        "CX 0\n",
        "CX 0 0\n",
        "MPP *X0\n",
        "MPP X0**Y1\n",
        "MPAD 2\n",
        "MPAD sweep[0]\n",
        "DETECTOR 0\n",
    ] {
        assert!(
            Circuit::from_stim_str(rejected).is_err(),
            "accepted invalid gate targets {rejected:?}"
        );
    }

    let mpp = parse_instruction("MPP X0*!Y1 Z2\n");
    assert_eq!(
        mpp.target_groups()
            .into_iter()
            .map(<[Target]>::to_vec)
            .collect::<Vec<_>>(),
        [
            vec![
                Target::pauli(Pauli::X, qubit(0), false),
                Target::combiner(),
                Target::pauli(Pauli::Y, qubit(1), true),
            ],
            vec![Target::pauli(Pauli::Z, qubit(2), false)],
        ]
    );
}

#[test]
fn dem_counts_coordinates_and_value_lifecycle_preserves_semantics() {
    let mut model = parse_dem(
        "error[head](0.25) D0 L2 ^ D1\n\
         detector(1, 2) D0\n\
         shift_detectors(10, 1) 1\n\
         repeat[loop] 3 {\n\
             error(0.5) D0\n\
             detector(2) D0\n\
             shift_detectors(5) 1\n\
         }\n\
         logical_observable L4\n",
    );

    assert_eq!(model.count_errors().expect("error count"), 4);
    assert_eq!(model.count_detectors().expect("detector count"), 4);
    assert_eq!(model.count_observables().expect("observable count"), 5);
    assert_eq!(model.total_detector_shift().expect("detector shift"), 4);
    assert_eq!(
        model.final_coordinate_shift().expect("coordinate shift"),
        [25.0, 1.0]
    );
    assert_eq!(
        model.detector_coordinates().expect("detector coordinates"),
        BTreeMap::from([
            (dem_detector(0), vec![1.0, 2.0]),
            (dem_detector(1), vec![12.0]),
            (dem_detector(2), vec![17.0]),
            (dem_detector(3), vec![22.0]),
        ])
    );
    assert_eq!(
        model
            .detector_coordinates_for([dem_detector(2)])
            .expect("selected detector coordinate"),
        BTreeMap::from([(dem_detector(2), vec![17.0])])
    );
    assert!(model.coordinates_of_detector(dem_detector(4)).is_err());

    let DemItem::Instruction(error) = &model.items()[0] else {
        panic!("first DEM item must be an error")
    };
    assert_eq!(error.kind(), DemInstructionKind::Error);
    assert_eq!(error.args(), [0.25]);
    assert_eq!(error.tag(), Some("head"));
    assert_eq!(
        error.target_groups(),
        [
            &[
                DemTarget::relative_detector(0).expect("D0"),
                DemTarget::logical_observable(2).expect("L2")
            ][..],
            &[DemTarget::relative_detector(1).expect("D1")][..],
        ]
    );
    let DemItem::RepeatBlock(repeat) = &model.items()[3] else {
        panic!("fourth DEM item must be a repeat")
    };
    assert_eq!(repeat.repeat_count().get(), 3);
    assert_eq!(repeat.tag(), Some("loop"));
    assert_eq!(repeat.body().items().len(), 3);
    assert_eq!(model.item_range(1..3).expect("valid DEM range").count(), 2);
    assert!(model.instruction_range(..4).is_err());
    assert!(
        model
            .item_range((Bound::Excluded(usize::MAX), Bound::Unbounded))
            .is_err()
    );

    for (target, text) in [
        (DemTarget::relative_detector(5).expect("D5"), "D5"),
        (DemTarget::logical_observable(6).expect("L6"), "L6"),
        (DemTarget::separator(), "^"),
    ] {
        assert_eq!(DemTarget::from_str(text).expect("valid DEM target"), target);
        assert_eq!(target.to_string(), text);
    }
    assert!(DemDetectorId::try_new(1_u64 << 62).is_err());
    assert!(DemObservableId::try_new(u64::from(u32::MAX) + 1).is_err());
    for malformed in ["", "5", "d5", "D-1", "L4294967296"] {
        assert!(DemTarget::from_str(malformed).is_err(), "{malformed:?}");
    }
    assert!(
        DemInstruction::new(
            DemInstructionKind::Error,
            vec![0.25],
            vec![DemTarget::separator()],
            None,
        )
        .is_err()
    );
    assert!(
        DemInstruction::new(
            DemInstructionKind::Detector,
            vec![f64::INFINITY],
            vec![DemTarget::relative_detector(0).expect("D0")],
            None,
        )
        .is_err()
    );
    let mut overflowing_shift = DetectorErrorModel::new();
    overflowing_shift.push_repeat_block(DemRepeatBlock::new(
        DemRepeatCount::new(u64::MAX),
        parse_dem("shift_detectors 2\n"),
        None,
    ));
    assert!(overflowing_shift.total_detector_shift().is_err());

    model
        .append_from_dem_text("detector[tail](9) D4\n")
        .expect("append valid DEM text");
    let before_failure = model.clone();
    assert!(model.append_from_dem_text("detector L0\n").is_err());
    assert_eq!(model, before_failure);

    let mut built = DetectorErrorModel::new();
    built.push_instruction(
        DemInstruction::error(
            Probability::try_new(0.125).expect("probability"),
            vec![DemTarget::relative_detector(0).expect("D0")],
            Some("built".into()),
        )
        .expect("error instruction"),
    );
    built.push_repeat_block(DemRepeatBlock::new(
        DemRepeatCount::new(2),
        parse_dem("shift_detectors 1\n"),
        Some("twice".into()),
    ));
    assert_eq!(
        built.to_dem_string(),
        "error[built](0.125) D0\nrepeat[twice] 2 {\n    shift_detectors 1\n}\n"
    );
    built.clear();
    assert!(built.is_empty());
}

#[test]
fn dem_syntax_round_trips_to_canonical_text() {
    let source = concat!(
        "# ignored\r\n",
        "ERROR[edge \\B\\C\\r\\n](.125) D0 L1 ^ D2\r\n",
        "DETECTOR[coord](1,2) D3\r\n",
        "LOGICAL_OBSERVABLE[obs] L4\r\n",
        "SHIFT_DETECTORS[shift](5,6) 7\r\n",
        "REPEAT[outer] 2 {\r\n",
        "  REPEAT[inner] 3 {\r\n",
        "    error(.5) D0\r\n",
        "  }\r\n",
        "}\r\n",
    );
    let canonical = concat!(
        "error[edge \\B\\C\\r\\n](0.125) D0 L1 ^ D2\n",
        "detector[coord](1, 2) D3\n",
        "logical_observable[obs] L4\n",
        "shift_detectors[shift](5, 6) 7\n",
        "repeat[outer] 2 {\n",
        "    repeat[inner] 3 {\n",
        "        error(0.5) D0\n",
        "    }\n",
        "}\n",
    );

    let parsed = parse_dem(source);
    assert_eq!(parsed.to_dem_string(), canonical);
    assert_eq!(parsed.to_dem_bytes(), canonical.as_bytes());
    assert_eq!(parse_dem(canonical), parsed);

    for malformed in [
        "error(1.1) D0\n",
        "error(0.1) D-1\n",
        "detector L0\n",
        "logical_observable D0\n",
        "shift_detectors -1\n",
        "repeat 2 {\nerror(0.1) D0\n",
    ] {
        assert!(
            DetectorErrorModel::from_dem_str(malformed).is_err(),
            "accepted malformed DEM text {malformed:?}"
        );
    }
}

fn parse_circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("valid circuit fixture")
}

fn parse_instruction(text: &str) -> CircuitInstruction {
    match parse_circuit(text)
        .items()
        .first()
        .expect("one circuit item")
    {
        CircuitItem::Instruction(instruction) => instruction.clone(),
        CircuitItem::RepeatBlock(_) => panic!("fixture must contain an instruction"),
    }
}

fn parse_dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).expect("valid DEM fixture")
}

fn qubit(id: u32) -> QubitId {
    QubitId::new(id).expect("fixture qubit id")
}

fn record(offset: i32) -> Target {
    Target::measurement_record(MeasureRecordOffset::try_new(offset).expect("record offset"))
}

fn dem_detector(id: u64) -> DemDetectorId {
    DemDetectorId::try_new(id).expect("fixture detector id")
}
