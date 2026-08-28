use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use stab_records::{DetectionSink, MeasurementBatchView};

use super::{
    DetectionExecutionError, DetectionRunError, DetectionRunProgress, DetectionRunStatus,
    DetectionRunSummary, DetectionSamplingPlan, DetectionSamplingPlanKind, DirectDetectionState,
    DirectDetectorFramePlan, MAX_BATCH_SHOTS, MAX_DETECTION_SESSION_STORAGE_BYTES,
    MeasurementToDetectionPlan, MeasurementToDetectionSession,
};
use crate::{
    RandomPolicy, RunError, SamplingCancellation, SamplingRunStatus, SamplingSession, ShotCount,
    SinkFailurePhase,
};

pub(super) fn run_direct<Sink>(
    plan: &DetectionSamplingPlan,
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
        batch,
        delivery,
        pending_start,
        pending_count,
        cancellation,
    } = state;
    let DetectionSamplingPlanKind::DirectDetectorFrame(direct) = &plan.inner.kind else {
        return Err(DetectionRunError::Engine {
            source: DetectionExecutionError::InternalInvariant {
                message: "direct detection session did not own a direct plan".to_owned(),
            },
            progress: DetectionRunProgress::new(0, 0),
        });
    };
    let mut remaining = shots.get();
    let mut committed = 0_u64;
    while remaining > 0 {
        if cancellation
            .get()
            .is_some_and(SamplingCancellation::is_cancelled)
        {
            break;
        }
        if *pending_count == 0 {
            direct
                .sample_batch(frame, rng, MAX_BATCH_SHOTS)
                .map_err(|source| DetectionRunError::Engine {
                    source: DetectionExecutionError::Conversion(source),
                    progress: DetectionRunProgress::new(committed, MAX_BATCH_SHOTS as u64),
                })?;
            let (detector_planes, observable_planes) = direct.output_planes(frame);
            batch
                .copy_from_planes(detector_planes, observable_planes)
                .map_err(|source| DetectionRunError::Engine {
                    source,
                    progress: DetectionRunProgress::new(committed, MAX_BATCH_SHOTS as u64),
                })?;
            *pending_start = 0;
            *pending_count = MAX_BATCH_SHOTS;
        }
        let batch_shots_u64 = remaining.min(*pending_count as u64);
        let batch_shots =
            usize::try_from(batch_shots_u64).map_err(|_| DetectionRunError::Engine {
                source: DetectionExecutionError::InternalInvariant {
                    message: "bounded direct-detection batch did not fit usize".to_owned(),
                },
                progress: DetectionRunProgress::new(committed, batch_shots_u64),
            })?;
        if *pending_start != 0 {
            delivery
                .copy_range_from(batch, *pending_start, batch_shots)
                .map_err(|source| DetectionRunError::Engine {
                    source,
                    progress: DetectionRunProgress::new(committed, batch_shots_u64),
                })?;
        }
        let view = if *pending_start == 0 {
            batch.view(batch_shots)
        } else {
            delivery.view(batch_shots)
        }
        .map_err(|source| DetectionRunError::Engine {
            source,
            progress: DetectionRunProgress::new(committed, batch_shots_u64),
        })?;
        if let Err(source) = sink.write_batch(view) {
            return Err(DetectionRunError::Sink {
                phase: SinkFailurePhase::WriteBatch,
                source,
                progress: DetectionRunProgress::new(committed, batch_shots_u64),
            });
        }
        committed += batch_shots_u64;
        remaining -= batch_shots_u64;
        *pending_start += batch_shots;
        *pending_count -= batch_shots;
        if *pending_count == 0 {
            *pending_start = 0;
        }
    }
    if let Err(source) = sink.finish() {
        return Err(DetectionRunError::Sink {
            phase: SinkFailurePhase::Finish,
            source,
            progress: DetectionRunProgress::new(committed, 0),
        });
    }
    Ok(DetectionRunSummary {
        status: if remaining == 0 {
            DetectionRunStatus::Completed
        } else {
            DetectionRunStatus::Cancelled
        },
        requested_shots: shots,
        committed_shots: ShotCount::new(committed),
        total_committed_shots: ShotCount::new(committed),
    })
}

pub(super) fn run_fused<Sink>(
    sampling: &mut SamplingSession,
    conversion: &mut MeasurementToDetectionSession,
    shots: ShotCount,
    sink: &mut Sink,
) -> Result<DetectionRunSummary, DetectionRunError<Sink::Error>>
where
    Sink: DetectionSink,
{
    let mut transaction =
        conversion
            .start_transaction(sink)
            .map_err(|source| DetectionRunError::Engine {
                source,
                progress: DetectionRunProgress::new(0, 0),
            })?;
    match sampling.run(shots, &mut transaction) {
        Ok(summary) => Ok(DetectionRunSummary {
            status: match summary.status() {
                SamplingRunStatus::Completed => DetectionRunStatus::Completed,
                SamplingRunStatus::Cancelled => DetectionRunStatus::Cancelled,
            },
            requested_shots: shots,
            committed_shots: summary.committed_shots(),
            total_committed_shots: summary.total_committed_shots(),
        }),
        Err(RunError::Engine { source, progress }) => Err(DetectionRunError::Engine {
            source: DetectionExecutionError::Sampling(source),
            progress: DetectionRunProgress::new(
                progress.committed_shots().get(),
                progress.attempted_batch_shots().get(),
            ),
        }),
        Err(RunError::Sink {
            source, progress, ..
        }) => flatten_transaction_error(source, progress),
    }
}

fn flatten_transaction_error<SinkError>(
    error: DetectionRunError<SinkError>,
    sampling_progress: crate::SamplingRunProgress,
) -> Result<DetectionRunSummary, DetectionRunError<SinkError>> {
    let progress = DetectionRunProgress::new(
        sampling_progress.committed_shots().get(),
        sampling_progress.attempted_batch_shots().get(),
    );
    Err(match error {
        DetectionRunError::Engine { source, .. } => DetectionRunError::Engine { source, progress },
        DetectionRunError::Sink { phase, source, .. } => DetectionRunError::Sink {
            phase,
            source,
            progress,
        },
    })
}

pub(super) fn copy_record(
    batch: MeasurementBatchView<'_>,
    shot_index: usize,
    output: &mut [bool],
    kind: &'static str,
) -> Result<(), DetectionExecutionError> {
    for (bit_index, slot) in output.iter_mut().enumerate() {
        *slot = batch.get(shot_index, bit_index).ok_or_else(|| {
            DetectionExecutionError::InternalInvariant {
                message: format!(
                    "{kind} batch escaped its declared dimensions at shot {shot_index}, bit {bit_index}"
                ),
            }
        })?;
    }
    Ok(())
}

pub(super) fn validate_conversion_session_storage(
    plan: &MeasurementToDetectionPlan,
) -> Result<(), DetectionExecutionError> {
    validate_session_storage(conversion_session_storage_bytes(plan))
}

pub(super) fn conversion_session_storage_bytes(plan: &MeasurementToDetectionPlan) -> u128 {
    let measurements = plan.measurement_width().get() as u128;
    let sweeps = plan.sweep_width().get() as u128;
    let detectors = plan.detector_width().get() as u128;
    let observables = plan.observable_width().get() as u128;
    let records = detectors.saturating_add(observables);
    let packed_batch_bytes = records
        .saturating_mul(MAX_BATCH_SHOTS as u128)
        .saturating_add(7)
        / 8;
    measurements
        .saturating_mul(2)
        .saturating_add(sweeps)
        .saturating_add(records)
        .saturating_add(packed_batch_bytes)
        .saturating_add(plan.inner.converter.sweep_correction_storage_bytes())
}

pub(super) fn construct_fused_state_after_admission<T>(
    sampling_bytes: u128,
    conversion_bytes: u128,
    construct: impl FnOnce() -> Result<T, DetectionExecutionError>,
) -> Result<T, DetectionExecutionError> {
    validate_combined_session_storage(sampling_bytes, conversion_bytes)?;
    construct()
}

pub(super) fn validate_combined_session_storage(
    first_component_bytes: u128,
    second_component_bytes: u128,
) -> Result<(), DetectionExecutionError> {
    validate_session_storage(first_component_bytes.saturating_add(second_component_bytes))
}

pub(super) fn validate_direct_session_storage(
    plan: &DirectDetectorFramePlan,
) -> Result<(), DetectionExecutionError> {
    let qubits = plan.qubit_count() as u128;
    let measurements = plan.measurement_count() as u128;
    let detectors = plan.detector_count() as u128;
    let observables = plan.observable_count() as u128;
    let records = detectors.saturating_add(observables);
    let packed_batch_bytes = records
        .saturating_mul(MAX_BATCH_SHOTS as u128)
        .saturating_add(7)
        / 8;
    validate_session_storage(
        qubits
            .saturating_mul(2)
            .saturating_add(measurements.saturating_mul(2))
            .saturating_add(detectors)
            .saturating_add(observables.saturating_mul(2))
            .saturating_add(packed_batch_bytes),
    )
}

fn validate_session_storage(estimated_bytes: u128) -> Result<(), DetectionExecutionError> {
    if estimated_bytes > u128::from(MAX_DETECTION_SESSION_STORAGE_BYTES) {
        return Err(DetectionExecutionError::SessionStorageLimit {
            estimated_bytes,
            limit_bytes: MAX_DETECTION_SESSION_STORAGE_BYTES,
        });
    }
    Ok(())
}

pub(super) fn detection_rng(policy: RandomPolicy) -> SmallRng {
    SmallRng::seed_from_u64(policy.seed().map_or_else(rand::random, |seed| seed.get()))
}

pub(super) fn storage_error(error: stab_records::FormatError) -> DetectionExecutionError {
    DetectionExecutionError::SessionStorageAllocation {
        message: error.to_string(),
    }
}

pub(super) fn invariant_error(error: stab_records::FormatError) -> DetectionExecutionError {
    DetectionExecutionError::InternalInvariant {
        message: error.to_string(),
    }
}
