//! Pooled sensitivity-set storage and the growth each merge admits before it allocates.
//!
//! The tracker meter counts what the sets actually retain: every stored set's capacity plus its
//! map entry, the recycled-set pool, and the merge and operand buffers. A merge that may allocate
//! first admits its worst-case growth, so a wide instruction is refused before it outgrows the
//! budget rather than after.

use stab_model::{DemTarget, QubitId};

use super::TargetSets;
use super::budget::{SET_ENTRY_OVERHEAD_BYTES, TrackerMeter, buffer_bytes, set_entry_bytes};
use crate::AnalysisResult;
use crate::target_xor_set::TargetXorSet;

/// Upper bound on recycled sensitivity buffers kept between operations.
const MAX_POOLED_SETS: usize = 32;

/// Smallest non-zero capacity a growing target buffer allocates.
const MIN_GROWN_CAPACITY: usize = 4;

/// Reused merge buffers. They hold no tracker state, so clones start empty and every
/// scratch value compares equal.
#[derive(Debug, Default)]
pub(super) struct TrackerScratch {
    pub(super) merge: Vec<DemTarget>,
    pub(super) operand: Vec<DemTarget>,
    pool: Vec<TargetXorSet>,
    /// Target bytes retained by the pooled sets' buffers.
    pool_bytes: u64,
}

impl TrackerScratch {
    pub(super) fn take_set(&mut self) -> TargetXorSet {
        let set = self.pool.pop().unwrap_or_default();
        self.pool_bytes = self.pool_bytes.saturating_sub(buffer_bytes(set.capacity()));
        set
    }

    pub(super) fn recycle(&mut self, mut set: TargetXorSet) {
        if self.pool.len() < MAX_POOLED_SETS {
            set.clear();
            self.pool_bytes = self.pool_bytes.saturating_add(buffer_bytes(set.capacity()));
            self.pool.push(set);
        }
    }

    pub(super) fn take_operand(&mut self) -> Vec<DemTarget> {
        let mut operand = std::mem::take(&mut self.operand);
        operand.clear();
        operand
    }

    /// Capacity of the set the next `take_set` returns.
    pub(super) fn next_set_capacity(&self) -> usize {
        self.pool.last().map_or(0, TargetXorSet::capacity)
    }

    /// Bytes retained by the pool and the merge and operand buffers.
    #[inline]
    pub(super) fn retained_bytes(&self) -> u64 {
        self.pool_bytes
            .saturating_add(buffer_bytes(self.merge.capacity()))
            .saturating_add(buffer_bytes(self.operand.capacity()))
            .saturating_add(
                (self.pool.capacity() as u64).saturating_mul(size_of::<TargetXorSet>() as u64),
            )
    }

    /// Admits `growth` new bytes on top of the tracker's retained state; free when nothing grows.
    #[inline]
    pub(super) fn admit_growth(&self, meter: &TrackerMeter, growth: u64) -> AnalysisResult<()> {
        if growth == 0 {
            return Ok(());
        }
        meter.admit(self.retained_bytes().saturating_add(growth))
    }
}

impl Clone for TrackerScratch {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for TrackerScratch {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for TrackerScratch {}

/// Bytes a target buffer of `capacity` allocates to hold `needed` targets.
///
/// A buffer that already fits allocates nothing; otherwise amortized growth reallocates it to
/// the larger of twice its capacity, `needed`, and the minimum non-zero capacity, freeing the old
/// buffer.
#[inline]
pub(super) fn buffer_growth_bytes(capacity: usize, needed: usize) -> u64 {
    if needed <= capacity {
        return 0;
    }
    buffer_bytes(
        capacity
            .saturating_mul(2)
            .max(needed)
            .max(MIN_GROWN_CAPACITY),
    )
}

/// Worst-case bytes a buffer of unknown capacity allocates to hold `needed` targets: a buffer
/// that must grow holds fewer than `needed`, so it grows to under twice `needed`.
#[inline]
pub(super) fn unknown_buffer_growth_bytes(needed: usize) -> u64 {
    if needed == 0 {
        return 0;
    }
    buffer_growth_bytes(needed - 1, needed)
}

/// Worst-case growth of XORing `other_len` targets into the set stored at one key.
///
/// The XOR of two sorted sets is at most the sum of their lengths. A missing set is created from
/// a buffer of `fresh_capacity` and an empty one is filled in place; a non-empty set merges
/// through the scratch buffer, which then becomes the set's storage.
#[inline]
pub(super) fn xor_growth_bytes(
    existing: Option<&TargetXorSet>,
    other_len: usize,
    fresh_capacity: usize,
    merge_capacity: usize,
) -> u64 {
    match existing {
        None => {
            SET_ENTRY_OVERHEAD_BYTES.saturating_add(buffer_growth_bytes(fresh_capacity, other_len))
        }
        Some(set) if set.is_empty() => buffer_growth_bytes(set.capacity(), other_len),
        Some(set) => buffer_growth_bytes(merge_capacity, set.len().saturating_add(other_len)),
    }
}

/// Worst-case growth of toggling one target in the set stored at one key.
#[inline]
pub(super) fn xor_item_growth_bytes(existing: Option<&TargetXorSet>) -> u64 {
    match existing {
        None => SET_ENTRY_OVERHEAD_BYTES.saturating_add(buffer_growth_bytes(0, 1)),
        Some(set) => buffer_growth_bytes(set.capacity(), set.len().saturating_add(1)),
    }
}

/// XORs `targets` into one qubit's set, dropping the entry when it cancels to empty.
pub(super) fn toggle_qubit_set(
    sets: &mut TargetSets<QubitId>,
    qubit: QubitId,
    targets: &[DemTarget],
    scratch: &mut TrackerScratch,
    meter: &mut TrackerMeter,
) -> AnalysisResult<()> {
    if targets.is_empty() {
        return Ok(());
    }
    let existing = sets.get(&qubit);
    scratch.admit_growth(
        meter,
        xor_growth_bytes(
            existing,
            targets.len(),
            scratch.next_set_capacity(),
            scratch.merge.capacity(),
        ),
    )?;
    let before = set_entry_bytes(existing);
    let set = sets.entry(qubit).or_insert_with(|| scratch.take_set());
    set.xor_sorted(targets, &mut scratch.merge);
    if set.is_empty()
        && let Some(removed) = sets.remove(&qubit)
    {
        scratch.recycle(removed);
    }
    let after = set_entry_bytes(sets.get(&qubit));
    meter.adjust(before, after);
    Ok(())
}

/// Stores one qubit's recomputed set, recycling whichever buffer it displaces.
pub(super) fn replace_qubit_set(
    sets: &mut TargetSets<QubitId>,
    qubit: QubitId,
    value: TargetXorSet,
    scratch: &mut TrackerScratch,
    meter: &mut TrackerMeter,
) {
    let before = set_entry_bytes(sets.get(&qubit));
    if value.is_empty() {
        if let Some(removed) = sets.remove(&qubit) {
            scratch.recycle(removed);
        }
        scratch.recycle(value);
    } else if let Some(replaced) = sets.insert(qubit, value) {
        scratch.recycle(replaced);
    }
    let after = set_entry_bytes(sets.get(&qubit));
    meter.adjust(before, after);
}
