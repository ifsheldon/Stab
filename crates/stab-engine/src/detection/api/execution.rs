use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use stab_model::{Cancellation, Cancelled};
use stab_records::DetectionSink;

use super::{
    DetectionExecutionError, DetectionRunError, DetectionRunProgress, DetectionRunStatus,
    DetectionRunSummary, DirectDetectionState, DirectDetectorFramePlan, MeasurementToDetectionPlan,
    detection_plane_view,
};
use crate::detection::conversion_window::WindowedConversion;
use crate::detection::frame::{
    DeliveryFailure, DeliveryOutcome, FrameBatchSource, FrameBatchState, FrameLanes, FrameWord,
    LaneFrameBatch, PendingLanes, Refill, sample_or_rewind,
};
use crate::sampling::session_cancellation;
use crate::work_poll::WorkPoll;
use crate::{LaneWidth, RandomPolicy, ShotCount, SinkFailurePhase};

pub(super) fn run_direct<Sink>(
    direct: &DirectDetectorFramePlan,
    state: &mut DirectDetectionState,
    shots: ShotCount,
    sink: &mut Sink,
) -> Result<DetectionRunSummary, DetectionRunError<Sink::Error>>
where
    Sink: DetectionSink,
{
    let DirectDetectionState {
        rng,
        frame,
        pending,
        cancellation,
    } = state;
    let cancellation = session_cancellation(cancellation);
    let delivered = match frame {
        LaneFrameBatch::Lanes64(frame) => deliver_lanes(
            pending,
            shots,
            DetectionBatchSource {
                direct,
                frame,
                rng,
                cancellation,
                sink: &mut *sink,
            },
        ),
        LaneFrameBatch::Lanes256(frame) => deliver_lanes(
            pending,
            shots,
            DetectionBatchSource {
                direct,
                frame,
                rng,
                cancellation,
                sink: &mut *sink,
            },
        ),
    };
    let outcome = delivered.map_err(
        |DeliveryFailure {
             error,
             committed,
             attempted,
         }| {
            let progress = DetectionRunProgress::new(committed, attempted);
            match error {
                DetectionDeliveryError::Engine(source) => {
                    DetectionRunError::Engine { source, progress }
                }
                DetectionDeliveryError::Sink(source) => DetectionRunError::Sink {
                    phase: SinkFailurePhase::WriteBatch,
                    source,
                    progress,
                },
            }
        },
    )?;
    if let Err(source) = sink.finish() {
        return Err(DetectionRunError::Sink {
            phase: SinkFailurePhase::Finish,
            source,
            progress: DetectionRunProgress::new(outcome.committed, 0),
        });
    }
    Ok(DetectionRunSummary {
        status: if outcome.completed {
            DetectionRunStatus::Completed
        } else {
            DetectionRunStatus::Cancelled
        },
        requested_shots: shots,
        committed_shots: ShotCount::new(outcome.committed),
        total_committed_shots: ShotCount::new(outcome.committed),
    })
}

enum DetectionDeliveryError<SinkError> {
    Engine(DetectionExecutionError),
    Sink(SinkError),
}

/// Runs the shared delivery loop over one session frame at a fixed lane word.
fn deliver_lanes<W, Sink>(
    pending: &mut PendingLanes,
    shots: ShotCount,
    mut source: DetectionBatchSource<'_, W, Sink>,
) -> Result<DeliveryOutcome, DeliveryFailure<DetectionDeliveryError<Sink::Error>>>
where
    W: FrameWord,
    Sink: DetectionSink,
{
    pending.deliver(shots.get(), &mut source)
}

/// One detection run's view of its session frame for the shared frame-batch delivery loop.
struct DetectionBatchSource<'run, W: FrameWord, Sink> {
    direct: &'run DirectDetectorFramePlan,
    frame: &'run mut FrameBatchState<W>,
    rng: &'run mut SmallRng,
    cancellation: &'run Cancellation,
    sink: &'run mut Sink,
}

impl<W, Sink> FrameBatchSource for DetectionBatchSource<'_, W, Sink>
where
    W: FrameWord,
    Sink: DetectionSink,
{
    type Error = DetectionDeliveryError<Sink::Error>;

    const BATCH_SHOTS: usize = W::LANE_COUNT;

    fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    fn refill(&mut self) -> Result<Refill, Self::Error> {
        let (direct, frame, cancellation) = (self.direct, &mut *self.frame, self.cancellation);
        let sampled = sample_or_rewind(self.rng, |rng| {
            direct.sample_batch(frame, rng, W::LANE_COUNT, cancellation)
        })
        .map_err(|source| DetectionDeliveryError::Engine(source.into()))?;
        Ok(match sampled {
            Some(()) => Refill::Sampled,
            None => Refill::Cancelled,
        })
    }

    fn deliver(&mut self, lanes: usize) -> Result<(), Self::Error> {
        let (detectors, observables) = self.frame.detection_planes();
        let view = detection_plane_view(detectors, observables, lanes)
            .map_err(DetectionDeliveryError::Engine)?;
        self.sink
            .write_batch(view)
            .map_err(DetectionDeliveryError::Sink)
    }

    fn discard(&mut self, lanes: usize) -> Refill {
        match self
            .frame
            .discard_delivered_lanes(lanes, &mut WorkPoll::new(self.cancellation))
        {
            Ok(()) => Refill::Sampled,
            Err(Cancelled) => Refill::Cancelled,
        }
    }
}

pub(super) fn validate_conversion_session_storage(
    plan: &MeasurementToDetectionPlan,
    memory_budget: stab_model::MemoryBudget,
) -> Result<(), DetectionExecutionError> {
    validate_session_storage(conversion_session_storage_bytes(plan), memory_budget)
}

/// Conservative reusable storage of a measurement-to-detection session: the storage of one
/// batch. Without sweep correction that is a windowed conversion's planes and records; with it,
/// one frame plane per measurement, sweep, detector, and observable bit, one transpose word per
/// input bit, and the sweep-correction frame state. The session reads the plan's packed
/// reference sample in place, so it holds no copy of it.
pub(super) fn conversion_session_storage_bytes(plan: &MeasurementToDetectionPlan) -> u128 {
    let converter = &plan.inner.converter;
    let measurements = plan.measurement_width().get() as u128;
    if !converter.has_sweep_correction() {
        return WindowedConversion::storage_bytes(&converter.plan);
    }
    let sweeps = plan.sweep_width().get() as u128;
    let detectors = plan.detector_width().get() as u128;
    let observables = plan.observable_width().get() as u128;
    let planes = measurements
        .saturating_add(sweeps)
        .saturating_add(detectors)
        .saturating_add(observables);
    planes
        .saturating_mul(size_of::<FrameLanes>() as u128)
        .saturating_add(
            measurements
                .max(sweeps)
                .saturating_mul(size_of::<u64>() as u128),
        )
        .saturating_add(plan.inner.converter.sweep_correction_storage_bytes())
}

pub(super) fn validate_direct_session_storage(
    plan: &DirectDetectorFramePlan,
    lane_width: LaneWidth,
    memory_budget: stab_model::MemoryBudget,
) -> Result<(), DetectionExecutionError> {
    validate_session_storage(
        direct_session_storage_bytes(plan, lane_width),
        memory_budget,
    )
}

/// Reusable storage of a detection-sampling session at `lane_width`: its frame batch state,
/// which holds the delivered detector and observable planes as well.
pub(super) fn direct_session_storage_bytes(
    plan: &DirectDetectorFramePlan,
    lane_width: LaneWidth,
) -> u128 {
    plan.state_storage_bytes(lane_width)
}

pub(super) fn validate_session_storage(
    estimated_bytes: u128,
    budget: stab_model::MemoryBudget,
) -> Result<(), DetectionExecutionError> {
    budget
        .admit(estimated_bytes)
        .map_err(|exceeded| DetectionExecutionError::SessionStorageLimit { exceeded })
}

pub(super) fn detection_rng(policy: RandomPolicy) -> SmallRng {
    SmallRng::seed_from_u64(policy.seed().map_or_else(rand::random, |seed| seed.get()))
}

pub(super) fn invariant_error(error: stab_records::FormatError) -> DetectionExecutionError {
    DetectionExecutionError::InternalInvariant {
        message: error.to_string(),
    }
}
