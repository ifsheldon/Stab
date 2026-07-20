use super::{RepositoryBinding, RepositoryEvidence, RollupError, require_clean_repository};
use crate::root::RepoRoot;

pub(super) fn require_current_producer(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    expected: &RepositoryEvidence,
) -> Result<(), RollupError> {
    let current = super::super::run::bound_repository_state(root, repository)?;
    require_current_producer_state(&current, expected)
}

pub(super) fn require_current_producer_state(
    current: &super::super::git::RepositoryState,
    expected: &RepositoryEvidence,
) -> Result<(), RollupError> {
    require_clean_repository(current)?;
    if current.commit != expected.commit_after {
        return Err(RollupError::RepositoryChanged {
            before: expected.commit_after.clone(),
            after: current.commit.clone(),
        });
    }
    Ok(())
}
