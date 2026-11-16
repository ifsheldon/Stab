//! Replay of records a caller retained while validating its replay input.

use stab_model::MemoryBudget;
use stab_records::DemSampleSink;

use super::batch::{initial_capacities, session_storage_bytes};
use super::{
    DemReplayBatchStatus, DemReplayTransaction, DemSamplingExecutionError, DemSamplingRunError,
    DemSamplingRunProgress, ReplayRecords,
};
use crate::ShotCount;
use crate::dem_sampling::replay_store::RetainedBatch;
use crate::dem_sampling::{DemReplayRecordStore, DemSamplerLimits, DemSamplingPlan};

impl DemSamplingPlan {
    /// Creates an empty store for the `expected_shots` replay records a caller's validation
    /// pass decodes, budgeted at the part of `limits`' memory budget that a replay session
    /// created under the same limits leaves free.
    ///
    /// Retained records and that session's storage therefore stay within the one budget. A
    /// store whose records cannot fit in any retention layout, including every store of a plan
    /// whose replay session would not fit, is released from the start.
    pub fn replay_record_store(
        &self,
        limits: &DemSamplerLimits,
        expected_shots: ShotCount,
    ) -> DemReplayRecordStore {
        let session_bytes = initial_capacities(self, limits).ok().map(|capacities| {
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
}

impl<Sink> DemReplayTransaction<'_, '_, Sink>
where
    Sink: DemSampleSink,
{
    /// Delivers every record of a store that still retains all records offered to it, as
    /// [`Self::write_packed_batch`] or [`Self::write_bit_plane_batch`] would deliver them batch by
    /// batch.
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
            if status == DemReplayBatchStatus::Cancelled {
                cancelled = true;
                return Ok(false);
            }
            Ok(true)
        })?;
        if cancelled {
            return Ok(DemReplayBatchStatus::Cancelled);
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
