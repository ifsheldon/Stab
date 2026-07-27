use crate::sampling::ReferenceSampleScratch;
use crate::{Circuit, CircuitError, CircuitResult, CompiledSampler};

use super::buffers::try_vec_with_capacity;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ReferenceSampleSource {
    Zero,
    Static(Vec<bool>),
    Sweep(CompiledSampler),
}

impl ReferenceSampleSource {
    pub(super) fn reusable_scratch(&self) -> Option<ReferenceSampleScratch> {
        match self {
            Self::Sweep(sampler) => Some(sampler.reusable_reference_sample_scratch()),
            Self::Zero | Self::Static(_) => None,
        }
    }

    pub(super) fn fill(
        &self,
        sweep_record: &[bool],
        measurement_count: usize,
        output: &mut Vec<bool>,
        reference_scratch: Option<&mut ReferenceSampleScratch>,
    ) -> CircuitResult<()> {
        output.clear();
        match self {
            Self::Zero => output.resize(measurement_count, false),
            Self::Static(reference_sample) => output.extend_from_slice(reference_sample),
            Self::Sweep(sampler) => {
                let scratch = reference_scratch.ok_or_else(|| {
                    CircuitError::invalid_result_format(
                        "internal sweep reference conversion scratch is unavailable",
                    )
                })?;
                sampler.reference_measurement_record_with_sweep_and_scratch_into(
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
) -> CircuitResult<Vec<bool>> {
    let sampler = CompiledSampler::compile(circuit)?;
    let mut reference_sample =
        try_vec_with_capacity(measurement_count, "detection conversion reference sample")?;
    sampler.reference_measurement_record_with_sweep_into(&[], &mut reference_sample)?;
    validate_reference_sample_len(&reference_sample, measurement_count)?;
    Ok(reference_sample)
}

pub(super) fn validate_reference_sample_len(
    reference_sample: &[bool],
    measurement_count: usize,
) -> CircuitResult<()> {
    if reference_sample.len() == measurement_count {
        return Ok(());
    }
    Err(CircuitError::invalid_result_format(format!(
        "reference sample has {} measurement bits but detection conversion expected {measurement_count}",
        reference_sample.len()
    )))
}
