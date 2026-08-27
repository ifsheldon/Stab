use std::convert::Infallible;
use std::error::Error;

use crate::{
    Circuit, DetectionBatchView, DetectionSink, MeasurementBatchView,
    execution::{
        DetectionRunError, DetectionSamplingCompiler, MeasurementToDetectionCompiler, RandomPolicy,
        ReferenceSampleMode, Seed, ShotCount,
    },
};
use stab_records::PackedShotBatch;

type TestResult<T> = Result<T, Box<dyn Error>>;

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

pub(super) fn convert_detection_records(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    sweeps: Option<&[Vec<bool>]>,
    reference_mode: ReferenceSampleMode,
) -> TestResult<DetectionOutput> {
    if let Some(sweeps) = sweeps
        && sweeps.len() != measurements.len()
    {
        return Err(std::io::Error::other(format!(
            "measurement and sweep record counts differ: {} versus {}",
            measurements.len(),
            sweeps.len()
        ))
        .into());
    }
    let plan = MeasurementToDetectionCompiler::new()
        .reference_sample_mode(reference_mode)
        .compile(circuit)?;
    let mut session = plan.session()?;
    let mut sink = DetectionCollector::default();
    for (chunk_index, measurement_records) in measurements.chunks(64).enumerate() {
        let measurement_batch =
            PackedShotBatch::from_records(measurement_records, plan.measurement_width().get())?;
        let start = chunk_index * 64;
        let sweep_batch = sweeps
            .map(|records| -> TestResult<PackedShotBatch> {
                let end = start
                    .checked_add(measurement_records.len())
                    .ok_or_else(|| std::io::Error::other("sweep record range overflowed"))?;
                let records = records
                    .get(start..end)
                    .ok_or_else(|| std::io::Error::other("sweep record range is out of bounds"))?;
                Ok(PackedShotBatch::from_records(
                    records,
                    plan.sweep_width().get(),
                )?)
            })
            .transpose()?;
        session
            .run(
                MeasurementBatchView::new(measurement_batch.view()),
                sweep_batch
                    .as_ref()
                    .map(|batch| MeasurementBatchView::new(batch.view())),
                &mut sink,
            )
            .map_err(map_detection_run_error)?;
    }
    Ok(DetectionOutput {
        records: sink.records,
        detector_count: plan.detector_width().get(),
        observable_count: plan.observable_width().get(),
    })
}

pub(super) fn collect_detection_samples(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
) -> TestResult<DetectionOutput> {
    let plan = DetectionSamplingCompiler::new().compile(circuit)?;
    let random_policy = seed.map_or(RandomPolicy::Entropy, |seed| {
        RandomPolicy::Seeded(Seed::new(seed))
    });
    let mut session = plan.session(random_policy)?;
    let mut sink = DetectionCollector::default();
    session
        .run(ShotCount::new(u64::try_from(shots)?), &mut sink)
        .map_err(map_detection_run_error)?;
    Ok(DetectionOutput {
        records: sink.records,
        detector_count: plan.detector_width().get(),
        observable_count: plan.observable_width().get(),
    })
}

fn map_detection_run_error(error: DetectionRunError<Infallible>) -> Box<dyn Error> {
    match error {
        DetectionRunError::Engine { source, .. } => Box::new(source),
        DetectionRunError::Sink { source, .. } => match source {},
    }
}
