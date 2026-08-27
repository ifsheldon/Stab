use crate::SamplingPlan;

use super::buffers::{try_false_vec, try_vec_with_capacity};
use super::error::{DetectionError, DetectionResult};

const MAX_REFERENCE_SCRATCH_BYTES: u128 = 256 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(super) enum ReferenceSampleSource {
    Zero,
    Static(Vec<bool>),
}

impl ReferenceSampleSource {
    pub(super) fn fill(
        &self,
        measurement_count: usize,
        output: &mut Vec<bool>,
    ) -> DetectionResult<()> {
        output.clear();
        match self {
            Self::Zero => output.resize(measurement_count, false),
            Self::Static(reference_sample) => output.extend_from_slice(reference_sample),
        }
        validate_reference_sample_len(output, measurement_count)
    }
}

pub(super) fn static_reference_sample(
    sampling: &SamplingPlan,
    measurement_count: usize,
) -> DetectionResult<Vec<bool>> {
    validate_reference_scratch_storage(sampling)?;
    let mut reference_sample =
        try_vec_with_capacity(measurement_count, "detection conversion reference sample")?;
    let zero_sweeps = try_false_vec(
        sampling.sweep_bit_count(),
        "detection reference zero-sweep record",
    )?;
    sampling.reference_measurement_record_with_sweep_into(&zero_sweeps, &mut reference_sample)?;
    validate_reference_sample_len(&reference_sample, measurement_count)?;
    Ok(reference_sample)
}

fn validate_reference_scratch_storage(sampling: &SamplingPlan) -> DetectionResult<()> {
    let estimated_bytes = sampling.estimated_reference_work_storage_bytes();
    if estimated_bytes > MAX_REFERENCE_SCRATCH_BYTES {
        return Err(DetectionError::invalid_sampler_compilation(format!(
            "detection reference sampling needs an estimated {estimated_bytes} bytes of bounded storage, exceeding the {MAX_REFERENCE_SCRATCH_BYTES}-byte safety limit"
        )));
    }
    Ok(())
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
