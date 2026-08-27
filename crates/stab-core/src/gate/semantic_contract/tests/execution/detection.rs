use std::convert::Infallible;

use crate::{
    Circuit, CircuitError, CircuitResult, DetectionBatchView, DetectionSink, MeasurementBatchView,
    RandomPolicy, ReferenceSampleMode, Seed, ShotCount,
    execution::{DetectionRunError, DetectionSamplingCompiler, MeasurementToDetectionCompiler},
};
use stab_records::PackedShotBatch;

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
) -> CircuitResult<DetectionOutput> {
    if let Some(sweeps) = sweeps
        && sweeps.len() != measurements.len()
    {
        return Err(CircuitError::invalid_result_format(format!(
            "measurement and sweep record counts differ: {} versus {}",
            measurements.len(),
            sweeps.len()
        )));
    }
    let plan = MeasurementToDetectionCompiler::new()
        .reference_sample_mode(reference_mode)
        .compile(circuit)
        .map_err(CircuitError::from)?;
    let mut session = plan.session().map_err(CircuitError::from)?;
    let mut sink = DetectionCollector::default();
    for (chunk_index, measurement_records) in measurements.chunks(64).enumerate() {
        let measurement_batch =
            PackedShotBatch::from_records(measurement_records, plan.measurement_width().get())
                .map_err(CircuitError::from)?;
        let start = chunk_index * 64;
        let sweep_batch = sweeps
            .map(|records| -> CircuitResult<PackedShotBatch> {
                let end = start
                    .checked_add(measurement_records.len())
                    .ok_or_else(|| {
                        CircuitError::invalid_result_format("sweep record range overflowed")
                    })?;
                let records = records.get(start..end).ok_or_else(|| {
                    CircuitError::invalid_result_format("sweep record range is out of bounds")
                })?;
                PackedShotBatch::from_records(records, plan.sweep_width().get())
                    .map_err(CircuitError::from)
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
) -> CircuitResult<DetectionOutput> {
    let plan = DetectionSamplingCompiler::new()
        .compile(circuit)
        .map_err(CircuitError::from)?;
    let random_policy = seed.map_or(RandomPolicy::Entropy, |seed| {
        RandomPolicy::Seeded(Seed::new(seed))
    });
    let mut session = plan.session(random_policy).map_err(CircuitError::from)?;
    let mut sink = DetectionCollector::default();
    session
        .run(
            ShotCount::new(u64::try_from(shots).map_err(|_| {
                CircuitError::invalid_sampler_compilation("test shot count does not fit u64")
            })?),
            &mut sink,
        )
        .map_err(map_detection_run_error)?;
    Ok(DetectionOutput {
        records: sink.records,
        detector_count: plan.detector_width().get(),
        observable_count: plan.observable_width().get(),
    })
}

fn map_detection_run_error(error: DetectionRunError<Infallible>) -> CircuitError {
    match error {
        DetectionRunError::Engine { source, .. } => source.into(),
        DetectionRunError::Sink { source, .. } => match source {},
    }
}
