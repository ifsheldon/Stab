use stab_bits::BitSlice;
use stab_model::{Cancellation, MemoryBudget};

use super::api::{ReferenceSampleLoopPolicy, validate_reference_width};
use super::reference_bits::ReferenceBits;
use super::reference_run::{ReferenceWorkspace, has_reference_fold_candidate};
use super::reference_sample::{ReferenceSample, SweepRecord, validate_sweep_width};
use super::{
    SamplingExecutionError, SamplingPlan, reference_state_snapshot_fits,
    simulator_work_storage_bytes, validate_simulator_work_storage,
};
use crate::detection::frame::CircuitProgram;

impl SamplingPlan {
    #[cfg(test)]
    pub(crate) fn estimated_reference_work_storage_bytes(&self) -> u128 {
        reference_work_storage_bytes(
            &self.inner.program,
            self.inner.qubit_count,
            self.inner.reference_sample_loop_policy,
            self.inner.memory_budget,
        )
    }

    /// Computes the noiseless reference measurement record for one sweep assignment.
    ///
    /// The all-false sweep record returns the plan's cached reference sample without a copy;
    /// other sweep records run the inverse-tableau reference simulator once.
    ///
    /// # Errors
    ///
    /// Returns [`SamplingExecutionError::InvalidSweepRecordWidth`] when `sweep` was built for a
    /// plan of a different sweep width, and a storage, cancellation, or invariant error from the
    /// reference run.
    pub fn try_reference_sample_with_sweep(
        &self,
        sweep: &SweepRecord,
    ) -> Result<ReferenceSample, SamplingExecutionError> {
        validate_sweep_width(self.sweep_width(), sweep.width().get())?;
        if sweep.is_all_false() {
            return self.try_reference_sample();
        }
        program_reference_sample(
            &self.inner.program,
            self.inner.qubit_count,
            self.inner.reference_sample_loop_policy,
            sweep.as_bitslice(),
            self.inner.memory_budget,
            &self.inner.cancellation,
        )
    }
}

/// Whether a reference run of `program` with `loop_policy` would allocate a fold snapshot.
fn uses_snapshot(
    program: &CircuitProgram,
    qubit_count: usize,
    loop_policy: ReferenceSampleLoopPolicy,
    budget: MemoryBudget,
) -> bool {
    loop_policy == ReferenceSampleLoopPolicy::Fold
        && has_reference_fold_candidate(program, qubit_count)
        && reference_state_snapshot_fits(qubit_count, program.counts().measurements, budget)
}

/// Estimated scratch storage of one reference run, before any allocation.
pub(crate) fn reference_work_storage_bytes(
    program: &CircuitProgram,
    qubit_count: usize,
    loop_policy: ReferenceSampleLoopPolicy,
    budget: MemoryBudget,
) -> u128 {
    let measurements = program.counts().measurements;
    if measurements == 0 {
        return 0;
    }
    simulator_work_storage_bytes(
        qubit_count,
        measurements,
        uses_snapshot(program, qubit_count, loop_policy, budget),
    )
}

/// Runs the reference simulator over `program` for one sweep assignment.
///
/// `qubit_count` is the circuit's simulated qubit count, which bounds every program target, and
/// sweep bits past the end of `sweep_record` read as false.
pub(crate) fn program_reference_sample(
    program: &CircuitProgram,
    qubit_count: usize,
    loop_policy: ReferenceSampleLoopPolicy,
    sweep_record: BitSlice<'_>,
    budget: MemoryBudget,
    cancellation: &Cancellation,
) -> Result<ReferenceSample, SamplingExecutionError> {
    let measurement_count = program.counts().measurements;
    if measurement_count == 0 {
        return Ok(ReferenceSample::default());
    }
    let needs_snapshot = uses_snapshot(program, qubit_count, loop_policy, budget);
    validate_simulator_work_storage(qubit_count, measurement_count, needs_snapshot, budget)?;
    let mut output =
        ReferenceBits::try_with_capacity(measurement_count, "reference measurement output")?;
    ReferenceWorkspace::try_new(qubit_count, needs_snapshot)?.run(
        program,
        loop_policy,
        sweep_record,
        &mut output,
        cancellation,
    )?;
    validate_reference_width(output.len(), measurement_count)?;
    Ok(output.into_sample())
}
