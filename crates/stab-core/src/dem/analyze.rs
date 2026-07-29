use crate::{Circuit, CircuitResult, DetectorErrorModel, Probability};

pub use stab_analysis::{
    DisjointPauliProbabilities, ErrorAnalyzerOptions, IndependentPauliProbabilities,
};

pub fn circuit_to_detector_error_model(
    circuit: &Circuit,
    options: ErrorAnalyzerOptions,
) -> CircuitResult<DetectorErrorModel> {
    stab_analysis::circuit_to_detector_error_model(circuit, options).map_err(Into::into)
}

pub fn independent_to_disjoint_xyz_errors(
    x: Probability,
    y: Probability,
    z: Probability,
) -> CircuitResult<DisjointPauliProbabilities> {
    stab_analysis::independent_to_disjoint_xyz_errors(x, y, z).map_err(Into::into)
}

pub fn try_disjoint_to_independent_xyz_errors(
    x: Probability,
    y: Probability,
    z: Probability,
) -> CircuitResult<Option<IndependentPauliProbabilities>> {
    stab_analysis::try_disjoint_to_independent_xyz_errors(x, y, z).map_err(Into::into)
}
