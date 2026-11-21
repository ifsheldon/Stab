use stab_algebra::Flow;
use stab_model::{Circuit, MemoryBudget, ParseLimits};

use crate::{AnalysisError, AnalysisOperation, AnalysisResult, ResourceKind, ResourceLimitError};

/// Admits owned flow rows against the caller's flow-generator budget: `2n + m` rows for `n`
/// qubits and `m` measurements. The ignored-only and unitary generators return the `2n`
/// measurement-free rows, two `n`-qubit Pauli strings each, and building them is linear in those
/// bytes.
pub(super) fn admit_flow_generator_rows(
    qubit_count: usize,
    measurement_count: usize,
    budget: MemoryBudget,
) -> AnalysisResult<()> {
    let projected_bytes =
        measurement_rich_flow_generator_projected_bytes(qubit_count, measurement_count);
    if !budget.admits(u128::from(projected_bytes)) {
        return Err(ResourceLimitError::memory_budget(
            AnalysisOperation::FlowGeneration,
            ResourceKind::ProjectedPayloadBytes,
            projected_bytes,
            budget,
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
    crate::folded_walk::check_repeat_nesting(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        |depth, limit| flow_resource_error(ResourceKind::RepeatNesting, depth, limit),
    )
}

fn flow_resource_error(resource: ResourceKind, actual: u64, limit: u64) -> AnalysisError {
    ResourceLimitError::fixed_operation(AnalysisOperation::FlowGeneration, resource, actual, limit)
        .into()
}
