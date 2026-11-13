use super::api::{SamplingPlanKind, try_bool_buffer, validate_reference_width};
use super::reference_run::ReferenceWorkspace;
use super::{SamplingExecutionError, SamplingPlan, validate_general_frame_work_storage};

impl SamplingPlan {
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
        if let SamplingPlanKind::DirectZ(direct) = &self.inner.kind {
            record.push(direct.reference_bit());
            return Ok(());
        }
        if !sweep_record.contains(&true) {
            record.extend_from_slice(self.inner.cached_reference_sample()?);
            return Ok(());
        }
        let needs_snapshot = self.inner.uses_reference_state_snapshot();
        validate_general_frame_work_storage(
            self.inner.qubit_count,
            self.inner.measurement_count,
            needs_snapshot,
        )?;
        let mut output =
            try_bool_buffer(self.inner.measurement_count, "reference measurement output")?;
        ReferenceWorkspace::try_new(self.inner.qubit_count, needs_snapshot)?.run(
            &self.inner.operations,
            self.inner.reference_sample_loop_policy,
            sweep_record,
            &mut output,
        )?;
        validate_reference_width(&output, self.inner.measurement_count)?;
        record.extend_from_slice(&output);
        Ok(())
    }
}
