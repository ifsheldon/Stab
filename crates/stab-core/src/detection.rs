mod api;
mod buffers;
mod output;

pub use api::{
    DetectionCompileError, DetectionExecutionError, DetectionRunError, DetectionRunProgress,
    DetectionRunStatus, DetectionRunSummary, DetectionSamplingCompiler, DetectionSamplingPlan,
    DetectionSamplingSession, MeasurementToDetectionCompiler, MeasurementToDetectionPlan,
    MeasurementToDetectionSession, MeasurementToDetectionSinkAdapter,
};
pub use output::{
    write_detection_records, write_observable_records, write_ptb64_detection_records,
    write_ptb64_observable_records,
};
pub use stab_engine::{
    DetectionConversionLimits, DetectionConversionOptions, DetectionEventRecord,
};

use buffers::{
    try_clone_detection_record, try_false_vec, try_reserve_detection_record_slots,
    validate_buffer_bits,
};

use crate::resources::DetectionBufferLimitSubject;
use crate::{Circuit, CircuitError, CircuitResult};

/// Compatibility adapter over the engine-owned per-record converter.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledDetectionConverter {
    inner: stab_engine::CompiledDetectionConverter,
}

impl CompiledDetectionConverter {
    pub fn compile(circuit: &Circuit, options: DetectionConversionOptions) -> CircuitResult<Self> {
        Self::compile_with_limits(circuit, options, DetectionConversionLimits::default())
    }

    pub fn compile_with_limits(
        circuit: &Circuit,
        options: DetectionConversionOptions,
        limits: DetectionConversionLimits,
    ) -> CircuitResult<Self> {
        stab_engine::CompiledDetectionConverter::compile_with_limits(circuit, options, limits)
            .map(|inner| Self { inner })
            .map_err(Into::into)
    }

    pub fn measurement_count(&self) -> usize {
        self.inner.measurement_count()
    }

    pub fn sweep_bit_count(&self) -> usize {
        self.inner.sweep_bit_count()
    }

    pub fn detector_count(&self) -> usize {
        self.inner.detector_count()
    }

    pub fn observable_count(&self) -> usize {
        self.inner.observable_count()
    }

    pub fn convert_record(
        &self,
        measurement_record: &[bool],
    ) -> CircuitResult<DetectionEventRecord> {
        self.inner
            .convert_record(measurement_record)
            .map_err(Into::into)
    }

    pub fn try_for_each_detection_event<'a, E, I, F>(
        &self,
        measurements: I,
        mut visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        I: IntoIterator<Item = &'a [bool]>,
        F: FnMut(&DetectionEventRecord) -> Result<(), E>,
    {
        self.inner
            .try_for_each_detection_event::<FacadeVisitError<E>, _, _>(measurements, |record| {
                visit(record).map_err(FacadeVisitError::Visitor)
            })
            .map_err(FacadeVisitError::into_external)
    }

    pub fn try_for_each_detection_event_with_sweep<'a, 'b, E, M, S, F>(
        &self,
        measurements: M,
        sweeps: S,
        mut visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        M: IntoIterator<Item = &'a [bool]>,
        S: IntoIterator<Item = &'b [bool]>,
        F: FnMut(&DetectionEventRecord) -> Result<(), E>,
    {
        self.inner
            .try_for_each_detection_event_with_sweep::<FacadeVisitError<E>, _, _, _>(
                measurements,
                sweeps,
                |record| visit(record).map_err(FacadeVisitError::Visitor),
            )
            .map_err(FacadeVisitError::into_external)
    }

    pub fn reusable_detection_record(&self) -> DetectionEventRecord {
        self.inner.reusable_detection_record()
    }

    pub fn try_reusable_detection_record(&self) -> CircuitResult<DetectionEventRecord> {
        self.inner
            .try_reusable_detection_record()
            .map_err(Into::into)
    }

    pub fn reusable_reference_sample(&self) -> Vec<bool> {
        self.inner.reusable_reference_sample()
    }

    pub fn try_reusable_reference_sample(&self) -> CircuitResult<Vec<bool>> {
        self.inner
            .try_reusable_reference_sample()
            .map_err(Into::into)
    }

    pub fn convert_record_with_sweep_into(
        &self,
        measurement_record: &[bool],
        sweep_record: &[bool],
        reference_sample: &mut Vec<bool>,
        record: &mut DetectionEventRecord,
    ) -> CircuitResult<()> {
        self.inner
            .convert_record_with_sweep_into(
                measurement_record,
                sweep_record,
                reference_sample,
                record,
            )
            .map_err(Into::into)
    }
}

enum FacadeVisitError<E> {
    Engine(stab_engine::DetectionError),
    Visitor(E),
}

impl<E> From<stab_engine::DetectionError> for FacadeVisitError<E> {
    fn from(error: stab_engine::DetectionError) -> Self {
        Self::Engine(error)
    }
}

impl<E> FacadeVisitError<E>
where
    E: From<CircuitError>,
{
    fn into_external(self) -> E {
        match self {
            Self::Engine(error) => E::from(CircuitError::from(error)),
            Self::Visitor(error) => error,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionConversionOutput {
    pub records: Vec<DetectionEventRecord>,
    pub detector_count: usize,
    pub observable_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetectionObservableOutputMode {
    DetectorsOnly,
    Append,
    Prepend,
}

pub fn convert_measurements_to_detection_events(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    options: DetectionConversionOptions,
) -> CircuitResult<DetectionConversionOutput> {
    convert_measurements_to_detection_events_with_limits(
        circuit,
        measurements,
        options,
        DetectionConversionLimits::default(),
    )
}

pub fn convert_measurements_to_detection_events_with_limits(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    options: DetectionConversionOptions,
    limits: DetectionConversionLimits,
) -> CircuitResult<DetectionConversionOutput> {
    let converter = CompiledDetectionConverter::compile_with_limits(circuit, options, limits)?;
    validate_materialized_conversion(&converter, measurements.len(), limits)?;
    let mut records = Vec::new();
    try_reserve_detection_record_slots(&mut records, measurements.len())?;
    converter.try_for_each_detection_event(measurements.iter().map(Vec::as_slice), |record| {
        records.push(try_clone_detection_record(record)?);
        Ok::<(), CircuitError>(())
    })?;

    Ok(DetectionConversionOutput {
        records,
        detector_count: converter.detector_count(),
        observable_count: converter.observable_count(),
    })
}

pub fn convert_measurements_to_detection_events_with_sweep(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    sweeps: &[Vec<bool>],
    options: DetectionConversionOptions,
) -> CircuitResult<DetectionConversionOutput> {
    convert_measurements_to_detection_events_with_sweep_and_limits(
        circuit,
        measurements,
        sweeps,
        options,
        DetectionConversionLimits::default(),
    )
}

pub fn convert_measurements_to_detection_events_with_sweep_and_limits(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    sweeps: &[Vec<bool>],
    options: DetectionConversionOptions,
    limits: DetectionConversionLimits,
) -> CircuitResult<DetectionConversionOutput> {
    if measurements.len() != sweeps.len() {
        return Err(CircuitError::invalid_result_format(format!(
            "measurement records have {} shots but sweep records have {} shots",
            measurements.len(),
            sweeps.len()
        )));
    }
    let converter = CompiledDetectionConverter::compile_with_limits(circuit, options, limits)?;
    validate_materialized_conversion(&converter, measurements.len(), limits)?;
    validate_buffer_bits(
        DetectionBufferLimitSubject::SweepRecords,
        sweeps.len(),
        converter.sweep_bit_count(),
        limits.max_materialized_bits(),
    )?;
    let mut records = Vec::new();
    try_reserve_detection_record_slots(&mut records, measurements.len())?;
    converter.try_for_each_detection_event_with_sweep(
        measurements.iter().map(Vec::as_slice),
        sweeps.iter().map(Vec::as_slice),
        |record| {
            records.push(try_clone_detection_record(record)?);
            Ok::<(), CircuitError>(())
        },
    )?;

    Ok(DetectionConversionOutput {
        records,
        detector_count: converter.detector_count(),
        observable_count: converter.observable_count(),
    })
}

fn validate_materialized_conversion(
    converter: &CompiledDetectionConverter,
    shots: usize,
    limits: DetectionConversionLimits,
) -> CircuitResult<()> {
    validate_buffer_bits(
        DetectionBufferLimitSubject::MeasurementSamples,
        shots,
        converter.measurement_count(),
        limits.max_materialized_bits(),
    )?;
    let output_width = converter
        .detector_count()
        .checked_add(converter.observable_count())
        .ok_or_else(|| CircuitError::invalid_result_format("detection record width overflowed"))?;
    validate_buffer_bits(
        DetectionBufferLimitSubject::DetectionRecords,
        shots,
        output_width,
        limits.max_materialized_bits(),
    )
}

pub fn sample_detection_events(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
) -> CircuitResult<DetectionConversionOutput> {
    sample_detection_events_with_limits(circuit, shots, seed, DetectionConversionLimits::default())
}

pub fn sample_detection_events_with_limits(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
    limits: DetectionConversionLimits,
) -> CircuitResult<DetectionConversionOutput> {
    api::sample_materialized(circuit, shots, seed, limits)
}

pub fn try_for_each_sampled_detection_event<E, F>(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
    visit: F,
) -> Result<(), E>
where
    E: From<CircuitError>,
    F: FnMut(&DetectionEventRecord) -> Result<(), E>,
{
    try_for_each_sampled_detection_event_with_limits(
        circuit,
        shots,
        seed,
        DetectionConversionLimits::default(),
        visit,
    )
}

pub fn try_for_each_sampled_detection_event_with_limits<E, F>(
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
    api::try_for_each(circuit, shots, seed, limits, visit)
}

pub fn measurement_record_count(circuit: &Circuit) -> CircuitResult<usize> {
    measurement_record_count_with_limits(circuit, DetectionConversionLimits::default())
}

pub fn measurement_record_count_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> CircuitResult<usize> {
    stab_engine::measurement_record_count_with_limits(circuit, limits).map_err(Into::into)
}

pub fn detection_record_width(circuit: &Circuit) -> CircuitResult<usize> {
    detection_record_width_with_limits(circuit, DetectionConversionLimits::default())
}

pub fn detection_record_width_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> CircuitResult<usize> {
    stab_engine::detection_record_width_with_limits(circuit, limits).map_err(Into::into)
}

pub fn validate_detection_sampling_circuit(circuit: &Circuit) -> CircuitResult<()> {
    validate_detection_sampling_circuit_with_limits(circuit, DetectionConversionLimits::default())
}

pub fn validate_detection_sampling_circuit_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> CircuitResult<()> {
    stab_engine::validate_detection_sampling_circuit_with_limits(circuit, limits)
        .map_err(Into::into)
}

#[cfg(test)]
mod tests;
