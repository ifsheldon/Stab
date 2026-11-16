//! Budgeted retention of decoded replay records between a validation pass and the replay.

use stab_model::MemoryBudget;
use stab_records::{BitPlaneBatchView, PackedShotBatchView};

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

#[derive(Debug)]
enum StoreLayout {
    /// No records offered yet; the first batch selects error-major bit planes or sparse hits.
    Empty,
    /// One `error_count`-word group per retained batch of at most 64 records.
    ErrorMajor {
        words: Vec<u64>,
        batch_shots: Vec<usize>,
    },
    /// Set-error ordinals of sparse records, indexed by `shot_starts[shot]..shot_starts[shot + 1]`.
    Sparse {
        hits: Vec<usize>,
        shot_starts: Vec<usize>,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) enum RetainedBatch<'a> {
    ErrorMajor {
        words: &'a [u64],
        shots: usize,
    },
    Sparse {
        error_count: usize,
        hits: &'a [usize],
        shot_starts: &'a [usize],
    },
}

/// Decoded sampled-error records kept by a caller's validation pass, so that a replay which
/// must not write output before its whole input is known to be well formed need not decode the
/// input a second time.
///
/// Dense shot-major batches and [`BitPlaneBatchView`] groups are kept as one 64-record word per
/// error, which error-major replay consumes directly; sparse shot-major batches are kept as set
/// error ordinals per record, which per-record replay consumes without scanning zero words. Every
/// allocation is admitted against the part of the memory budget a replay session leaves free (see
/// [`DemSamplingPlan::replay_record_store`]); when the records would exceed that budget, the
/// store releases its storage, holds nothing, and the caller decodes its input again.
#[derive(Debug)]
pub struct DemReplayRecordStore {
    plan: DemSamplingPlan,
    expected_shots: u64,
    layout: StoreLayout,
    shot_count: u64,
    budget: MemoryBudget,
    released: bool,
}

impl DemReplayRecordStore {
    /// Creates a store for `expected_shots` records under `budget`, released at once when the
    /// session does not fit or neither retention layout could hold `expected_shots` records.
    pub(super) fn new(
        plan: DemSamplingPlan,
        expected_shots: u64,
        budget: MemoryBudget,
        session_fits: bool,
    ) -> Self {
        let mut store = Self {
            plan,
            expected_shots,
            layout: StoreLayout::Empty,
            shot_count: 0,
            budget,
            released: false,
        };
        if !session_fits {
            store.release();
            return store;
        }
        if expected_shots == 0 {
            return store;
        }
        let expected_batches = expected_shots.div_ceil(RETAINED_BATCH_SHOTS as u64);
        let fits_error_major = usize::try_from(expected_batches).is_ok_and(|batches| {
            store.budget.admits(store.error_major_bytes(batches))
                && store.plan.error_count().checked_mul(batches).is_some()
        });
        let fits_sparse = usize::try_from(expected_shots)
            .ok()
            .and_then(|shots| shots.checked_add(1))
            .is_some_and(|starts| store.budget.admits(sparse_bytes(starts, 0)));
        if !fits_error_major && !fits_sparse {
            store.release();
        }
        store
    }

    /// Copies `batch`, whose records must each hold exactly the plan's error count, into the
    /// store, or releases the store when the records would exceed its budget.
    ///
    /// The first non-empty batch chooses error-major bit planes when dense enough for error-major
    /// replay and sparse set-error ordinals otherwise. A released store stays released and
    /// declines every later batch.
    pub fn try_retain(&mut self, batch: PackedShotBatchView<'_>) -> DemResult<DemReplayRetention> {
        if self.released {
            return Ok(DemReplayRetention::Released);
        }
        let error_count = self.plan.error_count();
        if batch.bits_per_shot() != error_count {
            return Err(width_mismatch_error(error_count, batch.bits_per_shot()));
        }
        let mut first = 0;
        while first < batch.shot_count() {
            let shots = (batch.shot_count() - first).min(RETAINED_BATCH_SHOTS);
            if matches!(self.layout, StoreLayout::Empty)
                && !self.init_for_batch(prefers_error_major(&self.plan, batch, first, shots))
            {
                return Ok(self.release());
            }
            let retained = match &mut self.layout {
                StoreLayout::Empty => false,
                StoreLayout::ErrorMajor { .. } => {
                    self.retain_packed_error_major(batch, first, shots)?
                }
                StoreLayout::Sparse { .. } => self.retain_packed_sparse(batch, first, shots)?,
            };
            if !retained {
                return Ok(self.release());
            }
            self.shot_count = self.shot_count.saturating_add(shots as u64);
            first += shots;
        }
        Ok(DemReplayRetention::Retained)
    }

    /// Copies `batch`, whose bit planes must match the plan's error count, directly into
    /// error-major retention without a shot-major transpose, or releases the store when the
    /// planes would exceed its budget.
    pub fn try_retain_bit_planes(
        &mut self,
        batch: BitPlaneBatchView<'_>,
    ) -> DemResult<DemReplayRetention> {
        if self.released {
            return Ok(DemReplayRetention::Released);
        }
        let error_count = self.plan.error_count();
        if batch.bits_per_shot() != error_count {
            return Err(width_mismatch_error(error_count, batch.bits_per_shot()));
        }
        let mut word_index = 0_usize;
        let mut first = 0_usize;
        while first < batch.shot_count() {
            let shots = (batch.shot_count() - first).min(RETAINED_BATCH_SHOTS);
            if matches!(self.layout, StoreLayout::Empty) && !self.init_error_major() {
                return Ok(self.release());
            }
            if !self.retain_plane_word_column(batch, word_index, shots)? {
                return Ok(self.release());
            }
            self.shot_count = self.shot_count.saturating_add(shots as u64);
            first += shots;
            word_index += 1;
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

    /// Visits every retained batch in order, stopping early when `visit` returns `Ok(false)`.
    ///
    /// Returns the number of records delivered to `visit` calls that returned `Ok(true)`.
    pub(super) fn for_each_batch<E>(
        &self,
        mut visit: impl FnMut(RetainedBatch<'_>) -> Result<bool, E>,
    ) -> Result<u64, E> {
        let error_count = self.plan.error_count();
        let mut delivered = 0_u64;
        match &self.layout {
            StoreLayout::Empty => {}
            StoreLayout::ErrorMajor { words, batch_shots } => {
                for (index, &shots) in batch_shots.iter().enumerate() {
                    let Some(start) = index.checked_mul(error_count) else {
                        break;
                    };
                    let Some(end) = start.checked_add(error_count) else {
                        break;
                    };
                    let Some(batch_words) = words.get(start..end) else {
                        break;
                    };
                    if !visit(RetainedBatch::ErrorMajor {
                        words: batch_words,
                        shots,
                    })? {
                        return Ok(delivered);
                    }
                    delivered = delivered.saturating_add(shots as u64);
                }
            }
            StoreLayout::Sparse { hits, shot_starts } => {
                let total_shots = shot_starts.len().saturating_sub(1);
                let mut first = 0_usize;
                while first < total_shots {
                    let shots = (total_shots - first).min(RETAINED_BATCH_SHOTS);
                    let Some(starts) = shot_starts.get(first..=first + shots) else {
                        break;
                    };
                    if !visit(RetainedBatch::Sparse {
                        error_count,
                        hits,
                        shot_starts: starts,
                    })? {
                        return Ok(delivered);
                    }
                    delivered = delivered.saturating_add(shots as u64);
                    first += shots;
                }
            }
        }
        Ok(delivered)
    }

    fn init_for_batch(&mut self, error_major_first: bool) -> bool {
        if error_major_first {
            return self.init_error_major() || self.init_sparse();
        }
        self.init_sparse() || self.init_error_major()
    }

    fn init_error_major(&mut self) -> bool {
        let expected_batches = self.expected_shots.div_ceil(RETAINED_BATCH_SHOTS as u64);
        let Ok(batches) = usize::try_from(expected_batches) else {
            return false;
        };
        let batches = batches.max(1);
        if !self.budget.admits(self.error_major_bytes(batches)) {
            return false;
        }
        let Some(word_cap) = self.plan.error_count().checked_mul(batches) else {
            return false;
        };
        let mut words = Vec::new();
        let mut batch_shots = Vec::new();
        if words.try_reserve_exact(word_cap).is_err()
            || batch_shots.try_reserve_exact(batches).is_err()
        {
            return false;
        }
        self.layout = StoreLayout::ErrorMajor { words, batch_shots };
        true
    }

    fn init_sparse(&mut self) -> bool {
        let Some(starts_cap) = usize::try_from(self.expected_shots)
            .ok()
            .and_then(|shots| shots.max(1).checked_add(1))
        else {
            return false;
        };
        if !self.budget.admits(sparse_bytes(starts_cap, 0)) {
            return false;
        }
        let mut shot_starts = Vec::new();
        if shot_starts.try_reserve_exact(starts_cap).is_err() {
            return false;
        }
        shot_starts.push(0);
        self.layout = StoreLayout::Sparse {
            hits: Vec::new(),
            shot_starts,
        };
        true
    }

    fn retain_packed_error_major(
        &mut self,
        batch: PackedShotBatchView<'_>,
        first: usize,
        shots: usize,
    ) -> DemResult<bool> {
        let error_count = self.plan.error_count();
        let Some((words_cap, batches_cap)) = self.reserve_error_major_slot() else {
            return Ok(false);
        };
        let _ = (words_cap, batches_cap);
        let StoreLayout::ErrorMajor { words, batch_shots } = &mut self.layout else {
            return Ok(false);
        };
        let start = words.len();
        words.resize(start.saturating_add(error_count), 0);
        let target = words.get_mut(start..).ok_or_else(store_index_error)?;
        batch
            .copy_rows_into_bit_plane_words(first, shots, target)
            .map_err(|source| DemError::invalid_result_format(source.to_string()))?;
        batch_shots.push(shots);
        Ok(true)
    }

    fn retain_plane_word_column(
        &mut self,
        batch: BitPlaneBatchView<'_>,
        word_index: usize,
        shots: usize,
    ) -> DemResult<bool> {
        let error_count = self.plan.error_count();
        let Some(_) = self.reserve_error_major_slot() else {
            return Ok(false);
        };
        let StoreLayout::ErrorMajor { words, batch_shots } = &mut self.layout else {
            return Ok(false);
        };
        let start = words.len();
        words.resize(start.saturating_add(error_count), 0);
        let target = words.get_mut(start..).ok_or_else(store_index_error)?;
        if error_count > 0 {
            batch
                .copy_word_column_into(word_index, target)
                .map_err(|source| DemError::invalid_result_format(source.to_string()))?;
        }
        batch_shots.push(shots);
        Ok(true)
    }

    /// Ensures `ErrorMajor` can hold one more batch within `self.budget`, returning the active
    /// capacities or `None` when the next batch would exceed `self.budget`.
    fn reserve_error_major_slot(&mut self) -> Option<(usize, usize)> {
        let error_count = self.plan.error_count();
        let budget = self.budget;
        let StoreLayout::ErrorMajor { words, batch_shots } = &mut self.layout else {
            return None;
        };
        let next_batches = batch_shots.len().checked_add(1)?;
        let needed_words = next_batches.checked_mul(error_count)?;
        let words_cap = words.capacity().max(needed_words);
        let batches_cap = batch_shots.capacity().max(next_batches);
        let allocated_bytes = (words_cap as u128)
            .saturating_mul(std::mem::size_of::<u64>() as u128)
            .saturating_add(
                (batches_cap as u128).saturating_mul(std::mem::size_of::<usize>() as u128),
            );
        if !budget.admits(allocated_bytes) {
            return None;
        }
        if needed_words > words.capacity()
            && words.try_reserve_exact(needed_words - words.len()).is_err()
        {
            return None;
        }
        if next_batches > batch_shots.capacity()
            && batch_shots
                .try_reserve_exact(next_batches - batch_shots.len())
                .is_err()
        {
            return None;
        }
        Some((words.capacity(), batch_shots.capacity()))
    }

    fn retain_packed_sparse(
        &mut self,
        batch: PackedShotBatchView<'_>,
        first: usize,
        shots: usize,
    ) -> DemResult<bool> {
        let error_count = self.plan.error_count();
        let mut chunk_hits = 0_usize;
        for row_index in first..first + shots {
            let row = batch
                .shot(row_index)
                .map_err(|source| DemError::invalid_result_format(source.to_string()))?;
            chunk_hits = chunk_hits
                .checked_add(count_row_ones(row.words(), error_count))
                .ok_or_else(store_index_error)?;
        }
        let budget = self.budget;
        let StoreLayout::Sparse { hits, shot_starts } = &mut self.layout else {
            return Ok(false);
        };
        let Some(needed_starts) = shot_starts.len().checked_add(shots) else {
            return Ok(false);
        };
        let Some(needed_hits) = hits.len().checked_add(chunk_hits) else {
            return Ok(false);
        };
        let starts_cap = if needed_starts <= shot_starts.capacity() {
            shot_starts.capacity()
        } else {
            needed_starts
        };
        let hits_cap = if needed_hits <= hits.capacity() {
            hits.capacity()
        } else {
            let doubled = hits.capacity().saturating_mul(2).max(needed_hits).max(64);
            if budget.admits(sparse_bytes(starts_cap, doubled)) {
                doubled
            } else {
                needed_hits
            }
        };
        if !budget.admits(sparse_bytes(starts_cap, hits_cap)) {
            return Ok(false);
        }
        if starts_cap > shot_starts.capacity()
            && shot_starts
                .try_reserve_exact(starts_cap - shot_starts.len())
                .is_err()
        {
            return Ok(false);
        }
        if hits_cap > hits.capacity() && hits.try_reserve_exact(hits_cap - hits.len()).is_err() {
            return Ok(false);
        }
        for row_index in first..first + shots {
            let row = batch
                .shot(row_index)
                .map_err(|source| DemError::invalid_result_format(source.to_string()))?;
            append_row_hits(row.words(), error_count, hits)?;
            shot_starts.push(hits.len());
        }
        Ok(true)
    }

    /// Storage of `batches` error-major batches and their record counts.
    fn error_major_bytes(&self, batches: usize) -> u128 {
        let words_per_batch = self.plan.error_count() as u128;
        (batches as u128).saturating_mul(
            words_per_batch
                .saturating_mul(std::mem::size_of::<u64>() as u128)
                .saturating_add(std::mem::size_of::<usize>() as u128),
        )
    }

    fn release(&mut self) -> DemReplayRetention {
        self.layout = StoreLayout::Empty;
        self.shot_count = 0;
        self.released = true;
        DemReplayRetention::Released
    }
}

fn sparse_bytes(starts_cap: usize, hits_cap: usize) -> u128 {
    (starts_cap as u128)
        .saturating_add(hits_cap as u128)
        .saturating_mul(std::mem::size_of::<usize>() as u128)
}

fn count_row_ones(words: &[u64], bit_len: usize) -> usize {
    let full_words = bit_len / RETAINED_BATCH_SHOTS;
    let tail_bits = bit_len % RETAINED_BATCH_SHOTS;
    let tail_mask = (1_u64 << tail_bits).wrapping_sub(1);
    let full: usize = words
        .get(..full_words)
        .unwrap_or(words)
        .iter()
        .map(|word| word.count_ones() as usize)
        .sum();
    let tail = words
        .get(full_words)
        .map_or(0, |word| (word & tail_mask).count_ones() as usize);
    full.saturating_add(tail)
}

fn append_row_hits(words: &[u64], bit_len: usize, hits: &mut Vec<usize>) -> DemResult<()> {
    let used_words = bit_len.div_ceil(RETAINED_BATCH_SHOTS);
    let partial_word = bit_len / RETAINED_BATCH_SHOTS;
    let partial_mask = (1_u64 << (bit_len % RETAINED_BATCH_SHOTS)).wrapping_sub(1);
    for (word_index, &raw) in words.get(..used_words).unwrap_or(words).iter().enumerate() {
        let mut bits = if word_index == partial_word {
            raw & partial_mask
        } else {
            raw
        };
        if bits == 0 {
            continue;
        }
        let word_start = word_index
            .checked_mul(RETAINED_BATCH_SHOTS)
            .ok_or_else(store_index_error)?;
        while bits != 0 {
            let ordinal = word_start
                .checked_add(bits.trailing_zeros() as usize)
                .ok_or_else(store_index_error)?;
            bits &= bits - 1;
            hits.push(ordinal);
        }
    }
    Ok(())
}

fn width_mismatch_error(expected: usize, actual: usize) -> DemError {
    DemError::invalid_result_format(format!(
        "DEM error record expected {expected} bits, got {actual}"
    ))
}

fn store_index_error() -> DemError {
    DemError::invalid_sampler_compilation("DEM replay record store escaped its reserved words")
}
