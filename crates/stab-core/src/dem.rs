mod analyze;
mod arena_index;
mod error_traversal;
mod flatten;
#[cfg(test)]
mod generated_qec_tests;
mod graphlike;
mod hyper;
mod sat;
mod search_budget;
mod traversal;

pub use analyze::{
    DisjointPauliProbabilities, ErrorAnalyzerOptions, IndependentPauliProbabilities,
    circuit_to_detector_error_model, independent_to_disjoint_xyz_errors,
    try_disjoint_to_independent_xyz_errors,
};
pub use flatten::DemFlattenLimits;
pub(crate) use flatten::validate_flattening_budget_with_limits;
pub use sat::{
    SatMaterializationLimits, likeliest_error_sat_problem, likeliest_error_sat_problem_with_limits,
    shortest_error_sat_problem, shortest_error_sat_problem_with_limits,
};
pub use search_budget::LogicalErrorSearchLimits;
pub use stab_model::{
    DemDetectorId, DemFlattenedInstructionIter, DemInstruction, DemInstructionKind, DemItem,
    DemObservableId, DemRepeatBlock, DemTarget, DetectorErrorModel,
};

pub(crate) use stab_model::advanced::MAX_DEM_REPEAT_NESTING;
pub(crate) use traversal::{
    DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemItem, FoldedDemTraversal,
    FoldedDemVisitor,
};

#[cfg(test)]
use crate::{CircuitError, Probability};
use crate::{CircuitResult, DemRepeatCount};

pub(crate) const MAX_DEM_FLATTEN_REPEAT_UNROLL: u64 = 100_000;
pub(crate) const MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
pub(crate) const MAX_DEM_FLATTEN_REPEAT_ITERATIONS: u64 = 1_000_000;
pub(crate) const MAX_DEM_FLATTEN_TARGET_OCCURRENCES: u64 = 32_000_000;
pub(crate) const MAX_DEM_FLATTEN_ARGUMENT_VALUES: u64 = 16_000_000;
pub(crate) const MAX_DEM_FLATTEN_MATERIALIZED_BYTES: u64 = 512 * 1024 * 1024;

pub(crate) fn dem_try_reserve_items_exact(
    model: &mut DetectorErrorModel,
    additional: usize,
) -> CircuitResult<()> {
    stab_model::advanced::dem_try_reserve_items_exact(model, additional).map_err(Into::into)
}

pub(crate) fn dem_instruction_with_tag_bytes(
    kind: DemInstructionKind,
    args: Vec<f64>,
    targets: Vec<DemTarget>,
    tag: Option<&[u8]>,
) -> CircuitResult<DemInstruction> {
    stab_model::advanced::dem_instruction_with_tag_bytes(kind, args, targets, tag)
        .map_err(Into::into)
}

pub(crate) fn dem_repeat_block_with_tag_bytes(
    repeat_count: DemRepeatCount,
    body: DetectorErrorModel,
    tag: Option<&[u8]>,
) -> DemRepeatBlock {
    stab_model::advanced::dem_repeat_block_with_tag_bytes(repeat_count, body, tag)
}

pub(crate) fn dem_instruction_clear_tag(instruction: &mut DemInstruction) {
    stab_model::advanced::dem_instruction_clear_tag(instruction);
}

pub(crate) fn dem_instruction_detector_shift(instruction: &DemInstruction) -> CircuitResult<u64> {
    stab_model::advanced::dem_instruction_detector_shift(instruction).map_err(Into::into)
}

pub fn shortest_graphlike_undetectable_logical_error(
    model: &DetectorErrorModel,
    ignore_ungraphlike_errors: bool,
) -> CircuitResult<DetectorErrorModel> {
    graphlike::shortest_graphlike_undetectable_logical_error(model, ignore_ungraphlike_errors)
}

pub fn shortest_graphlike_undetectable_logical_error_with_limits(
    model: &DetectorErrorModel,
    ignore_ungraphlike_errors: bool,
    limits: LogicalErrorSearchLimits,
) -> CircuitResult<DetectorErrorModel> {
    graphlike::shortest_graphlike_undetectable_logical_error_with_limits(
        model,
        ignore_ungraphlike_errors,
        limits,
    )
}

pub fn find_undetectable_logical_error(
    model: &DetectorErrorModel,
    dont_explore_detection_event_sets_with_size_above: usize,
    dont_explore_edges_with_degree_above: usize,
    dont_explore_edges_increasing_symptom_degree: bool,
) -> CircuitResult<DetectorErrorModel> {
    hyper::find_undetectable_logical_error(
        model,
        dont_explore_detection_event_sets_with_size_above,
        dont_explore_edges_with_degree_above,
        dont_explore_edges_increasing_symptom_degree,
    )
}

pub fn find_undetectable_logical_error_with_limits(
    model: &DetectorErrorModel,
    dont_explore_detection_event_sets_with_size_above: usize,
    dont_explore_edges_with_degree_above: usize,
    dont_explore_edges_increasing_symptom_degree: bool,
    limits: LogicalErrorSearchLimits,
) -> CircuitResult<DetectorErrorModel> {
    hyper::find_undetectable_logical_error_with_limits(
        model,
        dont_explore_detection_event_sets_with_size_above,
        dont_explore_edges_with_degree_above,
        dont_explore_edges_increasing_symptom_degree,
        limits,
    )
}

#[cfg(test)]
mod tests;
