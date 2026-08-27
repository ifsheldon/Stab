use stab_model::Circuit;

use super::buffers::try_false_vec;
use super::{
    ConversionPlan, DetectionConversionLimits, DetectionError, DetectionRecordBuffer,
    DetectionResult, PreparedMeasurementToDetection,
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
    let sweep_record = try_false_vec(prepared.sweep_bit_count(), "reference-sign sweep record")?;
    let mut reference_sample = prepared.try_reusable_reference_sample()?;
    let mut reference_scratch = prepared.reference_sample.reusable_scratch()?;
    prepared.reference_sample.fill(
        &sweep_record,
        prepared.measurement_count(),
        &mut reference_sample,
        reference_scratch.as_mut(),
    )?;

    let mut record: DetectionRecordBuffer = prepared.try_reusable_detection_record()?;
    prepared
        .plan
        .reference_signs_into(&reference_sample, &mut record)?;
    Ok(CircuitReferenceSigns {
        detector_signs: record.detectors,
        observable_signs: record.observables,
    })
}

impl ConversionPlan {
    fn reference_signs_into(
        &self,
        reference_sample: &[bool],
        record: &mut DetectionRecordBuffer,
    ) -> DetectionResult<()> {
        super::reference::validate_reference_sample_len(reference_sample, self.measurement_count)?;
        record.detectors.clear();
        for terms in &self.detector_terms {
            record
                .detectors
                .push(parity_of_reference_terms(terms, reference_sample)?);
        }
        record.observables.clear();
        for terms in &self.observable_terms {
            record
                .observables
                .push(parity_of_reference_terms(terms, reference_sample)?);
        }
        Ok(())
    }
}

fn parity_of_reference_terms(terms: &[usize], reference_sample: &[bool]) -> DetectionResult<bool> {
    let mut parity = false;
    for index in terms {
        parity ^= reference_sample.get(*index).copied().ok_or_else(|| {
            DetectionError::invalid_result_format(format!(
                "reference sample index {index} is out of range"
            ))
        })?;
    }
    Ok(parity)
}
