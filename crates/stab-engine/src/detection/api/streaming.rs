//! One streaming detection-sampling run; see [`DetectionSamplingPlan::streaming_run`].

use std::fmt;
use std::sync::OnceLock;

use rand::rngs::SmallRng;
use stab_model::Cancellation;
use stab_records::{DetectionBatchView, DetectionSink};

use super::execution::{detection_rng, invariant_error, validate_session_storage};
use super::{
    DetectionExecutionError, DetectionRunError, DetectionRunProgress, DetectionRunStatus,
    DetectionRunSummary, DetectionSamplingPlan,
};
use crate::detection::frame::{FrameLanes, FrameWord, sample_or_rewind};
use crate::detection::streaming::{StreamingDetectionFrame, StreamingDetectionShape};
use crate::sampling::{session_cancellation, session_cancellation_slot};
use crate::{LaneWidth, RandomPolicy, ShotCount, SinkFailurePhase};

#[derive(Debug)]
enum StreamingFrame {
    Lanes64(StreamingDetectionFrame<u64>),
    Lanes256(StreamingDetectionFrame<FrameLanes>),
}

/// One prepared streaming detection-sampling run.
///
/// Its output equals the first [`super::DetectionSamplingSession::run`] of the same shots on a
/// session created with the same seed and lane width. It converts measurement planes to
/// detector planes window by window and keeps shot-major records of only the lanes it delivers,
/// so it retains neither one plane per measurement nor one per detector; because it keeps no
/// undelivered lanes, it runs once.
pub struct StreamingDetectionRun {
    plan: DetectionSamplingPlan,
    rng: SmallRng,
    frame: StreamingFrame,
    cancellation: OnceLock<Cancellation>,
    shots: ShotCount,
}

impl fmt::Debug for StreamingDetectionRun {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamingDetectionRun")
            .field("plan", &self.plan)
            .field("lane_width", &self.lane_width())
            .field("shots", &self.shots)
            .finish_non_exhaustive()
    }
}

impl DetectionSamplingPlan {
    /// Prepares one run of exactly `shots` shots that streams wide records.
    ///
    /// Storage is admitted for bounded windows of measurement and detector planes and for
    /// `min(shots, lane_width)` shot-major detector and observable records, instead of one plane
    /// per measurement and detector at `lane_width`.
    pub fn streaming_run(
        &self,
        random_policy: RandomPolicy,
        lane_width: LaneWidth,
        shots: ShotCount,
    ) -> Result<StreamingDetectionRun, DetectionExecutionError> {
        let direct = &self.inner.direct;
        let lanes = usize::try_from(shots.get()).map_or(lane_width.shots_per_batch(), |shots| {
            shots.min(lane_width.shots_per_batch())
        });
        let shape = StreamingDetectionShape::new(direct, lane_width, lanes);
        validate_session_storage(
            shape.storage_bytes(direct, lane_width),
            direct.memory_budget(),
        )?;
        let frame = match lane_width {
            LaneWidth::Lanes64 => {
                StreamingFrame::Lanes64(StreamingDetectionFrame::try_new(direct, shape)?)
            }
            LaneWidth::Lanes256 => {
                StreamingFrame::Lanes256(StreamingDetectionFrame::try_new(direct, shape)?)
            }
        };
        Ok(StreamingDetectionRun {
            plan: self.clone(),
            rng: detection_rng(random_policy),
            frame,
            cancellation: session_cancellation_slot(direct.cancellation()),
            shots,
        })
    }
}

impl StreamingDetectionRun {
    /// The signal the run polls between and inside batches, as for
    /// [`super::DetectionSamplingSession::cancellation`].
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.get_or_init(Cancellation::new).clone()
    }

    /// Shots each frame batch of the run samples together.
    pub const fn lane_width(&self) -> LaneWidth {
        match self.frame {
            StreamingFrame::Lanes64(_) => LaneWidth::Lanes64,
            StreamingFrame::Lanes256(_) => LaneWidth::Lanes256,
        }
    }

    /// Shots the run delivers.
    pub const fn shots(&self) -> ShotCount {
        self.shots
    }

    /// Executes the run with the sink lifecycle of [`super::DetectionSamplingSession::run`].
    ///
    /// Cooperative cancellation, observed before or inside a batch, ends the run with
    /// [`DetectionRunStatus::Cancelled`], the complete batches delivered, and the sink finalized.
    /// The run consumed itself, so nothing can resume it; a fresh run restarts the seeded stream.
    pub fn run<Sink>(
        mut self,
        sink: &mut Sink,
    ) -> Result<DetectionRunSummary, DetectionRunError<Sink::Error>>
    where
        Sink: DetectionSink,
    {
        let shots = self.shots;
        if shots.get() == 0 {
            return Ok(summary(DetectionRunStatus::Completed, shots, 0));
        }
        let cancellation = session_cancellation(&self.cancellation);
        let delivered = match &mut self.frame {
            StreamingFrame::Lanes64(frame) => {
                deliver(&self.plan, frame, &mut self.rng, cancellation, shots, sink)
            }
            StreamingFrame::Lanes256(frame) => {
                deliver(&self.plan, frame, &mut self.rng, cancellation, shots, sink)
            }
        };
        let (committed, completed) = delivered?;
        if let Err(source) = sink.finish() {
            return Err(DetectionRunError::Sink {
                phase: SinkFailurePhase::Finish,
                source,
                progress: DetectionRunProgress::new(committed, 0),
            });
        }
        let status = if completed {
            DetectionRunStatus::Completed
        } else {
            DetectionRunStatus::Cancelled
        };
        Ok(summary(status, shots, committed))
    }
}

const fn summary(
    status: DetectionRunStatus,
    requested_shots: ShotCount,
    committed: u64,
) -> DetectionRunSummary {
    DetectionRunSummary {
        status,
        requested_shots,
        committed_shots: ShotCount::new(committed),
        total_committed_shots: ShotCount::new(committed),
    }
}

/// Delivers `shots` shots in fresh batches; returns the committed shots and whether all were.
fn deliver<W, Sink>(
    plan: &DetectionSamplingPlan,
    frame: &mut StreamingDetectionFrame<W>,
    rng: &mut SmallRng,
    cancellation: &Cancellation,
    shots: ShotCount,
    sink: &mut Sink,
) -> Result<(u64, bool), DetectionRunError<Sink::Error>>
where
    W: FrameWord,
    Sink: DetectionSink,
{
    let mut remaining = shots.get();
    let mut committed = 0_u64;
    while remaining > 0 {
        if cancellation.is_cancelled() {
            break;
        }
        let lanes = usize::try_from(remaining)
            .map_or(W::LANE_COUNT, |remaining| remaining.min(W::LANE_COUNT));
        let progress = DetectionRunProgress::new(committed, lanes as u64);
        let engine =
            |source: DetectionExecutionError| DetectionRunError::Engine { source, progress };
        let sampled = sample_or_rewind(rng, |rng| {
            frame.sample_batch(&plan.inner.direct, lanes, rng, cancellation)
        })
        .map_err(|error| engine(error.into()))?;
        if sampled.is_none() {
            break;
        }
        let detectors = frame
            .detectors()
            .view()
            .map_err(|error| engine(error.into()))?;
        let observables = frame
            .observables()
            .view()
            .map_err(|error| engine(error.into()))?;
        let view = DetectionBatchView::try_new(detectors, observables)
            .map_err(|error| engine(invariant_error(error)))?;
        sink.write_batch(view)
            .map_err(|source| DetectionRunError::Sink {
                phase: SinkFailurePhase::WriteBatch,
                source,
                progress,
            })?;
        committed += lanes as u64;
        remaining -= lanes as u64;
    }
    Ok((committed, remaining == 0))
}
