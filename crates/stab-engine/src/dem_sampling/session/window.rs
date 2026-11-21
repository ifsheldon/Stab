//! Delivery of generated DEM samples as word-aligned windows of the sampled bit planes.

use stab_records::{BitPlaneBatchView, DemSampleBatchView, DemSampleSink, DetectionBatchView};

use super::{DemSamplingExecutionError, DemSamplingSession, internal_format_error};
use crate::{RunError, RunProgress, SinkFailurePhase};

impl DemSamplingSession {
    /// Delivers shots `window_start..window_start + window_shots` of the sampled planes, which
    /// hold `sample_shots` shots in bit-major order, as one bit-plane batch.
    pub(super) fn write_sample_window<Sink>(
        &mut self,
        sample_shots: usize,
        window_start: usize,
        window_shots: usize,
        include_sampled_errors: bool,
        sink: &mut Sink,
        committed: u64,
    ) -> Result<(), RunError<DemSamplingExecutionError, Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        let batch = match self.sample_window(
            sample_shots,
            window_start,
            window_shots,
            include_sampled_errors,
        ) {
            Ok(batch) => batch,
            Err(source) => {
                self.poisoned = true;
                return Err(RunError::Engine {
                    source,
                    progress: RunProgress::new(committed, window_shots as u64),
                });
            }
        };
        if let Err(source) = sink.write_batch(batch) {
            self.poisoned = true;
            return Err(RunError::Sink {
                phase: SinkFailurePhase::WriteBatch,
                source,
                progress: RunProgress::new(committed, window_shots as u64),
            });
        }
        Ok(())
    }

    fn sample_window(
        &self,
        sample_shots: usize,
        window_start: usize,
        window_shots: usize,
        include_sampled_errors: bool,
    ) -> Result<DemSampleBatchView<'_>, DemSamplingExecutionError> {
        let words_per_plane = sample_shots.div_ceil(u64::BITS as usize);
        let window = |planes, width| {
            plane_window(
                planes,
                width,
                words_per_plane,
                sample_shots,
                window_start,
                window_shots,
            )
        };
        let detection = DetectionBatchView::try_new(
            window(&self.detector_planes, self.plan.detector_count())?,
            window(&self.observable_planes, self.plan.observable_count())?,
        )
        .map_err(internal_format_error)?;
        let sampled_errors = if include_sampled_errors {
            let planes = self.error_planes.as_deref().ok_or_else(|| {
                DemSamplingExecutionError::InternalInvariant {
                    message: "DEM sample planes omitted requested sampled-error output".to_owned(),
                }
            })?;
            Some(window(planes, self.plan.error_count())?.into())
        } else {
            None
        };
        DemSampleBatchView::try_new(detection, sampled_errors).map_err(internal_format_error)
    }
}

fn plane_window(
    planes: &[u64],
    width: usize,
    words_per_plane: usize,
    sample_shots: usize,
    window_start: usize,
    window_shots: usize,
) -> Result<BitPlaneBatchView<'_>, DemSamplingExecutionError> {
    BitPlaneBatchView::try_from_words(planes, words_per_plane, sample_shots, width)
        .and_then(|planes| planes.shot_window(window_start, window_shots))
        .map_err(internal_format_error)
}
