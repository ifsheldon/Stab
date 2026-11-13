use stab_model::advanced::MAX_DEM_REPEAT_NESTING;
use stab_model::{Circuit, CircuitInstruction, CircuitItem};

use crate::{AnalysisError, AnalysisResult};

use super::is_pure_noise;

/// Validates the supported ErrorMatcher subset before any candidate work. Expanded work is not
/// capped; repeat nesting stays the shared fixed envelope.
pub(super) fn validate_error_matcher_circuit(circuit: &Circuit) -> AnalysisResult<()> {
    validate_error_matcher_circuit_items(circuit, 0, false)
}

fn validate_error_matcher_circuit_items(
    circuit: &Circuit,
    depth: usize,
    inside_repeat: bool,
) -> AnalysisResult<()> {
    if depth > MAX_DEM_REPEAT_NESTING {
        return Err(AnalysisError::invalid_detector_error_model(format!(
            "ErrorMatcher repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
        )));
    }
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if inside_repeat && has_repeat_contained_stochastic_effect(instruction)? {
                    return Err(AnalysisError::invalid_detector_error_model(
                        "ErrorMatcher does not yet support repeat-contained noise",
                    ));
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                validate_error_matcher_circuit_items(repeat.body(), depth + 1, true)?;
            }
        }
    }
    Ok(())
}

fn has_repeat_contained_stochastic_effect(
    instruction: &CircuitInstruction,
) -> AnalysisResult<bool> {
    let gate_name = instruction.gate().canonical_name();
    if is_pure_noise(gate_name)
        || matches!(gate_name, "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1")
    {
        return Ok(instruction
            .args()
            .iter()
            .any(|probability| *probability != 0.0));
    }
    if matches!(
        gate_name,
        "M" | "MX" | "MY" | "MR" | "MRX" | "MRY" | "MXX" | "MYY" | "MZZ" | "MPP"
    ) {
        return Ok(instruction
            .probability_argument()?
            .is_some_and(|probability| probability.get() != 0.0));
    }
    Ok(false)
}
