#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "PF1 circuit API compatibility tests use direct assertions for compact diagnostics"
)]

use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    ops::Bound,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use stab_core::{
    Circuit, CircuitDetectorId, CircuitError, CircuitInstruction, CircuitItem, CompiledSampler,
    QubitId, RepeatBlock,
};

const OVERSIZED_CIRCUIT_FILE_BYTES: u64 = 64 * 1024 * 1024 + 1;

fn single_item(input: &str) -> CircuitItem {
    let circuit = Circuit::from_stim_str(input).expect("parse single item circuit");
    assert_eq!(circuit.len(), 1);
    circuit.items().first().expect("single item").clone()
}

fn single_instruction(input: &str) -> Option<CircuitInstruction> {
    match single_item(input) {
        CircuitItem::Instruction(instruction) => Some(instruction),
        CircuitItem::RepeatBlock(_) => None,
    }
}

fn single_repeat_block(input: &str) -> Option<RepeatBlock> {
    match single_item(input) {
        CircuitItem::Instruction(_) => None,
        CircuitItem::RepeatBlock(repeat) => Some(repeat),
    }
}

fn temp_test_dir(name: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "stab-circuit-api-{name}-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&dir).expect("create temp test dir");
    dir
}

fn circuit_item_names<'a>(items: impl Iterator<Item = &'a CircuitItem>) -> Vec<String> {
    items
        .map(|item| match item {
            CircuitItem::Instruction(instruction) => {
                instruction.gate().canonical_name().to_string()
            }
            CircuitItem::RepeatBlock(_) => "REPEAT".to_string(),
        })
        .collect()
}

fn circuit_instruction_lines<'a>(
    instructions: impl Iterator<Item = &'a CircuitInstruction>,
) -> Vec<String> {
    instructions
        .map(|instruction| {
            let mut circuit = Circuit::new();
            circuit.append_instruction(instruction.clone());
            circuit.to_stim_string().trim_end().to_string()
        })
        .collect()
}

#[test]
fn pf1_circuit_stats_counts_match_owned_upstream_semantics() {
    let circuit = Circuit::from_stim_str(
        "M 0 1\n\
         REPEAT 100 {\n\
             TICK\n\
             M 2\n\
             DETECTOR rec[-1]\n\
             OBSERVABLE_INCLUDE(2) rec[-1]\n\
             CY sweep[77] 3\n\
         }\n",
    )
    .expect("parse circuit");

    assert_eq!(circuit.len(), 2);
    assert!(!circuit.is_empty());
    assert_eq!(circuit.count_measurements().expect("measurements"), 102);
    assert_eq!(circuit.count_detectors().expect("detectors"), 100);
    assert_eq!(circuit.count_observables().expect("observables"), 3);
    assert_eq!(circuit.count_ticks().expect("ticks"), 100);
    assert_eq!(circuit.count_sweep_bits().expect("sweep bits"), 78);
}

#[test]
fn pf1_circuit_stats_measurement_counts_use_result_groups() {
    let circuit = Circuit::from_stim_str(
        "MPP X0*X1 Y2*Y3 Z4\n\
         MXX 5 6 7 8\n\
         HERALDED_ERASE(0.25) 9 10\n\
         MPAD 0 1 0\n",
    )
    .expect("parse circuit");

    assert_eq!(circuit.count_measurements().expect("measurements"), 10);
}

#[test]
fn pf1_circuit_stats_counts_do_not_unroll_large_repeats() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 1000000 {\n\
             REPEAT 1000000 {\n\
                 M 0\n\
                 DETECTOR rec[-1]\n\
                 OBSERVABLE_INCLUDE(4) rec[-1]\n\
             }\n\
         }\n",
    )
    .expect("parse circuit");

    assert_eq!(
        circuit.count_measurements().expect("measurements"),
        1_000_000_000_000
    );
    assert_eq!(
        circuit.count_detectors().expect("detectors"),
        1_000_000_000_000
    );
    assert_eq!(circuit.count_observables().expect("observables"), 5);
}

#[test]
fn pf1_circuit_stats_counts_reject_folded_overflow() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 18446744073709551615 {\n\
             M 0 1\n\
         }\n",
    )
    .expect("parse circuit");

    let error = circuit
        .count_measurements()
        .expect_err("reject count overflow");

    assert!(
        error.to_string().contains("circuit count overflowed"),
        "{error}"
    );
}

#[test]
fn pf1_circuit_stats_final_coordinate_shift_matches_nested_upstream_case() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 1000 {\n\
             REPEAT 2000 {\n\
                 REPEAT 3000 {\n\
                     SHIFT_COORDS(0, 0, 1)\n\
                 }\n\
                 SHIFT_COORDS(1)\n\
             }\n\
             SHIFT_COORDS(0, 1)\n\
         }\n",
    )
    .expect("parse circuit");

    assert_eq!(
        circuit
            .final_coordinate_shift()
            .expect("final coordinate shift"),
        vec![2_000_000.0, 1000.0, 6_000_000_000.0]
    );
}

#[test]
fn pf1_circuit_stats_final_qubit_coordinates_apply_shifts_and_repeats() {
    let circuit = Circuit::from_stim_str(
        "QUBIT_COORDS(1, 2, 3) 0\n\
         QUBIT_COORDS(2) 1\n\
         SHIFT_COORDS(5)\n\
         QUBIT_COORDS(3) 4\n\
         REPEAT 3 {\n\
             SHIFT_COORDS(10, 1)\n\
             QUBIT_COORDS(7) 1\n\
         }\n\
         QUBIT_COORDS(0, 0) 2\n",
    )
    .expect("parse circuit");

    let expected = BTreeMap::from([
        (QubitId::new(0).unwrap(), vec![1.0, 2.0, 3.0]),
        (QubitId::new(1).unwrap(), vec![42.0, 3.0]),
        (QubitId::new(2).unwrap(), vec![35.0, 3.0]),
        (QubitId::new(4).unwrap(), vec![8.0]),
    ]);

    assert_eq!(
        circuit
            .final_qubit_coordinates()
            .expect("final qubit coordinates"),
        expected
    );
}

#[test]
fn pf1_circuit_stats_clear_resets_items_and_counts() {
    let mut circuit = Circuit::from_stim_str("H 0\nM 0\nDETECTOR rec[-1]\n").expect("parse");
    circuit.clear();

    assert!(circuit.is_empty());
    assert_eq!(circuit.len(), 0);
    assert_eq!(circuit.to_stim_string(), "");
    assert_eq!(circuit.count_measurements().expect("measurements"), 0);
    assert_eq!(circuit.count_detectors().expect("detectors"), 0);
}

#[test]
fn pf1_circuit_iterators_top_level_range_views_are_typed() {
    let circuit = Circuit::from_stim_str(
        "H 0\n\
         REPEAT[tag] 2 {\n\
             M 0\n\
         }\n\
         S 1\n",
    )
    .expect("parse circuit");

    assert_eq!(
        circuit_item_names(circuit.iter_items()),
        vec!["H", "REPEAT", "S"]
    );
    assert_eq!(
        circuit_item_names(circuit.item_range(1..).expect("item range")),
        vec!["REPEAT", "S"]
    );
    assert_eq!(
        circuit_item_names(circuit.item_range(..=0).expect("inclusive item range")),
        vec!["H"]
    );
    assert!(
        circuit
            .items()
            .get(1)
            .and_then(CircuitItem::as_repeat_block)
            .is_some()
    );
    assert!(
        circuit
            .items()
            .first()
            .and_then(CircuitItem::as_instruction)
            .is_some()
    );

    assert_eq!(
        circuit_instruction_lines(
            circuit
                .instruction_range(0..1)
                .expect("instruction-only range")
        ),
        vec!["H 0"]
    );
    assert_eq!(
        circuit_instruction_lines(
            circuit
                .instruction_range(2..3)
                .expect("instruction-only range")
        ),
        vec!["S 1"]
    );

    let repeat_error = circuit
        .instruction_range(0..2)
        .err()
        .expect("repeat block is not an instruction");
    assert!(
        repeat_error
            .to_string()
            .contains("circuit instruction range")
            && repeat_error.to_string().contains("repeat block"),
        "{repeat_error}"
    );

    let range_error = circuit
        .item_range(2..5)
        .err()
        .expect("reject out-of-range item view");
    assert!(
        range_error.to_string().contains("circuit item range"),
        "{range_error}"
    );

    let overflow_error = circuit
        .item_range((Bound::Excluded(usize::MAX), Bound::Unbounded))
        .err()
        .expect("reject overflowing range bound");
    assert!(
        overflow_error
            .to_string()
            .contains("excluded start index overflowed"),
        "{overflow_error}"
    );
}

#[test]
fn pf1_circuit_iterators_flatten_nested_repeats_in_stim_order() {
    let circuit = Circuit::from_stim_str(
        "H 0\n\
         M 0 1\n\
         REPEAT 2 {\n\
             X 1\n\
             REPEAT 3 {\n\
                 Y 2\n\
             }\n\
         }\n",
    )
    .expect("parse circuit");

    assert_eq!(
        circuit_instruction_lines(circuit.iter_flattened_instructions()),
        vec![
            "H 0", "M 0 1", "X 1", "Y 2", "Y 2", "Y 2", "X 1", "Y 2", "Y 2", "Y 2",
        ]
    );
    assert_eq!(
        circuit_instruction_lines(circuit.iter_flattened_instructions_reverse()),
        vec![
            "Y 2", "Y 2", "Y 2", "X 1", "Y 2", "Y 2", "Y 2", "X 1", "M 0 1", "H 0",
        ]
    );

    let huge_repeat =
        Circuit::from_stim_str("REPEAT 1000000000000 {\n    H 0\n}\n").expect("parse repeat");
    assert_eq!(
        circuit_instruction_lines(huge_repeat.iter_flattened_instructions().take(3)),
        vec!["H 0", "H 0", "H 0"]
    );
    assert_eq!(
        circuit_instruction_lines(huge_repeat.iter_flattened_instructions_reverse().take(3)),
        vec!["H 0", "H 0", "H 0"]
    );
}

#[test]
fn pf1_circuit_file_helpers_read_and_write_canonical_stim_text() {
    let dir = temp_test_dir("read-write");
    let input_path = dir.join("input.stim");
    fs::write(
        &input_path,
        "H[test] 5\ncnot 0 1\nREPEAT[tag] 2 {\n    H 2\n}\n",
    )
    .expect("write input circuit");

    let circuit = Circuit::from_stim_file(&input_path).expect("read circuit");
    assert_eq!(
        circuit.to_stim_string(),
        concat!(
            "H[test] 5\n",
            "CX 0 1\n",
            "REPEAT[tag] 2 {\n",
            "    H 2\n",
            "}\n",
        )
    );

    let output_path = dir.join("output.stim");
    circuit
        .write_stim_file(&output_path)
        .expect("write canonical circuit");
    assert_eq!(
        fs::read_to_string(&output_path).expect("read output circuit"),
        circuit.to_stim_string()
    );

    fs::remove_dir_all(dir).expect("cleanup temp test dir");
}

#[test]
fn pf1_circuit_file_helpers_report_read_and_write_errors() {
    let dir = temp_test_dir("io-errors");
    let missing_path = dir.join("missing.stim");

    let read_error = Circuit::from_stim_file(&missing_path).expect_err("reject missing file");
    assert!(matches!(
        read_error,
        CircuitError::CircuitIo {
            operation: "read",
            kind: ErrorKind::NotFound,
            ..
        }
    ));
    assert!(
        read_error
            .to_string()
            .contains("failed to read circuit file"),
        "{read_error}"
    );

    let invalid_path = dir.join("invalid.stim");
    fs::write(&invalid_path, "UNKNOWN 0\n").expect("write invalid circuit");
    let parse_error = Circuit::from_stim_file(&invalid_path).expect_err("reject invalid circuit");
    assert!(
        matches!(parse_error, CircuitError::ParseLine { .. }),
        "{parse_error}"
    );

    let oversized_path = dir.join("oversized.stim");
    fs::File::create(&oversized_path)
        .expect("create oversized circuit")
        .set_len(OVERSIZED_CIRCUIT_FILE_BYTES)
        .expect("resize oversized circuit");
    let oversized_error =
        Circuit::from_stim_file(&oversized_path).expect_err("reject oversized circuit");
    assert!(matches!(
        oversized_error,
        CircuitError::InvalidDomainValue {
            kind: "circuit file size",
            ..
        }
    ));

    let circuit = Circuit::from_stim_str("H 0\n").expect("parse circuit");
    let write_error = circuit
        .write_stim_file(dir.join("missing-parent").join("out.stim"))
        .expect_err("reject missing output parent");
    assert!(matches!(
        write_error,
        CircuitError::CircuitIo {
            operation: "write",
            kind: ErrorKind::NotFound,
            ..
        }
    ));
    assert!(
        write_error
            .to_string()
            .contains("failed to write circuit file"),
        "{write_error}"
    );

    fs::remove_dir_all(dir).expect("cleanup temp test dir");
}

#[test]
fn pf1_circuit_append_text_preserves_tags_repeats_and_fuses() {
    let mut circuit = Circuit::from_stim_str("H[tag] 0\n").expect("parse base");

    circuit
        .append_from_stim_text(
            "H[tag] 1\n\
             REPEAT[loop] 2 {\n\
                 M[meas] 0\n\
                 DETECTOR[det] rec[-1]\n\
             }\n",
        )
        .expect("append text");

    assert_eq!(
        circuit.to_stim_string(),
        concat!(
            "H[tag] 0 1\n",
            "REPEAT[loop] 2 {\n",
            "    M[meas] 0\n",
            "    DETECTOR[det] rec[-1]\n",
            "}\n",
        )
    );
    assert_eq!(circuit.len(), 2);
    assert_eq!(circuit.count_measurements().expect("measurements"), 2);
    assert_eq!(circuit.count_detectors().expect("detectors"), 2);
}

#[test]
fn pf1_circuit_append_text_program_alias_appends() {
    let mut circuit = Circuit::new();

    circuit
        .append_from_stim_program_text(
            "H[test] 0\n\
             CX[test2] 1 2\n",
        )
        .expect("append text");

    assert_eq!(
        circuit.to_stim_string(),
        "H[test] 0\n\
         CX[test2] 1 2\n"
    );
}

#[test]
fn pf1_circuit_append_text_is_atomic_on_parse_error() {
    let mut circuit = Circuit::from_stim_str("H 0\nM 0\n").expect("parse base");
    let before = circuit.clone();

    let error = circuit
        .append_from_stim_text(
            "H 1\n\
             NOT_A_GATE 2\n",
        )
        .expect_err("reject invalid append text");

    assert!(error.to_string().contains("unknown gate"), "{error}");
    assert_eq!(circuit, before);
}

#[test]
fn pf1_circuit_concat_append_circuit_and_concatenated_fuse_boundary() {
    let mut circuit = Circuit::from_stim_str("H[tag] 0\n").expect("parse base");
    let rhs = Circuit::from_stim_str("H[tag] 1\nM 0\n").expect("parse rhs");

    let concatenated = circuit.concatenated(&rhs);
    circuit.append_circuit(&rhs);

    let expected = "H[tag] 0 1\nM 0\n";
    assert_eq!(circuit.to_stim_string(), expected);
    assert_eq!(concatenated.to_stim_string(), expected);
    assert_eq!(rhs.to_stim_string(), "H[tag] 1\nM 0\n");
}

#[test]
fn pf1_circuit_repeat_matches_upstream_special_cases() {
    let circuit = Circuit::from_stim_str("Y 3\nM 4\n").expect("parse circuit");

    assert_eq!(
        circuit.repeated(0).expect("repeat zero").to_stim_string(),
        ""
    );
    assert_eq!(
        circuit.repeated(1).expect("repeat one").to_stim_string(),
        "Y 3\nM 4\n"
    );
    assert_eq!(
        circuit.repeated(2).expect("repeat two").to_stim_string(),
        concat!("REPEAT 2 {\n", "    Y 3\n", "    M 4\n", "}\n")
    );

    let mut in_place = circuit.clone();
    in_place.repeat_in_place(3).expect("repeat in place");
    assert_eq!(
        in_place.to_stim_string(),
        concat!("REPEAT 3 {\n", "    Y 3\n", "    M 4\n", "}\n")
    );
}

#[test]
fn pf1_circuit_repeat_fuses_single_repeat_block_counts() {
    let circuit =
        Circuit::from_stim_str("REPEAT[tag] 2 {\n    H[tag2] 0\n}\n").expect("parse circuit");

    assert_eq!(
        circuit.repeated(3).expect("repeat nested").to_stim_string(),
        concat!("REPEAT 6 {\n", "    H[tag2] 0\n", "}\n")
    );
}

#[test]
fn pf1_circuit_repeat_rejects_fused_repeat_count_overflow() {
    let circuit =
        Circuit::from_stim_str("REPEAT 1234567890123456789 {\n    H 0\n}\n").expect("parse");

    let error = circuit
        .repeated(16)
        .expect_err("reject repeat count overflow");

    assert_eq!(
        error,
        stab_core::CircuitError::InvalidDomainValue {
            kind: "repetition count",
            value: "overflowed".to_string()
        }
    );
}

#[test]
fn pf1_circuit_insert_pop_insert_instruction_fuses_boundaries() {
    let base = "CX 0 1\nH 0\nS 0\nCX 0 1\n";

    let mut circuit = Circuit::from_stim_str(base).expect("parse base");
    circuit
        .insert_item(2, single_item("H 1\n"))
        .expect("insert H");
    assert_eq!(circuit.to_stim_string(), "CX 0 1\nH 0 1\nS 0\nCX 0 1\n");

    let mut circuit = Circuit::from_stim_str(base).expect("parse base");
    circuit
        .insert_item(2, single_item("S 1\n"))
        .expect("insert S");
    assert_eq!(circuit.to_stim_string(), "CX 0 1\nH 0\nS 1 0\nCX 0 1\n");

    let mut circuit = Circuit::from_stim_str("H 0\nH 2\n").expect("parse base");
    circuit
        .insert_instruction(1, single_instruction("H 1\n").expect("parse instruction"))
        .expect("insert instruction");
    assert_eq!(circuit.to_stim_string(), "H 0 2 1\n");

    let mut circuit = Circuit::from_stim_str(base).expect("parse base");
    circuit
        .insert_item(0, single_item("X 1\n"))
        .expect("insert at start");
    circuit
        .insert_item(circuit.len(), single_item("X 1\n"))
        .expect("insert at end");
    assert_eq!(
        circuit.to_stim_string(),
        "X 1\nCX 0 1\nH 0\nS 0\nCX 0 1\nX 1\n"
    );
}

#[test]
fn pf1_circuit_insert_pop_insert_circuit_fuses_both_boundaries() {
    let mut circuit = Circuit::from_stim_str("CX 0 1\nH 0\nS 0\nCX 0 1\n").expect("parse base");
    let inserted = Circuit::from_stim_str("H 1\nX 3\nS 2\n").expect("parse inserted");

    circuit
        .insert_circuit(2, &inserted)
        .expect("insert circuit");

    assert_eq!(
        circuit.to_stim_string(),
        "CX 0 1\nH 0 1\nX 3\nS 2 0\nCX 0 1\n"
    );
    assert_eq!(inserted.to_stim_string(), "H 1\nX 3\nS 2\n");
}

#[test]
fn pf1_circuit_insert_pop_insert_repeat_block_and_reject_bad_index() {
    let mut circuit = Circuit::from_stim_str("H 0\nM 0\n").expect("parse base");

    circuit
        .insert_repeat_block(
            1,
            single_repeat_block("REPEAT[tag] 2 {\n    X 1\n}\n").expect("parse repeat"),
        )
        .expect("insert repeat");
    assert_eq!(
        circuit.to_stim_string(),
        concat!("H 0\n", "REPEAT[tag] 2 {\n", "    X 1\n", "}\n", "M 0\n",)
    );

    let error = circuit
        .insert_circuit(circuit.len() + 1, &Circuit::new())
        .expect_err("reject bad insert index");
    assert!(
        error.to_string().contains("circuit insertion index"),
        "{error}"
    );
}

#[test]
fn pf1_circuit_insert_pop_pop_item_removes_without_fusing_neighbors() {
    let mut circuit = Circuit::from_stim_str("H 0\nX 1\nH 2\n").expect("parse circuit");

    let popped = circuit.pop_item(1).expect("pop middle");
    assert_eq!(popped, single_item("X 1\n"));
    assert_eq!(circuit.to_stim_string(), "H 0\nH 2\n");

    let last = circuit.pop_last_item().expect("pop last");
    assert_eq!(last, single_item("H 2\n"));
    assert_eq!(circuit.to_stim_string(), "H 0\n");

    let error = Circuit::new()
        .pop_last_item()
        .expect_err("reject empty pop");
    assert!(error.to_string().contains("circuit pop index"), "{error}");
}

#[test]
fn pf1_circuit_reference_determined_reference_sample_matches_compiled_sampler() {
    let empty_measurement_circuit = Circuit::from_stim_str("H 0\nCX 0 1\n").expect("parse");
    assert_eq!(
        empty_measurement_circuit
            .reference_sample()
            .expect("reference sample"),
        Vec::<bool>::new()
    );

    let simple_reference = Circuit::from_stim_str("X 0\nM 0\n").expect("parse");
    assert_eq!(
        simple_reference
            .reference_sample()
            .expect("reference sample"),
        vec![true]
    );

    let sweep_controlled = Circuit::from_stim_str("X 0\nCX sweep[0] 0\nM 0\n").expect("parse");
    assert_eq!(
        sweep_controlled
            .reference_sample()
            .expect("reference sample"),
        vec![true]
    );
    assert_eq!(sweep_controlled.count_sweep_bits().expect("sweep bits"), 1);

    let circuit = Circuit::from_stim_str(
        "H 0 1\n\
         CX 0 2 1 3\n\
         MPP X0*X1 Y0*Y1 Z0*Z1\n\
         X 0 2 4 6\n\
         M 0 1 2 3 4 5 6 7\n",
    )
    .expect("parse circuit");
    let expected = CompiledSampler::compile(&circuit)
        .expect("compile sampler")
        .reference_sample();

    assert_eq!(
        circuit.reference_sample().expect("reference sample"),
        expected
    );
    assert_eq!(
        expected.len(),
        usize::try_from(circuit.count_measurements().expect("measurements"))
            .expect("measurement count fits usize")
    );
    assert!(expected.iter().any(|bit| *bit));
}

#[test]
fn pf1_circuit_reference_determined_reference_sample_tree_decompresses_reference_sample() {
    let circuit = Circuit::from_stim_str("M 0\nX 0\nM 0\n").expect("parse circuit");
    let tree = circuit
        .reference_sample_tree()
        .expect("reference sample tree");

    assert_eq!(
        tree.decompress(),
        circuit.reference_sample().expect("reference sample")
    );
    assert_eq!(tree.size(), 2);
    assert_eq!(tree.get(0), Some(false));
    assert_eq!(tree.get(1), Some(true));
    assert_eq!(tree.get(2), None);

    let repeated = Circuit::from_stim_str(
        "REPEAT 3 {\n\
             R 0\n\
             M 0\n\
             X 0\n\
             M 0\n\
         }\n",
    )
    .expect("parse repeated circuit");
    let repeated_tree = repeated
        .reference_sample_tree()
        .expect("reference sample tree");
    assert_eq!(
        repeated_tree.decompress(),
        vec![false, true, false, true, false, true]
    );
    assert_eq!(repeated_tree.size(), 6);
}

#[test]
fn pf1_circuit_reference_determined_count_determined_measurements_matches_public_helper_subset() {
    let tagged = Circuit::from_stim_str(
        "R[test1] 0\n\
         M[test3] 0\n\
         DETECTOR[test4](1, 2) rec[-1]\n",
    )
    .expect("parse tagged circuit");
    assert_eq!(
        tagged
            .count_determined_measurements(false)
            .expect("count determined"),
        1
    );

    let unknown_input = Circuit::from_stim_str(
        "MPP Z0*Z1 X2*X3\n\
         TICK\n\
         MPP Z0*Z1 X2*X3\n",
    )
    .expect("parse unknown-input circuit");
    assert_eq!(
        unknown_input
            .count_determined_measurements(true)
            .expect("count with unknown input"),
        2
    );
    assert_eq!(
        unknown_input
            .count_determined_measurements(false)
            .expect("count with known zero input"),
        3
    );

    let sweep_controlled =
        Circuit::from_stim_str("X 0\nCX sweep[0] 0\nM 0\n").expect("parse sweep circuit");
    assert_eq!(
        sweep_controlled
            .count_determined_measurements(false)
            .expect("count deterministic sweep circuit"),
        1
    );
    assert_eq!(
        sweep_controlled
            .count_determined_measurements(true)
            .expect("count unknown-input sweep circuit"),
        0
    );
}

#[test]
fn pf1_circuit_stats_coordinate_queries_reject_non_finite_folded_shift() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 1000000000000 {\n\
             SHIFT_COORDS(1e308)\n\
         }\n",
    )
    .expect("parse circuit");

    let error = circuit
        .final_coordinate_shift()
        .expect_err("reject infinite coordinate shift");

    assert!(
        error.to_string().contains("coordinate shift overflowed"),
        "{error}"
    );
}

#[test]
fn pf1_circuit_detector_coords_include_empty_and_shifted_coordinates() {
    let circuit = Circuit::from_stim_str(
        "M 0\n\
         DETECTOR rec[-1]\n\
         DETECTOR(1, 2, 3) rec[-1]\n\
         REPEAT 3 {\n\
             DETECTOR(42) rec[-1]\n\
             SHIFT_COORDS(100)\n\
         }\n",
    )
    .expect("parse circuit");

    let expected = BTreeMap::from([
        (detector(0), vec![]),
        (detector(1), vec![1.0, 2.0, 3.0]),
        (detector(2), vec![42.0]),
        (detector(3), vec![142.0]),
        (detector(4), vec![242.0]),
    ]);

    assert_eq!(
        circuit.detector_coordinates().expect("all coordinates"),
        expected
    );
    assert_eq!(
        circuit
            .coordinates_of_detector(detector(0))
            .expect("detector zero"),
        vec![]
    );
    assert_eq!(
        circuit
            .detector_coordinates_for([detector(1), detector(3)])
            .expect("selected coordinates"),
        BTreeMap::from([
            (detector(1), vec![1.0, 2.0, 3.0]),
            (detector(3), vec![142.0])
        ])
    );
}

#[test]
fn pf1_circuit_detector_coords_fold_nested_repeat_queries() {
    let circuit = Circuit::from_stim_str(
        "TICK\n\
         REPEAT 1000 {\n\
             REPEAT 2000 {\n\
                 REPEAT 1000 {\n\
                     DETECTOR(0, 0, 0, 4)\n\
                     SHIFT_COORDS(1, 0, 0)\n\
                 }\n\
                 DETECTOR(0, 0, 0, 3)\n\
                 SHIFT_COORDS(0, 1, 0)\n\
             }\n\
             DETECTOR(0, 0, 0, 2)\n\
             SHIFT_COORDS(0, 0, 1)\n\
         }\n\
         DETECTOR(0, 0, 0, 1)\n",
    )
    .expect("parse circuit");

    assert_eq!(
        circuit
            .coordinates_of_detector(detector(0))
            .expect("detector 0"),
        vec![0.0, 0.0, 0.0, 4.0]
    );
    assert_eq!(
        circuit
            .coordinates_of_detector(detector(1002))
            .expect("detector 1002"),
        vec![1001.0, 1.0, 0.0, 4.0]
    );
    assert_eq!(
        circuit
            .detector_coordinates_for([
                detector(0),
                detector(1),
                detector(999),
                detector(1000),
                detector(1001),
                detector(1002),
            ])
            .expect("selected coordinates"),
        BTreeMap::from([
            (detector(0), vec![0.0, 0.0, 0.0, 4.0]),
            (detector(1), vec![1.0, 0.0, 0.0, 4.0]),
            (detector(999), vec![999.0, 0.0, 0.0, 4.0]),
            (detector(1000), vec![1000.0, 0.0, 0.0, 3.0]),
            (detector(1001), vec![1000.0, 1.0, 0.0, 4.0]),
            (detector(1002), vec![1001.0, 1.0, 0.0, 4.0]),
        ])
    );
}

#[test]
fn pf1_circuit_detector_coords_skip_detector_free_repeat_shift() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 1000 {\n\
             SHIFT_COORDS(1)\n\
         }\n\
         DETECTOR(5)\n",
    )
    .expect("parse circuit");

    assert_eq!(
        circuit
            .coordinates_of_detector(detector(0))
            .expect("detector after shift-only repeat"),
        vec![1005.0]
    );
}

#[test]
fn pf1_circuit_detector_coords_reject_missing_detector_id() {
    let circuit = Circuit::from_stim_str("M 0\nDETECTOR rec[-1]\n").expect("parse");

    let error = circuit
        .coordinates_of_detector(detector(1))
        .expect_err("reject missing detector");

    assert!(error.to_string().contains("Detector index 1 is too big"));
}

fn detector(id: u64) -> CircuitDetectorId {
    CircuitDetectorId::new(id)
}
