use rand::SeedableRng as _;
use rand::rngs::SmallRng;

use super::stabilizer_frame::StabilizerFrame;
use super::{CompiledSampler, ExecutionMode};
use crate::{CircuitError, CircuitResult};

#[derive(Debug)]
pub(crate) struct ReferenceSampleScratch {
    rng: SmallRng,
    frame: StabilizerFrame,
    record: Vec<bool>,
    output: Vec<bool>,
}

impl CompiledSampler {
    pub(crate) fn reference_measurement_record_with_sweep_into(
        &self,
        sweep_record: &[bool],
        record: &mut Vec<bool>,
    ) -> CircuitResult<()> {
        let mut scratch = self.reusable_reference_sample_scratch();
        self.reference_measurement_record_with_sweep_and_scratch_into(
            sweep_record,
            &mut scratch,
            record,
        )
    }

    pub(crate) fn reusable_reference_sample_scratch(&self) -> ReferenceSampleScratch {
        ReferenceSampleScratch {
            rng: SmallRng::seed_from_u64(0),
            frame: StabilizerFrame::new(self.qubit_count),
            record: Vec::with_capacity(self.measurement_count),
            output: Vec::with_capacity(self.measurement_count),
        }
    }

    pub(crate) fn reference_measurement_record_with_sweep_and_scratch_into(
        &self,
        sweep_record: &[bool],
        scratch: &mut ReferenceSampleScratch,
        record: &mut Vec<bool>,
    ) -> CircuitResult<()> {
        if sweep_record.len() != self.sweep_bit_count {
            return Err(CircuitError::invalid_result_format(format!(
                "sweep record expected {} bits, got {}",
                self.sweep_bit_count,
                sweep_record.len()
            )));
        }
        self.sample_shot_in_mode_into(
            &mut scratch.rng,
            ExecutionMode::ReferenceSample,
            sweep_record,
            &mut scratch.frame,
            &mut scratch.record,
            &mut scratch.output,
        );
        record.clear();
        record.extend_from_slice(&scratch.record);
        Ok(())
    }
}
