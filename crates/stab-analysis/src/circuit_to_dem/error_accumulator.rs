//! Accumulates error-class probabilities keyed by symptom targets and tag.
//!
//! Every recorded error looks its class up once by hashing a borrowed target slice. New classes
//! copy their targets into one arena, so a lookup never allocates a key. Classes are ordered by
//! `(targets, tag)` only when the model is emitted, which reproduces the ordered-map iteration
//! that pinned Stim's `std::map` (and the previous `BTreeMap`) produced.

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};

use hashbrown::HashTable;
use stab_model::{DemTarget, MemoryBudget, Probability};

use super::probabilities::xor_probability;
use crate::{AnalysisError, AnalysisOperation, AnalysisResult, ResourceKind, ResourceLimitError};

/// Conservative factor covering geometric `Vec` growth and `HashTable` load-factor slack while
/// keeping admission strictly monotonic in the caller's memory budget.
const ARENA_SLACK_FACTOR: u64 = 2;

/// Interned error tag; the untagged class uses [`TagId::UNTAGGED`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TagId(u32);

impl TagId {
    pub(super) const UNTAGGED: Self = Self(0);
}

/// Index of one accumulated error class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ClassId(usize);

#[derive(Debug)]
struct ErrorClass {
    start: usize,
    len: usize,
    tag: TagId,
    probability: Probability,
    /// Removed classes keep their arena slot; a later merge into the same key revives them.
    live: bool,
}

#[derive(Debug)]
pub(super) struct ErrorAccumulator {
    targets: Vec<DemTarget>,
    classes: Vec<ErrorClass>,
    index: HashTable<usize>,
    hash_builder: RandomState,
    /// Tag bytes for `TagId(n)` at position `n - 1`.
    tags: Vec<Box<[u8]>>,
    tag_ids: HashMap<Box<[u8]>, TagId>,
    interned_tag_bytes: u64,
    base_bytes: u64,
    budget: MemoryBudget,
}

impl ErrorAccumulator {
    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self::with_budget(MemoryBudget::from_bytes(u64::MAX))
    }

    pub(super) fn with_budget(budget: MemoryBudget) -> Self {
        Self {
            targets: Vec::new(),
            classes: Vec::new(),
            index: HashTable::new(),
            hash_builder: RandomState::new(),
            tags: Vec::new(),
            tag_ids: HashMap::new(),
            interned_tag_bytes: 0,
            base_bytes: 0,
            budget,
        }
    }

    /// Updates the bytes concurrently held by the analyzer's emitted model and reverse tracker.
    pub(super) fn set_base_bytes(&mut self, base_bytes: u64) {
        self.base_bytes = base_bytes;
    }

    /// Deterministic conservative retained bytes of the accumulator's live arena, classes, hash
    /// index, and interned tags.
    pub(super) fn admitted_bytes(&self) -> u64 {
        Self::arena_bytes(self.targets.len(), self.classes.len())
            .saturating_add(self.interned_tag_bytes)
    }

    fn arena_bytes(target_len: usize, class_len: usize) -> u64 {
        let target_bytes = (target_len as u64)
            .saturating_mul(size_of::<DemTarget>() as u64)
            .saturating_mul(ARENA_SLACK_FACTOR);
        let per_class_bytes = (size_of::<ErrorClass>() + size_of::<usize>() + 1) as u64;
        let class_bytes = (class_len as u64)
            .saturating_mul(per_class_bytes)
            .saturating_mul(ARENA_SLACK_FACTOR);
        target_bytes.saturating_add(class_bytes)
    }

    fn admit_accumulator_bytes(&self, accumulator_bytes: u64) -> AnalysisResult<()> {
        let total = self.base_bytes.saturating_add(accumulator_bytes);
        if self.budget.admits(u128::from(total)) {
            return Ok(());
        }
        Err(ResourceLimitError::memory_budget(
            AnalysisOperation::CircuitToDetectorErrorModel,
            ResourceKind::MaterializedBytes,
            total,
            self.budget.bytes(),
        )
        .into())
    }

    pub(super) fn intern_tag(&mut self, tag: Option<&[u8]>) -> AnalysisResult<TagId> {
        let Some(tag) = tag else {
            return Ok(TagId::UNTAGGED);
        };
        if let Some(id) = self.tag_ids.get(tag) {
            return Ok(*id);
        }
        let id = u32::try_from(self.tags.len())
            .ok()
            .and_then(|count| count.checked_add(1))
            .map(TagId)
            .ok_or_else(|| {
                AnalysisError::internal(
                    AnalysisOperation::CircuitToDetectorErrorModel,
                    "error analysis interned more distinct tags than fit u32",
                )
            })?;
        let entry_overhead = (size_of::<Box<[u8]>>() + size_of::<(Box<[u8]>, TagId)>()) as u64;
        let added_tag_bytes = (tag.len() as u64)
            .saturating_mul(2)
            .saturating_add(entry_overhead);
        let next_tag_bytes = self.interned_tag_bytes.saturating_add(added_tag_bytes);
        self.admit_accumulator_bytes(
            Self::arena_bytes(self.targets.len(), self.classes.len())
                .saturating_add(next_tag_bytes),
        )?;
        self.tags.push(Box::from(tag));
        self.tag_ids.insert(Box::from(tag), id);
        self.interned_tag_bytes = next_tag_bytes;
        Ok(id)
    }

    pub(super) fn tag_bytes(&self, tag: TagId) -> Option<&[u8]> {
        let position = usize::try_from(tag.0).ok()?.checked_sub(1)?;
        self.tags.get(position).map(AsRef::as_ref)
    }

    pub(super) fn targets(&self, class: ClassId) -> AnalysisResult<&[DemTarget]> {
        let entry = self.classes.get(class.0).ok_or_else(missing_class)?;
        self.entry_targets(entry)
    }

    pub(super) fn tag(&self, class: ClassId) -> AnalysisResult<TagId> {
        self.classes
            .get(class.0)
            .map(|entry| entry.tag)
            .ok_or_else(missing_class)
    }

    fn entry_targets(&self, entry: &ErrorClass) -> AnalysisResult<&[DemTarget]> {
        self.targets
            .get(entry.start..entry.start + entry.len)
            .ok_or_else(|| {
                AnalysisError::internal(
                    AnalysisOperation::CircuitToDetectorErrorModel,
                    "error class targets fall outside the accumulator arena",
                )
            })
    }

    pub(super) fn probability(&self, class: ClassId) -> AnalysisResult<Probability> {
        self.classes
            .get(class.0)
            .filter(|entry| entry.live)
            .map(|entry| entry.probability)
            .ok_or_else(missing_class)
    }

    /// Merges an independent error into its class with pinned Stim's fused XOR rounding.
    pub(super) fn merge_independent(
        &mut self,
        targets: &[DemTarget],
        tag: TagId,
        probability: Probability,
    ) -> AnalysisResult<()> {
        let hash = self.hash_key(targets, tag);
        let found = {
            let arena = &self.targets;
            let classes = &self.classes;
            self.index
                .find(hash, |&class| {
                    class_matches(arena, classes, class, targets, tag)
                })
                .copied()
        };
        if let Some(class) = found {
            let entry = self.classes.get_mut(class).ok_or_else(missing_class)?;
            entry.probability = if entry.live {
                xor_probability(entry.probability, probability)?
            } else {
                probability
            };
            entry.live = true;
            return Ok(());
        }
        let next_targets = self.targets.len().saturating_add(targets.len());
        let next_classes = self.classes.len().saturating_add(1);
        self.admit_accumulator_bytes(
            Self::arena_bytes(next_targets, next_classes).saturating_add(self.interned_tag_bytes),
        )?;
        let start = self.targets.len();
        self.targets.extend_from_slice(targets);
        let class = self.classes.len();
        self.classes.push(ErrorClass {
            start,
            len: targets.len(),
            tag,
            probability,
            live: true,
        });
        let arena = &self.targets;
        let classes = &self.classes;
        let hash_builder = &self.hash_builder;
        self.index.insert_unique(hash, class, |&existing| {
            classes.get(existing).map_or(0, |entry| {
                let targets = arena
                    .get(entry.start..entry.start + entry.len)
                    .unwrap_or(&[]);
                hash_with(hash_builder, targets, entry.tag)
            })
        });
        Ok(())
    }

    /// Removes a live class and returns its probability and tag.
    pub(super) fn remove(&mut self, class: ClassId) -> AnalysisResult<(Probability, TagId)> {
        let entry = self
            .classes
            .get_mut(class.0)
            .filter(|entry| entry.live)
            .ok_or_else(missing_class)?;
        entry.live = false;
        Ok((entry.probability, entry.tag))
    }

    /// Live classes in ascending `(targets, tag)` order.
    pub(super) fn sorted_classes(&self) -> AnalysisResult<Vec<ClassId>> {
        let live_count = self.classes.iter().filter(|entry| entry.live).count();
        let sort_bytes =
            (live_count as u64)
                .saturating_mul(size_of::<(&[DemTarget], Option<&[u8]>, ClassId)>() as u64);
        self.admit_accumulator_bytes(self.admitted_bytes().saturating_add(sort_bytes))?;
        let mut keyed = Vec::with_capacity(live_count);
        for (class, entry) in self.classes.iter().enumerate() {
            if entry.live {
                keyed.push((
                    self.entry_targets(entry)?,
                    self.tag_bytes(entry.tag),
                    ClassId(class),
                ));
            }
        }
        // Keys are unique among live classes, so the unstable sort is deterministic.
        keyed.sort_unstable_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)));
        Ok(keyed.into_iter().map(|(_, _, class)| class).collect())
    }

    /// Forgets every class while releasing arena allocations that would push resident memory
    /// alongside `base_bytes` past the budget.
    pub(super) fn clear(&mut self, base_bytes: u64) {
        self.targets.clear();
        self.classes.clear();
        self.index.clear();
        self.base_bytes = base_bytes;
        let resident_capacity_bytes = (self.targets.capacity() as u64)
            .saturating_mul(size_of::<DemTarget>() as u64)
            .saturating_add(
                (self.classes.capacity() as u64).saturating_mul(size_of::<ErrorClass>() as u64),
            )
            .saturating_add(self.index.allocation_size() as u64)
            .saturating_add(self.interned_tag_bytes);
        if !self.budget.admits(u128::from(
            base_bytes.saturating_add(resident_capacity_bytes),
        )) {
            self.targets = Vec::new();
            self.classes = Vec::new();
            self.index = HashTable::new();
        }
    }

    fn hash_key(&self, targets: &[DemTarget], tag: TagId) -> u64 {
        hash_with(&self.hash_builder, targets, tag)
    }
}

fn class_matches(
    arena: &[DemTarget],
    classes: &[ErrorClass],
    class: usize,
    targets: &[DemTarget],
    tag: TagId,
) -> bool {
    classes.get(class).is_some_and(|entry| {
        entry.tag == tag
            && entry.len == targets.len()
            && arena.get(entry.start..entry.start + entry.len) == Some(targets)
    })
}

fn hash_with(hash_builder: &RandomState, targets: &[DemTarget], tag: TagId) -> u64 {
    let mut hasher = hash_builder.build_hasher();
    hasher.write_u32(tag.0);
    hasher.write_usize(targets.len());
    DemTarget::hash_slice(targets, &mut hasher);
    hasher.finish()
}

fn missing_class() -> AnalysisError {
    AnalysisError::internal(
        AnalysisOperation::CircuitToDetectorErrorModel,
        "error decomposition lost a planned rewrite class",
    )
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "accumulator fixtures use direct assertions for compact diagnostics"
    )]

    use std::collections::BTreeMap;

    use proptest::prelude::*;
    use stab_model::{DemTarget, Probability};

    use super::{ErrorAccumulator, TagId};
    use crate::circuit_to_dem::probabilities::xor_probability;

    type Key = (Vec<DemTarget>, Option<Box<[u8]>>);

    proptest! {
        /// The accumulator matches the ordered map it replaced: same merged probabilities
        /// (merge order included) and the same `(targets, tag)` iteration order.
        #[test]
        fn accumulator_matches_ordered_map_model(
            errors in prop::collection::vec(
                (
                    prop::collection::btree_set(0_u64..6, 0..4),
                    prop::option::of(prop::collection::vec(any::<u8>(), 0..3)),
                    0.0_f64..0.5,
                    any::<bool>(),
                ),
                0..64,
            )
        ) {
            let mut accumulator = ErrorAccumulator::new();
            let mut model = BTreeMap::<Key, Probability>::new();
            for (detectors, tag, probability, remove_first) in errors {
                let targets = detectors
                    .into_iter()
                    .map(|id| DemTarget::relative_detector(id).expect("detector"))
                    .collect::<Vec<_>>();
                let probability = Probability::try_new(probability).expect("probability");
                let tag_id = accumulator.intern_tag(tag.as_deref()).expect("tag");
                let key: Key = (targets.clone(), tag.map(Vec::into_boxed_slice));
                if remove_first
                    && let Some(class) = accumulator
                        .sorted_classes()
                        .expect("sorted classes")
                        .into_iter()
                        .find(|&class| {
                            accumulator.targets(class).expect("targets") == targets.as_slice()
                                && accumulator.tag(class).expect("tag") == tag_id
                        })
                {
                    let (removed, _) = accumulator.remove(class).expect("live class");
                    prop_assert_eq!(Some(removed), model.remove(&key));
                }
                accumulator
                    .merge_independent(&targets, tag_id, probability)
                    .expect("merge");
                let merged = match model.get(&key) {
                    Some(existing) => xor_probability(*existing, probability).expect("xor"),
                    None => probability,
                };
                model.insert(key, merged);
            }
            let actual = accumulator
                .sorted_classes()
                .expect("sorted classes")
                .into_iter()
                .map(|class| {
                    (
                        (
                            accumulator.targets(class).expect("targets").to_vec(),
                            accumulator
                                .tag_bytes(accumulator.tag(class).expect("tag"))
                                .map(Box::from),
                        ),
                        accumulator.probability(class).expect("live").get().to_bits(),
                    )
                })
                .collect::<Vec<_>>();
            let expected = model
                .into_iter()
                .map(|(key, probability)| (key, probability.get().to_bits()))
                .collect::<Vec<_>>();
            prop_assert_eq!(actual, expected);
            prop_assert_eq!(accumulator.tag_bytes(TagId::UNTAGGED), None);
        }
    }
}
