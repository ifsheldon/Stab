use stab_model::Circuit;

use crate::sampling::ReferenceSampleScratch;
use crate::{SamplingCompiler, SamplingExecutionError, SamplingPlan};

use super::buffers::try_vec_with_capacity;
use super::error::{DetectionError, DetectionResult};

const MAX_REFERENCE_SCRATCH_BYTES: u128 = 256 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ReferenceSampleSource {
    Zero,
    Static(Vec<bool>),
    Sweep(SamplingPlan),
}

impl ReferenceSampleSource {
    pub(super) fn reusable_scratch(
        &self,
    ) -> Result<Option<ReferenceSampleScratch>, SamplingExecutionError> {
        match self {
            Self::Sweep(sampling) => sampling.try_reusable_reference_sample_scratch().map(Some),
            Self::Zero | Self::Static(_) => Ok(None),
        }
    }

    pub(super) fn reusable_scratch_storage_bytes(&self) -> u128 {
        match self {
            Self::Sweep(sampling) => reference_scratch_storage_bytes(sampling),
            Self::Zero | Self::Static(_) => 0,
        }
    }

    pub(super) fn fill(
        &self,
        sweep_record: &[bool],
        measurement_count: usize,
        output: &mut Vec<bool>,
        reference_scratch: Option<&mut ReferenceSampleScratch>,
    ) -> DetectionResult<()> {
        output.clear();
        match self {
            Self::Zero => output.resize(measurement_count, false),
            Self::Static(reference_sample) => output.extend_from_slice(reference_sample),
            Self::Sweep(sampling) => {
                let scratch = reference_scratch.ok_or_else(|| {
                    DetectionError::invalid_result_format(
                        "internal sweep reference conversion scratch is unavailable",
                    )
                })?;
                sampling.reference_measurement_record_with_sweep_and_scratch_into(
                    sweep_record,
                    scratch,
                    output,
                )?
            }
        }
        validate_reference_sample_len(output, measurement_count)
    }
}

pub(super) fn static_reference_sample(
    circuit: &Circuit,
    measurement_count: usize,
) -> DetectionResult<Vec<bool>> {
    let sampling = SamplingCompiler::new().compile(circuit)?;
    validate_reference_scratch_storage(&sampling)?;
    let mut reference_sample =
        try_vec_with_capacity(measurement_count, "detection conversion reference sample")?;
    sampling.reference_measurement_record_with_sweep_into(&[], &mut reference_sample)?;
    validate_reference_sample_len(&reference_sample, measurement_count)?;
    Ok(reference_sample)
}

fn validate_reference_scratch_storage(sampling: &SamplingPlan) -> DetectionResult<()> {
    let estimated_bytes = reference_scratch_storage_bytes(sampling);
    if estimated_bytes > MAX_REFERENCE_SCRATCH_BYTES {
        return Err(DetectionError::invalid_sampler_compilation(format!(
            "detection reference sampling needs an estimated {estimated_bytes} bytes of bounded storage, exceeding the {MAX_REFERENCE_SCRATCH_BYTES}-byte safety limit"
        )));
    }
    Ok(())
}

fn reference_scratch_storage_bytes(sampling: &SamplingPlan) -> u128 {
    sampling.estimated_reference_work_storage_bytes()
}

pub(super) fn validate_reference_sample_len(
    reference_sample: &[bool],
    measurement_count: usize,
) -> DetectionResult<()> {
    if reference_sample.len() == measurement_count {
        return Ok(());
    }
    Err(DetectionError::invalid_result_format(format!(
        "reference sample has {} measurement bits but detection conversion expected {measurement_count}",
        reference_sample.len()
    )))
}
