use stab_model::{Cancellation, MemoryBudget};

use super::DemSamplingPlan;

/// Options of one DEM sampling session; see [`DemSamplingPlan::session`].
///
/// The default draws its seed from entropy and runs under the plan's memory budget and
/// cancellation signal.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DemSamplingSessionOptions {
    /// How the session's counter-based sample streams are seeded.
    pub random: crate::RandomPolicy,
    /// Budget for the session's reusable storage; `None` uses the plan's
    /// [`crate::DemSamplingCompiler::memory_budget`].
    pub memory_budget: Option<MemoryBudget>,
    /// Signal the session polls between batches and while sampling a batch; `None` uses the
    /// plan's [`crate::DemSamplingCompiler::cancellation`].
    pub cancellation: Option<Cancellation>,
}

/// Options of one DEM sampled-error replay session, and of the replay input and record store
/// budgeted beside it; see [`DemSamplingPlan::replay_session`].
///
/// The default runs under the plan's memory budget and cancellation signal.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DemReplaySessionOptions {
    /// Budget for the replay session's reusable storage; `None` uses the plan's
    /// [`crate::DemSamplingCompiler::memory_budget`].
    pub memory_budget: Option<MemoryBudget>,
    /// Signal the replay polls between batches and while replaying a batch; `None` uses the
    /// plan's [`crate::DemSamplingCompiler::cancellation`].
    pub cancellation: Option<Cancellation>,
}

/// The memory budget and cancellation signal one DEM session runs under, resolved from its
/// options and its plan.
///
/// The memory budget bounds each session's reusable storage. Sessions stream shots through
/// bounded batches, so neither the requested shot count nor the per-shot sampling or replay work
/// is a resource limit. The cancellation signal is polled between batches and while a batch
/// walks repeat blocks.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DemSamplerLimits {
    pub(crate) memory_budget: MemoryBudget,
    pub(crate) cancellation: Cancellation,
}

impl DemSamplerLimits {
    #[cfg(test)]
    pub(crate) const fn new(memory_budget: MemoryBudget) -> Self {
        Self {
            memory_budget,
            cancellation: Cancellation::never(),
        }
    }

    pub(crate) const fn memory_budget(&self) -> MemoryBudget {
        self.memory_budget
    }

    pub(crate) const fn cancellation(&self) -> &Cancellation {
        &self.cancellation
    }

    /// The limits of a session whose options set `memory_budget` and `cancellation`, each
    /// falling back to `plan`'s compile limits when unset.
    pub(crate) fn resolve(
        plan: &DemSamplingPlan,
        memory_budget: Option<MemoryBudget>,
        cancellation: Option<&Cancellation>,
    ) -> Self {
        let compiled = &plan.inner.limits;
        Self {
            memory_budget: memory_budget.unwrap_or(compiled.memory_budget),
            cancellation: cancellation.unwrap_or(&compiled.cancellation).clone(),
        }
    }
}
