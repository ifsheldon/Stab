#![allow(
    clippy::expect_used,
    reason = "borrowed-view tests use fixed valid fixtures"
)]

use std::fmt::{self, Display, Formatter, Write};
use std::hint::black_box;

use stab_analysis::advanced::{
    CircuitErrorLocationView, CircuitTargetsInsideInstructionView, write_explained_error,
};
use stab_analysis::{
    CircuitErrorLocation, CircuitErrorLocationStackFrame, CircuitTargetsInsideInstruction,
    FlippedMeasurement, GateTargetWithCoords,
};
use stab_model::{Gate, QubitId, Target};

struct Discard;

impl Write for Discard {
    fn write_str(&mut self, _: &str) -> fmt::Result {
        Ok(())
    }
}

struct ExplainedView<'a> {
    locations: &'a [CircuitErrorLocation],
}

impl Display for ExplainedView<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write_explained_error(
            f,
            &[],
            self.locations.iter().map(CircuitErrorLocationView::from),
        )
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

fn explicit_view(location: &CircuitErrorLocation) -> CircuitErrorLocationView<'_> {
    CircuitErrorLocationView::new(
        location.noise_tag.as_deref(),
        location.tick_offset,
        &location.flipped_pauli_product,
        &location.flipped_measurement,
        CircuitTargetsInsideInstructionView::new(
            location.instruction_targets.gate,
            location.instruction_targets.gate_tag.as_deref(),
            &location.instruction_targets.args,
            location.instruction_targets.target_range_start,
            location.instruction_targets.target_range_end,
            &location.instruction_targets.targets_in_range,
        ),
        &location.stack_frames,
    )
}

#[test]
fn matched_error_borrowed_views_preserve_behavior_without_allocating() {
    let left = location(1_024, 0);
    let right = location(1_024, 1);
    assert_eq!(
        explicit_view(&left).to_string(),
        CircuitErrorLocationView::from(&left).to_string()
    );
    assert!(explicit_view(&left).is_simpler_than(explicit_view(&right)));
    let locations = [left, right];

    let allocations = allocation_counter::measure(|| {
        black_box(
            explicit_view(&locations[0]).is_simpler_than(explicit_view(black_box(&locations[1]))),
        );
        write!(
            &mut Discard,
            "{}",
            ExplainedView {
                locations: black_box(&locations)
            }
        )
        .expect("discard formatting");
    });
    assert_eq!(
        allocations.bytes_total, 0,
        "borrowed matched-error views allocated: {allocations:?}"
    );
}
