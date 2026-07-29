mod analyze;
mod flatten;
#[cfg(test)]
mod generated_qec_tests;
mod sat;

pub use analyze::{
    DisjointPauliProbabilities, ErrorAnalyzerOptions, IndependentPauliProbabilities,
    circuit_to_detector_error_model, independent_to_disjoint_xyz_errors,
    try_disjoint_to_independent_xyz_errors,
};
pub use flatten::DemFlattenLimits;
pub use sat::{
    SatMaterializationLimits, likeliest_error_sat_problem, likeliest_error_sat_problem_with_limits,
    shortest_error_sat_problem, shortest_error_sat_problem_with_limits,
};
pub use stab_analysis::LogicalErrorSearchLimits;
pub use stab_model::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemObservableId, DemRepeatBlock,
    DemTarget, DetectorErrorModel,
};

use crate::CircuitResult;
#[cfg(test)]
use crate::{CircuitError, Probability};

#[cfg(test)]
pub(crate) fn dem_instruction_detector_shift(instruction: &DemInstruction) -> CircuitResult<u64> {
    stab_model::advanced::dem_instruction_detector_shift(instruction).map_err(Into::into)
}

pub fn shortest_graphlike_undetectable_logical_error(
    model: &DetectorErrorModel,
    ignore_ungraphlike_errors: bool,
) -> CircuitResult<DetectorErrorModel> {
    stab_analysis::shortest_graphlike_undetectable_logical_error(model, ignore_ungraphlike_errors)
        .map_err(Into::into)
}

pub fn shortest_graphlike_undetectable_logical_error_with_limits(
    model: &DetectorErrorModel,
    ignore_ungraphlike_errors: bool,
    limits: LogicalErrorSearchLimits,
) -> CircuitResult<DetectorErrorModel> {
    stab_analysis::shortest_graphlike_undetectable_logical_error_with_limits(
        model,
        ignore_ungraphlike_errors,
        limits,
    )
    .map_err(Into::into)
}

pub fn find_undetectable_logical_error(
    model: &DetectorErrorModel,
    dont_explore_detection_event_sets_with_size_above: usize,
    dont_explore_edges_with_degree_above: usize,
    dont_explore_edges_increasing_symptom_degree: bool,
) -> CircuitResult<DetectorErrorModel> {
    stab_analysis::find_undetectable_logical_error(
        model,
        dont_explore_detection_event_sets_with_size_above,
        dont_explore_edges_with_degree_above,
        dont_explore_edges_increasing_symptom_degree,
    )
    .map_err(Into::into)
}

pub fn find_undetectable_logical_error_with_limits(
    model: &DetectorErrorModel,
    dont_explore_detection_event_sets_with_size_above: usize,
    dont_explore_edges_with_degree_above: usize,
    dont_explore_edges_increasing_symptom_degree: bool,
    limits: LogicalErrorSearchLimits,
) -> CircuitResult<DetectorErrorModel> {
    stab_analysis::find_undetectable_logical_error_with_limits(
        model,
        dont_explore_detection_event_sets_with_size_above,
        dont_explore_edges_with_degree_above,
        dont_explore_edges_increasing_symptom_degree,
        limits,
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests;
