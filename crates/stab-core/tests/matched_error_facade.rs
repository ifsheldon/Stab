#![allow(
    clippy::expect_used,
    reason = "matched-error facade tests use fixed valid fixtures"
)]

use std::fmt::{self, Write};
use std::hint::black_box;

use stab_core::{
    CircuitErrorLocation, CircuitErrorLocationStackFrame, CircuitTargetsInsideInstruction,
    ExplainedError, FlippedMeasurement, Gate, GateTargetWithCoords, QubitId, Target,
};

struct Discard;

impl Write for Discard {
    fn write_str(&mut self, _: &str) -> fmt::Result {
        Ok(())
    }
}

fn location(width: usize, tick_offset: u64) -> CircuitErrorLocation {
    let target = GateTargetWithCoords {
        gate_target: Target::qubit(QubitId::new(3).expect("valid qubit"), false),
        coords: Vec::new(),
    };
    CircuitErrorLocation {
        noise_tag: Some("noise".to_string()),
        tick_offset,
        flipped_pauli_product: vec![target.clone(); width],
        flipped_measurement: FlippedMeasurement {
            measurement_record_index: Some(5),
            measured_observable: vec![target.clone(); width],
        },
        instruction_targets: CircuitTargetsInsideInstruction {
            gate: Some(Gate::from_name("X_ERROR").expect("known gate")),
            gate_tag: None,
            args: Vec::new(),
            target_range_start: 0,
            target_range_end: width,
            targets_in_range: vec![target; width],
        },
        stack_frames: vec![CircuitErrorLocationStackFrame {
            instruction_offset: 2,
            iteration_index: 0,
            instruction_repetitions_arg: 0,
        }],
    }
}

#[test]
fn matched_error_facade_display_and_comparison_borrow_model_storage() {
    let left = location(1_024, 0);
    let right = location(1_024, 1);
    assert!(left.is_simpler_than(&right));

    let comparison_allocations = allocation_counter::measure(|| {
        black_box(left.is_simpler_than(black_box(&right)));
    });
    assert_eq!(
        comparison_allocations.bytes_total, 0,
        "comparison cloned matched-error storage: {comparison_allocations:?}"
    );

    let explained = ExplainedError {
        dem_error_terms: Vec::new(),
        circuit_error_locations: vec![left, right],
    };
    let display_allocations = allocation_counter::measure(|| {
        write!(&mut Discard, "{explained}").expect("discard formatting");
    });
    assert_eq!(
        display_allocations.bytes_total, 0,
        "display cloned matched-error storage: {display_allocations:?}"
    );
}
