use stab_algebra::{FlexPauliString, PauliBasis, PauliPhase, StabilizerError};
use stab_model::advanced::CancellationCountdown;
use stab_model::{DemTarget, MemoryBudget, QubitId};

use super::{SparseReverseFrameTracker, qubit_index};
use crate::{AnalysisError, AnalysisOperation, AnalysisResult, ResourceKind, ResourceLimitError};

const REGION_CHUNK_QUBITS: usize = CancellationCountdown::INTERVAL as usize;

impl SparseReverseFrameTracker {
    /// Returns the full-width region of `target`, admitted against `budget`.
    pub(crate) fn region_for_target(
        &self,
        target: DemTarget,
        budget: MemoryBudget,
    ) -> AnalysisResult<FlexPauliString> {
        self.region_for_target_with_len(
            target,
            self.qubit_count,
            budget,
            AnalysisOperation::DetectingRegions,
        )
    }

    /// Returns the region of `target` only as wide as its support, admitted against `budget` and
    /// refused as flow checking.
    pub(crate) fn compact_region_for_target(
        &self,
        target: DemTarget,
        budget: MemoryBudget,
    ) -> AnalysisResult<FlexPauliString> {
        let signal = self.cancellation_signal();
        let mut countdown = CancellationCountdown::new();
        let mut max_qubit: Option<u32> = None;
        for (qubit, targets) in self.xs.iter().chain(&self.zs) {
            countdown.tick(signal)?;
            if targets.contains(&target) {
                let id = qubit.get();
                max_qubit = Some(max_qubit.map_or(id, |prev| prev.max(id)));
            }
        }
        let len = max_qubit
            .map(|max_id| qubit_index(QubitId::new(max_id)?).map(|index| index + 1))
            .transpose()?
            .unwrap_or(0);
        self.region_for_target_with_len(target, len, budget, AnalysisOperation::FlowCheck)
    }

    fn region_for_target_with_len(
        &self,
        target: DemTarget,
        len: usize,
        budget: MemoryBudget,
        operation: AnalysisOperation,
    ) -> AnalysisResult<FlexPauliString> {
        let signal = self.cancellation_signal();
        signal.check()?;
        let mut countdown = CancellationCountdown::new();
        let mut bases = Vec::new();
        let mut remaining = len;
        while remaining > 0 {
            signal.check()?;
            let step = remaining.min(REGION_CHUNK_QUBITS);
            // The allocator, not the budget, refused these bases: the region itself is admitted
            // against the budget when the Pauli string is built below.
            bases.try_reserve(step).map_err(|_| {
                AnalysisError::allocation_failed(
                    operation,
                    format!("could not reserve {len} Pauli bases for a detecting region"),
                )
            })?;
            bases.resize(bases.len() + step, PauliBasis::I);
            remaining -= step;
        }
        for (qubit, xs) in &self.xs {
            countdown.tick(signal)?;
            if xs.contains(&target) {
                let index = qubit_index(*qubit)?;
                let basis = bases.get_mut(index).ok_or_else(|| {
                    AnalysisError::internal(
                        AnalysisOperation::ReverseFrameTracking,
                        format!(
                            "X-basis sensitivity qubit {} is outside its sparse region",
                            qubit.get()
                        ),
                    )
                })?;
                *basis = PauliBasis::X;
            }
        }
        for (qubit, zs) in &self.zs {
            countdown.tick(signal)?;
            if zs.contains(&target) {
                let index = qubit_index(*qubit)?;
                let basis = bases.get_mut(index).ok_or_else(|| {
                    AnalysisError::internal(
                        AnalysisOperation::ReverseFrameTracking,
                        format!(
                            "Z-basis sensitivity qubit {} is outside its sparse region",
                            qubit.get()
                        ),
                    )
                })?;
                *basis = match *basis {
                    PauliBasis::I => PauliBasis::Z,
                    PauliBasis::X => PauliBasis::Y,
                    actual => {
                        return Err(AnalysisError::internal(
                            AnalysisOperation::ReverseFrameTracking,
                            format!(
                                "unexpected {actual:?} basis while building sparse region for qubit {}",
                                qubit.get()
                            ),
                        ));
                    }
                };
            }
        }
        signal.check()?;
        FlexPauliString::from_phase_and_bases_with_budget(PauliPhase::Plus, bases, budget).map_err(
            |error| match error {
                StabilizerError::MemoryBudgetExceeded { exceeded, .. } => {
                    ResourceLimitError::budget_refusal(
                        operation,
                        ResourceKind::MaterializedBytes,
                        exceeded,
                    )
                    .into()
                }
                error => AnalysisError::internal(
                    AnalysisOperation::ReverseFrameTracking,
                    format!("failed to build detecting region for {target}: {error}"),
                ),
            },
        )
    }
}
