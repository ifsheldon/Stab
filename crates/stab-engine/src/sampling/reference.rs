use super::api::{
    MAX_SAMPLING_SESSION_STORAGE_BYTES, ReferenceSampleLoopPolicy, try_bool_buffer,
    validate_reference_width,
};
use super::reference_run::{ReferenceWorkspace, has_reference_fold_candidate};
use super::{
    SamplingExecutionError, SamplingPlan, general_frame_work_storage_bytes,
    reference_state_snapshot_fits, validate_general_frame_work_storage,
};
use crate::detection::frame::CircuitProgram;

impl SamplingPlan {
    #[cfg(test)]
    pub(crate) fn estimated_reference_work_storage_bytes(&self) -> u128 {
        reference_work_storage_bytes(
            &self.inner.program,
            self.inner.qubit_count,
            self.inner.reference_sample_loop_policy,
        )
    }

    /// Writes the noiseless reference measurement record for one sweep assignment.
    ///
    /// The all-false sweep record reuses the plan's cached reference sample; other sweep
    /// records run the inverse-tableau reference simulator once.
    pub(crate) fn reference_measurement_record_with_sweep_into(
        &self,
        sweep_record: &[bool],
        record: &mut Vec<bool>,
    ) -> Result<(), SamplingExecutionError> {
        if sweep_record.len() != self.inner.sweep_bit_count {
            return Err(SamplingExecutionError::InvalidSweepRecordWidth {
                expected: self.inner.sweep_bit_count,
                actual: sweep_record.len(),
            });
        }
        if record.capacity() < self.inner.measurement_count {
            record
                .try_reserve_exact(self.inner.measurement_count - record.capacity())
                .map_err(|error| SamplingExecutionError::SessionStorageAllocation {
                    message: format!(
                        "reference measurement record capacity {}: {error}",
                        self.inner.measurement_count
                    ),
                })?;
        }
        record.clear();
        if self.inner.measurement_count == 0 {
            return Ok(());
        }
        if !sweep_record.contains(&true) {
            record.extend_from_slice(self.inner.cached_reference_sample()?);
            return Ok(());
        }
        let output = program_reference_sample(
            &self.inner.program,
            self.inner.qubit_count,
            self.inner.reference_sample_loop_policy,
            sweep_record,
        )?;
        record.extend_from_slice(&output);
        Ok(())
    }
}

/// Whether a reference run of `program` with `loop_policy` would allocate a fold snapshot.
fn uses_snapshot(
    program: &CircuitProgram,
    qubit_count: usize,
    loop_policy: ReferenceSampleLoopPolicy,
) -> bool {
    loop_policy == ReferenceSampleLoopPolicy::Fold
        && has_reference_fold_candidate(program, qubit_count)
        && reference_state_snapshot_fits(
            qubit_count,
            program.counts().measurements,
            MAX_SAMPLING_SESSION_STORAGE_BYTES,
        )
}

/// Estimated scratch storage of one reference run, before any allocation.
pub(crate) fn reference_work_storage_bytes(
    program: &CircuitProgram,
    qubit_count: usize,
    loop_policy: ReferenceSampleLoopPolicy,
) -> u128 {
    let measurements = program.counts().measurements;
    if measurements == 0 {
        return 0;
    }
    general_frame_work_storage_bytes(
        qubit_count,
        measurements,
        uses_snapshot(program, qubit_count, loop_policy),
    )
}

/// Runs the reference simulator over `program` for one sweep assignment.
///
/// `qubit_count` is the circuit's simulated qubit count, which bounds every program target.
pub(crate) fn program_reference_sample(
    program: &CircuitProgram,
    qubit_count: usize,
    loop_policy: ReferenceSampleLoopPolicy,
    sweep_record: &[bool],
) -> Result<Vec<bool>, SamplingExecutionError> {
    let measurement_count = program.counts().measurements;
    if measurement_count == 0 {
        return Ok(Vec::new());
    }
    let needs_snapshot = uses_snapshot(program, qubit_count, loop_policy);
    validate_general_frame_work_storage(qubit_count, measurement_count, needs_snapshot)?;
    let mut output = try_bool_buffer(measurement_count, "reference measurement output")?;
    ReferenceWorkspace::try_new(qubit_count, needs_snapshot)?.run(
        program,
        loop_policy,
        sweep_record,
        &mut output,
    )?;
    validate_reference_width(&output, measurement_count)?;
    Ok(output)
}
