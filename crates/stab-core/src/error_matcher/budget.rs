use crate::dem::MAX_DEM_REPEAT_NESTING;
use crate::{Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult};

use super::is_pure_noise;

const MAX_ERROR_MATCHER_REPEAT_UNROLL: u64 = 100_000;
const MAX_ERROR_MATCHER_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
const MAX_ERROR_MATCHER_REPEAT_ITERATIONS: u64 = 1_000_000;

#[derive(Debug, Default)]
struct ErrorMatcherExpansionBudget {
    expanded_instructions: u64,
    repeat_iterations: u64,
}

impl ErrorMatcherExpansionBudget {
    fn add_expanded_instructions(&mut self, count: u64) -> CircuitResult<()> {
        self.expanded_instructions =
            self.expanded_instructions
                .checked_add(count)
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "ErrorMatcher expanded instruction count overflowed",
                    )
                })?;
        if self.expanded_instructions > MAX_ERROR_MATCHER_EXPANDED_INSTRUCTIONS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "ErrorMatcher currently supports at most {MAX_ERROR_MATCHER_EXPANDED_INSTRUCTIONS} expanded instructions, got at least {}",
                self.expanded_instructions
            )));
        }
        Ok(())
    }

    fn add_repeat_iterations(&mut self, count: u64) -> CircuitResult<()> {
        self.repeat_iterations = self.repeat_iterations.checked_add(count).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "ErrorMatcher repeat iteration count overflowed",
            )
        })?;
        if self.repeat_iterations > MAX_ERROR_MATCHER_REPEAT_ITERATIONS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "ErrorMatcher currently supports at most {MAX_ERROR_MATCHER_REPEAT_ITERATIONS} expanded repeat iterations, got at least {}",
                self.repeat_iterations
            )));
        }
        Ok(())
    }
}

pub(super) fn validate_error_matcher_circuit(circuit: &Circuit) -> CircuitResult<()> {
    let mut budget = ErrorMatcherExpansionBudget::default();
    validate_error_matcher_circuit_items(circuit, 1, 0, false, &mut budget)
}

fn validate_error_matcher_circuit_items(
    circuit: &Circuit,
    multiplier: u64,
    depth: usize,
    inside_repeat: bool,
    budget: &mut ErrorMatcherExpansionBudget,
) -> CircuitResult<()> {
    if depth > MAX_DEM_REPEAT_NESTING {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "ErrorMatcher repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
        )));
    }
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                budget.add_expanded_instructions(multiplier)?;
                if inside_repeat && has_repeat_contained_stochastic_effect(instruction)? {
                    return Err(CircuitError::invalid_detector_error_model(
                        "ErrorMatcher does not yet support repeat-contained noise",
                    ));
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let repeat_count = repeat.repeat_count().get();
                if repeat_count > MAX_ERROR_MATCHER_REPEAT_UNROLL {
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "ErrorMatcher currently supports repeat counts up to {MAX_ERROR_MATCHER_REPEAT_UNROLL}, got {repeat_count}"
                    )));
                }
                let repeated_multiplier =
                    multiplier.checked_mul(repeat_count).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "ErrorMatcher repeat expansion count overflowed",
                        )
                    })?;
                budget.add_repeat_iterations(repeated_multiplier)?;
                validate_error_matcher_circuit_items(
                    repeat.body(),
                    repeated_multiplier,
                    depth + 1,
                    true,
                    budget,
                )?;
            }
        }
    }
    Ok(())
}

fn has_repeat_contained_stochastic_effect(instruction: &CircuitInstruction) -> CircuitResult<bool> {
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
