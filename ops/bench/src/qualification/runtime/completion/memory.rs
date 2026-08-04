use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{CompletionError, CompletionScope};
use crate::qualification::runtime::artifact::{DirectQualificationArtifactPath, RepositoryBinding};
use crate::qualification::runtime::git::RepositoryState;
use crate::qualification::runtime::probe::{DemMemoryReceiptEvidence, inspect_memory_receipt};
use crate::qualification::runtime::rollup::RollupReplayEvidence;
use crate::root::RepoRoot;

pub(super) const RELEASE_SOFT_NOFILE_LIMIT: u64 = 1024;
pub(super) const ACCEPTED_MAXIMUM_MEMORY_GROUPS: [&str; 2] =
    [super::scope::DEM_PARSE_GROUP, super::scope::DEM_PRINT_GROUP];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompletionAcceptedMaximumMemoryReceipt {
    pub(super) group_id: String,
    pub(super) path: String,
    pub(super) report_sha256: String,
}

pub(super) fn admit_paths(
    output: &DirectQualificationArtifactPath,
    rollups: &[DirectQualificationArtifactPath],
    paths: &[PathBuf],
) -> Result<Vec<DirectQualificationArtifactPath>, CompletionError> {
    if paths.len() != ACCEPTED_MAXIMUM_MEMORY_GROUPS.len() {
        return Err(CompletionError::MemoryReceiptCount(paths.len()));
    }
    let mut unique = rollups.iter().cloned().collect::<BTreeSet<_>>();
    unique.insert(output.clone());
    paths
        .iter()
        .map(|path| {
            let path = DirectQualificationArtifactPath::try_new(path)?;
            if !unique.insert(path.clone()) {
                return Err(CompletionError::DuplicatePath(path.into_path_buf()));
            }
            Ok(path)
        })
        .collect()
}

pub(super) fn collect(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    paths: &[DirectQualificationArtifactPath],
    shared: &RollupReplayEvidence,
    repository_state: &RepositoryState,
) -> Result<Vec<DemMemoryReceiptEvidence>, CompletionError> {
    let mut by_group = BTreeMap::new();
    for path in paths {
        let receipt = inspect_memory_receipt(root, source_root, repository, path)?;
        let group_id = receipt.runtime_group_id.clone();
        if by_group.insert(group_id.clone(), receipt).is_some() {
            return Err(CompletionError::DuplicateMemoryReceipt(group_id));
        }
    }
    let mut ordered = Vec::with_capacity(ACCEPTED_MAXIMUM_MEMORY_GROUPS.len());
    for group_id in ACCEPTED_MAXIMUM_MEMORY_GROUPS {
        let receipt = by_group
            .remove(group_id)
            .ok_or_else(|| CompletionError::MissingMemoryReceipt(group_id.to_string()))?;
        if receipt.repository.commit_before != repository_state.commit
            || receipt.repository.commit_after != repository_state.commit
            || receipt.host.policy_sha256 != shared.host_policy_sha256
            || receipt.host.profile_id != shared.host_profile_id
            || receipt.host.operating_system != shared.operating_system
            || receipt.host.architecture != shared.architecture
            || receipt.host.cpu_identity != shared.cpu_identity
            || receipt.probe.stim_source_sha256 != shared.workers.stim_source_sha256
            || receipt.probe.stim_build_fingerprint != shared.workers.stim_build_fingerprint
            || receipt.probe.stim_binary_sha256 != shared.workers.stim_binary_sha256
            || receipt.probe.stab_source_sha256 != shared.workers.stab_source_sha256
            || receipt.probe.stab_build_fingerprint != shared.workers.stab_build_fingerprint
            || receipt.probe.stab_binary_sha256 != shared.workers.stab_binary_sha256
        {
            return Err(CompletionError::MemoryReceiptIdentity(group_id.to_string()));
        }
        ordered.push(receipt);
    }
    if !by_group.is_empty() {
        return Err(CompletionError::MemoryReceiptCount(paths.len()));
    }
    Ok(ordered)
}

pub(super) fn require_nofile_limit(scope: &CompletionScope) -> Result<u64, CompletionError> {
    let current = rustix::process::getrlimit(rustix::process::Resource::Nofile).current;
    validate_nofile_limit(scope, current)
}

pub(super) fn validate_nofile_limit(
    scope: &CompletionScope,
    current: Option<u64>,
) -> Result<u64, CompletionError> {
    let current = current.ok_or(CompletionError::DescriptorLimit {
        expected: RELEASE_SOFT_NOFILE_LIMIT,
        actual: None,
    })?;
    if scope.id == super::scope::RELEASE_SCOPE_ID && current != RELEASE_SOFT_NOFILE_LIMIT {
        return Err(CompletionError::DescriptorLimit {
            expected: RELEASE_SOFT_NOFILE_LIMIT,
            actual: Some(current),
        });
    }
    Ok(current)
}
