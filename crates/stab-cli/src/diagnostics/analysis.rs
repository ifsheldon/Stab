//! Typed JSON context of analysis failures.

use serde_json::{Value, json};
use stab_analysis::{AnalysisError, AnalysisOperation};
use stab_model::DemTarget;

use super::{analysis_resource_context, model_error_context};

pub(super) fn analysis_error_context(error: &AnalysisError) -> Value {
    match error {
        AnalysisError::Model(error) => model_error_context(error),
        AnalysisError::ResourceLimit(error) => analysis_resource_context(error),
        AnalysisError::Cancelled(_) | AnalysisError::NonGraphlikeError => json!({}),
        AnalysisError::NonDeterministicTargets(targets) => json!({
            "targets": dem_target_names(targets.targets()),
        }),
        AnalysisError::NonUnitaryOperation {
            operation,
            gate,
            reason,
        } => json!({
            "operation": operation.as_str(),
            "gate": gate.canonical_name(),
            "reason": reason.as_str(),
        }),
        AnalysisError::PauliNotPreserved { gate, collapse } => json!({
            "operation": AnalysisOperation::PauliConjugation.as_str(),
            "gate": gate.canonical_name(),
            "collapse": collapse.as_str(),
            "qubit": collapse.qubit().map(|qubit| qubit.get()),
        }),
        AnalysisError::AntiHermitianPauliProduct {
            operation, gate, ..
        } => json!({
            "operation": operation.as_str(),
            "gate": gate.canonical_name(),
        }),
        AnalysisError::UnsupportedInput {
            operation, gate, ..
        } => json!({
            "operation": operation.as_str(),
            "gate": gate.map(|gate| gate.canonical_name()),
        }),
        AnalysisError::NoiseRequiresApproximation { gate, excess } => json!({
            "gate": gate.canonical_name(),
            "probability": excess.map(|excess| excess.probability().get()),
            "threshold": excess.map(|excess| excess.threshold().get()),
        }),
        AnalysisError::UnanalyzableNoise(noise) => json!({
            "gate": noise.gate().canonical_name(),
            "reason": noise.as_str(),
        }),
        AnalysisError::ErrorDecompositionFailed {
            component,
            remnant_edges_blocked,
        } => json!({
            "component": dem_target_names(component),
            "remnant_edges_blocked": remnant_edges_blocked,
        }),
        AnalysisError::InvalidRequest(request) => json!({
            "request": request.as_str(),
        }),
        AnalysisError::InvalidGeneratorParameter { parameter, value } => json!({
            "parameter": parameter.as_str(),
            "value": value,
        }),
        AnalysisError::AllocationFailed { operation, .. }
        | AnalysisError::CountOverflow { operation, .. }
        | AnalysisError::NonFiniteCoordinate { operation }
        | AnalysisError::MeasurementRecordOutOfRange { operation, .. }
        | AnalysisError::InternalInvariant { operation, .. } => json!({
            "operation": operation.as_str(),
        }),
    }
}

fn dem_target_names(targets: &[DemTarget]) -> Vec<String> {
    targets.iter().map(ToString::to_string).collect()
}
