use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use super::{CompletionError, RollupReplayEvidence, canonical_json};
use crate::qualification::runtime::artifact::{
    DirectQualificationArtifactPath, RetainedArtifactContext, RetainedArtifactDirectory,
};
use crate::qualification::runtime::correctness::CorrectnessArtifactBinding;
use crate::root::RepoRoot;

#[derive(Default)]
pub(super) struct RetainedBindings {
    retained: BTreeMap<Vec<u8>, Arc<CorrectnessArtifactBinding>>,
}

pub(super) struct RetainedRollupArtifacts {
    bindings: Vec<Arc<RetainedArtifactDirectory>>,
}

impl RetainedRollupArtifacts {
    pub(super) fn bind(
        root: &RepoRoot,
        context: &Arc<RetainedArtifactContext>,
        rollup: &RollupReplayEvidence,
    ) -> Result<Self, CompletionError> {
        let mut bindings = Vec::with_capacity(rollup.sources.len().saturating_add(1));
        let rollup_path = DirectQualificationArtifactPath::try_new(&rollup.output)?;
        bindings.push(context.bind_digests(
            root,
            &rollup_path,
            &[
                (
                    "report.json",
                    &rollup.report_sha256,
                    super::super::rollup::MAX_ROLLUP_REPORT_BYTES,
                ),
                (
                    "preflight.json",
                    &rollup.preflight_sha256,
                    super::super::rollup::MAX_ROLLUP_PREFLIGHT_BYTES,
                ),
                (
                    "report.md",
                    &rollup.markdown_sha256,
                    super::super::rollup::MAX_ROLLUP_MARKDOWN_BYTES,
                ),
            ],
        )?);
        for source in &rollup.sources {
            let source_path = DirectQualificationArtifactPath::try_new(Path::new(&source.path))?;
            bindings.push(context.bind_digests(
                root,
                &source_path,
                &[
                    (
                        "report.json",
                        &source.report_sha256,
                        super::super::report::MAX_PUBLISHED_REPORT_BYTES,
                    ),
                    (
                        "preflight.json",
                        &source.preflight_sha256,
                        super::super::report::MAX_PUBLISHED_PREFLIGHT_BYTES,
                    ),
                    (
                        "report.md",
                        &source.markdown_sha256,
                        super::super::report::MAX_PUBLISHED_MARKDOWN_BYTES,
                    ),
                ],
            )?);
        }
        Ok(Self { bindings })
    }

    pub(super) fn require_current(&self, root: &RepoRoot) -> Result<(), CompletionError> {
        for binding in &self.bindings {
            binding.require_current(root)?;
        }
        Ok(())
    }
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
