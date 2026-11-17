//! Sorted sparse XOR sets of DEM targets.
//!
//! Sensitivity regions are small sorted sets that are mostly combined by symmetric difference,
//! so they are stored as sorted vectors like pinned Stim's `SparseXorVec` and merged in one
//! linear pass into reused buffers.

use std::cmp::Ordering;

use stab_model::DemTarget;

/// A sorted, duplicate-free set of DEM targets combined by symmetric difference.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TargetXorSet {
    sorted: Vec<DemTarget>,
}

impl TargetXorSet {
    pub(crate) const fn new() -> Self {
        Self { sorted: Vec::new() }
    }

    #[cfg(test)]
    pub(crate) fn from_target(target: DemTarget) -> Self {
        Self {
            sorted: vec![target],
        }
    }

    pub(crate) fn as_slice(&self) -> &[DemTarget] {
        &self.sorted
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sorted.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.sorted.len()
    }

    /// Targets the set's buffer holds without reallocating.
    pub(crate) fn capacity(&self) -> usize {
        self.sorted.capacity()
    }

    pub(crate) fn iter(&self) -> std::slice::Iter<'_, DemTarget> {
        self.sorted.iter()
    }

    pub(crate) fn contains(&self, target: &DemTarget) -> bool {
        self.sorted.binary_search(target).is_ok()
    }

    pub(crate) fn clear(&mut self) {
        self.sorted.clear();
    }

    /// Toggles one target.
    pub(crate) fn xor_item(&mut self, target: DemTarget) {
        match self.sorted.binary_search(&target) {
            Ok(position) => {
                self.sorted.remove(position);
            }
            Err(position) => self.sorted.insert(position, target),
        }
    }

    /// Replaces this set with `self ^ other`, using `scratch` as the merge buffer.
    ///
    /// `other` must be sorted and duplicate-free. The previous storage becomes the new scratch
    /// buffer, so repeated merges reuse capacity instead of allocating.
    pub(crate) fn xor_sorted(&mut self, other: &[DemTarget], scratch: &mut Vec<DemTarget>) {
        if other.is_empty() {
            return;
        }
        if self.sorted.is_empty() {
            self.sorted.extend_from_slice(other);
            return;
        }
        xor_merge_into(&self.sorted, other, scratch);
        std::mem::swap(&mut self.sorted, scratch);
    }

    /// Replaces this set with a copy of a sorted, duplicate-free slice.
    pub(crate) fn assign_sorted(&mut self, sorted: &[DemTarget]) {
        self.sorted.clear();
        self.sorted.extend_from_slice(sorted);
    }

    /// Maps every target through an order-preserving function.
    ///
    /// `map` must be strictly monotone over the stored targets, such as a uniform detector
    /// shift, so the result stays sorted without re-sorting.
    pub(crate) fn try_map_monotone<E>(
        &mut self,
        mut map: impl FnMut(DemTarget) -> Result<DemTarget, E>,
    ) -> Result<(), E> {
        for target in &mut self.sorted {
            *target = map(*target)?;
        }
        debug_assert!(self.sorted.is_sorted_by(|left, right| left < right));
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn to_btree_set(&self) -> std::collections::BTreeSet<DemTarget> {
        self.sorted.iter().copied().collect()
    }
}

impl<'a> IntoIterator for &'a TargetXorSet {
    type Item = &'a DemTarget;
    type IntoIter = std::slice::Iter<'a, DemTarget>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Writes the symmetric difference of two sorted, duplicate-free slices into `out`.
pub(crate) fn xor_merge_into(left: &[DemTarget], right: &[DemTarget], out: &mut Vec<DemTarget>) {
    out.clear();
    out.reserve(left.len() + right.len());
    let mut left = left.iter().peekable();
    let mut right = right.iter().peekable();
    loop {
        match (left.peek(), right.peek()) {
            (Some(&&l), Some(&&r)) => match l.cmp(&r) {
                Ordering::Less => {
                    out.push(l);
                    left.next();
                }
                Ordering::Greater => {
                    out.push(r);
                    right.next();
                }
                Ordering::Equal => {
                    left.next();
                    right.next();
                }
            },
            (Some(_), None) => {
                out.extend(left);
                return;
            }
            (None, Some(_)) => {
                out.extend(right);
                return;
            }
            (None, None) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use proptest::prelude::*;
    use stab_model::DemTarget;

    use super::{TargetXorSet, xor_merge_into};

    fn target(code: u8) -> DemTarget {
        // Mix detectors and observables so variant ordering is exercised.
        let value = u64::from(code % 32);
        if code >= 200 {
            DemTarget::logical_observable(value).unwrap_or(DemTarget::separator())
        } else {
            DemTarget::relative_detector(value).unwrap_or(DemTarget::separator())
        }
    }

    fn model_xor(model: &mut BTreeSet<DemTarget>, values: &BTreeSet<DemTarget>) {
        for value in values {
            if !model.remove(value) {
                model.insert(*value);
            }
        }
    }

    proptest! {
        #[test]
        fn xor_set_matches_btree_set_model(
            operations in prop::collection::vec(
                (any::<bool>(), prop::collection::vec(any::<u8>(), 0..12)),
                0..48,
            )
        ) {
            let mut set = TargetXorSet::new();
            let mut model = BTreeSet::new();
            let mut scratch = Vec::new();
            for (single, codes) in operations {
                let operand = codes.into_iter().map(target).collect::<BTreeSet<_>>();
                if single {
                    for value in &operand {
                        set.xor_item(*value);
                        model_xor(&mut model, &BTreeSet::from([*value]));
                    }
                } else {
                    let sorted = operand.iter().copied().collect::<Vec<_>>();
                    set.xor_sorted(&sorted, &mut scratch);
                    model_xor(&mut model, &operand);
                }
                prop_assert!(set.as_slice().is_sorted_by(|left, right| left < right));
                prop_assert_eq!(set.to_btree_set(), model.clone());
                prop_assert_eq!(set.is_empty(), model.is_empty());
                prop_assert_eq!(set.len(), model.len());
                for value in (0..=255).map(target) {
                    prop_assert_eq!(set.contains(&value), model.contains(&value));
                }
            }
        }

        #[test]
        fn merge_cancels_shared_targets_and_keeps_order(
            left in prop::collection::btree_set(any::<u8>(), 0..16),
            right in prop::collection::btree_set(any::<u8>(), 0..16),
        ) {
            let left = left.into_iter().map(target).collect::<BTreeSet<_>>();
            let right = right.into_iter().map(target).collect::<BTreeSet<_>>();
            let mut out = vec![target(7)];
            xor_merge_into(
                &left.iter().copied().collect::<Vec<_>>(),
                &right.iter().copied().collect::<Vec<_>>(),
                &mut out,
            );
            let expected = left.symmetric_difference(&right).copied().collect::<Vec<_>>();
            prop_assert_eq!(&out, &expected);
            let left = left.iter().copied().collect::<Vec<_>>();
            let mut cancelled = TargetXorSet::from_target(target(9));
            cancelled.assign_sorted(&left);
            cancelled.xor_sorted(&left, &mut out);
            prop_assert!(cancelled.is_empty(), "a set XOR itself cancels to empty");
        }
    }
}
