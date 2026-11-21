//! Replay of records a caller retained while validating its replay input.

use stab_model::MemoryBudget;
use stab_records::DemSampleSink;

use super::batch::{SessionUse, initial_capacities, session_storage_bytes};
use super::{DemReplayTransaction, DemSamplingExecutionError, ReplayRecords};
use crate::dem_sampling::DemResourceLimitError;
use crate::dem_sampling::plan::replay_limits;
use crate::dem_sampling::replay_store::RetainedBatch;
use crate::dem_sampling::{
    DemReplayRecordStore, DemReplaySessionOptions, DemReplayStoreState, DemSamplingPlan,
};
use crate::{RunError, RunProgress, RunStatus, ShotCount};

impl DemSamplingPlan {
    /// Creates an empty store for the `expected_shots` replay records a caller's validation
    /// pass decodes, budgeted at the part of the memory budget that a replay session created
    /// with the same `options` leaves free.
    ///
    /// Retained records and that session's storage therefore stay within the one budget. A
    /// store whose records cannot fit in any retention layout, including every store of a plan
    /// whose replay session would not fit, is released from the start.
    pub fn replay_record_store(
        &self,
        options: &DemReplaySessionOptions,
        expected_shots: ShotCount,
    ) -> DemReplayRecordStore {
        let limits = &replay_limits(self, options);
        let session_bytes = initial_capacities(self, limits, SessionUse::Replay)
            .ok()
            .map(|capacities| {
                session_storage_bytes(self, capacities.output, capacities.sample, true)
            });
        let budget_bytes = limits.memory_budget().bytes();
        let (free, session_fits) = match session_bytes.and_then(|bytes| u64::try_from(bytes).ok()) {
            Some(bytes) if bytes <= budget_bytes => (budget_bytes - bytes, true),
            _ => (0, false),
        };
        DemReplayRecordStore::new(
            self.clone(),
            expected_shots.get(),
            MemoryBudget::from_bytes(free),
            session_fits,
        )
    }

    /// Admits `input_bytes` of a caller's replay-input storage (its record reader, for
    /// example) beside the smallest replay session, a one-record batch with its sampled-error
    /// planes, and returns the part of the memory budget of a replay session created with
    /// `options` that both leave free.
    ///
    /// A caller that holds the input storage and creates its session under the rest of the
    /// budget can give part of the returned bytes to other storage and still leave the session
    /// room for at least its smallest batch; the session chooses its batch shape and admits its
    /// own storage when it is created. A budget below the sum is refused as
    /// [`DemResourceKind::ActiveBatchBytes`] for that sum.
    ///
    /// [`DemResourceKind::ActiveBatchBytes`]: crate::DemResourceKind::ActiveBatchBytes
    pub fn admit_replay_input(
        &self,
        options: &DemReplaySessionOptions,
        input_bytes: u64,
    ) -> Result<MemoryBudget, DemSamplingExecutionError> {
        let budget = replay_limits(self, options).memory_budget();
        let required =
            u128::from(input_bytes).saturating_add(session_storage_bytes(self, 1, 1, true));
        match u64::try_from(required) {
            Ok(required) if required <= budget.bytes() => {
                Ok(MemoryBudget::from_bytes(budget.bytes() - required))
            }
            _ => Err(DemSamplingExecutionError::InvalidRequest(
                DemResourceLimitError::active_batch_bytes(
                    u64::try_from(required).unwrap_or(u64::MAX),
                    budget,
                )
                .into(),
            )),
        }
    }
}

impl<Sink> DemReplayTransaction<'_, '_, Sink>
where
    Sink: DemSampleSink,
{
    /// Delivers every record a store holds, as [`Self::write_packed_batch`] or
    /// [`Self::write_bit_plane_batch`] would deliver them batch by batch.
    ///
    /// A store that retained every record offered to it delivers all of them; a full store
    /// delivers the prefix it kept, after which the caller delivers the records it declined. A
    /// released store is rejected, since it no longer holds the records it was offered.
    pub fn write_retained(
        &mut self,
        records: &DemReplayRecordStore,
    ) -> Result<RunStatus, RunError<DemSamplingExecutionError, Sink::Error>> {
        let lifecycle_error = |message, committed| RunError::Engine {
            source: DemSamplingExecutionError::ReplayLifecycle { message },
            progress: RunProgress::new(committed, 0),
        };
        if records.state() == DemReplayStoreState::Released {
            return Err(lifecycle_error(
                "a released replay record store holds no records to deliver",
                self.session.committed_shots,
            ));
        }
        let mut cancelled = false;
        let delivered = records.for_each_batch(|batch| {
            let batch_records = match batch {
                RetainedBatch::ErrorMajor { words, shots } => {
                    ReplayRecords::ErrorMajor { words, shots }
                }
                RetainedBatch::Sparse {
                    error_count,
                    hits,
                    shot_starts,
                } => ReplayRecords::Sparse {
                    error_count,
                    hits,
                    shot_starts,
                },
            };
            let status = self.write_replay_records(batch_records)?;
            if status == RunStatus::Cancelled {
                cancelled = true;
                return Ok(false);
            }
            Ok(true)
        })?;
        if cancelled {
            return Ok(RunStatus::Cancelled);
        }
        if delivered != records.shot_count() {
            return Err(lifecycle_error(
                "a replay record store delivered fewer records than it retained",
                self.session.committed_shots,
            ));
        }
        Ok(RunStatus::Completed)
    }
}
