use stab_records::{DetectionBatchView, DetectionSink};

use super::{
    DetectionExecutionError, DetectionRunError, DetectionSamplingCompiler, DetectionSamplingPlan,
};
use crate::resources::DetectionBufferLimitSubject;
use crate::{
    Circuit, CircuitError, CircuitResult, DetectionConversionLimits, DetectionConversionOutput,
    DetectionEventRecord, RandomPolicy, Seed, ShotCount,
};

use crate::detection::{try_false_vec, try_reserve_detection_record_slots, validate_buffer_bits};

pub(in crate::detection) fn sample_materialized(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
    limits: DetectionConversionLimits,
) -> CircuitResult<DetectionConversionOutput> {
    let plan = DetectionSamplingCompiler::new()
        .limits(limits)
        .compile(circuit)
        .map_err(|error| error.into_circuit_error())?;
    let output_width = plan
        .detector_width()
        .get()
        .checked_add(plan.observable_width().get())
        .ok_or_else(|| CircuitError::invalid_result_format("detection record width overflowed"))?;
    validate_buffer_bits(
        DetectionBufferLimitSubject::DetectionRecords,
        shots,
        output_width,
        limits.max_materialized_bits(),
    )?;
    let mut sink = MaterializingSink::new(shots)?;
    let mut session = plan
        .session(random_policy(seed))
        .map_err(DetectionExecutionError::into_circuit_error)?;
    session
        .run(shot_count(shots)?, &mut sink)
        .map_err(map_circuit_run_error)?;
    Ok(DetectionConversionOutput {
        records: sink.records,
        detector_count: plan.detector_width().get(),
        observable_count: plan.observable_width().get(),
    })
}

pub(in crate::detection) fn try_for_each<E, F>(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
    limits: DetectionConversionLimits,
    visit: F,
) -> Result<(), E>
where
    E: From<CircuitError>,
    F: FnMut(&DetectionEventRecord) -> Result<(), E>,
{
    let plan = DetectionSamplingCompiler::new()
        .limits(limits)
        .compile(circuit)
        .map_err(|error| E::from(error.into_circuit_error()))?;
    let mut session = plan
        .session(random_policy(seed))
        .map_err(|error| E::from(error.into_circuit_error()))?;
    let mut sink = CallbackSink::new(&plan, visit).map_err(E::from)?;
    session
        .run(shot_count(shots).map_err(E::from)?, &mut sink)
        .map(|_| ())
        .map_err(map_callback_run_error)
}

struct MaterializingSink {
    records: Vec<DetectionEventRecord>,
}

impl MaterializingSink {
    fn new(shots: usize) -> CircuitResult<Self> {
        let mut records = Vec::new();
        try_reserve_detection_record_slots(&mut records, shots)?;
        Ok(Self { records })
    }
}

impl DetectionSink for MaterializingSink {
    type Error = CircuitError;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> CircuitResult<()> {
        for shot_index in 0..batch.shot_count() {
            let mut detectors =
                try_false_vec(batch.detector_width().get(), "materialized detector record")?;
            let mut observables = try_false_vec(
                batch.observable_width().get(),
                "materialized observable record",
            )?;
            copy_part(batch.detectors(), shot_index, &mut detectors, "detector")?;
            copy_part(
                batch.observables(),
                shot_index,
                &mut observables,
                "observable",
            )?;
            self.records.push(DetectionEventRecord {
                detectors,
                observables,
            });
        }
        Ok(())
    }

    fn finish(&mut self) -> CircuitResult<()> {
        Ok(())
    }
}

struct CallbackSink<F> {
    visit: F,
    record: DetectionEventRecord,
}

impl<F> CallbackSink<F> {
    fn new<E>(plan: &DetectionSamplingPlan, visit: F) -> CircuitResult<Self>
    where
        F: FnMut(&DetectionEventRecord) -> Result<(), E>,
    {
        Ok(Self {
            visit,
            record: DetectionEventRecord {
                detectors: try_false_vec(
                    plan.detector_width().get(),
                    "detection callback detector record",
                )?,
                observables: try_false_vec(
                    plan.observable_width().get(),
                    "detection callback observable record",
                )?,
            },
        })
    }
}

enum CallbackError<E> {
    Engine(CircuitError),
    Visitor(E),
}

impl<E, F> DetectionSink for CallbackSink<F>
where
    F: FnMut(&DetectionEventRecord) -> Result<(), E>,
{
    type Error = CallbackError<E>;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        for shot_index in 0..batch.shot_count() {
            copy_part(
                batch.detectors(),
                shot_index,
                &mut self.record.detectors,
                "detector",
            )
            .map_err(CallbackError::Engine)?;
            copy_part(
                batch.observables(),
                shot_index,
                &mut self.record.observables,
                "observable",
            )
            .map_err(CallbackError::Engine)?;
            (self.visit)(&self.record).map_err(CallbackError::Visitor)?;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn copy_part(
    records: stab_records::PackedShotBatchView<'_>,
    shot_index: usize,
    output: &mut [bool],
    kind: &'static str,
) -> CircuitResult<()> {
    for (bit_index, slot) in output.iter_mut().enumerate() {
        *slot = records.get(shot_index, bit_index).ok_or_else(|| {
            CircuitError::invalid_result_format(format!(
                "{kind} batch escaped its dimensions at shot {shot_index}, bit {bit_index}"
            ))
        })?;
    }
    Ok(())
}

fn random_policy(seed: Option<u64>) -> RandomPolicy {
    seed.map_or(RandomPolicy::Entropy, |seed| {
        RandomPolicy::Seeded(Seed::new(seed))
    })
}

fn shot_count(shots: usize) -> CircuitResult<ShotCount> {
    u64::try_from(shots)
        .map(ShotCount::new)
        .map_err(|_| CircuitError::invalid_sampler_compilation("shot count does not fit u64"))
}

fn map_circuit_run_error(error: DetectionRunError<CircuitError>) -> CircuitError {
    match error {
        DetectionRunError::Engine { source, .. } => source.into_circuit_error(),
        DetectionRunError::Sink { source, .. } => source,
    }
}

fn map_callback_run_error<E>(error: DetectionRunError<CallbackError<E>>) -> E
where
    E: From<CircuitError>,
{
    match error {
        DetectionRunError::Engine { source, .. } => E::from(source.into_circuit_error()),
        DetectionRunError::Sink {
            source: CallbackError::Engine(source),
            ..
        } => E::from(source),
        DetectionRunError::Sink {
            source: CallbackError::Visitor(source),
            ..
        } => source,
    }
}
