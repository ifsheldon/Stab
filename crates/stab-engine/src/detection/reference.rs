use stab_model::Circuit;

use super::buffers::try_false_vec;
use super::error::{DetectionError, DetectionResult};
use super::frame::CircuitProgram;
use crate::ReferenceSampleLoopPolicy;
use crate::sampling::{program_reference_sample, reference_work_storage_bytes};

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

/// Computes the all-zero-sweep reference sample from the conversion's resolved program.
pub(super) fn static_reference_sample(
    circuit: &Circuit,
    program: &CircuitProgram,
    measurement_count: usize,
) -> DetectionResult<Vec<bool>> {
    let qubit_count = stab_model::advanced::circuit_simulated_qubit_count(circuit);
    let loop_policy = ReferenceSampleLoopPolicy::Fold;
    let estimated_bytes = reference_work_storage_bytes(program, qubit_count, loop_policy);
    if estimated_bytes > MAX_REFERENCE_SCRATCH_BYTES {
        return Err(DetectionError::invalid_sampler_compilation(format!(
            "detection reference sampling needs an estimated {estimated_bytes} bytes of bounded storage, exceeding the {MAX_REFERENCE_SCRATCH_BYTES}-byte safety limit"
        )));
    }
    let zero_sweeps = try_false_vec(
        program.counts().sweep_bits,
        "detection reference zero-sweep record",
    )?;
    let reference_sample =
        program_reference_sample(program, qubit_count, loop_policy, &zero_sweeps)?;
    validate_reference_sample_len(&reference_sample, measurement_count)?;
    Ok(reference_sample)
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
