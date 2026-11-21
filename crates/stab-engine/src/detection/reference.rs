use stab_bits::BitSlice;
use stab_model::{Cancellation, Circuit, MemoryBudget};

use super::error::{DetectionError, DetectionResourceLimitError, DetectionResult};
use super::frame::CircuitProgram;
use crate::sampling::{program_reference_sample, reference_work_storage_bytes};
use crate::{ReferenceSample, ReferenceSampleLoopPolicy};

#[derive(Clone, Debug)]
pub(super) enum ReferenceSampleSource {
    Zero,
    Static(ReferenceSample),
}

impl ReferenceSampleSource {
    /// The reference sample a conversion XORs into every shot, or `None` for the all-zero one.
    pub(super) const fn sample(&self) -> Option<&ReferenceSample> {
        match self {
            Self::Zero => None,
            Self::Static(sample) => Some(sample),
        }
    }
}

/// Computes the all-zero-sweep reference sample from the conversion's resolved program.
pub(super) fn static_reference_sample(
    circuit: &Circuit,
    program: &CircuitProgram,
    measurement_count: usize,
    budget: MemoryBudget,
    cancellation: &Cancellation,
) -> DetectionResult<ReferenceSample> {
    let qubit_count = stab_model::advanced::circuit_simulated_qubit_count(circuit);
    let loop_policy = ReferenceSampleLoopPolicy::Fold;
    let estimated_bytes = reference_work_storage_bytes(program, qubit_count, loop_policy, budget);
    if !budget.admits(estimated_bytes) {
        return Err(DetectionResourceLimitError::detection_compiled_bytes(
            u64::try_from(estimated_bytes).unwrap_or(u64::MAX),
            budget,
        )
        .into());
    }
    // Sweep bits past the end of the empty record read as false.
    let reference_sample = program_reference_sample(
        program,
        qubit_count,
        loop_policy,
        BitSlice::default(),
        budget,
        cancellation,
    )?;
    validate_reference_sample_len(&reference_sample, measurement_count)?;
    Ok(reference_sample)
}

pub(super) fn validate_reference_sample_len(
    reference_sample: &ReferenceSample,
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
