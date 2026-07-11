use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash};

use hashbrown::HashTable;

use crate::{CircuitError, CircuitResult};

#[derive(Clone, Debug)]
pub(super) struct ArenaIndex {
    entries: HashTable<usize>,
    hash_builder: RandomState,
}

impl ArenaIndex {
    pub(super) fn new() -> Self {
        Self {
            entries: HashTable::new(),
            hash_builder: RandomState::new(),
        }
    }

    pub(super) fn from_arena<T: Eq + Hash>(arena: &[T]) -> Self {
        let mut index = Self {
            entries: HashTable::with_capacity(arena.len()),
            hash_builder: RandomState::new(),
        };
        for (position, value) in arena.iter().enumerate() {
            let hash = index.hash(value);
            index.insert_reserved(hash, position, arena);
        }
        index
    }

    pub(super) fn find<T: Eq + Hash>(&self, value: &T, arena: &[T]) -> Option<usize> {
        let hash = self.hash(value);
        self.entries
            .find(hash, |index| arena.get(*index) == Some(value))
            .copied()
    }

    pub(super) fn hash<T: Hash>(&self, value: &T) -> u64 {
        self.hash_builder.hash_one(value)
    }

    pub(super) fn try_reserve<T: Hash>(
        &mut self,
        arena: &[T],
        context: &'static str,
    ) -> CircuitResult<()> {
        let hash_builder = &self.hash_builder;
        self.entries
            .try_reserve(1, |index| {
                arena
                    .get(*index)
                    .map_or(0, |value| hash_builder.hash_one(value))
            })
            .map_err(|_| {
                CircuitError::invalid_detector_error_model(format!(
                    "{context} cannot allocate another arena index entry"
                ))
            })
    }

    pub(super) fn insert_reserved<T: Hash>(&mut self, hash: u64, index: usize, arena: &[T]) {
        let hash_builder = &self.hash_builder;
        self.entries.insert_unique(hash, index, |index| {
            arena
                .get(*index)
                .map_or(0, |value| hash_builder.hash_one(value))
        });
    }
}

impl Default for ArenaIndex {
    fn default() -> Self {
        Self::new()
    }
}
