use std::marker::PhantomData;

use crate::{CircuitError, CircuitResult, DemSampleBatchView, DemSampleSink, DetectionEventRecord};
use stab_engine::{DemSamplingPlan, DemSamplingRunError};

pub(super) struct DetectionVisitorSink<E, F> {
    visit: F,
    record: DetectionEventRecord,
    error: PhantomData<fn() -> E>,
}

impl<E, F> DetectionVisitorSink<E, F> {
    pub(super) fn try_new(plan: &DemSamplingPlan, visit: F) -> CircuitResult<Self> {
        Ok(Self {
            visit,
            record: plan.try_reusable_detection_record()?,
            error: PhantomData,
        })
    }
}

impl<E, F> DemSampleSink for DetectionVisitorSink<E, F>
where
    E: From<CircuitError>,
    F: FnMut(&DetectionEventRecord) -> Result<(), E>,
{
    type Error = E;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        let detection = batch.detection();
        for shot_index in 0..batch.detection().shot_count() {
            copy_record(detection, shot_index, &mut self.record).map_err(E::from)?;
            (self.visit)(&self.record)?;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(super) struct DetectionAndErrorVisitorSink<E, F> {
    visit: F,
    record: DetectionEventRecord,
    error_record: Vec<bool>,
    error: PhantomData<fn() -> E>,
}

impl<E, F> DetectionAndErrorVisitorSink<E, F> {
    pub(super) fn try_new(plan: &DemSamplingPlan, visit: F) -> CircuitResult<Self> {
        Ok(Self {
            visit,
            record: plan.try_reusable_detection_record()?,
            error_record: plan.try_reusable_error_record()?,
            error: PhantomData,
        })
    }
}

impl<E, F> DemSampleSink for DetectionAndErrorVisitorSink<E, F>
where
    E: From<CircuitError>,
    F: FnMut(&DetectionEventRecord, &[bool]) -> Result<(), E>,
{
    type Error = E;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        let detection = batch.detection();
        let sampled_errors = batch.sampled_errors().ok_or_else(|| {
            E::from(CircuitError::invalid_sampler_compilation(
                "DEM compatibility sink expected sampled-error records",
            ))
        })?;
        if self.error_record.capacity() < sampled_errors.bits_per_shot() {
            return Err(E::from(CircuitError::invalid_sampler_compilation(
                "DEM compatibility sink sampled-error scratch lost its admitted capacity",
            )));
        }
        for shot_index in 0..detection.shot_count() {
            copy_record(detection, shot_index, &mut self.record).map_err(E::from)?;
            self.error_record.clear();
            for bit_index in 0..sampled_errors.bits_per_shot() {
                self.error_record
                    .push(sampled_errors.get(shot_index, bit_index).ok_or_else(|| {
                        E::from(CircuitError::invalid_sampler_compilation(
                            "DEM compatibility sink sampled-error bit escaped its batch",
                        ))
                    })?);
            }
            (self.visit)(&self.record, &self.error_record)?;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(super) fn map_run_error<E>(error: DemSamplingRunError<E>) -> E
where
    E: From<CircuitError>,
{
    match error {
        DemSamplingRunError::Engine { source, .. } => E::from(CircuitError::from(source)),
        DemSamplingRunError::Sink { source, .. } => source,
    }
}

fn copy_record(
    detection: crate::DetectionBatchView<'_>,
    shot_index: usize,
    record: &mut DetectionEventRecord,
) -> CircuitResult<()> {
    if record.detectors.len() != detection.detector_width().get()
        || record.observables.len() != detection.observable_width().get()
    {
        return Err(CircuitError::invalid_sampler_compilation(
            "DEM compatibility sink record scratch width changed after admission",
        ));
    }
    for bit_index in 0..record.detectors.len() {
        let bit = detection
            .detectors()
            .get(shot_index, bit_index)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM detector bit escaped its batch dimensions",
                )
            })?;
        let output = record.detectors.get_mut(bit_index).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation(
                "DEM detector scratch escaped its admitted width",
            )
        })?;
        *output = bit;
    }
    for bit_index in 0..record.observables.len() {
        let bit = detection
            .observables()
            .get(shot_index, bit_index)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM observable bit escaped its batch dimensions",
                )
            })?;
        let output = record.observables.get_mut(bit_index).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation(
                "DEM observable scratch escaped its admitted width",
            )
        })?;
        *output = bit;
    }
    Ok(())
}
