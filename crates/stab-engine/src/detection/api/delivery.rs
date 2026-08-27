use std::fmt;

use stab_records::{DetectionSink, MeasurementBatchView, MeasurementSink};

use super::{
    DetectionExecutionError, DetectionRunError, DetectionRunProgress, DetectionRunStatus,
    DetectionRunSummary, MeasurementToDetectionSession,
};

/// One short-lived conversion transaction bound to exactly one detection sink.
pub struct MeasurementToDetectionTransaction<'session, 'sink, Sink> {
    session: &'session mut MeasurementToDetectionSession,
    sink: &'sink mut Sink,
    committed_shots: u64,
    finished: bool,
}

impl<Sink> fmt::Debug for MeasurementToDetectionTransaction<'_, '_, Sink> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MeasurementToDetectionTransaction")
            .field("session", &self.session)
            .field("sink_type", &std::any::type_name::<Sink>())
            .field("committed_shots", &self.committed_shots)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl<'session, 'sink, Sink> MeasurementToDetectionTransaction<'session, 'sink, Sink>
where
    Sink: DetectionSink,
{
    pub(super) fn new(
        session: &'session mut MeasurementToDetectionSession,
        sink: &'sink mut Sink,
    ) -> Self {
        Self {
            session,
            sink,
            committed_shots: 0,
            finished: false,
        }
    }

    /// Converts and immediately writes one validated batch without finalizing this delivery.
    ///
    /// `measurements` and `sweeps`, when supplied, must contain the same number of records and no
    /// more than 64 records.
    pub fn write_batch_with_sweep(
        &mut self,
        measurements: MeasurementBatchView<'_>,
        sweeps: Option<MeasurementBatchView<'_>>,
    ) -> Result<DetectionRunSummary, DetectionRunError<Sink::Error>> {
        if self.finished {
            return Err(DetectionRunError::Engine {
                source: DetectionExecutionError::DeliveryFinished,
                progress: DetectionRunProgress::new(self.committed_shots, 0),
            });
        }
        let summary = self.session.write_batch_with_progress(
            measurements,
            sweeps,
            self.sink,
            self.committed_shots,
        )?;
        self.committed_shots = self
            .committed_shots
            .checked_add(summary.committed_shots().get())
            .ok_or_else(|| DetectionRunError::Engine {
                source: DetectionExecutionError::ShotCounterOverflow,
                progress: DetectionRunProgress::new(self.committed_shots, 0),
            })?;
        Ok(summary)
    }

    /// Finalizes this delivery exactly once.
    pub fn finish(mut self) -> Result<(), DetectionRunError<Sink::Error>> {
        self.finish_once()
    }

    fn finish_once(&mut self) -> Result<(), DetectionRunError<Sink::Error>> {
        if self.finished {
            return Err(DetectionRunError::Engine {
                source: DetectionExecutionError::DeliveryFinished,
                progress: DetectionRunProgress::new(self.committed_shots, 0),
            });
        }
        self.finished = true;
        let result = self.session.finish_sink(self.sink, self.committed_shots);
        self.session.transaction_active = false;
        result
    }
}

impl<Sink> MeasurementSink for MeasurementToDetectionTransaction<'_, '_, Sink>
where
    Sink: DetectionSink,
{
    type Error = DetectionRunError<Sink::Error>;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        let summary = self.write_batch_with_sweep(batch, None)?;
        if summary.status() == DetectionRunStatus::Cancelled {
            return Err(DetectionRunError::Engine {
                source: DetectionExecutionError::CancelledComposition,
                progress: DetectionRunProgress::new(
                    self.committed_shots,
                    summary.requested_shots().get(),
                ),
            });
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finish_once()
    }
}

impl<Sink> Drop for MeasurementToDetectionTransaction<'_, '_, Sink> {
    fn drop(&mut self) {
        self.session.transaction_active = false;
        if !self.finished && self.committed_shots != 0 {
            self.session.poisoned = true;
        }
    }
}
