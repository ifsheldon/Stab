#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "CQ2 compatibility tests use direct fixture assertions for precise failures"
)]

use stab_model::{
    Circuit, CircuitDetectorId, CircuitInstruction, CircuitItem, Gate, ModelError, ObservableId,
    Probability, QubitId, RepeatBlock, RepeatCount, Target, ValidationError,
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
            .map(Probability::get)
            .collect::<Vec<_>>(),
        vec![0.5]
    );
    assert_eq!(x_error.observable_id_argument().unwrap(), None);
    assert_eq!(x_error.coordinate_arguments(), None);
    assert_eq!(x_error.tag(), None);
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
    let largest_observable = instruction("OBSERVABLE_INCLUDE(18446744073709549568) rec[-1]\n");
    assert_eq!(
        largest_observable
            .observable_id_argument()
            .expect("typed largest observable")
            .expect("largest observable id")
            .get(),
        18_446_744_073_709_549_568
    );
    let overflowing_observable = instruction("OBSERVABLE_INCLUDE(18446744073709551616) rec[-1]\n");
    assert_eq!(
        overflowing_observable
            .observable_id_argument()
            .expect_err("2^64 is outside the ObservableId domain"),
        ModelError::from(ValidationError::InvalidArgument {
            gate: "OBSERVABLE_INCLUDE",
            argument: "18446744073709552000".to_string(),
        })
    );

    let channel = instruction("PAULI_CHANNEL_1(0.1, 0.2, 0.3) 0\n");
    assert_eq!(channel.probability_argument().unwrap(), None);
    assert_eq!(
        channel
            .probability_arguments()
            .expect("typed probabilities")
            .expect("probability list")
            .into_iter()
            .map(Probability::get)
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
    assert_eq!(
        QubitId::new(1 << 24),
        Err(ModelError::Validation(
            ValidationError::InvalidDomainValue {
                kind: "qubit id",
                value: (1 << 24).to_string(),
            }
        ))
    );

    let observable = ObservableId::new(u64::MAX);
    assert_eq!(observable.get(), u64::MAX);

    let detector = CircuitDetectorId::new(u64::MAX);
    assert_eq!(detector.get(), u64::MAX);

    assert_eq!(
        RepeatCount::try_new(0),
        Err(ModelError::Validation(
            ValidationError::InvalidDomainValue {
                kind: "repeat count",
                value: "0".to_string(),
            }
        ))
    );
    let repeat_count = RepeatCount::try_new(u64::MAX).expect("largest repeat count");
    assert_eq!(repeat_count.get(), u64::MAX);
}

#[test]
fn cq2_circuit_api_value_items_and_repeat_blocks_are_independent() {
    let mut original = Circuit::new();
    assert_eq!(original.to_string(), "");

    original.append_instruction(instruction("H 0\n"));
    let clone = original.clone();
    original.append_instruction(instruction("M 0\n"));
    assert_eq!(clone.to_string(), "H 0\n");
    assert_eq!(original.to_string(), "H 0\nM 0\n");

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

    let empty_tag = RepeatBlock::new(
        RepeatCount::try_new(1).expect("repeat count"),
        Circuit::new(),
        Some(String::new()),
    );
    assert_eq!(empty_tag.tag(), None);

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
