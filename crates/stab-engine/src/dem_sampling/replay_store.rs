//! Budgeted retention of decoded replay records between a validation pass and the replay.

use stab_model::MemoryBudget;
use stab_records::PackedShotBatchView;

use super::plan::DemSamplingPlan;
use super::replay_planes::prefers_error_major;
use super::{DemError, DemResult};

/// Most records one retained batch holds: one word per error.
const RETAINED_BATCH_SHOTS: usize = u64::BITS as usize;

/// Outcome of offering one decoded batch to a [`DemReplayRecordStore`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DemReplayRetention {
    /// The batch was copied into the store.
    Retained,
    /// The store holds no records and accepts no more, so the caller replays by decoding its
    /// input again.
    Released,
}

/// Decoded sampled-error records kept by a caller's validation pass, so that a replay which
/// must not write output before its whole input is known to be well formed need not decode the
/// input a second time.
///
/// Records are kept transposed, one 64-record word per error, which is the form error-major
/// replay consumes. A store keeps records only when every expected record fits the part of the
/// memory budget a replay session leaves free (see
/// [`DemSamplingPlan::replay_record_store`]) and when the first offered batch is dense enough
/// for error-major replay; sparse records are cheaper to decode again than to hold transposed.
/// Otherwise it is released, holds nothing, and the caller decodes its input again.
#[derive(Debug)]
pub struct DemReplayRecordStore {
    plan: DemSamplingPlan,
    /// One `error_count`-word group per retained batch.
    words: Vec<u64>,
    /// Records in each retained batch, at most 64.
    batch_shots: Vec<usize>,
    shot_count: u64,
    budget: MemoryBudget,
    released: bool,
}

impl DemReplayRecordStore {
    /// Creates a store for `expected_shots` records under `budget`, released at once when
    /// those records cannot fit.
    pub(super) fn new(plan: DemSamplingPlan, expected_shots: u64, budget: MemoryBudget) -> Self {
        let mut store = Self {
            plan,
            words: Vec::new(),
            batch_shots: Vec::new(),
            shot_count: 0,
            budget,
            released: false,
        };
        let batches = expected_shots.div_ceil(RETAINED_BATCH_SHOTS as u64);
        let fits = usize::try_from(batches).is_ok_and(|batches| {
            store.budget.admits(store.bytes_for(batches))
                && store
                    .plan
                    .error_count()
                    .checked_mul(batches)
                    .is_some_and(|words| store.words.try_reserve_exact(words).is_ok())
                && store.batch_shots.try_reserve_exact(batches).is_ok()
        });
        if !fits {
            store.release();
        }
        store
    }

    /// Copies `batch`, whose records must each hold exactly the plan's error count, into the
    /// store, or releases the store when the records would exceed its budget or the first batch
    /// is too sparse for error-major replay.
    ///
    /// A released store stays released and declines every later batch.
    pub fn try_retain(&mut self, batch: PackedShotBatchView<'_>) -> DemResult<DemReplayRetention> {
        if self.released {
            return Ok(DemReplayRetention::Released);
        }
        let error_count = self.plan.error_count();
        if batch.bits_per_shot() != error_count {
            return Err(DemError::invalid_result_format(format!(
                "DEM error record expected {error_count} bits, got {}",
                batch.bits_per_shot()
            )));
        }
        let mut first = 0;
        while first < batch.shot_count() {
            let shots = (batch.shot_count() - first).min(RETAINED_BATCH_SHOTS);
            if self.batch_shots.is_empty() && !prefers_error_major(&self.plan, batch, first, shots)
            {
                return Ok(self.release());
            }
            let batches = self.batch_shots.len().saturating_add(1);
            if !self.budget.admits(self.bytes_for(batches))
                || self.words.try_reserve(error_count).is_err()
                || self.batch_shots.try_reserve(1).is_err()
            {
                return Ok(self.release());
            }
            let start = self.words.len();
            self.words.resize(start.saturating_add(error_count), 0);
            let target = self.words.get_mut(start..).ok_or_else(store_index_error)?;
            batch
                .copy_rows_into_bit_plane_words(first, shots, target)
                .map_err(|source| DemError::invalid_result_format(source.to_string()))?;
            self.batch_shots.push(shots);
            self.shot_count = self.shot_count.saturating_add(shots as u64);
            first += shots;
        }
        Ok(DemReplayRetention::Retained)
    }

    /// Whether the store still holds every batch offered to it.
    pub const fn is_retaining(&self) -> bool {
        !self.released
    }

    /// Records held by the store.
    pub const fn shot_count(&self) -> u64 {
        self.shot_count
    }

    /// The retained batches in the order they were offered, as one word per error and the
    /// number of records those words hold.
    ///
    /// Every retained batch owns exactly its words by construction; a batch whose words were
    /// missing would end the iteration early, which the delivered record count then exposes.
    pub(super) fn batches(&self) -> impl Iterator<Item = (&[u64], usize)> {
        let error_count = self.plan.error_count();
        self.batch_shots
            .iter()
            .enumerate()
            .map_while(move |(index, shots)| {
                let start = index.checked_mul(error_count)?;
                Some((
                    self.words.get(start..start.checked_add(error_count)?)?,
                    *shots,
                ))
            })
    }

    /// Storage of `batches` retained batches and their record counts.
    fn bytes_for(&self, batches: usize) -> u128 {
        let words_per_batch = self.plan.error_count() as u128;
        (batches as u128).saturating_mul(
            words_per_batch
                .saturating_mul(std::mem::size_of::<u64>() as u128)
                .saturating_add(std::mem::size_of::<usize>() as u128),
        )
    }

    fn release(&mut self) -> DemReplayRetention {
        self.words = Vec::new();
        self.batch_shots = Vec::new();
        self.shot_count = 0;
        self.released = true;
        DemReplayRetention::Released
    }
}

fn store_index_error() -> DemError {
    DemError::invalid_sampler_compilation("DEM replay record store escaped its reserved words")
}
