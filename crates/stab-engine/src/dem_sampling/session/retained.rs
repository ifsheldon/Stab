//! Replay of records a caller retained while validating its replay input.

use stab_model::MemoryBudget;
use stab_records::DemSampleSink;

use super::batch::{initial_capacities, session_storage_bytes};
use super::{
    DemReplayBatchStatus, DemReplayTransaction, DemSamplingExecutionError, DemSamplingRunError,
    DemSamplingRunProgress, ReplayRecords,
};
use crate::ShotCount;
use crate::dem_sampling::{DemReplayRecordStore, DemSamplerLimits, DemSamplingPlan};

impl DemSamplingPlan {
    /// Creates an empty store for the `expected_shots` replay records a caller's validation
    /// pass decodes, budgeted at the part of `limits`' memory budget that a replay session
    /// created under the same limits leaves free.
    ///
    /// Retained records and that session's storage therefore stay within the one budget. A
    /// store whose records cannot all fit, including every store of a plan whose replay session
    /// would not fit, is released from the start.
    pub fn replay_record_store(
        &self,
        limits: &DemSamplerLimits,
        expected_shots: ShotCount,
    ) -> DemReplayRecordStore {
        let session_bytes = initial_capacities(self, limits).map_or(u128::MAX, |capacities| {
            session_storage_bytes(self, capacities.output, capacities.sample, true)
        });
        // A session size beyond u64 exceeds every budget, which leaves nothing free.
        let session_bytes = u64::try_from(session_bytes).unwrap_or(u64::MAX);
        let free = limits.memory_budget().bytes().saturating_sub(session_bytes);
        DemReplayRecordStore::new(
            self.clone(),
            expected_shots.get(),
            MemoryBudget::from_bytes(free),
        )
    }
}

impl<Sink> DemReplayTransaction<'_, '_, Sink>
where
    Sink: DemSampleSink,
{
    /// Delivers every record of a store that still retains all records offered to it, as
    /// [`Self::write_packed_batch`] would deliver them batch by batch.
    ///
    /// A released store is rejected, since it no longer holds the records it was offered.
    pub fn write_retained(
        &mut self,
        records: &DemReplayRecordStore,
    ) -> Result<DemReplayBatchStatus, DemSamplingRunError<Sink::Error>> {
        let lifecycle_error = |message, committed| DemSamplingRunError::Engine {
            source: DemSamplingExecutionError::ReplayLifecycle { message },
            progress: DemSamplingRunProgress::new(committed, 0),
        };
        if !records.is_retaining() {
            return Err(lifecycle_error(
                "a released replay record store holds no records to deliver",
                self.session.committed_shots,
            ));
        }
        let mut delivered = 0_u64;
        for (words, shots) in records.batches() {
            let status = self
                .session
                .write_batch_to(ReplayRecords::ErrorMajor { words, shots }, self.sink)?;
            if status == DemReplayBatchStatus::Cancelled {
                return Ok(status);
            }
            delivered = delivered.saturating_add(shots as u64);
        }
        if delivered != records.shot_count() {
            return Err(lifecycle_error(
                "a replay record store delivered fewer records than it retained",
                self.session.committed_shots,
            ));
        }
        Ok(DemReplayBatchStatus::Accepted)
    }
}
