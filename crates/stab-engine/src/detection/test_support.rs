#![allow(
    clippy::expect_used,
    reason = "test-only collection follows dimensions already validated by DetectionBatchView"
)]

use std::convert::Infallible;
use std::fmt::{Display, Formatter};

use stab_model::Circuit;
use stab_records::{DetectionBatchView, DetectionSink};

use super::{
    CompiledDetectionConverter, DetectionConversionOptions, DetectionEventRecord,
    DetectionSamplingCompiler,
};
use crate::{RandomPolicy, Seed, ShotCount};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DetectionConversionOutput {
    pub(super) records: Vec<DetectionEventRecord>,
    pub(super) detector_count: usize,
    pub(super) observable_count: usize,
}

#[derive(Debug)]
pub(super) struct TestDetectionError(String);

impl Display for TestDetectionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TestDetectionError {}

impl From<TestDetectionError> for super::DetectionError {
    fn from(error: TestDetectionError) -> Self {
        Self::invalid_sampler_compilation(error.0)
    }
}

pub(super) fn convert_measurements_to_detection_events(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    options: DetectionConversionOptions,
) -> Result<DetectionConversionOutput, TestDetectionError> {
    let converter = CompiledDetectionConverter::compile(circuit, options).map_err(display_error)?;
    let mut records = Vec::with_capacity(measurements.len());
    converter
        .try_for_each_detection_event::<super::DetectionError, _, _>(
            measurements.iter().map(Vec::as_slice),
            |record| {
                records.push(record.clone());
                Ok(())
            },
        )
        .map_err(display_error)?;
    Ok(DetectionConversionOutput {
        records,
        detector_count: converter.detector_count(),
        observable_count: converter.observable_count(),
    })
}

pub(super) fn convert_measurements_to_detection_events_with_sweep(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    sweeps: &[Vec<bool>],
    options: DetectionConversionOptions,
) -> Result<DetectionConversionOutput, TestDetectionError> {
    if measurements.len() != sweeps.len() {
        return Err(TestDetectionError(format!(
            "invalid result format data: measurement records have {} shots but sweep records have {} shots",
            measurements.len(),
            sweeps.len()
        )));
    }
    let converter = CompiledDetectionConverter::compile(circuit, options).map_err(display_error)?;
    let mut records = Vec::with_capacity(measurements.len());
    converter
        .try_for_each_detection_event_with_sweep::<super::DetectionError, _, _, _>(
            measurements.iter().map(Vec::as_slice),
            sweeps.iter().map(Vec::as_slice),
            |record| {
                records.push(record.clone());
                Ok(())
            },
        )
        .map_err(display_error)?;
    Ok(DetectionConversionOutput {
        records,
        detector_count: converter.detector_count(),
        observable_count: converter.observable_count(),
    })
}

pub(super) fn sample_detection_events(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
) -> Result<DetectionConversionOutput, TestDetectionError> {
    let plan = DetectionSamplingCompiler::new()
        .compile(circuit)
        .map_err(display_error)?;
    let mut session = plan
        .session(seed.map_or(RandomPolicy::Entropy, |seed| {
            RandomPolicy::Seeded(Seed::new(seed))
        }))
        .map_err(display_error)?;
    let mut sink = CollectSink::default();
    session
        .run(
            ShotCount::new(
                u64::try_from(shots).map_err(|error| TestDetectionError(error.to_string()))?,
            ),
            &mut sink,
        )
        .map_err(|error| TestDetectionError(error.to_string()))?;
    Ok(DetectionConversionOutput {
        records: sink.records,
        detector_count: plan.detector_width().get(),
        observable_count: plan.observable_width().get(),
    })
}

pub(super) fn try_for_each_sampled_detection_event<E>(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
    mut visit: impl FnMut(&DetectionEventRecord) -> Result<(), E>,
) -> Result<(), E>
where
    E: From<TestDetectionError>,
{
    let output = sample_detection_events(circuit, shots, seed).map_err(E::from)?;
    for record in &output.records {
        visit(record)?;
    }
    Ok(())
}

#[derive(Default)]
struct CollectSink {
    records: Vec<DetectionEventRecord>,
}

impl DetectionSink for CollectSink {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        for shot_index in 0..batch.shot_count() {
            let detectors = (0..batch.detector_width().get())
                .map(|bit_index| {
                    batch
                        .detectors()
                        .get(shot_index, bit_index)
                        .expect("declared detector bit")
                })
                .collect();
            let observables = (0..batch.observable_width().get())
                .map(|bit_index| {
                    batch
                        .observables()
                        .get(shot_index, bit_index)
                        .expect("declared observable bit")
                })
                .collect();
            self.records.push(DetectionEventRecord {
                detectors,
                observables,
            });
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn display_error(error: impl Display) -> TestDetectionError {
    TestDetectionError(error.to_string())
}
