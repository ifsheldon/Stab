use stab_model::Circuit;

use super::{
    DetectionConversionLimits, DetectionError, DetectionRecordBuffer,
    PreparedMeasurementToDetection,
};

/// Detector and observable parities implied by a circuit's noiseless reference sample.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitReferenceSigns {
    detector_signs: Vec<bool>,
    observable_signs: Vec<bool>,
}

impl CircuitReferenceSigns {
    /// Detector signs in circuit declaration order.
    pub fn detector_signs(&self) -> &[bool] {
        &self.detector_signs
    }

    /// Observable signs indexed by observable id, including unused ids below the maximum id.
    pub fn observable_signs(&self) -> &[bool] {
        &self.observable_signs
    }
}

/// Computes detector and observable signs using the noiseless all-zero-sweep reference sample.
///
/// Detection-plan work is admitted with [`DetectionConversionLimits::default`]. Use
/// [`circuit_reference_signs_with_limits`] when the caller owns a different resource policy.
pub fn circuit_reference_signs(circuit: &Circuit) -> Result<CircuitReferenceSigns, DetectionError> {
    circuit_reference_signs_with_limits(circuit, DetectionConversionLimits::default())
}

/// Computes reference signs after applying the caller's detection-conversion resource limits.
pub fn circuit_reference_signs_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> Result<CircuitReferenceSigns, DetectionError> {
    let prepared = PreparedMeasurementToDetection::compile_with_limits(
        circuit,
        crate::ReferenceSampleMode::UseReferenceSample,
        limits,
    )?;
    let mut reference_sample = prepared.try_reusable_reference_sample()?;
    prepared
        .reference_sample
        .fill(prepared.measurement_count(), &mut reference_sample)?;

    let mut record: DetectionRecordBuffer = prepared.try_reusable_detection_record()?;
    prepared
        .plan
        .reference_signs_into(&reference_sample, &mut record)?;
    Ok(CircuitReferenceSigns {
        detector_signs: record.detectors,
        observable_signs: record.observables,
    })
}
