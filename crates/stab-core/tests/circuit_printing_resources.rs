#![allow(
    clippy::expect_used,
    reason = "resource regressions use direct fixture assertions for precise failures"
)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use stab_core::{Circuit, CircuitInstruction, Gate, QubitId, Target};

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn temp_output(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "stab-circuit-printing-{}-{}-{name}.stim",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn canonical_printer_qualification_cycle_uses_one_retained_allocation() {
    let source = [
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
    let circuit = Circuit::from_stim_str(&source).expect("parse qualification cycle");

    let mut output = String::new();
    let allocations = allocation_counter::measure(|| {
        output = circuit.to_stim_string();
    });

    assert_eq!(output, source);
    assert_eq!(allocations.count_total, 1, "{allocations:?}");
    assert_eq!(allocations.count_max, 1, "{allocations:?}");
}

#[test]
fn float_heavy_printer_uses_one_exactly_sized_output_allocation() {
    let args = [0.0, 1.25, 1.2345e-100, 12_345.7]
        .into_iter()
        .cycle()
        .take(4_096)
        .collect::<Vec<_>>();
    let instruction = CircuitInstruction::new(
        Gate::from_name("SHIFT_COORDS").expect("SHIFT_COORDS gate"),
        args,
        Vec::new(),
        None,
    )
    .expect("construct coordinate instruction");
    let mut circuit = Circuit::new();
    circuit.append_instruction(instruction);

    let mut output = String::new();
    let allocations = allocation_counter::measure(|| {
        output = circuit.to_stim_string();
    });

    assert!(output.starts_with("SHIFT_COORDS(0, 1.25, 1.2345e-100, 12345.7, "));
    assert!(output.ends_with(")\n"));
    assert_eq!(output.capacity(), output.len());
    assert_eq!(allocations.count_total, 1, "{allocations:?}");
    assert_eq!(allocations.count_max, 1, "{allocations:?}");
}

#[test]
fn streaming_file_writer_allocations_do_not_scale_with_target_count() {
    let gate = Gate::from_name("H").expect("H gate");
    let make_circuit = |target_count: u32| {
        let targets = (0..target_count)
            .map(|id| {
                Target::qubit(
                    QubitId::new(id).expect("resource fixture qubit id is valid"),
                    false,
                )
            })
            .collect::<Vec<_>>();
        let instruction =
            CircuitInstruction::new(gate, Vec::new(), targets, None).expect("construct H");
        let mut circuit = Circuit::new();
        circuit.append_instruction(instruction);
        circuit
    };
    let small = make_circuit(1);
    let large = make_circuit(4_096);
    let small_path = temp_output("small");
    let large_path = temp_output("large");

    let small_allocations = allocation_counter::measure(|| {
        small
            .write_stim_file(&small_path)
            .expect("write small circuit");
    });
    let large_allocations = allocation_counter::measure(|| {
        large
            .write_stim_file(&large_path)
            .expect("write large circuit");
    });

    assert_eq!(
        large_allocations.count_total, small_allocations.count_total,
        "small={small_allocations:?} large={large_allocations:?}"
    );
    assert_eq!(
        large_allocations.count_max, small_allocations.count_max,
        "small={small_allocations:?} large={large_allocations:?}"
    );
    assert_eq!(
        fs::read_to_string(&small_path).expect("read small"),
        "H 0\n"
    );
    let large_text = fs::read_to_string(&large_path).expect("read large");
    assert!(large_text.starts_with("H 0 1 2 3 "));
    assert!(large_text.ends_with(" 4095\n"));

    fs::remove_file(small_path).expect("remove small output");
    fs::remove_file(large_path).expect("remove large output");
}
