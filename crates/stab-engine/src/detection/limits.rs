use stab_model::{Cancellation, MemoryBudget};

/// Admission and cancellation policy for compiling and executing detection conversion.
///
/// The memory budget bounds the compiled plan, including one byte per record bit of every
/// record the plan's consumers hold, and separately bounds each session's reusable storage.
/// Repeat nesting is the shared fixed recursion envelope; expanded work is not capped.
///
/// The cancellation signal is polled while the reference sample is computed and by every session
/// of a plan compiled with these limits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DetectionConversionLimits {
    memory_budget: MemoryBudget,
    cancellation: Cancellation,
}

impl DetectionConversionLimits {
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
