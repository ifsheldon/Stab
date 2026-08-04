use std::collections::BTreeMap;
use std::sync::Arc;

use super::{CompletionError, RollupReplayEvidence, canonical_json};
use crate::qualification::runtime::correctness::CorrectnessArtifactBinding;

#[derive(Default)]
pub(super) struct RetainedBindings {
    retained: BTreeMap<Vec<u8>, Arc<CorrectnessArtifactBinding>>,
}

impl RetainedBindings {
    pub(super) fn admit(
        &mut self,
        rollup: &mut RollupReplayEvidence,
    ) -> Result<(), CompletionError> {
        if rollup.correctness_bindings.len() != 1 {
            return Err(CompletionError::GroupCorrectness(rollup.group_id.clone()));
        }
        let identity = canonical_json(&rollup.correctness_preflight)?;
        let binding = rollup
            .correctness_bindings
            .pop()
            .ok_or_else(|| CompletionError::GroupCorrectness(rollup.group_id.clone()))?;
        binding.require_current()?;
        self.retained.entry(identity).or_insert(binding);
        Ok(())
    }

    pub(super) fn into_values(self) -> Vec<Arc<CorrectnessArtifactBinding>> {
        self.retained.into_values().collect()
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.retained.len()
    }
}
