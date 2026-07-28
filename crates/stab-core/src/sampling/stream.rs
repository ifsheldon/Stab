use crate::{MeasurementBatchView, MeasurementSink};

use super::{
    CompiledSampler, RunError, SamplingExecutionError, SamplingRunProgress, SamplingRunSummary,
    ShotCount, legacy_random_policy, legacy_reference_mode,
};

impl CompiledSampler {
    /// Visits samples while preserving both visitor and engine failures.
    pub fn try_for_each_sample_with_seed_and_reference_mode<E, F>(
        &self,
        shots: usize,
        seed: Option<u64>,
        skip_reference_sample: bool,
        visit: F,
    ) -> Result<SamplingRunSummary, RunError<E>>
    where
        F: FnMut(&[bool]) -> Result<(), E>,
    {
        let zero_progress = SamplingRunProgress::new(0, 0);
        let mut session = self
            .plan
            .session_with_reference_mode(
                legacy_random_policy(seed),
                legacy_reference_mode(skip_reference_sample),
            )
            .map_err(|source| RunError::Engine {
                source,
                progress: zero_progress,
            })?;
        let mut record = Vec::new();
        record
            .try_reserve_exact(self.plan.inner.measurement_count)
            .map_err(|error| RunError::Engine {
                source: SamplingExecutionError::SessionStorageAllocation {
                    message: format!(
                        "legacy callback record capacity {}: {error}",
                        self.plan.inner.measurement_count
                    ),
                },
                progress: zero_progress,
            })?;
        let mut sink = CallbackMeasurementSink { visit, record };
        let shots = ShotCount::try_from(shots).map_err(|source| RunError::Engine {
            source,
            progress: zero_progress,
        })?;
        session
            .run(shots, &mut sink)
            .map_err(map_callback_run_error)
    }

    pub fn for_each_sample_with_seed_and_reference_mode<E, F>(
        &self,
        shots: usize,
        seed: Option<u64>,
        skip_reference_sample: bool,
        visit: F,
    ) -> Result<(), E>
    where
        F: FnMut(&[bool]) -> Result<(), E>,
    {
        match self.try_for_each_sample_with_seed_and_reference_mode(
            shots,
            seed,
            skip_reference_sample,
            visit,
        ) {
            Ok(_) => Ok(()),
            Err(RunError::Sink { source, .. }) => Err(source),
            Err(RunError::Engine { source, .. }) => legacy_adapter_failure(source),
        }
    }
}

struct CallbackMeasurementSink<F> {
    visit: F,
    record: Vec<bool>,
}

impl<E, F> MeasurementSink for CallbackMeasurementSink<F>
where
    F: FnMut(&[bool]) -> Result<(), E>,
{
    type Error = CallbackSinkError<E>;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        for shot_index in 0..batch.shot_count() {
            self.record.clear();
            for bit_index in 0..batch.width().get() {
                let Some(bit) = batch.get(shot_index, bit_index) else {
                    return Err(CallbackSinkError::Engine(
                        SamplingExecutionError::InternalInvariant {
                            message: "packed callback record escaped its declared dimensions"
                                .to_owned(),
                        },
                    ));
                };
                self.record.push(bit);
            }
            (self.visit)(&self.record).map_err(CallbackSinkError::Visitor)?;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

enum CallbackSinkError<E> {
    Visitor(E),
    Engine(SamplingExecutionError),
}

fn map_callback_run_error<E>(error: RunError<CallbackSinkError<E>>) -> RunError<E> {
    match error {
        RunError::Engine { source, progress } => RunError::Engine { source, progress },
        RunError::Sink {
            phase,
            source: CallbackSinkError::Visitor(source),
            progress,
        } => RunError::Sink {
            phase,
            source,
            progress,
        },
        RunError::Sink {
            source: CallbackSinkError::Engine(source),
            progress,
            ..
        } => RunError::Engine { source, progress },
    }
}

#[allow(
    clippy::panic,
    reason = "the source-compatible callback adapter cannot represent engine-side failures"
)]
fn legacy_adapter_failure(error: SamplingExecutionError) -> ! {
    panic!("legacy CompiledSampler callback adapter failed: {error}")
}
