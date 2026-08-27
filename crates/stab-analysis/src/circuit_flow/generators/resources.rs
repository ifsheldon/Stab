use stab_algebra::Flow;
use stab_model::{Circuit, CircuitItem};

use crate::{AnalysisError, AnalysisResult, ResourceKind, ResourceLimitError, ResourceOperation};

pub(crate) const MAX_FLOW_GENERATOR_PROJECTED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_FLOW_GENERATOR_EXPANDED_OPERATIONS: u64 = 1_000_000;
const MAX_FLOW_GENERATOR_REPEAT_NESTING: usize = 256;
// Two rows per qubit, two Pauli strings per row, and two bit planes per string.
const MAX_IGNORED_ONLY_FLOW_GENERATOR_PROJECTED_BYTES: u64 = 4096 * 4096;

pub(super) fn validate_ignored_only_flow_generator_resources(
    qubit_count: usize,
) -> AnalysisResult<()> {
    let qubits = qubit_count as u128;
    let projected_bytes = u64::try_from(qubits.saturating_mul(qubits)).unwrap_or(u64::MAX);
    if projected_bytes > MAX_IGNORED_ONLY_FLOW_GENERATOR_PROJECTED_BYTES {
        return Err(flow_resource_error(
            ResourceKind::ProjectedPayloadBytes,
            projected_bytes,
            MAX_IGNORED_ONLY_FLOW_GENERATOR_PROJECTED_BYTES,
        ));
    }
    Ok(())
}

pub(crate) fn validate_measurement_rich_flow_generator_resources(
    qubit_count: usize,
    measurement_count: usize,
) -> AnalysisResult<()> {
    let projected_bytes =
        measurement_rich_flow_generator_projected_bytes(qubit_count, measurement_count);
    if projected_bytes > MAX_FLOW_GENERATOR_PROJECTED_BYTES {
        return Err(flow_resource_error(
            ResourceKind::ProjectedPayloadBytes,
            projected_bytes,
            MAX_FLOW_GENERATOR_PROJECTED_BYTES,
        ));
    }
    Ok(())
}

pub(crate) fn measurement_rich_flow_generator_projected_bytes(
    qubit_count: usize,
    measurement_count: usize,
) -> u64 {
    let qubits = qubit_count as u128;
    let measurements = measurement_count as u128;
    let rows = qubits.saturating_mul(2).saturating_add(measurements);
    let bytes_per_row = (std::mem::size_of::<Flow>() as u128)
        .saturating_add(qubits.saturating_mul(2))
        .saturating_add(measurements.saturating_mul(std::mem::size_of::<i32>() as u128));
    u64::try_from(rows.saturating_mul(bytes_per_row)).unwrap_or(u64::MAX)
}

pub(super) fn validate_flow_generator_expanded_work(circuit: &Circuit) -> AnalysisResult<()> {
    fn count(
        circuit: &Circuit,
        multiplier: u64,
        depth: usize,
        total: &mut u64,
    ) -> AnalysisResult<()> {
        if depth > MAX_FLOW_GENERATOR_REPEAT_NESTING {
            return Err(flow_resource_error(
                ResourceKind::RepeatNesting,
                u64::try_from(depth).unwrap_or(u64::MAX),
                MAX_FLOW_GENERATOR_REPEAT_NESTING as u64,
            ));
        }
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(_) => {
                    let actual = total.checked_add(multiplier).unwrap_or(u64::MAX);
                    if actual > MAX_FLOW_GENERATOR_EXPANDED_OPERATIONS {
                        return Err(flow_resource_error(
                            ResourceKind::ExpandedOperations,
                            actual,
                            MAX_FLOW_GENERATOR_EXPANDED_OPERATIONS,
                        ));
                    }
                    *total = actual;
                }
                CircuitItem::RepeatBlock(repeat) => {
                    let repeated_multiplier =
                        multiplier.saturating_mul(repeat.repeat_count().get());
                    count(
                        repeat.body(),
                        repeated_multiplier,
                        depth.saturating_add(1),
                        total,
                    )?;
                }
            }
        }
        Ok(())
    }

    let mut total = 0;
    count(circuit, 1, 0, &mut total)
}

fn flow_resource_error(resource: ResourceKind, actual: u64, limit: u64) -> AnalysisError {
    ResourceLimitError::fixed_operation(ResourceOperation::FlowGeneration, resource, actual, limit)
        .into()
}
