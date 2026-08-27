#![allow(
    dead_code,
    clippy::expect_used,
    reason = "integration-test fixtures expose only the canonical sampling behavior under test"
)]

use std::convert::Infallible;
use std::fmt::{Display, Formatter};

use stab_core::{
    Circuit, DetectionBatchView, DetectionSink, MeasurementBatchView, MeasurementSink,
    RandomPolicy, ReferenceSampleMode, SamplingCompileError, SamplingCompiler,
    SamplingExecutionError, SamplingPlan, Seed, ShotCount,
    execution::{
        DetectionCompileError, DetectionConversionLimits, DetectionExecutionError,
        DetectionRunError, DetectionSamplingCompiler, MeasurementToDetectionCompiler,
    },
};
use stab_records::{FormatError, PackedShotBatch};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DetectionRecord {
    pub(super) detectors: Vec<bool>,
    pub(super) observables: Vec<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DetectionOutput {
    pub(super) records: Vec<DetectionRecord>,
    pub(super) detector_count: usize,
    pub(super) observable_count: usize,
}

#[derive(Debug)]
pub(super) enum DetectionFixtureError {
    Compile(DetectionCompileError),
    Execution(DetectionExecutionError),
    Format(FormatError),
    ShotCount,
}

impl Display for DetectionFixtureError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compile(error) => Display::fmt(error, formatter),
            Self::Execution(error) => Display::fmt(error, formatter),
            Self::Format(error) => Display::fmt(error, formatter),
            Self::ShotCount => formatter.write_str("test shot count does not fit u64"),
        }
    }
}

impl std::error::Error for DetectionFixtureError {}

impl From<DetectionCompileError> for DetectionFixtureError {
    fn from(error: DetectionCompileError) -> Self {
        Self::Compile(error)
    }
}

impl From<DetectionExecutionError> for DetectionFixtureError {
    fn from(error: DetectionExecutionError) -> Self {
        Self::Execution(error)
    }
}

impl From<FormatError> for DetectionFixtureError {
    fn from(error: FormatError) -> Self {
        Self::Format(error)
    }
}

pub(super) fn convert_detection_records(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    sweeps: Option<&[Vec<bool>]>,
    reference_mode: ReferenceSampleMode,
) -> Result<DetectionOutput, DetectionFixtureError> {
    let plan = MeasurementToDetectionCompiler::new()
        .reference_sample_mode(reference_mode)
        .compile(circuit)?;
    let measurement_batch =
        PackedShotBatch::from_records(measurements, plan.measurement_width().get())?;
    let sweep_batch = sweeps
        .map(|records| PackedShotBatch::from_records(records, plan.sweep_width().get()))
        .transpose()?;
    let mut session = plan.session()?;
    let mut sink = DetectionCollector::default();
    session
        .run(
            MeasurementBatchView::new(measurement_batch.view()),
            sweep_batch
                .as_ref()
                .map(|batch| MeasurementBatchView::new(batch.view())),
            &mut sink,
        )
        .map_err(map_detection_run_error)?;
    Ok(DetectionOutput {
        records: sink.records,
        detector_count: plan.detector_width().get(),
        observable_count: plan.observable_width().get(),
    })
}

pub(super) fn sample_detections(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
) -> Result<DetectionOutput, DetectionFixtureError> {
    sample_detections_with_limits(circuit, shots, seed, DetectionConversionLimits::default())
}

pub(super) fn sample_detections_with_limits(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
    limits: DetectionConversionLimits,
) -> Result<DetectionOutput, DetectionFixtureError> {
    let plan = DetectionSamplingCompiler::new()
        .limits(limits)
        .compile(circuit)?;
    let random_policy = seed.map_or(RandomPolicy::Entropy, |seed| {
        RandomPolicy::Seeded(Seed::new(seed))
    });
    let mut session = plan.session(random_policy)?;
    let mut sink = DetectionCollector::default();
    let shots = u64::try_from(shots)
        .map(ShotCount::new)
        .map_err(|_| DetectionFixtureError::ShotCount)?;
    session
        .run(shots, &mut sink)
        .map_err(map_detection_run_error)?;
    Ok(DetectionOutput {
        records: sink.records,
        detector_count: plan.detector_width().get(),
        observable_count: plan.observable_width().get(),
    })
}

#[derive(Default)]
struct DetectionCollector {
    records: Vec<DetectionRecord>,
}

impl DetectionSink for DetectionCollector {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        for shot in 0..batch.shot_count() {
            self.records.push(DetectionRecord {
                detectors: (0..batch.detector_width().get())
                    .map(|bit| {
                        batch
                            .detectors()
                            .get(shot, bit)
                            .expect("validated detector coordinate")
                    })
                    .collect(),
                observables: (0..batch.observable_width().get())
                    .map(|bit| {
                        batch
                            .observables()
                            .get(shot, bit)
                            .expect("validated observable coordinate")
                    })
                    .collect(),
            });
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn map_detection_run_error(error: DetectionRunError<Infallible>) -> DetectionFixtureError {
    match error {
        DetectionRunError::Engine { source, .. } => source.into(),
        DetectionRunError::Sink { source, .. } => match source {},
    }
}
