//! Incremental and whole-buffer sampled-error replay sessions and transactions.

use std::fmt;

use stab_model::Cancellation;
use stab_records::{BitPlaneBatchView, DemSampleSink, PackedShotBatchView};

use super::batch::SessionUse;
use super::replay_planes::ErrorMajorRecords;
use super::replay_records::ReplayRecords;
use super::{
    DemSamplingExecutionError, DemSamplingRunError, DemSamplingRunProgress, DemSamplingRunStatus,
    DemSamplingRunSummary, DemSamplingSession, MAX_REPLAY_BATCH_SHOTS, preflight_error,
    preflight_execution_error,
};
use crate::dem_sampling::replay_planes::prefers_error_major;
use crate::dem_sampling::{DemError, DemSamplerLimits, DemSamplingPlan};
use crate::{RandomPolicy, ShotCount};

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
    pub(super) session: DemSamplingSession,
    expected_shots: ShotCount,
    pub(super) committed_shots: u64,
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
    pub(in crate::dem_sampling) fn new(
        plan: DemSamplingPlan,
        expected_shots: ShotCount,
        limits: DemSamplerLimits,
    ) -> Result<Self, DemSamplingExecutionError> {
        let mut session = DemSamplingSession::with_use(
            plan,
            RandomPolicy::Seeded(crate::Seed::new(0)),
            limits,
            SessionUse::Replay,
        )?;
        session.preflight_common(expected_shots)?;
        if expected_shots.get() != 0 {
            session
                .plan
                .validate_replay(expected_shots)
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

    /// The signal this replay polls; see [`DemSamplingSession::cancellation`].
    pub fn cancellation(&self) -> Cancellation {
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
                .validate_error_record_width(error_record.as_slice(), Some(shot_index))
                .map_err(preflight_error)?;
        }
        let mut transaction = self.start_transaction(sink)?;
        for records in error_records.chunks(MAX_REPLAY_BATCH_SHOTS) {
            if transaction.write_batch(records)? == DemReplayBatchStatus::Cancelled {
                break;
            }
        }
        transaction.finish()
    }

    fn write_batch_to<Sink>(
        &mut self,
        error_records: ReplayRecords<'_>,
        sink: &mut Sink,
    ) -> Result<DemReplayBatchStatus, DemSamplingRunError<Sink::Error>>
    where
        Sink: DemSampleSink,
    {
        self.ensure_transaction_state()?;
        if error_records.len() > MAX_REPLAY_BATCH_SHOTS {
            return Err(DemSamplingRunError::Engine {
                source: DemSamplingExecutionError::InvalidRequest(DemError::invalid_result_format(
                    format!(
                        "DEM replay batch supports at most {MAX_REPLAY_BATCH_SHOTS} records, got {}",
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
        // Every record of a packed, bit-plane, error-major, or sparse batch shares the batch's
        // width, so its first record decides; dense caller records are checked one by one.
        let validated_records = match error_records {
            ReplayRecords::Dense(records) => records.len(),
            ReplayRecords::Packed(_)
            | ReplayRecords::BitPlanes(_)
            | ReplayRecords::ErrorMajor { .. }
            | ReplayRecords::Sparse { .. } => error_records.len().min(1),
        };
        for local_index in 0..validated_records {
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
            error_records
                .validate_width(&self.session.plan, local_index, shot_index)
                .map_err(|source| DemSamplingRunError::Engine {
                    source,
                    progress: DemSamplingRunProgress::new(
                        self.committed_shots,
                        error_records.len() as u64,
                    ),
                })?;
        }

        // An error-major or bit-plane batch replays its whole word per error at once; shot-major
        // and sparse records are delivered in chunks the packed output batch holds.
        let chunk_shots = match error_records {
            ReplayRecords::BitPlanes(_) | ReplayRecords::ErrorMajor { .. } => error_records.len(),
            _ => self.session.batch.capacity(),
        }
        .max(1);
        let mut chunk_start = 0_usize;
        while chunk_start < error_records.len() {
            let chunk_end = chunk_start
                .saturating_add(chunk_shots)
                .min(error_records.len());
            let records = chunk_start..chunk_end;
            chunk_start = chunk_end;
            if self.session.is_cancelled() {
                self.cancelled = true;
                return Ok(DemReplayBatchStatus::Cancelled);
            }
            let batch_shots = records.len();
            let error_major = match error_records {
                ReplayRecords::Packed(packed) => {
                    prefers_error_major(&self.session.plan, packed, records.start, batch_shots)
                        .then(|| ErrorMajorRecords::Packed(packed, records.clone()))
                }
                ReplayRecords::BitPlanes(planes) => Some(ErrorMajorRecords::BitPlanes(planes)),
                ReplayRecords::ErrorMajor { words, .. } => Some(ErrorMajorRecords::Words(words)),
                ReplayRecords::Dense(_) | ReplayRecords::Sparse { .. } => None,
            };
            let replays_error_major = error_major.is_some();
            let filled = match error_major {
                Some(error_major) => self.session.fill_replay_planes(error_major),
                None => self.session.fill_replay_batch(error_records, records),
            };
            if let Err(source) = filled {
                if matches!(
                    source,
                    DemSamplingExecutionError::InvalidRequest(DemError::Cancelled(_))
                ) {
                    self.cancelled = true;
                    return Ok(DemReplayBatchStatus::Cancelled);
                }
                self.session.poisoned = true;
                self.finished = true;
                return Err(DemSamplingRunError::Engine {
                    source,
                    progress: DemSamplingRunProgress::new(self.committed_shots, batch_shots as u64),
                });
            }
            let written = if replays_error_major {
                self.session.write_sample_window(
                    batch_shots,
                    0,
                    batch_shots,
                    true,
                    sink,
                    self.committed_shots,
                )
            } else {
                self.session
                    .write_active_batch(batch_shots, true, sink, self.committed_shots)
            };
            if let Err(error) = written {
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
    pub(super) session: &'session mut DemReplaySession,
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
        self.write_replay_records(ReplayRecords::Dense(error_records))
    }

    /// Delivers at most 64 packed replay records without finalizing this transaction.
    ///
    /// Every shot of the batch is one sampled-error record of exactly the plan's error count,
    /// with the same validation, work accounting, and limits as [`Self::write_batch`].
    pub fn write_packed_batch(
        &mut self,
        error_records: PackedShotBatchView<'_>,
    ) -> Result<DemReplayBatchStatus, DemSamplingRunError<Sink::Error>> {
        self.write_replay_records(ReplayRecords::Packed(error_records))
    }

    /// Delivers at most 64 bit-plane replay records directly into error-major replay without a
    /// shot-major transpose.
    ///
    /// The view must carry exactly the plan's error count as its bit width, with the same
    /// validation, work accounting, and limits as [`Self::write_packed_batch`].
    pub fn write_bit_plane_batch(
        &mut self,
        error_records: BitPlaneBatchView<'_>,
    ) -> Result<DemReplayBatchStatus, DemSamplingRunError<Sink::Error>> {
        self.write_replay_records(ReplayRecords::BitPlanes(error_records))
    }

    pub(super) fn write_replay_records(
        &mut self,
        error_records: ReplayRecords<'_>,
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

#[cfg(test)]
mod tests {
    use std::assert_matches;

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

        assert_matches!(
            replay.reset(),
            Err(DemSamplingExecutionError::ShotCounterOverflow)
        );
        assert!(
            replay.finished,
            "a rejected reset must not reopen the replay"
        );
    }
}
