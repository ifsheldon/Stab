#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "canonical owner tests use fixed compatibility fixtures and inspect exact values"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;
use std::str::FromStr;

use stab_model::{
    Circuit, CircuitDetectorId, CircuitInstruction, CircuitItem, DemDetectorId, DemInstruction,
    DemInstructionKind, DemItem, DemObservableId, DemRepeatBlock, DemRepeatCount, DemTarget,
    DetectorErrorModel, Estimate, Gate, MeasureRecordOffset, ParseLimits, Probability, QubitId,
    RepeatBlock, RepeatCount, RepeatNestingLimit, SourceLineLimit, Target,
};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("valid circuit fixture")
}

fn dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).expect("valid DEM fixture")
}

fn instruction(text: &str) -> CircuitInstruction {
    match circuit(text).items().first().expect("one circuit item") {
        CircuitItem::Instruction(instruction) => instruction.clone(),
        CircuitItem::RepeatBlock(_) => panic!("expected instruction fixture"),
    }
}

fn item_name(item: &CircuitItem) -> &'static str {
    match item {
        CircuitItem::Instruction(instruction) => instruction.gate().canonical_name(),
        CircuitItem::RepeatBlock(_) => "REPEAT",
    }
}

#[test]
fn pf1_circuit_concat() {
    let mut left = circuit("H[tag] 0\n");
    let right = circuit("H[tag] 1\nM 0\n");

    let concatenated = left.concatenated(&right);
    left.append_circuit(&right);

    assert_eq!(left.to_stim_string(), "H[tag] 0 1\nM 0\n");
    assert_eq!(concatenated, left);
    assert_eq!(right.to_stim_string(), "H[tag] 1\nM 0\n");
}

#[test]
fn pf1_circuit_iterators() {
    let circuit = circuit("H 0\nREPEAT 2 {\n    M 0\n    X 1\n}\nS 2\n");
    assert_eq!(
        circuit.iter_items().map(item_name).collect::<Vec<_>>(),
        ["H", "REPEAT", "S"]
    );
    assert_eq!(
        circuit
            .item_range(1..)
            .expect("valid range")
            .map(item_name)
            .collect::<Vec<_>>(),
        ["REPEAT", "S"]
    );
    assert!(circuit.instruction_range(..2).is_err());
    assert!(
        circuit
            .item_range((Bound::Excluded(usize::MAX), Bound::Unbounded))
            .is_err()
    );

    let forward = circuit
        .iter_flattened_instructions()
        .map(|instruction| instruction.gate().canonical_name())
        .collect::<Vec<_>>();
    let reverse = circuit
        .iter_flattened_instructions_reverse()
        .map(|instruction| instruction.gate().canonical_name())
        .collect::<Vec<_>>();
    assert_eq!(forward, ["H", "M", "X", "M", "X", "S"]);
    assert_eq!(reverse, ["S", "X", "M", "X", "M", "H"]);
}

#[test]
fn pf1_circuit_detector_coordinates() {
    let circuit = circuit(
        "M 0\n\
         DETECTOR(1, 2) rec[-1]\n\
         SHIFT_COORDS(10, 20)\n\
         DETECTOR(3) rec[-1]\n",
    );
    let d0 = CircuitDetectorId::new(0);
    let d1 = CircuitDetectorId::new(1);
    assert_eq!(
        circuit.detector_coordinates().expect("all coordinates"),
        BTreeMap::from([(d0, vec![1.0, 2.0]), (d1, vec![13.0])])
    );
    assert_eq!(
        circuit
            .detector_coordinates_for([d1])
            .expect("selected coordinates"),
        BTreeMap::from([(d1, vec![13.0])])
    );
    assert_eq!(
        circuit
            .coordinates_of_detector(d0)
            .expect("single detector"),
        vec![1.0, 2.0]
    );
    assert!(
        circuit
            .coordinates_of_detector(CircuitDetectorId::new(2))
            .is_err()
    );
}

#[test]
fn pf1_circuit_insert_pop() {
    let mut subject = circuit("H 0\nS 0\n");
    subject
        .insert_instruction(1, instruction("H 1\n"))
        .expect("insert and fuse");
    assert_eq!(subject.to_stim_string(), "H 0 1\nS 0\n");

    let inserted = circuit("X 2\nS 3\n");
    subject
        .insert_circuit(1, &inserted)
        .expect("insert circuit at item boundary");
    assert_eq!(subject.to_stim_string(), "H 0 1\nX 2\nS 3 0\n");
    assert_eq!(
        subject.pop_item(1).expect("pop middle item"),
        CircuitItem::Instruction(instruction("X 2\n"))
    );
    assert_eq!(
        subject.pop_last_item().expect("pop last item"),
        CircuitItem::Instruction(instruction("S 3 0\n"))
    );
    assert_eq!(subject.to_stim_string(), "H 0 1\n");
    assert!(
        subject
            .insert_item(2, CircuitItem::Instruction(instruction("X 0\n")))
            .is_err()
    );
}

#[test]
fn cq2_dem_counts_and_shifts_contract() {
    let model = dem("shift_detectors 50\n\
         repeat 3 {\n\
             detector(1) D0\n\
             error(0.125) D0 D2 L6\n\
             shift_detectors(2, 3) 4\n\
         }\n\
         logical_observable L5\n");
    assert_eq!(model.total_detector_shift().expect("shift"), 62);
    assert_eq!(model.count_detectors().expect("detectors"), 61);
    assert_eq!(model.count_observables().expect("observables"), 7);
    assert_eq!(model.count_errors().expect("errors"), 3);
    assert_eq!(
        model.final_coordinate_shift().expect("coordinate shift"),
        [6.0, 9.0]
    );
}

#[test]
fn cq2_dem_model_parse_print_tag_newline_contract() {
    let source = concat!(
        "# comment\r\n",
        "ERROR[first](.125) D0\r\n",
        "REPEAT[outer] 2 {\r\n",
        "SHIFT_DETECTORS[step](1.5,3) 10\r\n",
        "}\r\n",
        "LOGICAL_OBSERVABLE[obs] L0\r\n",
    );
    let expected = concat!(
        "error[first](0.125) D0\n",
        "repeat[outer] 2 {\n",
        "    shift_detectors[step](1.5, 3) 10\n",
        "}\n",
        "logical_observable[obs] L0\n",
    );
    let model = dem(source);
    assert_eq!(model.to_dem_string(), expected);
    assert_eq!(model.to_string(), expected);
    assert_eq!(dem(expected), model);
}

#[test]
fn cq2_circuit_api_repetition() {
    let source = circuit("Y 3\nM 4\n");
    assert_eq!(source.repeated(0).expect("zero").to_stim_string(), "");
    assert_eq!(
        source.repeated(1).expect("one").to_stim_string(),
        "Y 3\nM 4\n"
    );
    assert_eq!(
        source.repeated(2).expect("two").to_stim_string(),
        "REPEAT 2 {\n    Y 3\n    M 4\n}\n"
    );

    let mut in_place = source;
    in_place.repeat_in_place(3).expect("in-place repeat");
    assert_eq!(
        in_place.to_stim_string(),
        "REPEAT 3 {\n    Y 3\n    M 4\n}\n"
    );
    assert!(
        circuit("REPEAT 1234567890123456789 {\n    H 0\n}\n")
            .repeated(16)
            .is_err()
    );
}

#[test]
fn cq2_circuit_api_final_coordinate_shift() {
    let circuit = circuit(
        "REPEAT 1000 {\n\
             REPEAT 2000 {\n\
                 REPEAT 3000 {\n\
                     SHIFT_COORDS(0, 0, 1)\n\
                 }\n\
                 SHIFT_COORDS(1)\n\
             }\n\
             SHIFT_COORDS(0, 1)\n\
         }\n",
    );
    assert_eq!(
        circuit.final_coordinate_shift().expect("folded shift"),
        [2_000_000.0, 1000.0, 6_000_000_000.0]
    );
}

#[test]
fn cq2_stim_format_append_program_alias() {
    let mut circuit = Circuit::new();
    circuit
        .append_from_stim_program_text("H[test] 0\ncnot[test2] 1 2\n")
        .expect("append program text");
    assert_eq!(circuit.to_stim_string(), "H[test] 0\nCX[test2] 1 2\n");
}

#[test]
fn cq2_stim_format_append_text() {
    let mut circuit = circuit("H[tag] 0\n");
    circuit
        .append_from_stim_text(
            "H[tag] 1\nREPEAT[loop] 2 {\n    M[meas] 0\n    DETECTOR[det] rec[-1]\n}\n",
        )
        .expect("append valid text");
    assert_eq!(
        circuit.to_stim_string(),
        concat!(
            "H[tag] 0 1\n",
            "REPEAT[loop] 2 {\n",
            "    M[meas] 0\n",
            "    DETECTOR[det] rec[-1]\n",
            "}\n"
        )
    );
    let before = circuit.clone();
    assert!(circuit.append_from_stim_text("UNKNOWN 2\n").is_err());
    assert_eq!(circuit, before);
}

#[test]
fn cq2_circuit_api_final_qubit_coordinates() {
    let circuit = circuit(
        "QUBIT_COORDS(1, 2, 3) 0\n\
         SHIFT_COORDS(5)\n\
         REPEAT 3 {\n\
             SHIFT_COORDS(10, 1)\n\
             QUBIT_COORDS(7) 1\n\
         }\n\
         QUBIT_COORDS(0, 0) 2\n",
    );
    assert_eq!(
        circuit.final_qubit_coordinates().expect("coordinates"),
        BTreeMap::from([
            (QubitId::new(0).expect("q0"), vec![1.0, 2.0, 3.0]),
            (QubitId::new(1).expect("q1"), vec![42.0, 3.0]),
            (QubitId::new(2).expect("q2"), vec![35.0, 3.0]),
        ])
    );
}

#[test]
fn cq2_circuit_api_qubit_count() {
    assert_eq!(Circuit::new().count_qubits(), 0);
    assert_eq!(circuit("H 5\nMPAD 1\n").count_qubits(), 6);
    let folded = circuit(
        "REPEAT 999999 {\n\
             REPEAT 999999 {\n\
                 X 1\n\
                 REPEAT 999999 {\n\
                     Y 2\n\
                 }\n\
             }\n\
         }\n",
    );
    assert_eq!(folded.count_qubits(), 3);
}

#[test]
fn cq2_circuit_api_clear() {
    let mut circuit = circuit("H[tag] 0\nM[measure] 0\nDETECTOR[det] rec[-1]\n");
    circuit.clear();
    assert!(circuit.is_empty());
    assert_eq!(circuit.len(), 0);
    assert_eq!(circuit.to_stim_string(), "");
    assert_eq!(circuit.count_measurements().expect("measurements"), 0);
    assert_eq!(circuit.count_detectors().expect("detectors"), 0);
}

#[test]
fn cq2_dem_model_value_mutation_repeat_contract() {
    let mut body = DetectorErrorModel::new();
    body.push_instruction(
        DemInstruction::shift_detectors(Vec::new(), 3, Some("step".to_string())).expect("shift"),
    );
    let repeat = DemRepeatBlock::new(DemRepeatCount::new(5), body.clone(), Some("loop".into()));
    assert_eq!(repeat.repeat_count().get(), 5);
    assert_eq!(repeat.body(), &body);
    assert_eq!(repeat.tag(), Some("loop"));

    let mut model = DetectorErrorModel::new();
    model.push_instruction(
        DemInstruction::error(
            Probability::try_new(0.25).expect("probability"),
            vec![DemTarget::relative_detector(0).expect("D0")],
            Some("head".into()),
        )
        .expect("error"),
    );
    model.push_repeat_block(repeat.clone());
    model
        .append_from_dem_text("logical_observable[tail] L2\n")
        .expect("append");
    assert_eq!(model.len(), 3);
    assert!(model.items()[0].as_instruction().is_some());
    assert_eq!(model.items()[1].as_repeat_block(), Some(&repeat));
    assert!(model.instruction_range(..2).is_err());
    let before = model.clone();
    assert!(model.append_from_dem_text("detector L0\n").is_err());
    assert_eq!(model, before);
    model.clear();
    assert!(model.is_empty());
}

#[test]
fn cq2_stim_format_measure_record_offset() {
    for accepted in [-1, -2, -((1 << 24) - 1)] {
        let offset = MeasureRecordOffset::try_new(accepted).expect("accepted offset");
        assert_eq!(offset.get(), accepted);
        assert_eq!(offset.stim_text().to_string(), accepted.to_string());
    }
    assert!(MeasureRecordOffset::try_new(0).is_err());
    assert!(MeasureRecordOffset::try_new(1).is_err());
    assert!(MeasureRecordOffset::try_new(-(1 << 24)).is_err());
}

#[test]
fn cq2_dem_coordinate_query_contract() {
    let model = dem("detector(1, 2, 3) D0\n\
         shift_detectors 1\n\
         repeat 3 {\n\
             detector(2) D0\n\
             shift_detectors(5) 1\n\
         }\n");
    let d0 = DemDetectorId::try_new(0).expect("D0");
    let d1 = DemDetectorId::try_new(1).expect("D1");
    let d3 = DemDetectorId::try_new(3).expect("D3");
    assert_eq!(
        model.detector_coordinates().expect("all coordinates"),
        BTreeMap::from([
            (d0, vec![1.0, 2.0, 3.0]),
            (d1, vec![2.0]),
            (DemDetectorId::try_new(2).expect("D2"), vec![7.0]),
            (d3, vec![12.0]),
        ])
    );
    assert_eq!(
        model.detector_coordinates_for([d1, d3]).expect("selected"),
        BTreeMap::from([(d1, vec![2.0]), (d3, vec![12.0])])
    );
    assert!(
        model
            .coordinates_of_detector(DemDetectorId::try_new(4).expect("D4"))
            .is_err()
    );
}

#[test]
fn cq2_dem_target_value_and_parse_contract() {
    let detector = DemDetectorId::try_new(5).expect("D5");
    let observable = DemObservableId::try_new(6).expect("L6");
    assert_eq!(detector.get(), 5);
    assert_eq!(observable.get(), 6);
    for (target, text) in [
        (DemTarget::relative_detector(5).expect("D5"), "D5"),
        (DemTarget::logical_observable(6).expect("L6"), "L6"),
        (DemTarget::separator(), "^"),
    ] {
        assert_eq!(target.to_string(), text);
        assert_eq!(DemTarget::from_str(text).expect("parse target"), target);
    }
    assert!(DemDetectorId::try_new(1_u64 << 62).is_err());
    assert!(DemObservableId::try_new(u64::from(u32::MAX) + 1).is_err());
    for rejected in ["", "5", "d5", "D-1", "L4294967296"] {
        assert!(DemTarget::from_str(rejected).is_err(), "{rejected:?}");
    }
}

#[test]
fn cq2_circuit_api_flattened_iterator_values() {
    let circuit = circuit("H 0\nREPEAT 2 {\n    M 0\n    X 1\n}\n");
    let mut forward = circuit.iter_flattened_instructions();
    let mut clone = forward.clone();
    assert_eq!(forward.next(), clone.next());
    assert_eq!(
        forward
            .map(|instruction| instruction.gate().canonical_name())
            .collect::<Vec<_>>(),
        ["M", "X", "M", "X"]
    );
    assert_eq!(
        clone
            .map(|instruction| instruction.gate().canonical_name())
            .collect::<Vec<_>>(),
        ["M", "X", "M", "X"]
    );
    assert_eq!(
        circuit
            .iter_flattened_instructions_reverse()
            .map(|instruction| instruction.gate().canonical_name())
            .collect::<Vec<_>>(),
        ["X", "M", "X", "M", "H"]
    );
}

#[test]
fn cq2_circuit_api_instruction_value() {
    let instruction = CircuitInstruction::new(
        Gate::from_name("M").expect("M"),
        vec![0.125],
        vec![
            Target::from_str("!0").expect("inverted qubit"),
            Target::from_str("1").expect("qubit"),
        ],
        Some("measure".into()),
    )
    .expect("valid instruction");
    assert_eq!(instruction.gate().canonical_name(), "M");
    assert_eq!(instruction.args(), [0.125]);
    assert_eq!(instruction.tag(), Some("measure"));
    assert_eq!(instruction.targets().len(), 2);
    assert_eq!(
        instruction
            .probability_argument()
            .expect("valid probability argument")
            .map(Probability::get),
        Some(0.125)
    );
    assert_eq!(instruction.target_groups().len(), 2);
    assert!(
        CircuitInstruction::new(
            Gate::from_name("H").expect("H"),
            vec![0.5],
            vec![Target::from_str("0").expect("q0")],
            None,
        )
        .is_err()
    );
}

#[test]
fn a2_circuit_parse_policy_admission() {
    let limits = ParseLimits::new(
        SourceLineLimit::new(2),
        RepeatNestingLimit::try_new(2).expect("depth"),
    );
    assert!(Circuit::from_stim_str_with_limits("H 0\nM 0\n", limits).is_ok());
    assert!(Circuit::from_stim_str_with_limits("H 0\nM 0\nX 0\n", limits).is_err());
    assert!(
        Circuit::from_stim_str_with_limits(
            "REPEAT 2 {\n    REPEAT 2 {\n        H 0\n    }\n}\n",
            limits.with_source_line_limit(SourceLineLimit::new(16)),
        )
        .is_ok()
    );
}

#[test]
fn a2_dem_parse_policy_admission() {
    let limits = ParseLimits::new(
        SourceLineLimit::new(2),
        RepeatNestingLimit::try_new(2).expect("depth"),
    );
    assert!(
        DetectorErrorModel::from_dem_str_with_limits("error(0.1) D0\ndetector D0\n", limits)
            .is_ok()
    );
    assert!(
        DetectorErrorModel::from_dem_str_with_limits(
            "error(0.1) D0\ndetector D0\nlogical_observable L0\n",
            limits,
        )
        .is_err()
    );
}

#[test]
fn cq2_circuit_api_counts() {
    let circuit = circuit(
        "M 0 1\n\
         REPEAT 100 {\n\
             TICK\n\
             M 2\n\
             DETECTOR rec[-1]\n\
             OBSERVABLE_INCLUDE(2) rec[-1]\n\
             CY sweep[77] 3\n\
         }\n",
    );
    assert_eq!(circuit.len(), 2);
    assert_eq!(circuit.count_qubits(), 4);
    assert_eq!(circuit.count_measurements().expect("measurements"), 102);
    assert_eq!(circuit.count_detectors().expect("detectors"), 100);
    assert_eq!(circuit.count_observables().expect("observables"), 3);
    assert_eq!(circuit.count_ticks().expect("ticks"), 100);
    assert_eq!(circuit.count_sweep_bits().expect("sweep bits"), 78);
}

#[test]
fn cq2_gate_registry_contract() {
    assert_eq!(Gate::all().len(), 81);
    let names = Gate::all()
        .map(|gate| gate.canonical_name())
        .collect::<Vec<_>>();
    assert_eq!(names.iter().copied().collect::<BTreeSet<_>>().len(), 81);
    assert!(names.contains(&"I"));
    assert!(names.contains(&"MPAD"));
    assert_eq!(
        Gate::from_name("CNOT").expect("alias").canonical_name(),
        "CX"
    );
}

#[test]
fn cq2_dem_instruction_target_groups() {
    let model = dem("error(0.1) D0 ^ D2 L0 ^ D1 D2 D3\nerror(0.2) D4\nerror(0.3)\n");
    let instructions = model
        .items()
        .iter()
        .filter_map(DemItem::as_instruction)
        .collect::<Vec<_>>();
    assert_eq!(
        instructions[0]
            .target_groups()
            .into_iter()
            .map(<[DemTarget]>::to_vec)
            .collect::<Vec<_>>(),
        [
            vec![DemTarget::relative_detector(0).expect("D0")],
            vec![
                DemTarget::relative_detector(2).expect("D2"),
                DemTarget::logical_observable(0).expect("L0"),
            ],
            vec![
                DemTarget::relative_detector(1).expect("D1"),
                DemTarget::relative_detector(2).expect("D2"),
                DemTarget::relative_detector(3).expect("D3"),
            ],
        ]
    );
    assert_eq!(instructions[2].target_groups(), vec![&[][..]]);
}

#[test]
fn cq2_circuit_api_value_items_repeat_blocks() {
    let circuit = circuit("H[tag] 0\nREPEAT[loop] 2 {\n    M 0\n}\n");
    let mut items = circuit.items().to_vec();
    let CircuitItem::Instruction(first_instruction) = &mut items[0] else {
        panic!("first item is an instruction")
    };
    *first_instruction = instruction("X 1\n");
    let CircuitItem::RepeatBlock(repeat) = &items[1] else {
        panic!("second item is a repeat block")
    };
    assert_eq!(repeat.repeat_count().get(), 2);
    assert_eq!(repeat.tag(), Some("loop"));
    assert_eq!(repeat.body().to_stim_string(), "M 0\n");
    assert_eq!(
        circuit.to_stim_string(),
        "H[tag] 0\nREPEAT[loop] 2 {\n    M 0\n}\n"
    );
}

#[test]
fn cq2_circuit_api_append_items() {
    let mut subject = circuit("H[tag] 0\n");
    subject.append_instruction(instruction("H[tag] 1\n"));
    subject.append_instruction(instruction("M 0\n"));
    subject.append_repeat_block(RepeatBlock::new(
        RepeatCount::try_new(2).expect("repeat count"),
        circuit("X 2\n"),
        Some("loop".into()),
    ));
    assert_eq!(
        subject.to_stim_string(),
        "H[tag] 0 1\nM 0\nREPEAT[loop] 2 {\n    X 2\n}\n"
    );
}

#[test]
fn a2_parse_repeat_policy_admission() {
    let limits = ParseLimits::new(
        SourceLineLimit::new(64),
        RepeatNestingLimit::try_new(2).expect("depth"),
    );
    for accepted in [
        "REPEAT 2 {\n    REPEAT 2 {\n        H 0\n    }\n}\n",
        "repeat 2 {\n    repeat 2 {\n        error(0.1) D0\n    }\n}\n",
    ] {
        if accepted.starts_with("REPEAT") {
            assert!(Circuit::from_stim_str_with_limits(accepted, limits).is_ok());
        } else {
            assert!(DetectorErrorModel::from_dem_str_with_limits(accepted, limits).is_ok());
        }
    }
    assert!(
        Circuit::from_stim_str_with_limits(
            "REPEAT 2 {\n    REPEAT 2 {\n        REPEAT 2 {\n            H 0\n        }\n    }\n}\n",
            limits,
        )
        .is_err()
    );
}

#[test]
fn cq2_dem_instruction_value_validation_print_contract() {
    let error = DemInstruction::error(
        Probability::try_new(0.125).expect("probability"),
        vec![
            DemTarget::relative_detector(3).expect("D3"),
            DemTarget::logical_observable(6).expect("L6"),
        ],
        Some("err".into()),
    )
    .expect("error instruction");
    let shift = DemInstruction::shift_detectors(vec![3.5], 7, Some("shift".into())).expect("shift");
    let mut model = DetectorErrorModel::new();
    model.push_instruction(error.clone());
    model.push_instruction(shift);
    assert_eq!(error.kind(), DemInstructionKind::Error);
    assert_eq!(error.args(), [0.125]);
    assert_eq!(
        model.to_dem_string(),
        "error[err](0.125) D3 L6\nshift_detectors[shift](3.5) 7\n"
    );
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
}

#[test]
fn a2_sampling_request_resource_estimate() {
    let estimate = stab_model::advanced::resource_estimate_for_sampling_request(
        Estimate::Exact(7),
        Estimate::UpperBound(13),
        Estimate::Exact(5),
        Estimate::UpperBound(1024),
    );
    assert_eq!(estimate.input_items(), Estimate::Exact(7));
    assert_eq!(estimate.expanded_operations(), Estimate::UpperBound(13));
    assert_eq!(estimate.folded_traversal(), Estimate::Exact(5));
    assert_eq!(estimate.output_bytes(), Estimate::UpperBound(1024));
    assert_eq!(estimate.input_bytes(), Estimate::Unknown);
    assert_eq!(estimate.scratch_bytes(), Estimate::Unknown);
}

#[test]
fn cq2_stim_format_canonical_round_trip() {
    let source = "h 0\ncnot 0 1\nREPEAT[tag] 2 {\n    m 0\n}\n";
    let canonical = "H 0\nCX 0 1\nREPEAT[tag] 2 {\n    M 0\n}\n";
    let parsed = circuit(source);
    assert_eq!(parsed.to_stim_string(), canonical);
    assert_eq!(circuit(canonical), parsed);
    assert_eq!(
        Circuit::from_stim_bytes(source.as_bytes()).expect("bytes"),
        parsed
    );
    assert_eq!(parsed.to_stim_bytes(), canonical.as_bytes());
}

#[test]
fn cq2_gate_name_lookup_contract() {
    for (name, canonical) in [
        ("H", "H"),
        ("h", "H"),
        ("H_XZ", "H"),
        ("CNOT", "CX"),
        ("cnot", "CX"),
        ("MZ", "M"),
    ] {
        assert_eq!(
            Gate::from_name(name).expect("known gate").canonical_name(),
            canonical
        );
    }
    assert!(Gate::from_name("not-a-gate").is_err());
}
