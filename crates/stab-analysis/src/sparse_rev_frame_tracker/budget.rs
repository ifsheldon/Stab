use stab_model::{DemTarget, MemoryBudget};

use super::SparseReverseFrameTracker;
use crate::target_xor_set::TargetXorSet;
use crate::{AnalysisResult, ResourceKind, ResourceLimitError, ResourceOperation};

const SET_ENTRY_OVERHEAD_BYTES: u64 = size_of::<(u64, TargetXorSet)>() as u64;
const GAUGE_SET_OVERHEAD_BYTES: u64 = size_of::<TargetXorSet>() as u64;
const TARGET_BYTES: u64 = size_of::<DemTarget>() as u64;

#[derive(Clone, Copy, Debug)]
pub(super) struct TrackerBudgetPolicy {
    operation: ResourceOperation,
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
        operation: ResourceOperation,
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
            .saturating_add(self.live_bytes)
            .saturating_add(extra_bytes);
        if policy.budget.admits(u128::from(required)) {
            return Ok(());
        }
        Err(ResourceLimitError::memory_budget(
            policy.operation,
            policy.resource,
            required,
            policy.budget.bytes(),
        )
        .into())
    }
}

pub(super) fn set_entry_bytes(set: Option<&TargetXorSet>) -> u64 {
    set.map_or(0, |targets| {
        SET_ENTRY_OVERHEAD_BYTES.saturating_add((targets.len() as u64).saturating_mul(TARGET_BYTES))
    })
}

pub(super) fn gauge_set_bytes(set: &TargetXorSet) -> u64 {
    GAUGE_SET_OVERHEAD_BYTES.saturating_add((set.len() as u64).saturating_mul(TARGET_BYTES))
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
    let anticommute_bytes = (tracker.anticommutations.len() as u64)
        .saturating_mul(size_of::<super::Anticommutation>() as u64);
    map_bytes
        .saturating_add(gauge_bytes)
        .saturating_add(anticommute_bytes)
}
