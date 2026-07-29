use stab_model::advanced::MAX_DEM_REPEAT_NESTING;
use stab_model::{Circuit, CircuitItem};

use super::MAX_ANALYZER_REPEAT_UNROLL;
use crate::{AnalysisError, AnalysisResult};

const MAX_ANALYZER_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
const MAX_ANALYZER_REPEAT_ITERATIONS: u64 = 1_000_000;

#[derive(Debug, Default)]
struct AnalyzerExpansionBudget {
    expanded_instructions: u64,
    repeat_iterations: u64,
}

impl AnalyzerExpansionBudget {
    fn add_expanded_instructions(&mut self, count: u64) -> AnalysisResult<()> {
        self.expanded_instructions =
            self.expanded_instructions
                .checked_add(count)
                .ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(
                        "analyze_errors expanded instruction count overflowed",
                    )
                })?;
        if self.expanded_instructions > MAX_ANALYZER_EXPANDED_INSTRUCTIONS {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "analyze_errors currently supports at most {MAX_ANALYZER_EXPANDED_INSTRUCTIONS} expanded instructions, got at least {}",
                self.expanded_instructions
            )));
        }
        Ok(())
    }

    fn add_repeat_iterations(&mut self, count: u64) -> AnalysisResult<()> {
        self.repeat_iterations = self.repeat_iterations.checked_add(count).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "analyze_errors repeat iteration count overflowed",
            )
        })?;
        if self.repeat_iterations > MAX_ANALYZER_REPEAT_ITERATIONS {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "analyze_errors currently supports at most {MAX_ANALYZER_REPEAT_ITERATIONS} expanded repeat iterations, got at least {}",
                self.repeat_iterations
            )));
        }
        Ok(())
    }
}

pub(super) fn validate_analyzer_expansion_budget(circuit: &Circuit) -> AnalysisResult<()> {
    let mut budget = AnalyzerExpansionBudget::default();
    validate_analyzer_expansion_budget_items(circuit, 1, 0, &mut budget)
}

fn validate_analyzer_expansion_budget_items(
    circuit: &Circuit,
    multiplier: u64,
    depth: usize,
    budget: &mut AnalyzerExpansionBudget,
) -> AnalysisResult<()> {
    if depth > MAX_DEM_REPEAT_NESTING {
        return Err(AnalysisError::invalid_detector_error_model(format!(
            "analyze_errors repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
        )));
    }
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(_) => budget.add_expanded_instructions(multiplier)?,
            CircuitItem::RepeatBlock(repeat) => {
                let repeat_count = repeat.repeat_count().get();
                if repeat_count > MAX_ANALYZER_REPEAT_UNROLL {
                    return Err(AnalysisError::invalid_detector_error_model(format!(
                        "analyze_errors currently supports repeat counts up to {MAX_ANALYZER_REPEAT_UNROLL}, got {repeat_count}"
                    )));
                }
                let repeated_multiplier =
                    multiplier.checked_mul(repeat_count).ok_or_else(|| {
                        AnalysisError::invalid_detector_error_model(
                            "analyze_errors repeat expansion count overflowed",
                        )
                    })?;
                budget.add_repeat_iterations(repeated_multiplier)?;
                validate_analyzer_expansion_budget_items(
                    repeat.body(),
                    repeated_multiplier,
                    depth + 1,
                    budget,
                )?;
            }
        }
    }
    Ok(())
}
