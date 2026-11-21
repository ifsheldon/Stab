use stab_model::{Cancellation, MemoryBudget};

/// Admission and cancellation policy of [`crate::detection_record_width`] and
/// [`crate::circuit_reference_signs`], and the limits a detection compiler hands its plan.
///
/// The memory budget bounds the compiled plan, including one byte per record bit of every
/// record the plan's consumers hold, and separately bounds each session's reusable storage.
/// Repeat nesting is the shared fixed recursion envelope; expanded work is not capped.
///
/// The cancellation signal is polled while the reference sample is computed and by every session
/// of a plan compiled with these limits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DetectionConversionLimits {
    /// Budget for the compiled conversion and, by default, for each session's storage.
    pub memory_budget: MemoryBudget,
    /// Signal polled while compiling and by every session of the plan.
    pub cancellation: Cancellation,
}
