#![allow(
    clippy::expect_used,
    reason = "PF1 circuit model compatibility tests use direct assertions for compact diagnostics"
)]

use std::collections::BTreeMap;

use stab_model::{
    Circuit, CircuitDetectorId, CircuitInstruction, CircuitItem, MemoryBudget, QubitId,
    RepeatBlock, RepeatCount, RepeatNestingLimit, ResourceKind, ResourceOperation,
};

fn single_item(input: &str) -> CircuitItem {
    let circuit = Circuit::from_stim_str(input).expect("parse single item circuit");
    assert_eq!(circuit.len(), 1);
    circuit.items().first().expect("single item").clone()
}

fn single_repeat_block(input: &str) -> Option<RepeatBlock> {
    match single_item(input) {
        CircuitItem::Instruction(_) => None,
        CircuitItem::RepeatBlock(repeat) => Some(repeat),
    }
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

fn detector(id: u64) -> CircuitDetectorId {
    CircuitDetectorId::new(id)
}

fn qubit(id: u32) -> QubitId {
    QubitId::new(id).expect("test qubit id is in range")
}

#[test]
fn circuit_public_and_simulated_qubit_counts_distinguish_stim_mpad_behavior() {
    let circuit = Circuit::from_stim_str("H 0\nMPAD 1\n").expect("parse MPAD circuit");
    assert_eq!(circuit.count_qubits(), 2);
    assert_eq!(
        stab_model::advanced::circuit_simulated_qubit_count(&circuit),
        1
    );

    let pad_only = Circuit::from_stim_str("MPAD 0 1 0\n").expect("parse pad-only circuit");
    assert_eq!(pad_only.count_qubits(), 2);
    assert_eq!(
        stab_model::advanced::circuit_simulated_qubit_count(&pad_only),
        0
    );

    let nested = Circuit::from_stim_str("REPEAT 3 {\n    MPAD 1\n    M 4\n}\n")
        .expect("parse repeat-nested MPAD circuit");
    assert_eq!(nested.count_qubits(), 5);
    assert_eq!(
        stab_model::advanced::circuit_simulated_qubit_count(&nested),
        5
    );
}

#[test]
fn count_qubits_and_drop_fit_the_accepted_repeat_depth_on_a_small_stack() {
    let worker = std::thread::Builder::new()
        .name("count-qubits-accepted-depth".to_string())
        .stack_size(64 * 1024)
        .spawn(|| {
            let repeat_count = RepeatCount::try_new(1).expect("nonzero repeat count");
            let mut circuit =
                Circuit::from_stim_str("H 1023\n").expect("parse innermost instruction");
            for _ in 0..RepeatNestingLimit::HARD_MAX {
                let mut outer = Circuit::new();
                outer.append_repeat_block(RepeatBlock::new(repeat_count, circuit, None));
                circuit = outer;
            }

            assert_eq!(circuit.count_qubits(), 1024);
            drop(circuit);
        })
        .expect("spawn constrained-stack worker");

    worker
        .join()
        .expect("counting and normal destruction stay within the constrained stack");
}

#[test]
fn count_qubits_temporary_storage_depends_on_depth_not_repeat_siblings() {
    fn repeated_siblings(count: usize) -> Circuit {
        let repeat_count = RepeatCount::try_new(1).expect("nonzero repeat count");
        let body = Circuit::from_stim_str("H 31\n").expect("parse repeated body");
        let mut circuit = Circuit::new();
        for _ in 0..count {
            circuit.append_repeat_block(RepeatBlock::new(repeat_count, body.clone(), None));
        }
        circuit
    }

    let narrow = repeated_siblings(1);
    let broad = repeated_siblings(4_096);
    let narrow_allocations = allocation_counter::measure(|| assert_eq!(narrow.count_qubits(), 32));
    let broad_allocations = allocation_counter::measure(|| assert_eq!(broad.count_qubits(), 32));

    assert_eq!(
        broad_allocations.count_total,
        narrow_allocations.count_total
    );
    assert_eq!(
        broad_allocations.bytes_total,
        narrow_allocations.bytes_total
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
        (qubit(0), vec![1.0, 2.0, 3.0]),
        (qubit(1), vec![42.0, 3.0]),
        (qubit(2), vec![35.0, 3.0]),
        (qubit(4), vec![8.0]),
    ]);

    assert_eq!(
        circuit
            .final_qubit_coordinates()
            .expect("final qubit coordinates"),
        expected
    );
}

#[test]
fn cq2_circuit_api_final_qubit_coordinates_fold_huge_repeats() {
    let circuit = Circuit::from_stim_str(
        "QUBIT_COORDS(0) 0\n\
         REPEAT 1000 {\n\
             QUBIT_COORDS(1, 1) 1\n\
             REPEAT 2000 {\n\
                 QUBIT_COORDS(2, 0.5) 2\n\
                 REPEAT 4000 {\n\
                     QUBIT_COORDS(3) 3\n\
                     REPEAT 8000 {\n\
                         QUBIT_COORDS(4) 4\n\
                         SHIFT_COORDS(100)\n\
                         QUBIT_COORDS(5) 5\n\
                     }\n\
                     SHIFT_COORDS(10)\n\
                     QUBIT_COORDS(6) 6\n\
                 }\n\
                 QUBIT_COORDS(7) 7\n\
             }\n\
             QUBIT_COORDS(8) 8\n\
         }\n\
         QUBIT_COORDS(9) 9\n",
    )
    .expect("parse huge folded coordinate circuit");

    let total_shift = 6_400_080_000_000_000.0;
    let expected = BTreeMap::from([
        (qubit(0), vec![0.0]),
        (qubit(1), vec![total_shift + 1.0 - 6_400_080_000_000.0, 1.0]),
        (qubit(2), vec![total_shift + 2.0 - 3_200_040_000.0, 0.5]),
        (qubit(3), vec![total_shift + 3.0 - 800_010.0]),
        (qubit(4), vec![total_shift + 4.0 - 110.0]),
        (qubit(5), vec![total_shift + 5.0 - 10.0]),
        (qubit(6), vec![total_shift + 6.0]),
        (qubit(7), vec![total_shift + 7.0]),
        (qubit(8), vec![total_shift + 8.0]),
        (qubit(9), vec![total_shift + 9.0]),
    ]);

    assert_eq!(
        circuit
            .final_qubit_coordinates()
            .expect("fold huge repeats"),
        expected
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
fn pf1_circuit_repeat_fuses_single_repeat_block_counts() {
    let circuit =
        Circuit::from_stim_str("REPEAT[tag] 2 {\n    H[tag2] 0\n}\n").expect("parse circuit");

    assert_eq!(
        circuit.repeated(3).expect("repeat nested").to_stim_string(),
        concat!("REPEAT 6 {\n", "    H[tag2] 0\n", "}\n")
    );
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

/// A small circuit whose `REPEAT` declares 10^12 detectors cannot make the all-detector query
/// allocate per detector: the map is admitted against the memory budget first, exactly like the
/// DEM query, while selected lookups stay cheap.
#[test]
fn circuit_detector_coordinate_map_is_admitted_against_the_memory_budget() {
    let huge = Circuit::from_stim_str(
        "M 0\n\
         REPEAT 1000000000000 {\n\
             DETECTOR(1, 2) rec[-1]\n\
             SHIFT_COORDS(3, 4)\n\
         }\n",
    )
    .expect("parse huge-repeat circuit");
    let measured = allocation_counter::measure(|| {
        let error = huge
            .detector_coordinates()
            .expect_err("an all-detector map beyond the default budget is refused");
        let resource = error
            .resource_limit_error()
            .expect("typed resource refusal");
        assert_eq!(resource.resource(), ResourceKind::MaterializedBytes);
        assert_eq!(
            resource.operation(),
            ResourceOperation::CircuitDetectorCoordinates
        );
        assert_eq!(resource.limit(), MemoryBudget::DEFAULT.bytes());
        assert_eq!(resource.actual(), 1_000_000_000_000 * (96 + 2 * 8));
    });
    assert!(measured.bytes_max < 4096, "{measured:?}");
    assert_eq!(
        huge.coordinates_of_detector(detector(1))
            .expect("selected huge-repeat coordinates"),
        vec![4.0, 6.0]
    );

    let small = Circuit::from_stim_str(
        "M 0\n\
         REPEAT 3 {\n\
             DETECTOR(1, 2) rec[-1]\n\
             SHIFT_COORDS(3, 4)\n\
         }\n",
    )
    .expect("parse small circuit");
    // Three entries of 96 bytes plus two coordinate scalars each fit exactly 336 bytes.
    let exact = MemoryBudget::from_bytes(3 * (96 + 2 * 8));
    assert_eq!(
        small
            .detector_coordinates_with_memory_budget(exact)
            .expect("exact budget admits the map")
            .len(),
        3
    );
    let excess = MemoryBudget::from_bytes(exact.bytes() - 1);
    let error = small
        .detector_coordinates_with_memory_budget(excess)
        .expect_err("one byte below the requirement is refused");
    assert_eq!(
        error
            .resource_limit_error()
            .map(|resource| resource.actual()),
        Some(exact.bytes())
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

#[test]
fn model_admitted_bytes_matches_parse_admission_for_fused_and_unfused_models() {
    let stim = b"H[tag1] 0\nX_ERROR(0.125) 0 1\nREPEAT[rep] 3 {\n    M[m] 0\n    DETECTOR(1, 2) rec[-1]\n}\n";
    let (circuit, parsed_circuit_bytes) =
        Circuit::from_stim_bytes_admitted(stim, stab_model::ParseLimits::default())
            .expect("parse circuit");
    assert_eq!(circuit.admitted_bytes(), parsed_circuit_bytes);

    // A fused line returns its item slot, arguments, and tag, so the parse holds exactly what
    // the fused circuit retains, on the text path and on the byte-tag path that fuses after
    // restoring opaque tags.
    for (source, items) in [
        (
            &b"H 0\nH 1\nX_ERROR[t](0.25) 0 1 2\nX_ERROR[t](0.25) 3 4 5\n"[..],
            2,
        ),
        (&b"H[\xff] 0\nH[\xff] 1 2 3 4 5\nM 0\n"[..], 2),
    ] {
        let (fused, fused_parse_bytes) =
            Circuit::from_stim_bytes_admitted(source, stab_model::ParseLimits::default())
                .expect("parse fused circuit");
        assert_eq!(fused.items().len(), items);
        assert_eq!(fused.admitted_bytes(), fused_parse_bytes);
    }

    let dem_bytes = b"error[etag](0.125) D0 L0\nrepeat[rtag] 4 {\n    error(0.05) D1\n    shift_detectors(1) 1\n}\n";
    let (dem, parsed_dem_bytes) = stab_model::DetectorErrorModel::from_dem_bytes_admitted(
        dem_bytes,
        stab_model::ParseLimits::default(),
    )
    .expect("parse DEM");
    assert_eq!(dem.admitted_bytes(), parsed_dem_bytes);
}
