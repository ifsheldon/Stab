use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use rand::rngs::SmallRng;
use stab_records::FormatError as RecordsFormatError;
use thiserror::Error;

use super::dem_sampler_rng;
use super::limits::DemSamplerLimits;
use super::plan::DemSamplingPlan;
use crate::{
    CircuitError, DemSampleBatchView, DemSampleSink, DetectionBatchView, DetectionEventRecord,
    PackedShotBatch, RandomPolicy, ShotCount, SinkFailurePhase,
};

const MAX_BATCH_SHOTS: usize = 64;
const MAX_DEM_SESSION_STORAGE_BYTES: u64 = 256 * 1024 * 1024;

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
    InvalidRequest(#[from] CircuitError),

    #[error("DEM sampling execution violated an internal batch invariant: {message}")]
    InternalInvariant { message: String },
}

impl DemSamplingExecutionError {
    pub fn into_circuit_error(self) -> CircuitError {
        match self {
            Self::InvalidRequest(error) => error,
            other => CircuitError::invalid_sampler_compilation(other.to_string()),
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
    rng: SmallRng,
    record: DetectionEventRecord,
    error_record: Option<Vec<bool>>,
    batch: SessionBatch,
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
        let batch_capacity = initial_batch_capacity(&plan, limits)?;
        let record = plan.try_reusable_detection_record().map_err(|source| {
            DemSamplingExecutionError::SessionStorageAllocation {
                message: source.to_string(),
            }
        })?;
        let batch = SessionBatch::try_new(&plan, batch_capacity)?;
        let rng = match random_policy {
            RandomPolicy::Entropy => dem_sampler_rng(None),
            RandomPolicy::Seeded(seed) => dem_sampler_rng(Some(seed.get())),
            _ => dem_sampler_rng(None),
        };
        Ok(Self {
            plan,
            limits,
            rng,
            record,
            error_record: None,
            batch,
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

    /// Replays caller-owned sampled-error records without advancing this session's RNG.
    pub fn replay<Sink>(
        &mut self,
        error_records: &[Vec<bool>],
        sink: &mut Sink,
    ) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        let shots = u64::try_from(error_records.len())
            .map(ShotCount::new)
            .map_err(|_| DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::ShotCounterOverflow,
                progress: DemSamplingRunProgress::new(0, 0),
            })?;
        self.preflight_replay_request(shots)?;
        for (shot_index, error_record) in error_records.iter().enumerate() {
            self.plan
                .validate_error_record_width(error_record, Some(shot_index))
                .map_err(preflight_error)?;
        }
        let mut replay = self.start_replay_after_preflight(shots, sink)?;
        for records in error_records.chunks(MAX_BATCH_SHOTS) {
            if replay.write_batch(records)? == DemReplayBatchStatus::Cancelled {
                break;
            }
        }
        replay.finish()
    }

    /// Starts one replay sink lifecycle after the caller has validated and rewound its source.
    ///
    /// The returned delivery object accepts record batches incrementally and finalizes the sink
    /// exactly once. Dropping it after committed output poisons the parent session because the
    /// sink lifecycle is then incomplete.
    pub fn start_replay<'session, 'sink, Sink>(
        &'session mut self,
        expected_shots: ShotCount,
        sink: &'sink mut Sink,
    ) -> Result<DemReplaySession<'session, 'sink, Sink>, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.preflight_replay_request(expected_shots)?;
        self.start_replay_after_preflight(expected_shots, sink)
    }

    fn preflight_replay_request<SinkError>(
        &self,
        expected_shots: ShotCount,
    ) -> Result<(), DemSamplingRunError<SinkError>> {
        self.preflight_common(expected_shots)?;
        let shots =
            usize::try_from(expected_shots.get()).map_err(|_| DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::ShotCounterOverflow,
                progress: DemSamplingRunProgress::new(0, 0),
            })?;
        if shots != 0 {
            self.plan
                .validate_replay_with_limits(expected_shots, self.limits)
                .map_err(preflight_error)?;
        }
        Ok(())
    }

    fn start_replay_after_preflight<'session, 'sink, Sink>(
        &'session mut self,
        expected_shots: ShotCount,
        sink: &'sink mut Sink,
    ) -> Result<DemReplaySession<'session, 'sink, Sink>, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        if expected_shots.get() != 0 {
            self.ensure_sampled_error_storage()
                .map_err(preflight_execution_error)?;
        }
        Ok(DemReplaySession {
            session: self,
            sink,
            expected_shots,
            committed_shots: 0,
            cancelled: false,
            finished: false,
        })
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
        self.preflight_common(shots)?;
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
        while remaining > 0 {
            if self.is_cancelled() {
                break;
            }
            let batch_shots_u64 = remaining.min(self.batch.capacity as u64);
            let batch_shots =
                usize::try_from(batch_shots_u64).map_err(|_| DemSamplingRunError::Engine {
                    source: DemSamplingExecutionError::InternalInvariant {
                        message: "bounded DEM batch shot count did not fit usize".to_owned(),
                    },
                    progress: DemSamplingRunProgress::new(committed, batch_shots_u64),
                })?;
            if let Err(source) = self.fill_sample_batch(batch_shots, mode) {
                self.poisoned = true;
                return Err(DemSamplingRunError::Engine {
                    source,
                    progress: DemSamplingRunProgress::new(committed, batch_shots_u64),
                });
            }
            self.write_active_batch(batch_shots, mode.includes_sampled_errors(), sink, committed)?;
            committed += batch_shots_u64;
            self.total_committed_shots += batch_shots_u64;
            remaining -= batch_shots_u64;
        }
        self.finish_run(sink, shots, committed, remaining == 0)
    }

    fn preflight_common<SinkError>(
        &self,
        shots: ShotCount,
    ) -> Result<(), DemSamplingRunError<SinkError>> {
        if self.poisoned {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::SessionPoisoned,
                progress: DemSamplingRunProgress::new(0, 0),
            });
        }
        if self
            .total_committed_shots
            .checked_add(shots.get())
            .is_none()
        {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::ShotCounterOverflow,
                progress: DemSamplingRunProgress::new(0, 0),
            });
        }
        Ok(())
    }

    fn fill_sample_batch(
        &mut self,
        shot_count: usize,
        mode: RunMode,
    ) -> Result<(), DemSamplingExecutionError> {
        for shot_index in 0..shot_count {
            match mode {
                RunMode::DetectorOnly => self
                    .plan
                    .sample_detection_record_into(&mut self.rng, &mut self.record)
                    .map_err(DemSamplingExecutionError::InvalidRequest)?,
                RunMode::WithSampledErrors => {
                    let error_record = self.error_record.as_mut().ok_or_else(|| {
                        DemSamplingExecutionError::InternalInvariant {
                            message: "sampled-error run omitted its reusable error record"
                                .to_owned(),
                        }
                    })?;
                    self.plan
                        .sample_detection_record_and_error_record_into(
                            &mut self.rng,
                            &mut self.record,
                            error_record,
                        )
                        .map_err(DemSamplingExecutionError::InvalidRequest)?;
                    if error_record.len() != self.plan.error_count() {
                        return Err(DemSamplingExecutionError::InternalInvariant {
                            message: format!(
                                "DEM sampled-error engine produced {} bits for declared width {}",
                                error_record.len(),
                                self.plan.error_count()
                            ),
                        });
                    }
                    let error_batch = self.batch.sampled_errors.as_mut().ok_or_else(|| {
                        DemSamplingExecutionError::InternalInvariant {
                            message: "sampled-error run omitted its reusable error batch"
                                .to_owned(),
                        }
                    })?;
                    error_batch
                        .copy_shot_from_bools(shot_index, error_record)
                        .map_err(internal_format_error)?;
                }
            }
            self.batch
                .detectors
                .copy_shot_from_bools(shot_index, &self.record.detectors)
                .map_err(internal_format_error)?;
            self.batch
                .observables
                .copy_shot_from_bools(shot_index, &self.record.observables)
                .map_err(internal_format_error)?;
        }
        Ok(())
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
                .detectors
                .copy_shot_from_bools(shot_index, &self.record.detectors)
                .map_err(internal_format_error)?;
            self.batch
                .observables
                .copy_shot_from_bools(shot_index, &self.record.observables)
                .map_err(internal_format_error)?;
            self.batch
                .sampled_errors
                .as_mut()
                .ok_or_else(|| DemSamplingExecutionError::InternalInvariant {
                    message: "DEM replay omitted its reusable sampled-error batch".to_owned(),
                })?
                .copy_shot_from_bools(shot_index, error_record)
                .map_err(internal_format_error)?;
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
        if self.batch.sampled_errors.is_some() {
            return Ok(());
        }
        validate_session_storage(&self.plan, self.batch.capacity, true, self.limits)?;
        let error_record = self.plan.try_reusable_error_record().map_err(|source| {
            DemSamplingExecutionError::SessionStorageAllocation {
                message: source.to_string(),
            }
        })?;
        let sampled_errors = PackedShotBatch::zeros(self.batch.capacity, self.plan.error_count())
            .map_err(|source| {
            DemSamplingExecutionError::SessionStorageAllocation {
                message: source.to_string(),
            }
        })?;
        self.error_record = Some(error_record);
        self.batch.sampled_errors = Some(sampled_errors);
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

/// One incremental replay delivery and sink lifecycle.
pub struct DemReplaySession<'session, 'sink, Sink>
where
    Sink: DemSampleSink,
{
    session: &'session mut DemSamplingSession,
    sink: &'sink mut Sink,
    expected_shots: ShotCount,
    committed_shots: u64,
    cancelled: bool,
    finished: bool,
}

impl<Sink> fmt::Debug for DemReplaySession<'_, '_, Sink>
where
    Sink: DemSampleSink,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DemReplaySession")
            .field("expected_shots", &self.expected_shots)
            .field("committed_shots", &self.committed_shots)
            .field("cancelled", &self.cancelled)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl<Sink> DemReplaySession<'_, '_, Sink>
where
    Sink: DemSampleSink,
{
    /// Delivers at most 64 prevalidated-width replay records without finalizing the sink.
    pub fn write_batch(
        &mut self,
        error_records: &[Vec<bool>],
    ) -> Result<DemReplayBatchStatus, DemSamplingRunError<Sink::Error>> {
        if self.finished || self.session.poisoned {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::SessionPoisoned,
                progress: DemSamplingRunProgress::new(self.committed_shots, 0),
            });
        }
        if error_records.len() > MAX_BATCH_SHOTS {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::InvalidRequest(
                    CircuitError::invalid_result_format(format!(
                        "DEM replay batch supports at most {MAX_BATCH_SHOTS} records, got {}",
                        error_records.len()
                    )),
                ),
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
                source: DemSamplingExecutionError::InvalidRequest(
                    CircuitError::invalid_result_format(format!(
                        "DEM replay delivery would provide {delivered_end} records for an expected {}",
                        self.expected_shots.get()
                    )),
                ),
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

        for records in error_records.chunks(self.session.batch.capacity) {
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
                    .write_active_batch(batch_shots, true, self.sink, self.committed_shots)
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

    /// Finalizes this replay sink exactly once.
    pub fn finish(mut self) -> Result<DemSamplingRunSummary, DemSamplingRunError<Sink::Error>> {
        if self.finished || self.session.poisoned {
            self.finished = true;
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::SessionPoisoned,
                progress: DemSamplingRunProgress::new(self.committed_shots, 0),
            });
        }
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
                source: DemSamplingExecutionError::InvalidRequest(
                    CircuitError::invalid_result_format(format!(
                        "DEM replay delivery ended after {} records but {} were expected",
                        self.committed_shots,
                        self.expected_shots.get()
                    )),
                ),
                progress: DemSamplingRunProgress::new(self.committed_shots, 0),
            });
        }
        let completed = !self.cancelled;
        self.finished = true;
        self.session.finish_run(
            self.sink,
            self.expected_shots,
            self.committed_shots,
            completed,
        )
    }
}

impl<Sink> Drop for DemReplaySession<'_, '_, Sink>
where
    Sink: DemSampleSink,
{
    fn drop(&mut self) {
        if !self.finished && self.committed_shots != 0 {
            self.session.poisoned = true;
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

#[derive(Debug)]
struct SessionBatch {
    detectors: PackedShotBatch,
    observables: PackedShotBatch,
    sampled_errors: Option<PackedShotBatch>,
    capacity: usize,
}

impl SessionBatch {
    fn try_new(plan: &DemSamplingPlan, capacity: usize) -> Result<Self, DemSamplingExecutionError> {
        let detectors = PackedShotBatch::zeros(capacity, plan.detector_count())
            .map_err(storage_format_error)?;
        let observables = PackedShotBatch::zeros(capacity, plan.observable_count())
            .map_err(storage_format_error)?;
        Ok(Self {
            detectors,
            observables,
            sampled_errors: None,
            capacity,
        })
    }

    fn view(
        &self,
        shot_count: usize,
        include_sampled_errors: bool,
    ) -> Result<DemSampleBatchView<'_>, DemSamplingExecutionError> {
        let detectors = self
            .detectors
            .view_prefix(shot_count)
            .map_err(internal_format_error)?;
        let observables = self
            .observables
            .view_prefix(shot_count)
            .map_err(internal_format_error)?;
        let detection =
            DetectionBatchView::try_new(detectors, observables).map_err(internal_format_error)?;
        let sampled_errors = if include_sampled_errors {
            Some(
                self.sampled_errors
                    .as_ref()
                    .ok_or_else(|| DemSamplingExecutionError::InternalInvariant {
                        message: "DEM batch omitted requested sampled-error storage".to_owned(),
                    })?
                    .view_prefix(shot_count)
                    .map_err(internal_format_error)?,
            )
        } else {
            None
        };
        DemSampleBatchView::try_new(detection, sampled_errors).map_err(internal_format_error)
    }
}

fn initial_batch_capacity(
    plan: &DemSamplingPlan,
    limits: DemSamplerLimits,
) -> Result<usize, DemSamplingExecutionError> {
    let combined_fits = validate_session_storage(plan, 1, true, limits).is_ok();
    let include_sampled_errors = combined_fits;
    for capacity in (1..=MAX_BATCH_SHOTS).rev() {
        if validate_session_storage(plan, capacity, include_sampled_errors, limits).is_ok() {
            return Ok(capacity);
        }
    }
    validate_session_storage(plan, 1, false, limits)?;
    Err(DemSamplingExecutionError::InternalInvariant {
        message: "one-shot DEM session storage passed admission but no batch capacity was selected"
            .to_owned(),
    })
}

fn validate_session_storage(
    plan: &DemSamplingPlan,
    shot_count: usize,
    include_sampled_errors: bool,
    limits: DemSamplerLimits,
) -> Result<(), DemSamplingExecutionError> {
    plan.validate_sample_buffer_units_with_limits(1, include_sampled_errors, limits)?;
    let estimated_bytes = session_storage_bytes(plan, shot_count, include_sampled_errors);
    if estimated_bytes > limits.max_materialized_bytes() as u128 {
        let actual = usize::try_from(estimated_bytes).map_err(|_| {
            DemSamplingExecutionError::InvalidRequest(CircuitError::invalid_sampler_compilation(
                "DEM sampling session active byte estimate overflowed usize",
            ))
        })?;
        return Err(DemSamplingExecutionError::InvalidRequest(
            crate::ResourceLimitError::dem_materialized_bytes(
                actual,
                limits.max_materialized_bytes(),
            )
            .into(),
        ));
    }
    if estimated_bytes > u128::from(MAX_DEM_SESSION_STORAGE_BYTES) {
        return Err(DemSamplingExecutionError::SessionStorageLimit {
            estimated_bytes,
            limit_bytes: MAX_DEM_SESSION_STORAGE_BYTES,
        });
    }
    Ok(())
}

fn session_storage_bytes(
    plan: &DemSamplingPlan,
    shot_count: usize,
    include_sampled_errors: bool,
) -> u128 {
    let detector_width = plan.detector_count() as u128;
    let observable_width = plan.observable_count() as u128;
    let sampled_error_width = if include_sampled_errors {
        plan.error_count() as u128
    } else {
        0
    };
    let scratch = (std::mem::size_of::<DetectionEventRecord>() as u128)
        .saturating_add(detector_width)
        .saturating_add(observable_width)
        .saturating_add(if include_sampled_errors {
            (std::mem::size_of::<Vec<bool>>() as u128).saturating_add(sampled_error_width)
        } else {
            0
        });
    let packed_rows = packed_row_bytes(plan.detector_count())
        .saturating_add(packed_row_bytes(plan.observable_count()))
        .saturating_add(if include_sampled_errors {
            packed_row_bytes(plan.error_count())
        } else {
            0
        });
    scratch.saturating_add(packed_rows.saturating_mul(shot_count as u128))
}

fn packed_row_bytes(width: usize) -> u128 {
    (width.div_ceil(u64::BITS as usize) as u128).saturating_mul(std::mem::size_of::<u64>() as u128)
}

fn storage_format_error(source: RecordsFormatError) -> DemSamplingExecutionError {
    DemSamplingExecutionError::SessionStorageAllocation {
        message: source.to_string(),
    }
}

fn internal_format_error(source: RecordsFormatError) -> DemSamplingExecutionError {
    DemSamplingExecutionError::InternalInvariant {
        message: source.to_string(),
    }
}

fn preflight_error<SinkError>(source: CircuitError) -> DemSamplingRunError<SinkError> {
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
