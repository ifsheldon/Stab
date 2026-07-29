use std::fmt;

use stab_records::{DetectionSink, MeasurementBatchView, MeasurementSink};

use super::{
    DetectionExecutionError, DetectionRunError, DetectionRunSummary, run_error_from_engine,
    summary_from_engine,
};

/// Adapts a detection sink into the measurement-sink seam consumed by a sampling session.
pub struct MeasurementToDetectionSinkAdapter<'session, 'sink, Sink>
where
    Sink: DetectionSink,
{
    inner: stab_engine::MeasurementToDetectionSinkAdapter<'session, 'sink, Sink>,
}

impl<Sink> fmt::Debug for MeasurementToDetectionSinkAdapter<'_, '_, Sink>
where
    Sink: DetectionSink,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MeasurementToDetectionSinkAdapter")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<'session, 'sink, Sink> MeasurementToDetectionSinkAdapter<'session, 'sink, Sink>
where
    Sink: DetectionSink,
{
    pub(super) const fn from_engine(
        inner: stab_engine::MeasurementToDetectionSinkAdapter<'session, 'sink, Sink>,
    ) -> Self {
        Self { inner }
    }

    pub fn write_batch_with_sweep(
        &mut self,
        measurements: MeasurementBatchView<'_>,
        sweeps: Option<MeasurementBatchView<'_>>,
    ) -> Result<DetectionRunSummary, DetectionRunError<Sink::Error>> {
        self.inner
            .write_batch_with_sweep(measurements, sweeps)
            .map(summary_from_engine)
            .map_err(run_error_from_engine)
    }

    pub fn finish(self) -> Result<(), DetectionRunError<Sink::Error>> {
        self.inner.finish().map_err(run_error_from_engine)
    }
}

impl<Sink> MeasurementSink for MeasurementToDetectionSinkAdapter<'_, '_, Sink>
where
    Sink: DetectionSink,
{
    type Error = DetectionRunError<Sink::Error>;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        self.inner.write_batch(batch).map_err(run_error_from_engine)
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        MeasurementSink::finish(&mut self.inner).map_err(run_error_from_engine)
    }
}

impl From<stab_engine::DetectionExecutionError> for DetectionExecutionError {
    fn from(error: stab_engine::DetectionExecutionError) -> Self {
        Self::from_engine(error)
    }
}
