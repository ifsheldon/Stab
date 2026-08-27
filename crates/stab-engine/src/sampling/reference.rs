use rand::SeedableRng as _;
use rand::rngs::SmallRng;

use super::api::SamplingPlanKind;
use super::execute::{ExecutionBuffers, execute_reference_operations};
use super::stabilizer_frame::{StabilizerFrame, StabilizerStateSnapshot};
use super::{SamplingExecutionError, SamplingPlan, validate_general_frame_work_storage};

#[derive(Debug)]
pub(crate) struct ReferenceSampleScratch(ReferenceSampleScratchKind);

#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "reference scratch stays inline so fallible frame admission is not followed by an infallible box allocation"
)]
enum ReferenceSampleScratchKind {
    Empty,
    DirectZ,
    General {
        rng: SmallRng,
        frame: StabilizerFrame,
        snapshot: Option<StabilizerStateSnapshot>,
        output: Vec<bool>,
    },
}

impl SamplingPlan {
    pub(crate) fn reference_measurement_record_with_sweep_into(
        &self,
        sweep_record: &[bool],
        record: &mut Vec<bool>,
    ) -> Result<(), SamplingExecutionError> {
        let mut scratch = self.try_reusable_reference_sample_scratch()?;
        self.reference_measurement_record_with_sweep_and_scratch_into(
            sweep_record,
            &mut scratch,
            record,
        )
    }

    pub(crate) fn try_reusable_reference_sample_scratch(
        &self,
    ) -> Result<ReferenceSampleScratch, SamplingExecutionError> {
        if self.inner.measurement_count == 0 {
            return Ok(ReferenceSampleScratch(ReferenceSampleScratchKind::Empty));
        }
        if matches!(self.inner.kind, SamplingPlanKind::DirectZ(_)) {
            return Ok(ReferenceSampleScratch(ReferenceSampleScratchKind::DirectZ));
        }
        let needs_snapshot = self.inner.uses_reference_state_snapshot();
        validate_general_frame_work_storage(
            self.inner.qubit_count,
            self.inner.measurement_count,
            needs_snapshot,
        )?;
        let frame = StabilizerFrame::try_new(self.inner.qubit_count).map_err(|error| {
            SamplingExecutionError::SessionStorageAllocation {
                message: error.to_string(),
            }
        })?;
        let snapshot = needs_snapshot
            .then(|| StabilizerStateSnapshot::try_new(self.inner.qubit_count))
            .transpose()
            .map_err(|error| SamplingExecutionError::SessionStorageAllocation {
                message: error.to_string(),
            })?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(self.inner.measurement_count)
            .map_err(|error| SamplingExecutionError::SessionStorageAllocation {
                message: format!(
                    "reference measurement output capacity {}: {error}",
                    self.inner.measurement_count
                ),
            })?;
        Ok(ReferenceSampleScratch(
            ReferenceSampleScratchKind::General {
                rng: SmallRng::seed_from_u64(0),
                frame,
                snapshot,
                output,
            },
        ))
    }

    pub(crate) fn reference_measurement_record_with_sweep_and_scratch_into(
        &self,
        sweep_record: &[bool],
        scratch: &mut ReferenceSampleScratch,
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
        match (&self.inner.kind, &mut scratch.0) {
            (_, ReferenceSampleScratchKind::Empty) if self.inner.measurement_count == 0 => {
                record.clear();
            }
            (SamplingPlanKind::DirectZ(direct), ReferenceSampleScratchKind::DirectZ) => {
                record.clear();
                record.push(direct.reference_bit());
            }
            (
                SamplingPlanKind::SmallFrame | SamplingPlanKind::GeneralFrame,
                ReferenceSampleScratchKind::General {
                    rng,
                    frame,
                    snapshot,
                    output,
                },
            ) => {
                frame.reset_to_z_basis();
                record.clear();
                output.clear();
                let mut correlated_error_occurred = false;
                let mut buffers = ExecutionBuffers {
                    frame,
                    record,
                    output,
                    correlated_error_occurred: &mut correlated_error_occurred,
                };
                execute_reference_operations(
                    &self.inner.operations,
                    &mut buffers,
                    rng,
                    sweep_record,
                    self.inner.reference_sample_loop_policy,
                    snapshot.as_mut(),
                )?;
            }
            _ => {
                return Err(SamplingExecutionError::InternalInvariant {
                    message: "reference sample scratch does not match the sampling backend"
                        .to_owned(),
                });
            }
        }
        Ok(())
    }
}
