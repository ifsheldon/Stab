use crate::{CircuitResult, DemFlattenLimits, DetectorErrorModel};

/// Returns a compact copy of `model` with every instruction and repeat-block tag removed.
pub fn detector_error_model_without_tags(model: &DetectorErrorModel) -> DetectorErrorModel {
    stab_analysis::detector_error_model_without_tags(model)
}

/// Returns a materialized DEM with repeat blocks expanded and detector shifts applied.
pub fn flattened_detector_error_model(
    model: &DetectorErrorModel,
) -> CircuitResult<DetectorErrorModel> {
    stab_analysis::flattened_detector_error_model(model).map_err(Into::into)
}

/// Returns a materialized DEM under the given repeat-expansion resource policy.
pub fn flattened_detector_error_model_with_limits(
    model: &DetectorErrorModel,
    limits: DemFlattenLimits,
) -> CircuitResult<DetectorErrorModel> {
    stab_analysis::flattened_detector_error_model_with_limits(model, limits).map_err(Into::into)
}

/// Returns a compact copy of `model` with error probabilities rounded to `digits` decimal places.
pub fn rounded_detector_error_model(
    model: &DetectorErrorModel,
    digits: u8,
) -> CircuitResult<DetectorErrorModel> {
    stab_analysis::rounded_detector_error_model(model, digits).map_err(Into::into)
}
