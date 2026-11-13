use stab_model::MemoryBudget;

/// Admission policy for compiling and executing detection conversion.
///
/// The memory budget bounds the compiled plan, including one byte per record bit of every
/// record the plan's consumers hold, and separately bounds each session's reusable storage.
/// Repeat nesting is the shared fixed recursion envelope; expanded work is not capped.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DetectionConversionLimits {
    memory_budget: MemoryBudget,
}

impl DetectionConversionLimits {
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
