use std::convert::Infallible;

use stab_model::advanced::{FoldedCircuitEvent, visit_folded_circuit};
use stab_model::{Cancellation, Circuit, Estimate, ParseLimits, ResourceEstimate};
use stab_records::RecordFormat;

/// Estimates cheaply knowable resource properties of a sampling request without compiling or
/// executing it.
///
/// Structural item and expanded-operation counts are exact when representable. Output bytes are
/// exact for fixed-width encodings and unknown for sparse encodings whose size depends on sampled
/// values. Execution work, scratch, and resident memory remain unknown until the plan/session
/// architecture can estimate them without guessing.
pub fn estimate_sampling_request(
    circuit: &Circuit,
    shots: usize,
    output_format: RecordFormat,
) -> ResourceEstimate {
    let (input_items, expanded_operations) = operation_counts(circuit);
    let output_bytes = circuit
        .count_measurements()
        .ok()
        .and_then(|measurements| usize::try_from(measurements).ok())
        .map_or(Estimate::Unknown, |measurements| {
            output_format.estimate_output_bytes(shots, measurements)
        });

    ResourceEstimate::builder()
        .input_items(input_items.map_or(Estimate::Unknown, Estimate::Exact))
        .expanded_operations(expanded_operations.map_or(Estimate::Unknown, Estimate::Exact))
        .folded_traversal(input_items.map_or(Estimate::Unknown, Estimate::Exact))
        .output_bytes(output_bytes)
        .build()
}

/// Counts folded items and expanded instructions in one folded walk.
///
/// The expanded count is unknown once any repeat block's expanded iterations exceed `u64`, even
/// when that block's body is empty, and both counts are unknown for nesting beyond the shared
/// repeat envelope.
fn operation_counts(circuit: &Circuit) -> (Option<usize>, Option<usize>) {
    let mut structural_items = Some(0usize);
    let mut expanded_operations = Some(0u64);
    let walked = visit_folded_circuit(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        &Cancellation::never(),
        |event| {
            structural_items = structural_items.and_then(|count| count.checked_add(1));
            let added = match event {
                FoldedCircuitEvent::Instruction { executions, .. } => executions.exact(),
                FoldedCircuitEvent::Repeat { iterations, .. } => iterations.exact().map(|_| 0),
            };
            expanded_operations = expanded_operations
                .zip(added)
                .and_then(|(count, added)| count.checked_add(added));
            Ok::<(), Infallible>(())
        },
    );
    if walked.is_err() {
        return (None, None);
    }
    (
        structural_items,
        expanded_operations.and_then(|count| usize::try_from(count).ok()),
    )
}
