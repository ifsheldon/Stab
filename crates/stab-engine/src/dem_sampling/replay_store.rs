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
    /// The batch would exceed the store's budget, so none of its records were kept; the store
    /// keeps every batch it retained before and declines every later one, and the caller replays
    /// those records from the store and decodes the rest of its input again.
    Full,
    /// The store holds no records and accepts no more, so the caller replays by decoding its
    /// input again.
    Released,
}

/// Where a [`DemReplayRecordStore`] is in its lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DemReplayStoreState {
    /// The store holds every batch offered to it and accepts more.
    Retaining,
    /// The store keeps the whole batches it retained and declines every later one.
    Full,
    /// The store holds no records and accepts no more.
    Released,
}

/// Lengths of a store's vectors before one offered batch, restored when the batch does not fit.
#[derive(Clone, Copy, Debug)]
struct RetainedMark {
    words: usize,
    batches: usize,
    shot_count: u64,
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
    ///
    /// Ordinals are `u32`, so only models of at most `2^32` errors retain sparse records.
    Sparse {
        hits: Vec<u32>,
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
        hits: &'a [u32],
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
/// [`DemSamplingPlan::replay_record_store`]). When a batch would exceed that budget, the store
/// becomes full: it keeps every whole batch retained before, which stays charged against the
/// budget, declines that batch and every later one, and the caller decodes only the records after
/// the retained prefix again. A store whose records fit no layout at all releases its storage and
/// holds nothing.
#[derive(Debug)]
pub struct DemReplayRecordStore {
    plan: DemSamplingPlan,
    expected_shots: u64,
    layout: StoreLayout,
    shot_count: u64,
    budget: MemoryBudget,
    state: DemReplayStoreState,
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
            state: DemReplayStoreState::Retaining,
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
        let fits_sparse = sparse_ordinals_fit(store.plan.error_count())
            && usize::try_from(expected_shots)
                .ok()
                .and_then(|shots| shots.checked_add(1))
                .is_some_and(|starts| store.budget.admits(sparse_bytes(starts, 0)));
        if !fits_error_major && !fits_sparse {
            store.release();
        }
        store
    }

    /// Copies `batch`, whose records must each hold exactly the plan's error count, into the
    /// store, or keeps none of it and becomes full when the records would exceed its budget.
    ///
    /// The first non-empty batch chooses error-major bit planes when dense enough for error-major
    /// replay and sparse set-error ordinals otherwise, and the store releases when neither layout
    /// can be allocated for it. A full or released store declines every later batch.
    pub fn try_retain(&mut self, batch: PackedShotBatchView<'_>) -> DemResult<DemReplayRetention> {
        if let Some(declined) = self.declined() {
            return Ok(declined);
        }
        let error_count = self.plan.error_count();
        if batch.bits_per_shot() != error_count {
            return Err(width_mismatch_error(error_count, batch.bits_per_shot()));
        }
        let mark = self.mark();
        let mut first = 0;
        while first < batch.shot_count() {
            let shots = (batch.shot_count() - first).min(RETAINED_BATCH_SHOTS);
            if matches!(self.layout, StoreLayout::Empty)
                && !self.init_for_batch(batch, first, shots)?
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
                return self.fill(mark);
            }
            self.shot_count = self.shot_count.saturating_add(shots as u64);
            first += shots;
        }
        Ok(DemReplayRetention::Retained)
    }

    /// Copies `batch`, whose bit planes must match the plan's error count, directly into
    /// error-major retention without a shot-major transpose, or keeps none of it and becomes
    /// full when the planes would exceed its budget.
    pub fn try_retain_bit_planes(
        &mut self,
        batch: BitPlaneBatchView<'_>,
    ) -> DemResult<DemReplayRetention> {
        if let Some(declined) = self.declined() {
            return Ok(declined);
        }
        let error_count = self.plan.error_count();
        if batch.bits_per_shot() != error_count {
            return Err(width_mismatch_error(error_count, batch.bits_per_shot()));
        }
        let mark = self.mark();
        let mut word_index = 0_usize;
        let mut first = 0_usize;
        while first < batch.shot_count() {
            let shots = (batch.shot_count() - first).min(RETAINED_BATCH_SHOTS);
            if matches!(self.layout, StoreLayout::Empty)
                && !(self.init_error_major() || self.init_error_major_prefix())
            {
                return Ok(self.release());
            }
            if !self.retain_plane_word_column(batch, word_index, shots)? {
                return self.fill(mark);
            }
            self.shot_count = self.shot_count.saturating_add(shots as u64);
            first += shots;
            word_index += 1;
        }
        Ok(DemReplayRetention::Retained)
    }

    /// Where the store is in its lifecycle.
    pub const fn state(&self) -> DemReplayStoreState {
        self.state
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

    /// Chooses the layout for the first offered chunk, `shots` records of `batch` from `first`.
    ///
    /// Sparse chunks take sparse ordinals, or error-major words when the record starts do not
    /// fit. Dense chunks take error-major words for every expected record when they fit, sparse
    /// ordinals when the chunk's set errors projected over every expected record fit, and
    /// otherwise whichever layout costs fewer bytes per record at that density, error-major
    /// words being sized to the whole batches the budget admits, so a dense replay that just
    /// misses retention still keeps a prefix.
    fn init_for_batch(
        &mut self,
        batch: PackedShotBatchView<'_>,
        first: usize,
        shots: usize,
    ) -> DemResult<bool> {
        if !prefers_error_major(&self.plan, batch, first, shots) {
            return Ok(self.init_sparse() || self.init_error_major());
        }
        if self.init_error_major() {
            return Ok(true);
        }
        let error_count = self.plan.error_count();
        let mut chunk_hits = 0_usize;
        for row_index in first..first + shots {
            let row = batch
                .shot(row_index)
                .map_err(|source| DemError::invalid_result_format(source.to_string()))?;
            chunk_hits = chunk_hits.saturating_add(count_row_ones(row.words(), error_count));
        }
        let shots = shots.max(1) as u128;
        let ordinal_bytes = std::mem::size_of::<u32>() as u128;
        // Sparse bytes per record at this chunk's density, scaled by the chunk's record count.
        let sparse_per_shots = (std::mem::size_of::<usize>() as u128)
            .saturating_mul(shots)
            .saturating_add((chunk_hits as u128).saturating_mul(ordinal_bytes));
        let projected_sparse = sparse_per_shots
            .saturating_mul(u128::from(self.expected_shots))
            .div_ceil(shots);
        // Error-major bytes for the same number of records.
        let error_major_per_shots = self
            .error_major_bytes(1)
            .saturating_mul(shots)
            .div_ceil(RETAINED_BATCH_SHOTS as u128);
        if !self.budget.admits(projected_sparse) && error_major_per_shots < sparse_per_shots {
            return Ok(self.init_error_major_prefix() || self.init_sparse());
        }
        Ok(self.init_sparse() || self.init_error_major_prefix())
    }

    fn init_error_major(&mut self) -> bool {
        let expected_batches = self.expected_shots.div_ceil(RETAINED_BATCH_SHOTS as u64);
        let Ok(batches) = usize::try_from(expected_batches) else {
            return false;
        };
        self.init_error_major_batches(batches.max(1))
    }

    /// Reserves error-major words for as many whole batches of the expected records as the
    /// budget admits, at least one.
    fn init_error_major_prefix(&mut self) -> bool {
        let expected_batches = self.expected_shots.div_ceil(RETAINED_BATCH_SHOTS as u64);
        let admitted = u128::from(self.budget.bytes()) / self.error_major_bytes(1).max(1);
        let batches = u128::from(expected_batches).min(admitted);
        usize::try_from(batches)
            .is_ok_and(|batches| batches > 0 && self.init_error_major_batches(batches))
    }

    fn init_error_major_batches(&mut self, batches: usize) -> bool {
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
        if !sparse_ordinals_fit(self.plan.error_count()) {
            return false;
        }
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
                // Growing by exactly the next batch would reallocate on every batch, so the last
                // growth takes every ordinal the budget admits beside the record starts.
                sparse_hits_capacity(budget, starts_cap).max(needed_hits)
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
        self.state = DemReplayStoreState::Released;
        DemReplayRetention::Released
    }

    /// The outcome every batch offered to a full or released store receives.
    const fn declined(&self) -> Option<DemReplayRetention> {
        match self.state {
            DemReplayStoreState::Retaining => None,
            DemReplayStoreState::Full => Some(DemReplayRetention::Full),
            DemReplayStoreState::Released => Some(DemReplayRetention::Released),
        }
    }

    fn mark(&self) -> RetainedMark {
        let (words, batches) = match &self.layout {
            StoreLayout::Empty => (0, 0),
            StoreLayout::ErrorMajor { words, batch_shots } => (words.len(), batch_shots.len()),
            StoreLayout::Sparse { hits, shot_starts } => (hits.len(), shot_starts.len()),
        };
        RetainedMark {
            words,
            batches,
            shot_count: self.shot_count,
        }
    }

    /// Drops every record of the batch offered since `mark` and declines later batches, keeping
    /// the whole batches retained before it; a store that still holds nothing releases instead.
    fn fill(&mut self, mark: RetainedMark) -> DemResult<DemReplayRetention> {
        if mark.shot_count == 0 {
            return Ok(self.release());
        }
        match &mut self.layout {
            StoreLayout::Empty => return Err(store_index_error()),
            StoreLayout::ErrorMajor { words, batch_shots } => {
                words.truncate(mark.words);
                batch_shots.truncate(mark.batches);
            }
            StoreLayout::Sparse { hits, shot_starts } => {
                hits.truncate(mark.words);
                shot_starts.truncate(mark.batches);
            }
        }
        self.shot_count = mark.shot_count;
        self.state = DemReplayStoreState::Full;
        Ok(DemReplayRetention::Full)
    }
}

fn sparse_bytes(starts_cap: usize, hits_cap: usize) -> u128 {
    (starts_cap as u128)
        .saturating_mul(std::mem::size_of::<usize>() as u128)
        .saturating_add((hits_cap as u128).saturating_mul(std::mem::size_of::<u32>() as u128))
}

/// The most ordinals `budget` admits beside `starts_cap` record starts.
fn sparse_hits_capacity(budget: MemoryBudget, starts_cap: usize) -> usize {
    let starts_bytes = sparse_bytes(starts_cap, 0);
    let spare = u128::from(budget.bytes()).saturating_sub(starts_bytes);
    usize::try_from(spare / std::mem::size_of::<u32>() as u128).unwrap_or(usize::MAX)
}

/// Whether every set-error ordinal of a model with `error_count` errors fits a `u32`.
fn sparse_ordinals_fit(error_count: usize) -> bool {
    error_count
        .checked_sub(1)
        .is_none_or(|last| u32::try_from(last).is_ok())
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

fn append_row_hits(words: &[u64], bit_len: usize, hits: &mut Vec<u32>) -> DemResult<()> {
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
                .and_then(|ordinal| u32::try_from(ordinal).ok())
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
