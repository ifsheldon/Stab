use stab_model::{Cancellation, MemoryBudget, Probability};

/// Options of [`crate::circuit_to_detector_error_model`].
///
/// The default unrolls every loop, does not decompose errors, rejects gauge detectors, refuses
/// noise that needs approximation, and admits the output against [`MemoryBudget::DEFAULT`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ErrorAnalyzerOptions {
    /// Whether repeat blocks are analyzed by unrolling them or by folding a detected period.
    pub loops: LoopFolding,
    /// How errors are decomposed into graphlike components; `None` does not decompose.
    pub decomposition: Option<ErrorDecomposition>,
    /// Whether a detector with a random, rather than deterministic, result is an error.
    pub gauge_detectors: GaugeDetectors,
    /// Largest probability for which a disjoint channel is approximated by independent errors;
    /// `None` refuses every channel that needs the approximation.
    pub approximate_disjoint_errors: Option<Probability>,
    /// Budget for the conservative retained size of the produced model, charged before each
    /// output item is retained. Expanded analysis work is not capped.
    pub memory_budget: MemoryBudget,
    /// Signal polled once per analyzed instruction and repeat iteration; a set signal stops the
    /// analysis with [`crate::AnalysisError::Cancelled`].
    pub cancellation: Cancellation,
}

/// How error analysis treats repeat blocks.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum LoopFolding {
    /// Analyze every iteration of every repeat block.
    #[default]
    Unroll,
    /// Search each repeat block for a period and emit it as a folded `repeat` block, as Stim's
    /// `fold_loops` does.
    Fold,
}

/// Whether error analysis accepts gauge detectors.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum GaugeDetectors {
    /// Fail on a detector that anticommutes with a stabilizer of the circuit.
    #[default]
    Reject,
    /// Eliminate such detector gauges instead of failing, as Stim's `allow_gauge_detectors`
    /// does.
    Allow,
}

/// Decomposition of composite errors into graphlike components, as Stim's `decompose_errors`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ErrorDecomposition {
    /// What happens to an error that cannot be decomposed.
    pub on_failure: DecompositionFailure,
    /// Whether a decomposition may introduce a remnant edge that no other error has.
    pub remnant_edges: RemnantEdges,
}

/// What error decomposition does with an error it cannot decompose.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum DecompositionFailure {
    /// Fail the analysis.
    #[default]
    Fail,
    /// Keep the error undecomposed, as Stim's `ignore_decomposition_failures` does.
    Ignore,
}

/// Whether error decomposition may introduce remnant edges.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum RemnantEdges {
    /// Accept a decomposition that introduces an edge no other error has.
    #[default]
    Allow,
    /// Refuse such a decomposition, as Stim's
    /// `block_decomposition_from_introducing_remnant_edges` does.
    Block,
}
