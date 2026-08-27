mod budget;
mod decompose;
mod effects;
mod error_decomp;
mod options;
mod probabilities;
mod reverse_fold;

use stab_model::{Circuit, DetectorErrorModel};

use crate::AnalysisResult;
use effects::AnalyzerPauli;
pub use error_decomp::{
    DisjointPauliProbabilities, IndependentPauliProbabilities, independent_to_disjoint_xyz_errors,
    try_disjoint_to_independent_xyz_errors,
};
pub use options::ErrorAnalyzerOptions;

const MAX_ANALYZER_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
const MAX_ANALYZER_REPEAT_ITERATIONS: u64 = 1_000_000;

pub(super) type AnalyzerTag = Box<[u8]>;

pub(super) fn owned_tag(tag: Option<&[u8]>) -> Option<AnalyzerTag> {
    tag.map(Box::<[u8]>::from)
}

pub fn circuit_to_detector_error_model(
    circuit: &Circuit,
    options: ErrorAnalyzerOptions,
) -> AnalysisResult<DetectorErrorModel> {
    // The sparse reverse tracker is the only sensitivity-propagation engine
    // (WS2b Stage 4 deleted the original forward analyzer); with `fold_loops`
    // off it unrolls loops under the documented expansion budget.
    reverse_fold::try_analyze(circuit, options)
}
