mod budget;
mod decompose;
mod effects;
mod error_accumulator;
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

pub fn circuit_to_detector_error_model(
    circuit: &Circuit,
    options: ErrorAnalyzerOptions,
) -> AnalysisResult<DetectorErrorModel> {
    // The sparse reverse tracker is the only sensitivity-propagation engine
    // (WS2b Stage 4 deleted the original forward analyzer); with `fold_loops`
    // off it unrolls every loop, admitting only its retained output against the budget.
    reverse_fold::try_analyze(circuit, options)
}
