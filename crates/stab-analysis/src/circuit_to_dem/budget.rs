use stab_model::advanced::MAX_DEM_REPEAT_NESTING;
use stab_model::{Circuit, CircuitItem};

use super::{MAX_ANALYZER_EXPANDED_INSTRUCTIONS, MAX_ANALYZER_REPEAT_ITERATIONS};
use crate::{AnalysisResult, ResourceKind, ResourceLimitError};

#[derive(Debug, Default)]
struct AnalyzerExpansionBudget {
    expanded_instructions: u64,
    repeat_iterations: u64,
}

impl AnalyzerExpansionBudget {
    fn add_expanded_instructions(&mut self, count: u64) -> AnalysisResult<()> {
        self.expanded_instructions = self.expanded_instructions.saturating_add(count);
        if self.expanded_instructions > MAX_ANALYZER_EXPANDED_INSTRUCTIONS {
            return Err(ResourceLimitError::circuit_to_detector_error_model(
                ResourceKind::ExpandedOperations,
                self.expanded_instructions,
                MAX_ANALYZER_EXPANDED_INSTRUCTIONS,
            )
            .into());
        }
        Ok(())
    }

    fn add_repeat_iterations(&mut self, count: u64) -> AnalysisResult<()> {
        self.repeat_iterations = self.repeat_iterations.saturating_add(count);
        if self.repeat_iterations > MAX_ANALYZER_REPEAT_ITERATIONS {
            return Err(ResourceLimitError::circuit_to_detector_error_model(
                ResourceKind::RepeatIterations,
                self.repeat_iterations,
                MAX_ANALYZER_REPEAT_ITERATIONS,
            )
            .into());
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
        return Err(ResourceLimitError::circuit_to_detector_error_model(
            ResourceKind::RepeatNesting,
            depth as u64,
            MAX_DEM_REPEAT_NESTING as u64,
        )
        .into());
    }
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(_) => budget.add_expanded_instructions(multiplier)?,
            CircuitItem::RepeatBlock(repeat) => {
                let repeat_count = repeat.repeat_count().get();
                let repeated_multiplier = multiplier.saturating_mul(repeat_count);
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
