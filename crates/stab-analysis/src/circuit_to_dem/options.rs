use stab_model::{Cancellation, MemoryBudget, Probability};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ErrorAnalyzerOptions {
    pub fold_loops: bool,
    pub decompose_errors: bool,
    pub allow_gauge_detectors: bool,
    pub ignore_decomposition_failures: bool,
    pub block_decomposition_from_introducing_remnant_edges: bool,
    pub approximate_disjoint_errors_threshold: Option<Probability>,
    /// Budget for the conservative retained size of the produced model, charged before each
    /// output item is retained. Expanded analysis work is not capped.
    pub memory_budget: MemoryBudget,
    /// Signal polled once per analyzed instruction and repeat iteration; a set signal stops the
    /// analysis with [`crate::AnalysisError::Cancelled`].
    pub cancellation: Cancellation,
}
