use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::sync::OnceLock;

use stab_model::Cancellation;
use stab_records::{DemSampleSink, FormatError as RecordsFormatError};
use thiserror::Error;

use super::buffers::try_zero_words;
use super::plan::DemSamplingPlan;
use super::{DemError, DemSamplerLimits};
use crate::sampling::{session_cancellation, session_cancellation_slot};
use crate::{DetectionRecordBuffer, RandomPolicy, ShotCount, SinkFailurePhase};

/// Most records one replay batch holds.
const MAX_REPLAY_BATCH_SHOTS: usize = 64;
/// Most shots one generated-sample batch delivers: the batch bound every sampling session shares.
const FRAME_BATCH_SHOTS: usize = crate::MAX_BATCH_SHOTS;

mod batch;
mod replay_planes;
mod replay_records;
mod replay_session;
mod retained;
mod window;

pub use replay_session::{DemReplayBatchStatus, DemReplaySession, DemReplayTransaction};

use batch::{SessionBatch, SessionUse, initial_capacities, validate_session_storage};
use replay_records::{ReplayRecord, ReplayRecords};

/// Engine-side DEM sampling execution failure.
#[derive(Debug, Error)]
pub enum DemSamplingExecutionError {
    #[error("DEM sampling session is poisoned")]
    SessionPoisoned,

    #[error("DEM sampling session shot counter overflowed")]
    ShotCounterOverflow,

    #[error("DEM replay session lifecycle error: {message}")]
    ReplayLifecycle { message: &'static str },

    #[error("DEM sampling session could not allocate bounded storage: {message}")]
    SessionStorageAllocation { message: String },

    #[error(transparent)]
    InvalidRequest(#[from] DemError),

    #[error("DEM sampling execution violated an internal batch invariant: {message}")]
    InternalInvariant { message: String },
}

impl DemSamplingExecutionError {
    /// The memory-budget refusal of the wrapped request failure, if this error is one.
    pub fn memory_budget_exceeded(&self) -> Option<&stab_model::MemoryBudgetExceeded> {
        match self {
            Self::InvalidRequest(error) => error.memory_budget_exceeded(),
            _ => None,
        }
    }

    pub fn into_dem_error(self) -> DemError {
        match self {
            Self::InvalidRequest(error) => error,
            error @ Self::SessionStorageAllocation { .. } => {
                DemError::storage_allocation(error.to_string())
            }
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
    /// The engine's memory-budget refusal; a sink failure is the caller's own type and is not
    /// inspected.
    pub fn memory_budget_exceeded(&self) -> Option<&stab_model::MemoryBudgetExceeded> {
        match self {
            Self::Engine { source, .. } => source.memory_budget_exceeded(),
            Self::Sink { .. } => None,
        }
    }

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
    cancellation: OnceLock<Cancellation>,
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
                &session_cancellation(&self.cancellation).is_cancelled(),
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
        Self::with_use(plan, random_policy, limits, SessionUse::Sampling)
    }

    /// Creates a session whose sample planes are sized for `session_use`.
    fn with_use(
        plan: DemSamplingPlan,
        random_policy: RandomPolicy,
        limits: DemSamplerLimits,
        session_use: SessionUse,
    ) -> Result<Self, DemSamplingExecutionError> {
        let capacities = initial_capacities(&plan, &limits, session_use)?;
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
        let cancellation = session_cancellation_slot(limits.cancellation());
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
            cancellation,
            total_committed_shots: 0,
            poisoned: false,
            not_sync: PhantomData,
        })
    }

    /// The signal this session polls between batches and while sampling a batch.
    ///
    /// It is the signal of [`DemSamplerLimits::with_cancellation`] when that one is cancellable,
    /// and otherwise a session-local signal created by the first call. Shots are drawn from
    /// counter-based streams, so cancellation observed inside a batch discards that batch and
    /// the session resumes with the same seeded output.
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.get_or_init(Cancellation::new).clone()
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
    ///
    /// The detector and observable records equal those [`Self::run`] samples for the same seed
    /// and shot range, and replaying the sampled-error records reproduces them.
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
        usize::try_from(shots.get()).map_err(|_| DemSamplingRunError::Engine {
            source: DemSamplingExecutionError::ShotCounterOverflow,
            progress: DemSamplingRunProgress::new(0, 0),
        })?;
        if matches!(mode, RunMode::WithSampledErrors) {
            self.ensure_sampled_error_storage()
                .map_err(preflight_execution_error)?;
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
                if matches!(
                    source,
                    DemSamplingExecutionError::InvalidRequest(DemError::Cancelled(_))
                ) {
                    break;
                }
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
                let batch_shots = FRAME_BATCH_SHOTS.min(sample_shots - sample_offset);
                self.write_sample_window(
                    sample_shots,
                    sample_offset,
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
                    session_cancellation(&self.cancellation),
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
                        session_cancellation(&self.cancellation),
                    )
                    .map_err(DemSamplingExecutionError::InvalidRequest)?;
            }
        }
        Ok(())
    }

    fn fill_replay_batch(
        &mut self,
        error_records: ReplayRecords<'_>,
        range: std::ops::Range<usize>,
    ) -> Result<(), DemSamplingExecutionError> {
        let first = range.start;
        for record_index in range {
            let shot_index = record_index - first;
            let error_record = error_records
                .record(record_index)
                .map_err(internal_format_error)?;
            let cancellation = session_cancellation(&self.cancellation);
            match error_record {
                ReplayRecord::Dense(bits) => self.plan.detection_record_from_error_record_into(
                    bits,
                    &mut self.record,
                    cancellation,
                ),
                ReplayRecord::Packed(bits) => self.plan.detection_record_from_error_record_into(
                    &bits,
                    &mut self.record,
                    cancellation,
                ),
                ReplayRecord::Sparse(bits) => self.plan.detection_record_from_error_record_into(
                    &bits,
                    &mut self.record,
                    cancellation,
                ),
            }
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
            &self.limits,
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
        session_cancellation(&self.cancellation).is_cancelled()
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
mod storage_tests;
