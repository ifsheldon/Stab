use stab_model::CircuitInstruction;

use crate::{AnalysisError, AnalysisResult, ResourceKind, ResourceLimitError};

#[derive(Debug)]
pub(crate) struct AnalyzerProbeBudget {
    consumed_steps: u64,
    max_steps: u64,
}

pub(crate) trait ReverseTrackerWorkBudget {
    fn admit_probe_iteration(&mut self) -> AnalysisResult<()>;
    fn admit_instruction(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()>;
    fn admit_recurrence_search(&mut self) -> AnalysisResult<()> {
        Ok(())
    }
}

pub(crate) enum ReverseTrackerBudget<'a> {
    Unlimited,
    Metered(&'a mut dyn ReverseTrackerWorkBudget),
}

impl ReverseTrackerBudget<'_> {
    pub(crate) fn admit_probe_iteration(&mut self) -> AnalysisResult<()> {
        match self {
            Self::Unlimited => Ok(()),
            Self::Metered(budget) => budget.admit_probe_iteration(),
        }
    }

    pub(crate) fn admit_instruction(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        match self {
            Self::Unlimited => Ok(()),
            Self::Metered(budget) => budget.admit_instruction(instruction),
        }
    }

    pub(crate) fn admit_recurrence_search(&mut self) -> AnalysisResult<()> {
        match self {
            Self::Unlimited => Ok(()),
            Self::Metered(budget) => budget.admit_recurrence_search(),
        }
    }
}

impl AnalyzerProbeBudget {
    pub(crate) fn new(max_steps: u64) -> Self {
        Self {
            consumed_steps: 0,
            max_steps,
        }
    }

    fn consume_work_unit(&mut self) -> AnalysisResult<()> {
        let next = self.consumed_steps.checked_add(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "analyze_errors recurrence probe step count overflowed",
            )
        })?;
        if next > self.max_steps {
            return Err(ResourceLimitError::circuit_to_detector_error_model(
                ResourceKind::ExpandedOperations,
                next,
                self.max_steps,
            )
            .into());
        }
        self.consumed_steps = next;
        Ok(())
    }
}

impl ReverseTrackerWorkBudget for AnalyzerProbeBudget {
    fn admit_probe_iteration(&mut self) -> AnalysisResult<()> {
        self.consume_work_unit()
    }

    fn admit_instruction(&mut self, _instruction: &CircuitInstruction) -> AnalysisResult<()> {
        self.consume_work_unit()
    }
}
