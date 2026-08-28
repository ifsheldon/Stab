use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use stab_records::{DemSampleSink, FormatError as RecordsFormatError};
use thiserror::Error;

use super::buffers::try_zero_words;
use super::plan::DemSamplingPlan;
use super::{DemError, DemSamplerLimits};
use crate::{DetectionRecordBuffer, RandomPolicy, ShotCount, SinkFailurePhase};

const MAX_BATCH_SHOTS: usize = 64;
const MAX_DEM_SESSION_STORAGE_BYTES: u64 = 256 * 1024 * 1024;

mod batch;

use batch::{SessionBatch, initial_capacities, validate_session_storage};

/// Cooperative cancellation state checked between bounded DEM sampling batches.
#[derive(Clone, Debug, Default)]
pub struct DemSamplingCancellation {
    cancelled: Arc<AtomicBool>,
}

impl DemSamplingCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::Release);
    }

    #[inline(always)]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Engine-side DEM sampling execution failure.
#[derive(Debug, Error)]
pub enum DemSamplingExecutionError {
    #[error("DEM sampling session is poisoned")]
    SessionPoisoned,

    #[error("DEM sampling session shot counter overflowed")]
    ShotCounterOverflow,

    #[error("DEM replay session lifecycle error: {message}")]
    ReplayLifecycle { message: &'static str },

    #[error(
        "DEM sampling session needs an estimated {estimated_bytes} bytes of bounded storage, exceeding the {limit_bytes}-byte safety limit"
    )]
    SessionStorageLimit {
        estimated_bytes: u128,
        limit_bytes: u64,
    },

    #[error("DEM sampling session could not allocate bounded storage: {message}")]
    SessionStorageAllocation { message: String },

    #[error(transparent)]
    InvalidRequest(#[from] DemError),

    #[error("DEM sampling execution violated an internal batch invariant: {message}")]
    InternalInvariant { message: String },
}

impl DemSamplingExecutionError {
    pub fn into_dem_error(self) -> DemError {
        match self {
            Self::InvalidRequest(error) => error,
            other => DemError::invalid_sampler_compilation(other.to_string()),
        }
    }
}

/// Exact progress at a DEM sampling execution failure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DemSamplingRunProgress {
    committed_shots: ShotCount,
    attempted_batch_shots: ShotCount,
}

impl DemSamplingRunProgress {
    const fn new(committed_shots: u64, attempted_batch_shots: u64) -> Self {
        Self {
            committed_shots: ShotCount::new(committed_shots),
            attempted_batch_shots: ShotCount::new(attempted_batch_shots),
        }
    }

    pub const fn committed_shots(self) -> ShotCount {
        self.committed_shots
    }

    pub const fn attempted_batch_shots(self) -> ShotCount {
        self.attempted_batch_shots
    }
}

/// Non-lossy composition of DEM engine and sink failures.
#[derive(Debug)]
pub enum DemSamplingRunError<SinkError> {
    Engine {
        source: DemSamplingExecutionError,
        progress: DemSamplingRunProgress,
    },
    Sink {
        phase: SinkFailurePhase,
        source: SinkError,
        progress: DemSamplingRunProgress,
    },
}

impl<SinkError> DemSamplingRunError<SinkError> {
    pub const fn progress(&self) -> DemSamplingRunProgress {
        match self {
            Self::Engine { progress, .. } | Self::Sink { progress, .. } => *progress,
        }
    }
}

impl<SinkError: fmt::Display> fmt::Display for DemSamplingRunError<SinkError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine { source, progress } => write!(
                formatter,
                "{source} after {} committed shots",
                progress.committed_shots().get()
            ),
            Self::Sink {
                phase,
                source,
                progress,
            } => write!(
                formatter,
                "DEM sampling sink {} failed after {} committed shots while attempting {} shots: {source}",
                phase.as_str(),
                progress.committed_shots().get(),
                progress.attempted_batch_shots().get()
            ),
        }
    }
}

impl<SinkError> std::error::Error for DemSamplingRunError<SinkError>
where
    SinkError: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine { source, .. } => Some(source),
            Self::Sink { source, .. } => Some(source),
        }
    }
}

/// Completion state of one DEM sampling call.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DemSamplingRunStatus {
    Completed,
    Cancelled,
}

impl DemSamplingRunStatus {
    pub const fn is_completed(self) -> bool {
        matches!(self, Self::Completed)
    }

    pub const fn is_cancelled(self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

/// Summary of one completed or cooperatively cancelled DEM sampling call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DemSamplingRunSummary {
    status: DemSamplingRunStatus,
    requested_shots: ShotCount,
    committed_shots: ShotCount,
    total_committed_shots: ShotCount,
}

impl DemSamplingRunSummary {
    pub const fn status(self) -> DemSamplingRunStatus {
        self.status
    }

    pub const fn requested_shots(self) -> ShotCount {
        self.requested_shots
    }

    pub const fn committed_shots(self) -> ShotCount {
        self.committed_shots
    }

    pub const fn total_committed_shots(self) -> ShotCount {
        self.total_committed_shots
    }
}

/// Mutable reusable state for one immutable DEM sampling plan.
pub struct DemSamplingSession {
    plan: DemSamplingPlan,
    limits: DemSamplerLimits,
    sampling_seed: u64,
    record: DetectionRecordBuffer,
    detector_planes: Vec<u64>,
    observable_planes: Vec<u64>,
    error_planes: Option<Vec<u64>>,
    batch: SessionBatch,
    sample_capacity: usize,
    cancellation: OnceLock<Arc<AtomicBool>>,
    total_committed_shots: u64,
    poisoned: bool,
    not_sync: PhantomData<Cell<()>>,
}

impl fmt::Debug for DemSamplingSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DemSamplingSession")
            .field("plan", &self.plan)
            .field(
                "cancelled",
                &self
                    .cancellation
                    .get()
                    .is_some_and(|cancelled| cancelled.load(Ordering::Acquire)),
            )
            .field("total_committed_shots", &self.total_committed_shots)
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}

impl DemSamplingSession {
    pub(super) fn new(
        plan: DemSamplingPlan,
        random_policy: RandomPolicy,
        limits: DemSamplerLimits,
    ) -> Result<Self, DemSamplingExecutionError> {
        let capacities = initial_capacities(&plan, limits)?;
        let record = plan.try_reusable_detection_record().map_err(|source| {
            DemSamplingExecutionError::SessionStorageAllocation {
                message: source.to_string(),
            }
        })?;
        let batch = SessionBatch::try_new(&plan, capacities.output)?;
        let detector_planes = try_zero_words(
            sample_plane_len(plan.detector_count(), capacities.sample)?,
            "DEM detector planes",
        )
        .map_err(storage_dem_error)?;
        let observable_planes = try_zero_words(
            sample_plane_len(plan.observable_count(), capacities.sample)?,
            "DEM observable planes",
        )
        .map_err(storage_dem_error)?;
        let sampling_seed = random_policy
            .seed()
            .map_or_else(rand::random, |seed| seed.get());
        Ok(Self {
            plan,
            limits,
            sampling_seed,
            record,
            detector_planes,
            observable_planes,
            error_planes: None,
            batch,
            sample_capacity: capacities.sample,
            cancellation: OnceLock::new(),
            total_committed_shots: 0,
            poisoned: false,
            not_sync: PhantomData,
        })
    }

    pub fn cancellation(&self) -> DemSamplingCancellation {
        DemSamplingCancellation {
            cancelled: Arc::clone(
                self.cancellation
                    .get_or_init(|| Arc::new(AtomicBool::new(false))),
            ),
        }
    }

    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub const fn total_committed_shots(&self) -> ShotCount {
        ShotCount::new(self.total_committed_shots)
    }

    /// Samples detector and observable records without materializing sampled-error records.
    pub fn run<Sink>(
        &mut self,
        shots: ShotCount,
        sink: &mut Sink,
    ) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.run_mode(shots, sink, RunMode::DetectorOnly)
    }

    /// Samples detector, observable, and sampled-error records.
    pub fn run_with_sampled_errors<Sink>(
        &mut self,
        shots: ShotCount,
        sink: &mut Sink,
    ) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.run_mode(shots, sink, RunMode::WithSampledErrors)
    }

    fn run_mode<Sink>(
        &mut self,
        shots: ShotCount,
        sink: &mut Sink,
        mode: RunMode,
    ) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.preflight_common(shots)
            .map_err(preflight_execution_error)?;
        if shots.get() == 0 {
            return Ok(self.summary(DemSamplingRunStatus::Completed, shots, 0));
        }
        let shots_usize =
            usize::try_from(shots.get()).map_err(|_| DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::ShotCounterOverflow,
                progress: DemSamplingRunProgress::new(0, 0),
            })?;
        match mode {
            RunMode::DetectorOnly => self
                .plan
                .validate_detector_sample_work_units_with_limits(shots_usize, self.limits)
                .map_err(preflight_error)?,
            RunMode::WithSampledErrors => {
                self.plan
                    .validate_sampled_error_work_units_with_limits(shots_usize, self.limits)
                    .map_err(preflight_error)?;
                self.ensure_sampled_error_storage()
                    .map_err(preflight_execution_error)?;
            }
        }

        let mut remaining = shots.get();
        let mut committed = 0_u64;
        'sampling: while remaining > 0 {
            if self.is_cancelled() {
                break;
            }
            let sample_shots_u64 = remaining.min(self.sample_capacity as u64);
            let sample_shots =
                usize::try_from(sample_shots_u64).map_err(|_| DemSamplingRunError::Engine {
                    source: DemSamplingExecutionError::InternalInvariant {
                        message: "bounded DEM sample window did not fit usize".to_owned(),
                    },
                    progress: DemSamplingRunProgress::new(committed, sample_shots_u64),
                })?;
            if let Err(source) = self.fill_sample_planes(sample_shots, mode) {
                self.poisoned = true;
                return Err(DemSamplingRunError::Engine {
                    source,
                    progress: DemSamplingRunProgress::new(committed, sample_shots_u64),
                });
            }
            let mut sample_offset = 0_usize;
            while sample_offset < sample_shots {
                if self.is_cancelled() {
                    break 'sampling;
                }
                let batch_shots = self
                    .batch
                    .capacity()
                    .min(sample_shots.saturating_sub(sample_offset));
                let plane_word_index = sample_offset / u64::BITS as usize;
                if let Err(source) = self.copy_sample_chunk(
                    plane_word_index,
                    batch_shots,
                    mode.includes_sampled_errors(),
                ) {
                    self.poisoned = true;
                    return Err(DemSamplingRunError::Engine {
                        source,
                        progress: DemSamplingRunProgress::new(committed, batch_shots as u64),
                    });
                }
                self.write_active_batch(
                    batch_shots,
                    mode.includes_sampled_errors(),
                    sink,
                    committed,
                )?;
                let batch_shots_u64 = batch_shots as u64;
                sample_offset += batch_shots;
                committed += batch_shots_u64;
                self.total_committed_shots += batch_shots_u64;
                remaining -= batch_shots_u64;
            }
        }
        self.finish_run(sink, shots, committed, remaining == 0)
    }

    fn preflight_common(&self, shots: ShotCount) -> Result<(), DemSamplingExecutionError> {
        if self.poisoned {
            return Err(DemSamplingExecutionError::SessionPoisoned);
        }
        if self
            .total_committed_shots
            .checked_add(shots.get())
            .is_none()
        {
            return Err(DemSamplingExecutionError::ShotCounterOverflow);
        }
        Ok(())
    }

    fn fill_sample_planes(
        &mut self,
        shot_count: usize,
        mode: RunMode,
    ) -> Result<(), DemSamplingExecutionError> {
        match mode {
            RunMode::DetectorOnly => self
                .plan
                .sample_detection_planes_into(
                    self.sampling_seed,
                    self.total_committed_shots,
                    shot_count,
                    &mut self.detector_planes,
                    &mut self.observable_planes,
                )
                .map_err(DemSamplingExecutionError::InvalidRequest)?,
            RunMode::WithSampledErrors => {
                let error_planes = self.error_planes.as_mut().ok_or_else(|| {
                    DemSamplingExecutionError::InternalInvariant {
                        message: "sampled-error run omitted its reusable error planes".to_owned(),
                    }
                })?;
                self.plan
                    .sample_detection_and_error_planes_into(
                        self.sampling_seed,
                        self.total_committed_shots,
                        shot_count,
                        &mut self.detector_planes,
                        &mut self.observable_planes,
                        error_planes,
                    )
                    .map_err(DemSamplingExecutionError::InvalidRequest)?;
            }
        }
        Ok(())
    }

    fn copy_sample_chunk(
        &mut self,
        plane_word_index: usize,
        shot_count: usize,
        include_sampled_errors: bool,
    ) -> Result<(), DemSamplingExecutionError> {
        self.batch.copy_from_plane_chunk(
            &self.detector_planes,
            &self.observable_planes,
            self.error_planes.as_deref(),
            plane_word_index,
            shot_count,
            include_sampled_errors,
        )
    }

    fn fill_replay_batch(
        &mut self,
        error_records: &[Vec<bool>],
    ) -> Result<(), DemSamplingExecutionError> {
        for (shot_index, error_record) in error_records.iter().enumerate() {
            self.plan
                .detection_record_from_error_record_into(error_record, &mut self.record)
                .map_err(DemSamplingExecutionError::InvalidRequest)?;
            self.batch
                .copy_replay_record(shot_index, &self.record, error_record)?;
        }
        Ok(())
    }

    fn write_active_batch<Sink>(
        &mut self,
        shot_count: usize,
        include_sampled_errors: bool,
        sink: &mut Sink,
        committed: u64,
    ) -> Result<(), DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        let batch = self
            .batch
            .view(shot_count, include_sampled_errors)
            .map_err(|source| {
                self.poisoned = true;
                DemSamplingRunError::Engine {
                    source,
                    progress: DemSamplingRunProgress::new(committed, shot_count as u64),
                }
            })?;
        if let Err(source) = sink.write_batch(batch) {
            self.poisoned = true;
            return Err(DemSamplingRunError::Sink {
                phase: SinkFailurePhase::WriteBatch,
                source,
                progress: DemSamplingRunProgress::new(committed, shot_count as u64),
            });
        }
        Ok(())
    }

    fn finish_run<Sink>(
        &mut self,
        sink: &mut Sink,
        requested_shots: ShotCount,
        committed: u64,
        completed: bool,
    ) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        if let Err(source) = sink.finish() {
            self.poisoned = true;
            return Err(DemSamplingRunError::Sink {
                phase: SinkFailurePhase::Finish,
                source,
                progress: DemSamplingRunProgress::new(committed, 0),
            });
        }
        let status = if completed {
            DemSamplingRunStatus::Completed
        } else {
            DemSamplingRunStatus::Cancelled
        };
        Ok(self.summary(status, requested_shots, committed))
    }

    fn ensure_sampled_error_storage(&mut self) -> Result<(), DemSamplingExecutionError> {
        if self.batch.has_sampled_errors() {
            return Ok(());
        }
        validate_session_storage(
            &self.plan,
            self.batch.capacity(),
            self.sample_capacity,
            true,
            self.limits,
        )?;
        let error_planes = try_zero_words(
            sample_plane_len(self.plan.error_count(), self.sample_capacity)?,
            "DEM sampled-error planes",
        )
        .map_err(storage_dem_error)?;
        self.batch.ensure_sampled_errors(self.plan.error_count())?;
        self.error_planes = Some(error_planes);
        Ok(())
    }

    fn is_cancelled(&self) -> bool {
        self.cancellation
            .get()
            .is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
    }

    fn summary(
        &self,
        status: DemSamplingRunStatus,
        requested_shots: ShotCount,
        committed_shots: u64,
    ) -> DemSamplingRunSummary {
        DemSamplingRunSummary {
            status,
            requested_shots,
            committed_shots: ShotCount::new(committed_shots),
            total_committed_shots: ShotCount::new(self.total_committed_shots),
        }
    }
}

/// Outcome of one incremental replay delivery call.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DemReplayBatchStatus {
    Accepted,
    Cancelled,
}

impl DemReplayBatchStatus {
    pub const fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted)
    }

    pub const fn is_cancelled(self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

/// Owned mutable state for one incremental sampled-error replay.
pub struct DemReplaySession {
    session: DemSamplingSession,
    expected_shots: ShotCount,
    committed_shots: u64,
    cancelled: bool,
    finished: bool,
    transaction_active: bool,
}

impl fmt::Debug for DemReplaySession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DemReplaySession")
            .field("expected_shots", &self.expected_shots)
            .field("committed_shots", &self.committed_shots)
            .field("cancelled", &self.cancelled)
            .field("finished", &self.finished)
            .field("transaction_active", &self.transaction_active)
            .finish_non_exhaustive()
    }
}

impl DemReplaySession {
    pub(super) fn new(
        plan: DemSamplingPlan,
        expected_shots: ShotCount,
        limits: DemSamplerLimits,
    ) -> Result<Self, DemSamplingExecutionError> {
        let mut session =
            DemSamplingSession::new(plan, RandomPolicy::Seeded(crate::Seed::new(0)), limits)?;
        session.preflight_common(expected_shots)?;
        if expected_shots.get() != 0 {
            session
                .plan
                .validate_replay_with_limits(expected_shots, limits)
                .map_err(DemSamplingExecutionError::InvalidRequest)?;
            session.ensure_sampled_error_storage()?;
        }
        Ok(Self {
            session,
            expected_shots,
            committed_shots: 0,
            cancelled: false,
            finished: false,
            transaction_active: false,
        })
    }

    pub fn cancellation(&self) -> DemSamplingCancellation {
        self.session.cancellation()
    }

    pub const fn is_poisoned(&self) -> bool {
        self.session.is_poisoned()
    }

    pub const fn committed_shots(&self) -> ShotCount {
        ShotCount::new(self.committed_shots)
    }

    /// Reopens a completed replay lifecycle while retaining bounded session storage.
    pub fn reset(&mut self) -> Result<(), DemSamplingExecutionError> {
        if self.transaction_active {
            return Err(DemSamplingExecutionError::ReplayLifecycle {
                message: "finish the active replay transaction before resetting it",
            });
        }
        if self.session.poisoned {
            return Err(DemSamplingExecutionError::SessionPoisoned);
        }
        if !self.finished {
            return Err(DemSamplingExecutionError::ReplayLifecycle {
                message: "finish the current replay before resetting it",
            });
        }
        if self.session.is_cancelled() {
            return Err(DemSamplingExecutionError::ReplayLifecycle {
                message: "reset the replay cancellation token before reopening the replay",
            });
        }
        self.session.preflight_common(self.expected_shots)?;
        self.committed_shots = 0;
        self.cancelled = false;
        self.finished = false;
        Ok(())
    }

    /// Binds this reusable replay state to one sink for an incremental delivery lifecycle.
    pub fn start_transaction<'session, 'sink, Sink>(
        &'session mut self,
        sink: &'sink mut Sink,
    ) -> Result<DemReplayTransaction<'session, 'sink, Sink>, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.ensure_available()?;
        self.transaction_active = true;
        Ok(DemReplayTransaction {
            starting_committed_shots: self.committed_shots,
            session: self,
            sink,
            finished: false,
        })
    }

    /// Replays a complete caller-owned record set after validating it before delivery.
    pub fn run<Sink>(
        &mut self,
        error_records: &[Vec<bool>],
        sink: &mut Sink,
    ) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.ensure_available()?;
        if self.committed_shots != 0 {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::ReplayLifecycle {
                    message: "reset the replay before running a complete record set",
                },
                progress: DemSamplingRunProgress::new(self.committed_shots, 0),
            });
        }
        let actual_shots = u64::try_from(error_records.len()).map_err(|_| {
            preflight_execution_error(DemSamplingExecutionError::ShotCounterOverflow)
        })?;
        if actual_shots != self.expected_shots.get() {
            return Err(preflight_error(DemError::invalid_result_format(format!(
                "DEM replay expected {} records, got {actual_shots}",
                self.expected_shots.get()
            ))));
        }
        for (shot_index, error_record) in error_records.iter().enumerate() {
            self.session
                .plan
                .validate_error_record_width(error_record, Some(shot_index))
                .map_err(preflight_error)?;
        }
        let mut transaction = self.start_transaction(sink)?;
        for records in error_records.chunks(MAX_BATCH_SHOTS) {
            if transaction.write_batch(records)? == DemReplayBatchStatus::Cancelled {
                break;
            }
        }
        transaction.finish()
    }

    fn write_batch_to<Sink>(
        &mut self,
        error_records: &[Vec<bool>],
        sink: &mut Sink,
    ) -> Result<DemReplayBatchStatus, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.ensure_transaction_state()?;
        if error_records.len() > MAX_BATCH_SHOTS {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::InvalidRequest(DemError::invalid_result_format(
                    format!(
                        "DEM replay batch supports at most {MAX_BATCH_SHOTS} records, got {}",
                        error_records.len()
                    ),
                )),
                progress: DemSamplingRunProgress::new(
                    self.committed_shots,
                    error_records.len() as u64,
                ),
            });
        }
        if self.cancelled || self.session.is_cancelled() {
            self.cancelled = true;
            return Ok(DemReplayBatchStatus::Cancelled);
        }
        let delivered_end = self
            .committed_shots
            .checked_add(error_records.len() as u64)
            .ok_or_else(|| DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::ShotCounterOverflow,
                progress: DemSamplingRunProgress::new(
                    self.committed_shots,
                    error_records.len() as u64,
                ),
            })?;
        if delivered_end > self.expected_shots.get() {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::InvalidRequest(DemError::invalid_result_format(
                    format!(
                        "DEM replay delivery would provide {delivered_end} records for an expected {}",
                        self.expected_shots.get()
                    ),
                )),
                progress: DemSamplingRunProgress::new(
                    self.committed_shots,
                    error_records.len() as u64,
                ),
            });
        }
        let first_shot =
            usize::try_from(self.committed_shots).map_err(|_| DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::ShotCounterOverflow,
                progress: DemSamplingRunProgress::new(
                    self.committed_shots,
                    error_records.len() as u64,
                ),
            })?;
        for (local_index, error_record) in error_records.iter().enumerate() {
            let shot_index =
                first_shot
                    .checked_add(local_index)
                    .ok_or_else(|| DemSamplingRunError::Engine {
                        source: DemSamplingExecutionError::ShotCounterOverflow,
                        progress: DemSamplingRunProgress::new(
                            self.committed_shots,
                            error_records.len() as u64,
                        ),
                    })?;
            self.session
                .plan
                .validate_error_record_width(error_record, Some(shot_index))
                .map_err(|source| DemSamplingRunError::Engine {
                    source: DemSamplingExecutionError::InvalidRequest(source),
                    progress: DemSamplingRunProgress::new(
                        self.committed_shots,
                        error_records.len() as u64,
                    ),
                })?;
        }

        for records in error_records.chunks(self.session.batch.capacity()) {
            if self.session.is_cancelled() {
                self.cancelled = true;
                return Ok(DemReplayBatchStatus::Cancelled);
            }
            let batch_shots = records.len();
            if let Err(source) = self.session.fill_replay_batch(records) {
                self.session.poisoned = true;
                self.finished = true;
                return Err(DemSamplingRunError::Engine {
                    source,
                    progress: DemSamplingRunProgress::new(self.committed_shots, batch_shots as u64),
                });
            }
            if let Err(error) =
                self.session
                    .write_active_batch(batch_shots, true, sink, self.committed_shots)
            {
                self.finished = true;
                return Err(error);
            }
            let batch_shots_u64 = batch_shots as u64;
            self.committed_shots += batch_shots_u64;
            self.session.total_committed_shots += batch_shots_u64;
        }
        Ok(DemReplayBatchStatus::Accepted)
    }

    fn ensure_available<SinkError>(&self) -> Result<(), DemSamplingRunError<SinkError>> {
        if self.session.poisoned {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::SessionPoisoned,
                progress: DemSamplingRunProgress::new(self.committed_shots, 0),
            });
        }
        if self.transaction_active {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::ReplayLifecycle {
                    message: "finish the active replay transaction before starting another",
                },
                progress: DemSamplingRunProgress::new(self.committed_shots, 0),
            });
        }
        if self.finished {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::ReplayLifecycle {
                    message: "reset the completed replay before starting another transaction",
                },
                progress: DemSamplingRunProgress::new(self.committed_shots, 0),
            });
        }
        Ok(())
    }

    fn ensure_transaction_state<SinkError>(&self) -> Result<(), DemSamplingRunError<SinkError>> {
        if self.session.poisoned {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::SessionPoisoned,
                progress: DemSamplingRunProgress::new(self.committed_shots, 0),
            });
        }
        if self.finished {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::ReplayLifecycle {
                    message: "reset the completed replay before delivering more records",
                },
                progress: DemSamplingRunProgress::new(self.committed_shots, 0),
            });
        }
        Ok(())
    }

    fn finish_to<Sink>(
        &mut self,
        sink: &mut Sink,
    ) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.ensure_transaction_state()?;
        if self.expected_shots.get() == 0 {
            self.finished = true;
            return Ok(self.session.summary(
                DemSamplingRunStatus::Completed,
                self.expected_shots,
                0,
            ));
        }
        if self.session.is_cancelled() {
            self.cancelled = true;
        }
        if !self.cancelled && self.committed_shots != self.expected_shots.get() {
            if self.committed_shots != 0 {
                self.session.poisoned = true;
            }
            self.finished = true;
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::InvalidRequest(DemError::invalid_result_format(
                    format!(
                        "DEM replay delivery ended after {} records but {} were expected",
                        self.committed_shots,
                        self.expected_shots.get()
                    ),
                )),
                progress: DemSamplingRunProgress::new(self.committed_shots, 0),
            });
        }
        let completed = !self.cancelled;
        self.finished = true;
        self.session
            .finish_run(sink, self.expected_shots, self.committed_shots, completed)
    }
}

/// One short-lived replay transaction bound to exactly one output sink.
pub struct DemReplayTransaction<'session, 'sink, Sink>
where
    Sink: DemSampleSink,
{
    session: &'session mut DemReplaySession,
    sink: &'sink mut Sink,
    starting_committed_shots: u64,
    finished: bool,
}

impl<Sink> fmt::Debug for DemReplayTransaction<'_, '_, Sink>
where
    Sink: DemSampleSink,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DemReplayTransaction")
            .field("sink_type", &std::any::type_name::<Sink>())
            .field("starting_committed_shots", &self.starting_committed_shots)
            .field("committed_shots", &self.session.committed_shots)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl<Sink> DemReplayTransaction<'_, '_, Sink>
where
    Sink: DemSampleSink,
{
    /// Delivers at most 64 replay records without finalizing this transaction.
    pub fn write_batch(
        &mut self,
        error_records: &[Vec<bool>],
    ) -> Result<DemReplayBatchStatus, DemSamplingRunError<Sink::Error>> {
        self.session.write_batch_to(error_records, self.sink)
    }

    /// Finalizes the same sink that received every preceding replay batch.
    pub fn finish(mut self) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>> {
        let result = self.session.finish_to(self.sink);
        self.session.transaction_active = false;
        self.finished = true;
        result
    }
}

impl<Sink> Drop for DemReplayTransaction<'_, '_, Sink>
where
    Sink: DemSampleSink,
{
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        self.session.transaction_active = false;
        if self.session.committed_shots != self.starting_committed_shots {
            self.session.session.poisoned = true;
            self.session.finished = true;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum RunMode {
    DetectorOnly,
    WithSampledErrors,
}

impl RunMode {
    const fn includes_sampled_errors(self) -> bool {
        matches!(self, Self::WithSampledErrors)
    }
}

fn sample_plane_len(
    width: usize,
    sample_capacity: usize,
) -> Result<usize, DemSamplingExecutionError> {
    width
        .checked_mul(sample_capacity.div_ceil(u64::BITS as usize))
        .ok_or_else(|| {
            DemSamplingExecutionError::InvalidRequest(DemError::invalid_sampler_compilation(
                "DEM sample plane storage size overflowed",
            ))
        })
}

fn storage_dem_error(source: DemError) -> DemSamplingExecutionError {
    DemSamplingExecutionError::SessionStorageAllocation {
        message: source.to_string(),
    }
}

fn internal_format_error(source: RecordsFormatError) -> DemSamplingExecutionError {
    DemSamplingExecutionError::InternalInvariant {
        message: source.to_string(),
    }
}

fn preflight_error<SinkError>(source: DemError) -> DemSamplingRunError<SinkError> {
    preflight_execution_error(DemSamplingExecutionError::InvalidRequest(source))
}

fn preflight_execution_error<SinkError>(
    source: DemSamplingExecutionError,
) -> DemSamplingRunError<SinkError> {
    DemSamplingRunError::Engine {
        source,
        progress: DemSamplingRunProgress::new(0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DemSamplingCompiler;
    use stab_model::DetectorErrorModel;

    #[test]
    #[allow(clippy::expect_used, reason = "the fixture must exist before mutation")]
    fn replay_reset_rechecks_cumulative_shot_overflow() {
        let dem = DetectorErrorModel::from_dem_str("error(1) D0\n").expect("parse DEM");
        let plan = DemSamplingCompiler::new()
            .compile(&dem)
            .expect("compile DEM");
        let mut replay = plan
            .replay_session(ShotCount::new(1))
            .expect("create replay");
        replay.finished = true;
        replay.session.total_committed_shots = u64::MAX;

        assert!(matches!(
            replay.reset(),
            Err(DemSamplingExecutionError::ShotCounterOverflow)
        ));
        assert!(
            replay.finished,
            "a rejected reset must not reopen the replay"
        );
    }
}
