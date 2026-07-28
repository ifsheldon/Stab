use rand::SeedableRng as _;
use rand::rngs::SmallRng;

use super::stabilizer_frame::StabilizerFrame;
use super::{CompiledSampler, ExecutionMode, SamplingExecutionError};
use crate::{CircuitError, CircuitResult};

#[derive(Debug)]
pub(crate) struct ReferenceSampleScratch {
    rng: SmallRng,
    frame: StabilizerFrame,
    output: Vec<bool>,
}

impl CompiledSampler {
    pub(crate) fn reference_measurement_record_with_sweep_into(
        &self,
        sweep_record: &[bool],
        record: &mut Vec<bool>,
    ) -> CircuitResult<()> {
        let mut scratch = self
            .try_reusable_reference_sample_scratch()
            .map_err(SamplingExecutionError::into_circuit_error)?;
        self.reference_measurement_record_with_sweep_and_scratch_into(
            sweep_record,
            &mut scratch,
            record,
        )
    }

    pub(crate) fn try_reusable_reference_sample_scratch(
        &self,
    ) -> Result<ReferenceSampleScratch, SamplingExecutionError> {
        let frame = StabilizerFrame::try_new(self.plan.inner.qubit_count).map_err(|error| {
            SamplingExecutionError::SessionStorageAllocation {
                message: error.to_string(),
            }
        })?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(self.plan.inner.measurement_count)
            .map_err(|error| SamplingExecutionError::SessionStorageAllocation {
                message: format!(
                    "reference measurement output capacity {}: {error}",
                    self.plan.inner.measurement_count
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
    ) -> CircuitResult<()> {
        if sweep_record.len() != self.plan.inner.sweep_bit_count {
            return Err(CircuitError::invalid_result_format(format!(
                "sweep record expected {} bits, got {}",
                self.plan.inner.sweep_bit_count,
                sweep_record.len()
            )));
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
