use stab_algebra::Flow;
use stab_model::{Circuit, CircuitItem, MemoryBudget};

use crate::{AnalysisError, AnalysisResult, ResourceKind, ResourceLimitError, ResourceOperation};

/// Owned flow-generator rows are admitted against the default memory budget.
pub(crate) const FLOW_GENERATOR_MEMORY_BUDGET: MemoryBudget = MemoryBudget::DEFAULT;
const MAX_FLOW_GENERATOR_REPEAT_NESTING: usize = 256;

/// Admits the `2n` measurement-free flow rows that the ignored-only and unitary generators return,
/// two `n`-qubit Pauli strings each, against the default flow-generator budget. Building them is
/// linear in those bytes.
pub(super) fn validate_measurement_free_flow_generator_resources(
    qubit_count: usize,
) -> AnalysisResult<()> {
    validate_measurement_rich_flow_generator_resources(qubit_count, 0)
}

pub(crate) fn validate_measurement_rich_flow_generator_resources(
    qubit_count: usize,
    measurement_count: usize,
) -> AnalysisResult<()> {
    let projected_bytes =
        measurement_rich_flow_generator_projected_bytes(qubit_count, measurement_count);
    if !FLOW_GENERATOR_MEMORY_BUDGET.admits(u128::from(projected_bytes)) {
        return Err(ResourceLimitError::memory_budget(
            ResourceOperation::FlowGeneration,
            ResourceKind::ProjectedPayloadBytes,
            projected_bytes,
            FLOW_GENERATOR_MEMORY_BUDGET.bytes(),
        )
        .into());
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

/// Rejects repeat nesting beyond the fixed envelope before the solver recurses. Expanded solver
/// work is not capped: it mutates rows that the byte budget already bounds.
pub(super) fn validate_flow_generator_repeat_nesting(circuit: &Circuit) -> AnalysisResult<()> {
    fn count(circuit: &Circuit, depth: usize) -> AnalysisResult<()> {
        if depth > MAX_FLOW_GENERATOR_REPEAT_NESTING {
            return Err(flow_resource_error(
                ResourceKind::RepeatNesting,
                u64::try_from(depth).unwrap_or(u64::MAX),
                MAX_FLOW_GENERATOR_REPEAT_NESTING as u64,
            ));
        }
        for item in circuit.items() {
            if let CircuitItem::RepeatBlock(repeat) = item {
                count(repeat.body(), depth.saturating_add(1))?;
            }
        }
        Ok(())
    }

    count(circuit, 0)
}

fn flow_resource_error(resource: ResourceKind, actual: u64, limit: u64) -> AnalysisError {
    ResourceLimitError::fixed_operation(ResourceOperation::FlowGeneration, resource, actual, limit)
        .into()
}
