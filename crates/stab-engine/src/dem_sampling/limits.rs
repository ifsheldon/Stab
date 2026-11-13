use stab_model::MemoryBudget;

/// Admission policy for DEM sampling and replay sessions.
///
/// The memory budget bounds each session's reusable storage. Sessions stream shots through
/// bounded batches, so neither the requested shot count nor the per-shot sampling or replay work
/// is a resource limit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DemSamplerLimits {
    memory_budget: MemoryBudget,
}

impl DemSamplerLimits {
    pub const fn new(memory_budget: MemoryBudget) -> Self {
        Self { memory_budget }
    }

    pub const fn memory_budget(self) -> MemoryBudget {
        self.memory_budget
    }

    #[must_use]
    pub const fn with_memory_budget(mut self, memory_budget: MemoryBudget) -> Self {
        self.memory_budget = memory_budget;
        self
    }
}
