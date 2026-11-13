use stab_model::CircuitInstruction;

use crate::AnalysisResult;

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
