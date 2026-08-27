#![allow(
    dead_code,
    clippy::expect_used,
    reason = "integration-test fixtures expose only the canonical sampling behavior under test"
)]

use std::convert::Infallible;

use stab_core::{
    Circuit, MeasurementBatchView, MeasurementSink, RandomPolicy, ReferenceSampleMode,
    SamplingCompileError, SamplingCompiler, SamplingExecutionError, SamplingPlan, Seed, ShotCount,
};

#[derive(Debug)]
pub(super) struct SamplingFixture {
    plan: SamplingPlan,
}

impl SamplingFixture {
    pub(super) fn compile(circuit: &Circuit) -> Result<Self, SamplingCompileError> {
        SamplingCompiler::new()
            .compile(circuit)
            .map(|plan| Self { plan })
    }

    pub(super) const fn plan(&self) -> &SamplingPlan {
        &self.plan
    }

    pub(super) fn reference_sample(&self) -> Result<Vec<bool>, SamplingExecutionError> {
        self.plan.try_reference_sample()
    }

    pub(super) fn count_determined_measurements(
        &self,
        unknown_input: bool,
    ) -> Result<u64, SamplingExecutionError> {
        self.plan.try_count_determined_measurements(unknown_input)
    }

    pub(super) fn sample_zero_one(&self, shots: usize) -> Vec<Vec<bool>> {
        self.sample_zero_one_with_seed(shots, None)
    }

    pub(super) fn sample_zero_one_with_seed(
        &self,
        shots: usize,
        seed: Option<u64>,
    ) -> Vec<Vec<bool>> {
        self.sample_zero_one_with_seed_and_reference_mode(shots, seed, false)
    }

    pub(super) fn sample_zero_one_with_seed_and_reference_mode(
        &self,
        shots: usize,
        seed: Option<u64>,
        skip_reference_sample: bool,
    ) -> Vec<Vec<bool>> {
        let random_policy = seed.map_or(RandomPolicy::Entropy, |seed| {
            RandomPolicy::Seeded(Seed::new(seed))
        });
        let reference_mode = if skip_reference_sample {
            ReferenceSampleMode::SkipReferenceSample
        } else {
            ReferenceSampleMode::UseReferenceSample
        };
        let mut session = self
            .plan
            .session_with_reference_mode(random_policy, reference_mode)
            .expect("construct sampling fixture session");
        let mut sink = MeasurementCollector::default();
        session
            .run(
                ShotCount::new(u64::try_from(shots).expect("fixture shot count fits u64")),
                &mut sink,
            )
            .expect("run sampling fixture");
        sink.records
    }
}

#[derive(Default)]
struct MeasurementCollector {
    records: Vec<Vec<bool>>,
}

impl MeasurementSink for MeasurementCollector {
    type Error = Infallible;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        for shot in 0..batch.shot_count() {
            self.records.push(
                (0..batch.width().get())
                    .map(|bit| {
                        batch
                            .get(shot, bit)
                            .expect("validated fixture batch coordinate")
                    })
                    .collect(),
            );
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
