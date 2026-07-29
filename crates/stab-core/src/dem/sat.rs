use crate::{CircuitResult, DetectorErrorModel};

pub use stab_analysis::SatMaterializationLimits;

pub fn shortest_error_sat_problem(model: &DetectorErrorModel) -> CircuitResult<String> {
    stab_analysis::shortest_error_sat_problem(model).map_err(Into::into)
}

pub fn shortest_error_sat_problem_with_limits(
    model: &DetectorErrorModel,
    limits: SatMaterializationLimits,
) -> CircuitResult<String> {
    stab_analysis::shortest_error_sat_problem_with_limits(model, limits).map_err(Into::into)
}

pub fn likeliest_error_sat_problem(
    model: &DetectorErrorModel,
    quantization: u32,
) -> CircuitResult<String> {
    stab_analysis::likeliest_error_sat_problem(model, quantization).map_err(Into::into)
}

pub fn likeliest_error_sat_problem_with_limits(
    model: &DetectorErrorModel,
    quantization: u32,
    limits: SatMaterializationLimits,
) -> CircuitResult<String> {
    stab_analysis::likeliest_error_sat_problem_with_limits(model, quantization, limits)
        .map_err(Into::into)
}
