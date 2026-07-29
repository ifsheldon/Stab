use stab_model::{Circuit, CircuitInstruction, CircuitItem};

use crate::AnalysisResult;

pub(super) fn measurement_generator_instructions(
    circuit: &Circuit,
) -> AnalysisResult<Vec<CircuitInstruction>> {
    if circuit
        .items()
        .iter()
        .any(|item| matches!(item, CircuitItem::RepeatBlock(_)))
    {
        return crate::flattened_circuit_operations(circuit);
    }
    Ok(circuit
        .items()
        .iter()
        .filter_map(|item| match item {
            CircuitItem::Instruction(instruction) => Some(instruction.clone()),
            CircuitItem::RepeatBlock(_) => None,
        })
        .collect())
}
