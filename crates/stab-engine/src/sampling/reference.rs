use rand::SeedableRng as _;
use rand::rngs::SmallRng;

use super::stabilizer_frame::StabilizerFrame;
use super::{ExecutionMode, SamplingExecutionError, SamplingPlan};

#[derive(Debug)]
pub(crate) struct ReferenceSampleScratch {
    pub(super) rng: SmallRng,
    pub(super) frame: StabilizerFrame,
    pub(super) output: Vec<bool>,
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
        let frame = StabilizerFrame::try_new(self.inner.qubit_count).map_err(|error| {
            SamplingExecutionError::SessionStorageAllocation {
                message: error.to_string(),
            }
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
        Ok(ReferenceSampleScratch {
            rng: SmallRng::seed_from_u64(0),
            frame,
            output,
        })
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
        self.sample_shot_in_mode_into(
            &mut scratch.rng,
            ExecutionMode::ReferenceSample,
            sweep_record,
            &mut scratch.frame,
            record,
            &mut scratch.output,
        );
        Ok(())
    }
}
