#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "CQ2 compatibility tests use direct fixture assertions for precise failures"
)]

use std::collections::{BTreeSet, HashSet};
use std::io::ErrorKind;

use stab_core::{
    Circuit, CircuitDetectorId, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Gate,
    ObservableId, QubitId, RepeatBlock, RepeatCount, Target,
};

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
    assert_eq!(
        x_error
            .probability_argument()
            .expect("typed probability")
            .expect("probability")
            .get(),
        0.5
    );
    assert_eq!(
        x_error
            .probability_arguments()
            .expect("typed probabilities")
            .expect("probability list")
            .into_iter()
            .map(stab_core::Probability::get)
            .collect::<Vec<_>>(),
        vec![0.5]
    );
    assert_eq!(x_error.observable_id_argument().unwrap(), None);
    assert_eq!(x_error.coordinate_arguments(), None);
    assert_eq!(x_error.tag(), None);
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
    let tagged = CircuitInstruction::new(
        Gate::from_name("H").expect("H gate"),
        vec![],
        vec![q(0)],
        Some("constructor-tag".to_string()),
    )
    .expect("construct tagged instruction");
    assert_eq!(tagged.tag(), Some("constructor-tag"));
    let empty_tag = CircuitInstruction::new(
        Gate::from_name("H").expect("H gate"),
        vec![],
        vec![q(0)],
        Some(String::new()),
    )
    .expect("normalize empty instruction tag");
    assert_eq!(empty_tag.tag(), None);
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

    let tagged_detector = instruction("DETECTOR[tag](1, 2.5) rec[-1]\n");
    assert_eq!(
        tagged_detector.coordinate_arguments(),
        Some(&[1.0, 2.5][..])
    );
    assert_eq!(tagged_detector.tag(), Some("tag"));
    assert_eq!(tagged_detector.probability_argument().unwrap(), None);

    let observable = instruction("OBSERVABLE_INCLUDE(17) rec[-1]\n");
    assert_eq!(
        observable
            .observable_id_argument()
            .expect("typed observable")
            .expect("observable id")
            .get(),
        17
    );

    let channel = instruction("PAULI_CHANNEL_1(0.1, 0.2, 0.3) 0\n");
    assert_eq!(channel.probability_argument().unwrap(), None);
    assert_eq!(
        channel
            .probability_arguments()
            .expect("typed probabilities")
            .expect("probability list")
            .into_iter()
            .map(stab_core::Probability::get)
            .collect::<Vec<_>>(),
        vec![0.1, 0.2, 0.3]
    );

    let target_group_cases: [(&str, &[&str]); 13] = [
        ("MPAD 0 1 0\n", &["0", "1", "0"]),
        ("MPAD\n", &[]),
        ("H\n", &[]),
        ("H 1\n", &["1"]),
        ("H 2 3\n", &["2", "3"]),
        ("CX\n", &[]),
        ("CX 0 1\n", &["0 1"]),
        ("CX 2 3 5 7\n", &["2 3", "5 7"]),
        ("DETECTOR\n", &[]),
        ("CORRELATED_ERROR(0.001)\n", &[]),
        ("MPP\n", &[]),
        ("MPP X0*Y1 Z2\n", &["X0 * Y1", "Z2"]),
        ("QUBIT_COORDS 1 2\n", &["1", "2"]),
    ];
    for (text, expected) in target_group_cases {
        assert_eq!(target_group_text(&instruction(text)), expected, "{text:?}");
    }

    let overlapping_pairs = instruction("CX 0 1 2 3 1 4\n");
    assert_eq!(
        instruction_lines(overlapping_pairs.disjoint_target_segments()),
        vec!["CX 0 1 2 3", "CX 1 4"]
    );
    assert_eq!(
        instruction_lines(overlapping_pairs.disjoint_target_segments_reversed()),
        vec!["CX 2 3 1 4", "CX 0 1"]
    );
    let noisy_pairs = instruction("DEPOLARIZE2[tag](0.125) 0 1 2 3 1 4\n");
    let noisy_segments = noisy_pairs.disjoint_target_segments();
    assert_eq!(noisy_segments.len(), 2);
    assert!(
        noisy_segments
            .iter()
            .all(|segment| segment.args() == [0.125] && segment.tag() == Some("tag"))
    );
    assert!(
        CircuitInstruction::new(
            Gate::from_name("CX").expect("CX gate"),
            vec![],
            vec![q(0)],
            None,
        )
        .is_err()
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

#[test]
fn cq2_circuit_api_typed_ids_enforce_value_boundaries() {
    let zero = QubitId::new(0).expect("zero qubit");
    let largest = QubitId::new((1 << 24) - 1).expect("largest Stim qubit");
    assert_eq!(zero.get(), 0);
    assert_eq!(largest.get(), (1 << 24) - 1);
    assert!(zero < largest);
    assert_eq!(
        QubitId::new(1 << 24),
        Err(CircuitError::InvalidDomainValue {
            kind: "qubit id",
            value: (1 << 24).to_string(),
        })
    );

    let observable_zero = ObservableId::new(0);
    let observable = ObservableId::new(u64::MAX);
    assert_eq!(observable.get(), u64::MAX);
    assert!(observable_zero < observable);
    assert_eq!(
        BTreeSet::from([observable, observable_zero])
            .into_iter()
            .collect::<Vec<_>>(),
        vec![observable_zero, observable]
    );

    let detector_zero = CircuitDetectorId::new(0);
    let detector = CircuitDetectorId::new(u64::MAX);
    assert_eq!(detector.get(), u64::MAX);
    assert!(detector_zero < detector);
    let mut detector_set = HashSet::new();
    assert!(detector_set.insert(detector));
    assert!(!detector_set.insert(detector));

    assert_eq!(
        RepeatCount::try_new(0),
        Err(CircuitError::InvalidDomainValue {
            kind: "repeat count",
            value: "0".to_string(),
        })
    );
    let one_repeat = RepeatCount::try_new(1).expect("smallest repeat count");
    let repeat_count = RepeatCount::try_new(u64::MAX).expect("largest repeat count");
    assert_eq!(repeat_count.get(), u64::MAX);
    assert!(one_repeat < repeat_count);
}

#[test]
fn cq2_circuit_api_value_items_and_repeat_blocks_are_independent() {
    let mut original = Circuit::new();
    assert_eq!(original, Circuit::default());
    assert_eq!(original.to_string(), "");

    original.append_instruction(instruction("H 0\n"));
    let clone = original.clone();
    original.append_instruction(instruction("M 0\n"));
    assert_eq!(clone.to_string(), "H 0\n");
    assert_eq!(original.to_string(), "H 0\nM 0\n");
    assert_ne!(clone, original);

    let repeat = RepeatBlock::new(
        RepeatCount::try_new(5).expect("repeat count"),
        clone.clone(),
        Some("loop".to_string()),
    );
    assert_eq!(repeat.repeat_count().get(), 5);
    assert_eq!(repeat.body(), &clone);
    assert_eq!(repeat.tag(), Some("loop"));
    let mut body_copy = repeat.body().clone();
    body_copy.append_instruction(instruction("S 0\n"));
    assert_eq!(repeat.body().to_string(), "H 0\n");
    assert_ne!(&body_copy, repeat.body());
    assert_ne!(
        repeat,
        RepeatBlock::new(
            RepeatCount::try_new(5).expect("repeat count"),
            Circuit::new(),
            Some("loop".to_string()),
        )
    );
    assert_ne!(
        repeat,
        RepeatBlock::new(
            RepeatCount::try_new(4).expect("different repeat count"),
            clone.clone(),
            Some("loop".to_string()),
        )
    );
    assert_ne!(
        repeat,
        RepeatBlock::new(
            RepeatCount::try_new(5).expect("repeat count"),
            clone.clone(),
            Some("other".to_string()),
        )
    );
    assert!(RepeatCount::try_new(0).is_err());

    let empty_tag = RepeatBlock::new(
        RepeatCount::try_new(1).expect("repeat count"),
        Circuit::new(),
        Some(String::new()),
    );
    assert_eq!(empty_tag.tag(), None);

    let instruction_item = CircuitItem::Instruction(instruction("X 1\n"));
    assert_circuit_item_variant_is_covered(&instruction_item);
    assert!(instruction_item.as_instruction().is_some());
    assert!(instruction_item.as_repeat_block().is_none());
    let repeat_item = CircuitItem::RepeatBlock(repeat);
    assert_circuit_item_variant_is_covered(&repeat_item);
    assert!(repeat_item.as_instruction().is_none());
    assert_eq!(
        repeat_item.as_repeat_block().expect("repeat item").body(),
        &clone
    );

    assert_eq!(instruction("H[tag] 0\n"), instruction("H[tag] 0\n"));
    assert_ne!(instruction("H[tag] 0\n"), instruction("H[other] 0\n"));
    let tagged_repeat =
        Circuit::from_stim_str("REPEAT[tag] 2 {\n    H 0\n}\n").expect("parse tagged repeat");
    let untagged_repeat =
        Circuit::from_stim_str("REPEAT 2 {\n    H 0\n}\n").expect("parse untagged repeat");
    assert_ne!(tagged_repeat, untagged_repeat);

    let equality_a = Circuit::from_stim_str("H 0\nREPEAT 100 {\n    X_ERROR(0.25) 1\n}\n")
        .expect("parse equality circuit A");
    let equality_b = Circuit::from_stim_str("H 1\nREPEAT 100 {\n    X_ERROR(0.25) 1\n}\n")
        .expect("parse equality circuit B");
    let equality_c = Circuit::from_stim_str("H 0\nREPEAT 100 {\n    X_ERROR(0.125) 1\n}\n")
        .expect("parse equality circuit C");
    assert_ne!(equality_a, equality_b);
    assert_ne!(equality_a, equality_c);
    assert_ne!(equality_b, equality_c);

    let nested = Circuit::from_stim_str(
        "H 0\nM 0 1\nREPEAT 2 {\n    X 1\n    REPEAT 3 {\n        Y 2\n        M 2\n        X 0\n    }\n}\n",
    )
    .expect("parse nested repeat structure");
    assert_eq!(nested.items().len(), 3);
    let outer = nested
        .items()
        .get(2)
        .and_then(CircuitItem::as_repeat_block)
        .expect("outer repeat");
    assert_eq!(outer.body().items().len(), 2);
    let inner = outer
        .body()
        .items()
        .get(1)
        .and_then(CircuitItem::as_repeat_block)
        .expect("inner repeat");
    assert_eq!(inner.body().items().len(), 3);
    assert_eq!(nested.clone(), nested);
}

#[test]
fn cq2_circuit_api_append_items_preserve_tags_and_fuse() {
    let mut circuit = Circuit::new();
    circuit.append_instruction(instruction("H[tag] 0\n"));
    circuit.append_instruction(instruction("H[tag] 1\n"));
    assert_eq!(circuit.to_string(), "H[tag] 0 1\n");
    circuit.append_instruction(instruction("H[other] 2\n"));
    assert_eq!(circuit.len(), 2, "different tags must not fuse");

    let mut measurement = Circuit::new();
    measurement.append_instruction(instruction("M 0 1\n"));
    measurement.append_instruction(instruction("M 2 3\n"));
    assert_eq!(measurement.to_string(), "M 0 1 2 3\n");

    let mut non_fusing = Circuit::new();
    non_fusing.append_instruction(instruction("TICK\n"));
    non_fusing.append_instruction(instruction("TICK\n"));
    non_fusing.append_instruction(instruction("DETECTOR rec[-1]\n"));
    non_fusing.append_instruction(instruction("DETECTOR rec[-1]\n"));
    assert_eq!(non_fusing.len(), 4);

    let empty_reset =
        CircuitInstruction::new(Gate::from_name("R").expect("R gate"), vec![], vec![], None)
            .expect("construct empty reset");
    let mut reset = Circuit::from_stim_str("R 0\n").expect("parse reset");
    reset.append_instruction(empty_reset);
    assert_eq!(reset.to_string(), "R 0\n");
    let repeat = Circuit::from_stim_str("REPEAT[loop] 3 {\n    M[measure] 0\n}\n")
        .expect("parse repeat")
        .items()
        .first()
        .and_then(CircuitItem::as_repeat_block)
        .expect("repeat block")
        .clone();
    circuit.append_repeat_block(repeat);
    assert_eq!(
        circuit.to_string(),
        concat!(
            "H[tag] 0 1\n",
            "H[other] 2\n",
            "REPEAT[loop] 3 {\n",
            "    M[measure] 0\n",
            "}\n",
        )
    );
}

#[test]
fn cq2_circuit_api_without_tags_is_recursive_and_non_mutating() {
    let original = Circuit::from_stim_str(
        "H[top] 0\nREPEAT[loop] 2 {\n    M[measure](0.125) 0\n    DETECTOR[det] rec[-1]\n}\n",
    )
    .expect("parse tagged circuit");
    let stripped = original.without_tags();

    assert_eq!(
        stripped.to_string(),
        "H 0\nREPEAT 2 {\n    M(0.125) 0\n    DETECTOR rec[-1]\n}\n"
    );
    assert!(original.to_string().contains("[loop]"));
    for tag in ["[top]", "[loop]", "[measure]", "[det]"] {
        assert!(!stripped.to_string().contains(tag));
    }
}

#[test]
fn cq2_circuit_api_flattened_iterators_clone_without_sharing_position() {
    let circuit = Circuit::from_stim_str(
        "H 0\nREPEAT 2 {\n    X 1\n    REPEAT 2 {\n        Y 2\n    }\n}\nM 0\n",
    )
    .expect("parse nested circuit");

    let mut forward = circuit.iter_flattened_instructions();
    assert_eq!(
        instruction_line(forward.next().expect("first forward")),
        "H 0"
    );
    let forward_clone = forward.clone();
    assert_eq!(
        forward.map(instruction_line).collect::<Vec<_>>(),
        forward_clone.map(instruction_line).collect::<Vec<_>>()
    );

    let mut reverse = circuit.iter_flattened_instructions_reverse();
    assert_eq!(
        instruction_line(reverse.next().expect("first reverse")),
        "M 0"
    );
    let reverse_clone = reverse.clone();
    assert_eq!(
        reverse.map(instruction_line).collect::<Vec<_>>(),
        reverse_clone.map(instruction_line).collect::<Vec<_>>()
    );
}

#[test]
fn cq2_circuit_api_error_value_contract_is_exhaustive() {
    let cases = vec![
        (
            CircuitError::UnknownGate("NOPE".to_string()),
            "unknown gate NOPE",
        ),
        (
            CircuitError::InvalidDomainValue {
                kind: "repeat count",
                value: "0".to_string(),
            },
            "invalid repeat count value 0",
        ),
        (
            CircuitError::ParseLine {
                line: 3,
                message: "bad token".to_string(),
            },
            "failed to parse line 3: bad token",
        ),
        (
            CircuitError::InvalidArgumentCount {
                gate: "H",
                expected: "0",
                actual: 1,
            },
            "gate H expected 0 argument(s), got 1",
        ),
        (
            CircuitError::InvalidArgument {
                gate: "X_ERROR",
                argument: "nan".to_string(),
            },
            "gate X_ERROR received invalid argument nan",
        ),
        (
            CircuitError::InvalidTarget {
                gate: "H",
                target: "rec[-1]".to_string(),
            },
            "gate H received invalid target rec[-1]",
        ),
        (
            CircuitError::InvalidTargetCount {
                gate: "CX",
                count: 3,
            },
            "gate CX received invalid target count 3",
        ),
        (
            CircuitError::InvalidTableauConversion {
                message: "measurement".to_string(),
            },
            "cannot convert circuit to tableau: measurement",
        ),
        (
            CircuitError::InvalidCircuitSimplification {
                message: "anti-Hermitian product".to_string(),
            },
            "cannot simplify circuit: anti-Hermitian product",
        ),
        (
            CircuitError::InvalidSamplerCompilation {
                message: "unsupported gate".to_string(),
            },
            "cannot compile circuit sampler: unsupported gate",
        ),
        (
            CircuitError::InvalidResultFormat {
                message: "bad width".to_string(),
            },
            "invalid result format data: bad width",
        ),
        (
            CircuitError::CircuitIo {
                operation: "read",
                kind: ErrorKind::NotFound,
                message: "missing".to_string(),
            },
            "failed to read circuit file: missing",
        ),
        (
            CircuitError::InvalidDetectorErrorModel {
                message: "bad target".to_string(),
            },
            "invalid detector error model: bad target",
        ),
        (
            CircuitError::UnterminatedRepeatBlock,
            "unterminated repeat block",
        ),
        (
            CircuitError::UnexpectedRepeatTerminator,
            "unexpected repeat block terminator",
        ),
    ];
    for (error, expected) in cases {
        assert_circuit_error_variant_is_covered(&error);
        assert_eq!(error.to_string(), expected);
        assert_eq!(circuit_result_error(error.clone()), Err(error));
    }
}

fn assert_circuit_error_variant_is_covered(error: &CircuitError) {
    match error {
        CircuitError::UnknownGate(_)
        | CircuitError::InvalidDomainValue { .. }
        | CircuitError::ParseLine { .. }
        | CircuitError::InvalidArgumentCount { .. }
        | CircuitError::InvalidArgument { .. }
        | CircuitError::InvalidTarget { .. }
        | CircuitError::InvalidTargetCount { .. }
        | CircuitError::InvalidTableauConversion { .. }
        | CircuitError::InvalidCircuitSimplification { .. }
        | CircuitError::InvalidSamplerCompilation { .. }
        | CircuitError::InvalidResultFormat { .. }
        | CircuitError::CircuitIo { .. }
        | CircuitError::InvalidDetectorErrorModel { .. }
        | CircuitError::UnterminatedRepeatBlock
        | CircuitError::UnexpectedRepeatTerminator => {}
    }
}

fn assert_circuit_item_variant_is_covered(item: &CircuitItem) {
    match item {
        CircuitItem::Instruction(_) | CircuitItem::RepeatBlock(_) => {}
    }
}

fn circuit_result_error(error: CircuitError) -> CircuitResult<()> {
    Err(error)
}

fn instruction(text: &str) -> CircuitInstruction {
    Circuit::from_stim_str(text)
        .expect("parse instruction")
        .items()
        .first()
        .and_then(CircuitItem::as_instruction)
        .expect("single instruction")
        .clone()
}

fn instruction_lines(instructions: Vec<CircuitInstruction>) -> Vec<String> {
    instructions.iter().map(instruction_line).collect()
}

fn target_group_text(instruction: &CircuitInstruction) -> Vec<String> {
    instruction
        .target_groups()
        .into_iter()
        .map(|group| {
            group
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

fn instruction_line(instruction: &CircuitInstruction) -> String {
    let mut circuit = Circuit::new();
    circuit.append_instruction(instruction.clone());
    circuit.to_string().trim_end().to_string()
}

fn q(id: u32) -> Target {
    Target::qubit(QubitId::new(id).unwrap(), false)
}
