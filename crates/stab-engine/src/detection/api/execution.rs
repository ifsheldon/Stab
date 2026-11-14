use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use stab_model::Cancellation;
use stab_records::DetectionSink;

use super::{
    DetectionExecutionError, DetectionRunError, DetectionRunProgress, DetectionRunStatus,
    DetectionRunSummary, DirectDetectionState, DirectDetectorFramePlan, MeasurementToDetectionPlan,
    detection_plane_view,
};
use crate::detection::frame::{
    DeliveryFailure, FRAME_BATCH_SHOTS, FrameBatchSource, FrameBatchState, FrameLanes,
};
use crate::sampling::session_cancellation;
use crate::{RandomPolicy, ShotCount, SinkFailurePhase};

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
    let mut source = DetectionBatchSource {
        direct,
        frame,
        rng,
        cancellation: session_cancellation(cancellation),
        sink,
    };
    let outcome = pending.deliver(shots.get(), &mut source).map_err(
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
    if let Err(source) = source.sink.finish() {
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

/// One detection run's view of its session frame for the shared frame-batch delivery loop.
struct DetectionBatchSource<'run, Sink> {
    direct: &'run DirectDetectorFramePlan,
    frame: &'run mut FrameBatchState,
    rng: &'run mut SmallRng,
    cancellation: &'run Cancellation,
    sink: &'run mut Sink,
}

impl<Sink> FrameBatchSource for DetectionBatchSource<'_, Sink>
where
    Sink: DetectionSink,
{
    type Error = DetectionDeliveryError<Sink::Error>;

    fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    fn refill(&mut self) -> Result<(), Self::Error> {
        self.direct
            .sample_batch(self.frame, self.rng, FRAME_BATCH_SHOTS, self.cancellation)
            .map_err(|source| DetectionDeliveryError::Engine(source.into()))
    }

    fn deliver(&mut self, lanes: usize) -> Result<(), Self::Error> {
        let (detectors, observables) = self.frame.detection_planes();
        let view = detection_plane_view(detectors, observables, lanes)
            .map_err(DetectionDeliveryError::Engine)?;
        self.sink
            .write_batch(view)
            .map_err(DetectionDeliveryError::Sink)
    }

    fn discard(&mut self, lanes: usize) {
        self.frame.discard_delivered_lanes(lanes);
    }
}

pub(super) fn validate_conversion_session_storage(
    plan: &MeasurementToDetectionPlan,
) -> Result<(), DetectionExecutionError> {
    validate_session_storage(
        conversion_session_storage_bytes(plan),
        plan.inner.converter.plan.limits.memory_budget(),
    )
}

/// Conservative reusable storage of a measurement-to-detection session: the reference sample
/// at one byte per measurement, one frame plane per measurement, sweep, detector, and observable
/// bit, one transpose word per input bit, and the sweep-correction frame state.
pub(super) fn conversion_session_storage_bytes(plan: &MeasurementToDetectionPlan) -> u128 {
    let measurements = plan.measurement_width().get() as u128;
    let sweeps = plan.sweep_width().get() as u128;
    let detectors = plan.detector_width().get() as u128;
    let observables = plan.observable_width().get() as u128;
    let planes = measurements
        .saturating_add(sweeps)
        .saturating_add(detectors)
        .saturating_add(observables);
    measurements
        .saturating_add(planes.saturating_mul(size_of::<FrameLanes>() as u128))
        .saturating_add(
            measurements
                .max(sweeps)
                .saturating_mul(size_of::<u64>() as u128),
        )
        .saturating_add(plan.inner.converter.sweep_correction_storage_bytes())
}

pub(super) fn validate_direct_session_storage(
    plan: &DirectDetectorFramePlan,
) -> Result<(), DetectionExecutionError> {
    validate_session_storage(direct_session_storage_bytes(plan), plan.memory_budget())
}

/// Reusable storage of a detection-sampling session: its frame batch state, which holds the
/// delivered detector and observable planes as well.
pub(super) fn direct_session_storage_bytes(plan: &DirectDetectorFramePlan) -> u128 {
    plan.state_storage_bytes()
}

fn validate_session_storage(
    estimated_bytes: u128,
    budget: stab_model::MemoryBudget,
) -> Result<(), DetectionExecutionError> {
    if !budget.admits(estimated_bytes) {
        return Err(DetectionExecutionError::SessionStorageLimit {
            estimated_bytes,
            limit_bytes: budget.bytes(),
        });
    }
    Ok(())
}

pub(super) fn detection_rng(policy: RandomPolicy) -> SmallRng {
    SmallRng::seed_from_u64(policy.seed().map_or_else(rand::random, |seed| seed.get()))
}

pub(super) fn invariant_error(error: stab_records::FormatError) -> DetectionExecutionError {
    DetectionExecutionError::InternalInvariant {
        message: error.to_string(),
    }
}
