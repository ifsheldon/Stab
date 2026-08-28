mod api;
mod buffers;
mod conversion_plan;
mod error;
pub(crate) mod frame;
mod limits;
mod prepared;
mod reference;
mod reference_signs;

pub use api::{
    DetectionCompileError, DetectionExecutionError, DetectionRunError, DetectionRunProgress,
    DetectionRunStatus, DetectionRunSummary, DetectionSamplingCompiler, DetectionSamplingPlan,
    DetectionSamplingSession, MeasurementToDetectionCompiler, MeasurementToDetectionPlan,
    MeasurementToDetectionSession, MeasurementToDetectionTransaction,
};
pub use error::{
    DetectionError, DetectionRecordLimitSubject, DetectionResourceKind, DetectionResourceLimitError,
};
pub use limits::DetectionConversionLimits;
pub use reference_signs::{
    CircuitReferenceSigns, circuit_reference_signs, circuit_reference_signs_with_limits,
};

use buffers::{try_false_vec, try_vec_with_capacity};
use conversion_plan::ConversionPlan;
use error::DetectionResult;
use frame::{DetectorFrameState, SweepCorrectionPlan, admit_combined_compiled_storage};
use prepared::PreparedDetectionSampling;
use reference::ReferenceSampleSource;
use stab_model::Circuit;

use crate::{CompilationDescriptor, CompilationOperation, SamplingCompiler};

#[cfg(test)]
mod test_support;

const UNSUPPORTED_SWEEP_DETECTION_MESSAGE: &str =
    "sweep-conditioned detection conversion requires sweep input support";

/// Measurement-to-detection compiler registration.
pub const MEASUREMENT_TO_DETECTION_COMPILATION_DESCRIPTOR: CompilationDescriptor =
    CompilationDescriptor::new(
        CompilationOperation::MeasurementToDetection,
        stab_model::ModelDialect::StimCircuit,
        3,
        None,
        true,
    );

/// Circuit detection-sampling compiler registration.
pub const DETECTION_SAMPLING_COMPILATION_DESCRIPTOR: CompilationDescriptor =
    CompilationDescriptor::new(
        CompilationOperation::DetectionSampling,
        stab_model::ModelDialect::StimCircuit,
        3,
        None,
        true,
    );

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DetectionRecordBuffer {
    pub(crate) detectors: Vec<bool>,
    pub(crate) observables: Vec<bool>,
}

#[derive(Clone, Debug)]
struct PreparedMeasurementToDetection {
    plan: ConversionPlan,
    reference_sample: ReferenceSampleSource,
    sweep_correction: Option<SweepCorrectionPlan>,
}

impl PreparedMeasurementToDetection {
    fn compile_with_limits(
        circuit: &Circuit,
        reference_mode: crate::ReferenceSampleMode,
        limits: DetectionConversionLimits,
    ) -> DetectionResult<Self> {
        let conversion_admission =
            ConversionPlan::admission_from_circuit_with_limits(circuit, limits)?;
        let correction_admission = if conversion_admission.sweep_bit_count > 0 {
            let conversion_bytes = conversion_admission.compiled_storage_bytes()?;
            let admission =
                SweepCorrectionPlan::admit(circuit, conversion_bytes, limits.max_compiled_bytes())?;
            admit_combined_compiled_storage(
                conversion_bytes,
                admission.retained_bytes(),
                limits.max_compiled_bytes(),
            )?;
            Some(admission)
        } else {
            None
        };
        let plan =
            ConversionPlan::materialize_circuit_from_admission(circuit, conversion_admission)?;
        let sweep_correction = match correction_admission {
            Some(admission) => {
                let correction = SweepCorrectionPlan::materialize(
                    circuit,
                    admission,
                    plan.measurement_count,
                    plan.detector_count,
                    plan.observable_count,
                )?;
                admit_combined_compiled_storage(
                    plan.compiled_storage_bytes()?,
                    correction.retained_bytes(),
                    limits.max_compiled_bytes(),
                )?;
                Some(correction)
            }
            None => None,
        };
        let sampling = SamplingCompiler::new().compile(circuit)?;
        if sampling.measurement_width().get() != plan.measurement_count {
            return Err(DetectionError::invalid_result_format(format!(
                "reference sampler has {} measurements but detection conversion expected {}",
                sampling.measurement_width().get(),
                plan.measurement_count
            )));
        }
        if sampling.sweep_bit_count() != plan.sweep_bit_count {
            return Err(DetectionError::invalid_result_format(format!(
                "reference sampler has {} sweep bits but detection conversion expected {}",
                sampling.sweep_bit_count(),
                plan.sweep_bit_count
            )));
        }
        let reference_sample = if matches!(
            reference_mode,
            crate::ReferenceSampleMode::SkipReferenceSample
        ) {
            ReferenceSampleSource::Zero
        } else {
            ReferenceSampleSource::Static(reference::static_reference_sample(
                &sampling,
                plan.measurement_count,
            )?)
        };
        Self::from_plan_reference_and_correction(plan, reference_sample, sweep_correction)
    }

    fn measurement_count(&self) -> usize {
        self.plan.measurement_count
    }

    fn sweep_bit_count(&self) -> usize {
        self.plan.sweep_bit_count
    }

    fn detector_count(&self) -> usize {
        self.plan.detector_count
    }

    fn observable_count(&self) -> usize {
        self.plan.observable_count
    }

    fn try_reusable_detection_record(&self) -> DetectionResult<DetectionRecordBuffer> {
        Ok(DetectionRecordBuffer {
            detectors: try_false_vec(
                self.detector_count(),
                "detection conversion detector record",
            )?,
            observables: try_false_vec(
                self.observable_count(),
                "detection conversion observable record",
            )?,
        })
    }

    fn try_reusable_reference_sample(&self) -> DetectionResult<Vec<bool>> {
        try_false_vec(
            self.measurement_count(),
            "detection conversion reference sample",
        )
    }

    fn try_reusable_sweep_correction(&self) -> DetectionResult<Option<DetectorFrameState>> {
        self.sweep_correction
            .as_ref()
            .map(SweepCorrectionPlan::state)
            .transpose()
    }

    fn sweep_correction_storage_bytes(&self) -> u128 {
        self.sweep_correction
            .as_ref()
            .map_or(0, SweepCorrectionPlan::state_storage_bytes)
    }

    fn convert_record_with_sweep_into(
        &self,
        measurement_record: &[bool],
        sweep_record: &[bool],
        reference_sample: &mut Vec<bool>,
        record: &mut DetectionRecordBuffer,
        correction_state: Option<&mut DetectorFrameState>,
        correction_rng: &mut impl rand::Rng,
    ) -> DetectionResult<()> {
        self.validate_measurement_record_width(measurement_record)?;
        self.validate_sweep_record_width(sweep_record)?;
        self.reference_sample
            .fill(self.measurement_count(), reference_sample)?;
        self.plan
            .convert_record_into(measurement_record, reference_sample, record)?;
        match (&self.sweep_correction, correction_state) {
            (Some(correction), Some(state)) => {
                let (detectors, observables) =
                    correction.correct(&self.plan, sweep_record, state, correction_rng)?;
                xor_bits(&mut record.detectors, detectors, "detector")?;
                xor_bits(&mut record.observables, observables, "observable")?;
                Ok(())
            }
            (None, None) => Ok(()),
            _ => Err(DetectionError::invalid_result_format(
                "sweep correction state does not match the compiled conversion plan",
            )),
        }
    }

    fn from_plan_and_reference_sample(
        plan: ConversionPlan,
        reference_sample: ReferenceSampleSource,
    ) -> DetectionResult<Self> {
        Self::from_plan_reference_and_correction(plan, reference_sample, None)
    }

    fn from_plan_reference_and_correction(
        plan: ConversionPlan,
        reference_sample: ReferenceSampleSource,
        sweep_correction: Option<SweepCorrectionPlan>,
    ) -> DetectionResult<Self> {
        if let ReferenceSampleSource::Static(reference_sample) = &reference_sample {
            reference::validate_reference_sample_len(reference_sample, plan.measurement_count)?;
        }
        Ok(Self {
            plan,
            reference_sample,
            sweep_correction,
        })
    }

    fn validate_measurement_record_width(
        &self,
        measurement_record: &[bool],
    ) -> DetectionResult<()> {
        if measurement_record.len() == self.plan.measurement_count {
            return Ok(());
        }
        Err(DetectionError::invalid_result_format(format!(
            "measurement record expected {} bits, got {}",
            self.plan.measurement_count,
            measurement_record.len()
        )))
    }

    fn validate_sweep_record_width(&self, sweep_record: &[bool]) -> DetectionResult<()> {
        if sweep_record.len() == self.plan.sweep_bit_count {
            return Ok(());
        }
        Err(DetectionError::invalid_result_format(format!(
            "sweep record expected {} bits, got {}",
            self.plan.sweep_bit_count,
            sweep_record.len()
        )))
    }
}

fn xor_bits(output: &mut [bool], correction: &[bool], kind: &str) -> DetectionResult<()> {
    if output.len() != correction.len() {
        return Err(DetectionError::invalid_result_format(format!(
            "sweep {kind} correction has {} bits but output has {}",
            correction.len(),
            output.len()
        )));
    }
    for (output, correction) in output.iter_mut().zip(correction) {
        *output ^= correction;
    }
    Ok(())
}

pub fn measurement_record_count(circuit: &Circuit) -> DetectionResult<usize> {
    measurement_record_count_with_limits(circuit, DetectionConversionLimits::default())
}

pub fn measurement_record_count_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> DetectionResult<usize> {
    Ok(detection_conversion_plan_with_limits(circuit, limits)?.measurement_count)
}

pub fn detection_record_width(circuit: &Circuit) -> DetectionResult<usize> {
    detection_record_width_with_limits(circuit, DetectionConversionLimits::default())
}

pub fn detection_record_width_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> DetectionResult<usize> {
    detection_conversion_plan_with_limits(circuit, limits)?.output_bit_count()
}

pub fn validate_detection_sampling_circuit(circuit: &Circuit) -> DetectionResult<()> {
    validate_detection_sampling_circuit_with_limits(circuit, DetectionConversionLimits::default())
}

pub fn validate_detection_sampling_circuit_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> DetectionResult<()> {
    DetectionSamplingCompiler::new()
        .limits(limits)
        .compile(circuit)
        .map(|_| ())
        .map_err(|error| match error {
            DetectionCompileError::InvalidCircuit(error) => error,
        })
}

fn detection_conversion_plan_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> DetectionResult<ConversionPlan> {
    ConversionPlan::from_circuit_with_limits(circuit, limits)
}

#[cfg(test)]
mod tests;
