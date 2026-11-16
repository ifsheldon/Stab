use stab_algebra::{AlgebraMemoryBudget, FlexPauliString, PauliBasis, PauliPhase, StabilizerError};
use stab_model::{DemTarget, QubitId};

use super::{SparseReverseFrameTracker, qubit_index};
use crate::{AnalysisError, AnalysisResult, ResourceKind, ResourceLimitError, ResourceOperation};

impl SparseReverseFrameTracker {
    /// Returns the full-width region of `target`, admitted against `budget`.
    pub(crate) fn region_for_target(
        &self,
        target: DemTarget,
        budget: AlgebraMemoryBudget,
    ) -> AnalysisResult<FlexPauliString> {
        self.region_for_target_with_len(
            target,
            self.qubit_count,
            budget,
            ResourceOperation::DetectingRegions,
        )
    }

    /// Returns the region of `target` only as wide as its support, admitted against `budget` and
    /// refused as flow checking.
    pub(crate) fn compact_region_for_target(
        &self,
        target: DemTarget,
        budget: AlgebraMemoryBudget,
    ) -> AnalysisResult<FlexPauliString> {
        let len = self
            .xs
            .iter()
            .chain(&self.zs)
            .filter_map(|(qubit, targets)| targets.contains(&target).then_some(qubit.get()))
            .max()
            .map(|max_qubit| qubit_index(QubitId::new(max_qubit)?).map(|index| index + 1))
            .transpose()?
            .unwrap_or(0);
        self.region_for_target_with_len(target, len, budget, ResourceOperation::FlowCheck)
    }

    fn region_for_target_with_len(
        &self,
        target: DemTarget,
        len: usize,
        budget: AlgebraMemoryBudget,
        operation: ResourceOperation,
    ) -> AnalysisResult<FlexPauliString> {
        let mut bases = vec![PauliBasis::I; len];
        for (qubit, xs) in &self.xs {
            if xs.contains(&target) {
                let index = qubit_index(*qubit)?;
                let basis = bases.get_mut(index).ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(format!(
                        "X-basis sensitivity qubit {} is outside its sparse region",
                        qubit.get()
                    ))
                })?;
                *basis = PauliBasis::X;
            }
        }
        for (qubit, zs) in &self.zs {
            if zs.contains(&target) {
                let index = qubit_index(*qubit)?;
                let basis = bases.get_mut(index).ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(format!(
                        "Z-basis sensitivity qubit {} is outside its sparse region",
                        qubit.get()
                    ))
                })?;
                *basis = match *basis {
                    PauliBasis::I => PauliBasis::Z,
                    PauliBasis::X => PauliBasis::Y,
                    actual => {
                        return Err(AnalysisError::invalid_detector_error_model(format!(
                            "unexpected {actual:?} basis while building sparse region for qubit {}",
                            qubit.get()
                        )));
                    }
                };
            }
        }
        FlexPauliString::from_phase_and_bases_with_budget(PauliPhase::Plus, bases, budget).map_err(
            |error| match error {
                StabilizerError::MemoryBudgetExceeded {
                    requested_bytes,
                    budget_bytes,
                    ..
                } => ResourceLimitError::memory_budget(
                    operation,
                    ResourceKind::MaterializedBytes,
                    u64::try_from(requested_bytes).unwrap_or(u64::MAX),
                    budget_bytes,
                )
                .into(),
                error => AnalysisError::invalid_detector_error_model(format!(
                    "failed to build detecting region for {target}: {error}"
                )),
            },
        )
    }
}
