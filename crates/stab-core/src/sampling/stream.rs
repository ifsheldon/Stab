use super::{CompiledSampler, StabilizerFrame, sampler_rng};

impl CompiledSampler {
    pub fn for_each_sample_with_seed_and_reference_mode<E, F>(
        &self,
        shots: usize,
        seed: Option<u64>,
        skip_reference_sample: bool,
        mut visit: F,
    ) -> Result<(), E>
    where
        F: FnMut(&[bool]) -> Result<(), E>,
    {
        let mut rng = sampler_rng(seed);
        let reference_sample = skip_reference_sample.then(|| self.reference_sample());
        let mut frame = StabilizerFrame::new(self.qubit_count);
        let mut record = Vec::with_capacity(self.measurement_count);
        let mut output = Vec::with_capacity(self.measurement_count);
        for _ in 0..shots {
            self.sample_shot_with_reference_into(
                &mut rng,
                reference_sample.as_deref(),
                &mut frame,
                &mut record,
                &mut output,
            );
            visit(&output)?;
        }
        Ok(())
    }
}
