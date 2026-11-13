use stab_model::{Cancellation, MemoryBudget};

/// Admission and cancellation policy for DEM sampling and replay sessions.
///
/// The memory budget bounds each session's reusable storage. Sessions stream shots through
/// bounded batches, so neither the requested shot count nor the per-shot sampling or replay work
/// is a resource limit. The cancellation signal is polled between batches and while a batch
/// walks repeat blocks.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DemSamplerLimits {
    memory_budget: MemoryBudget,
    cancellation: Cancellation,
}

impl DemSamplerLimits {
    pub const fn new(memory_budget: MemoryBudget) -> Self {
        Self {
            memory_budget,
            cancellation: Cancellation::never(),
        }
    }

    pub const fn memory_budget(&self) -> MemoryBudget {
        self.memory_budget
    }

    pub const fn cancellation(&self) -> &Cancellation {
        &self.cancellation
    }

    #[must_use]
    pub const fn with_memory_budget(mut self, memory_budget: MemoryBudget) -> Self {
        self.memory_budget = memory_budget;
        self
    }

    #[must_use]
    pub fn with_cancellation(mut self, cancellation: Cancellation) -> Self {
        self.cancellation = cancellation;
        self
    }
}
