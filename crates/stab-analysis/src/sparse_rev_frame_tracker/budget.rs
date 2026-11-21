use stab_model::{DemTarget, MemoryBudget};

use super::SparseReverseFrameTracker;
use crate::target_xor_set::TargetXorSet;
use crate::{AnalysisOperation, AnalysisResult, ResourceKind, ResourceLimitError};

pub(super) const SET_ENTRY_OVERHEAD_BYTES: u64 = size_of::<(u64, TargetXorSet)>() as u64;
const GAUGE_SET_OVERHEAD_BYTES: u64 = size_of::<TargetXorSet>() as u64;
const TARGET_BYTES: u64 = size_of::<DemTarget>() as u64;

#[derive(Clone, Copy, Debug)]
pub(super) struct TrackerBudgetPolicy {
    operation: AnalysisOperation,
    resource: ResourceKind,
    budget: MemoryBudget,
}

/// Live-state byte counter and optional caller memory budget.
///
/// Neither the cached byte count nor the policy is mathematical tracker state, so equality
/// compares every meter equal while cloning copies both to probe trackers.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TrackerMeter {
    pub(super) live_bytes: u64,
    /// Bytes concurrently held by the owning operation outside this tracker (e.g. output regions).
    pub(super) base_bytes: u64,
    /// Bytes concurrently held by sibling recurrence-probe trackers on the undo stack.
    pub(super) sibling_tracker_bytes: u64,
    /// Bytes of an operand buffer taken out of the scratch or a map while it is merged.
    pub(super) held_bytes: u64,
    pub(super) policy: Option<TrackerBudgetPolicy>,
}

impl PartialEq for TrackerMeter {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for TrackerMeter {}

impl TrackerMeter {
    pub(super) fn set_policy(
        &mut self,
        operation: AnalysisOperation,
        resource: ResourceKind,
        budget: MemoryBudget,
    ) {
        self.policy = Some(TrackerBudgetPolicy {
            operation,
            resource,
            budget,
        });
    }

    #[inline]
    pub(super) fn adjust(&mut self, before: u64, after: u64) {
        self.live_bytes = self.live_bytes.saturating_sub(before).saturating_add(after);
    }

    #[inline]
    pub(super) fn admit(&self, extra_bytes: u64) -> AnalysisResult<()> {
        let Some(policy) = self.policy else {
            return Ok(());
        };
        let required = self
            .base_bytes
            .saturating_add(self.sibling_tracker_bytes)
            .saturating_add(self.held_bytes)
            .saturating_add(self.live_bytes)
            .saturating_add(extra_bytes);
        if policy.budget.admits(u128::from(required)) {
            return Ok(());
        }
        Err(ResourceLimitError::memory_budget(
            policy.operation,
            policy.resource,
            required,
            policy.budget,
        )
        .into())
    }
}

/// Conservative retained bytes of one entry of a unitary-repeat slot transform.
const SLOT_ENTRY_BYTES: u64 = 3 * size_of::<usize>() as u64;

pub(super) fn slot_transform_bytes(entries: usize) -> u64 {
    (entries as u64).saturating_mul(SLOT_ENTRY_BYTES)
}

/// Bytes of the seeded basis tracker of a unitary repeat with `slot_count` slots.
pub(super) fn seeded_slot_bytes(slot_count: usize) -> u64 {
    (slot_count as u64).saturating_mul(SET_ENTRY_OVERHEAD_BYTES.saturating_add(TARGET_BYTES))
}

/// Worst-case bytes of `targets` sensitivity targets merged into `sets` fresh sets: every set
/// and the shared merge buffer may grow to under twice its final size.
pub(super) fn target_run_bytes(targets: u64, sets: usize) -> u64 {
    targets
        .saturating_mul(4)
        .saturating_add((sets as u64).saturating_mul(8))
        .saturating_mul(TARGET_BYTES)
        .saturating_add((sets as u64).saturating_mul(SET_ENTRY_OVERHEAD_BYTES))
}

/// Bytes of a target buffer with room for `capacity` targets.
#[inline]
pub(super) fn buffer_bytes(capacity: usize) -> u64 {
    (capacity as u64).saturating_mul(TARGET_BYTES)
}

/// Retained bytes of one stored set: its map entry and its buffer's capacity.
#[inline]
pub(super) fn set_entry_bytes(set: Option<&TargetXorSet>) -> u64 {
    set.map_or(0, |targets| {
        SET_ENTRY_OVERHEAD_BYTES.saturating_add(buffer_bytes(targets.capacity()))
    })
}

pub(super) fn gauge_set_bytes(set: &TargetXorSet) -> u64 {
    GAUGE_SET_OVERHEAD_BYTES.saturating_add(buffer_bytes(set.capacity()))
}

/// Bytes of `count` recorded anticommutations.
pub(super) fn anticommutation_bytes(count: usize) -> u64 {
    (count as u64).saturating_mul(size_of::<super::Anticommutation>() as u64)
}

/// Clones a tracker for a recurrence probe.
///
/// Cloned sets keep only their length's capacity, so the clone's meter is recomputed rather than
/// copied; its scratch starts empty.
impl Clone for SparseReverseFrameTracker {
    fn clone(&self) -> Self {
        let mut clone = Self {
            xs: self.xs.clone(),
            zs: self.zs.clone(),
            qubit_count: self.qubit_count,
            rec_bits: self.rec_bits.clone(),
            measurement_count: self.measurement_count,
            detector_count: self.detector_count,
            observable_effects: self.observable_effects.clone(),
            anticommutation_policy: self.anticommutation_policy,
            error_analysis_mode: self.error_analysis_mode,
            eliminate_detector_gauges: self.eliminate_detector_gauges,
            gauge_errors: self.gauge_errors.clone(),
            anticommutations: self.anticommutations.clone(),
            scratch: self.scratch.clone(),
            meter: self.meter,
            cancellation: self.cancellation.clone(),
        };
        clone.meter.live_bytes = recompute_live_bytes(&clone);
        clone
    }
}

pub(super) fn recompute_live_bytes(tracker: &SparseReverseFrameTracker) -> u64 {
    let map_bytes = tracker
        .xs
        .values()
        .chain(tracker.zs.values())
        .chain(tracker.rec_bits.values())
        .chain(tracker.observable_effects.values())
        .fold(0_u64, |acc, set| {
            acc.saturating_add(set_entry_bytes(Some(set)))
        });
    let gauge_bytes = tracker
        .gauge_errors
        .iter()
        .fold(0_u64, |acc, set| acc.saturating_add(gauge_set_bytes(set)));
    let anticommute_bytes = anticommutation_bytes(tracker.anticommutations.len());
    map_bytes
        .saturating_add(gauge_bytes)
        .saturating_add(anticommute_bytes)
}
