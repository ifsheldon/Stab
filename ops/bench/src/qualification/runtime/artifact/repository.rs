use std::os::fd::{AsRawFd as _, OwnedFd};
use std::path::{Path, PathBuf};

use super::filesystem::{open_absolute_directory, same_file};
use super::{ArtifactError, ensure_linux};
use crate::root::RepoRoot;

#[derive(Debug)]
pub(in crate::qualification::runtime) struct RepositoryBinding {
    pub(super) path: PathBuf,
    pub(super) descriptor: OwnedFd,
}

impl RepositoryBinding {
    pub(in crate::qualification::runtime) fn open(root: &RepoRoot) -> Result<Self, ArtifactError> {
        ensure_linux()?;
        let descriptor = open_absolute_directory(&root.path)?;
        Ok(Self {
            path: root.path.clone(),
            descriptor,
        })
    }

    pub(in crate::qualification::runtime) fn require_current(
        &self,
        root: &RepoRoot,
    ) -> Result<(), ArtifactError> {
        self.as_bound().require_current(root)
    }

    pub(in crate::qualification::runtime) fn descriptor_root(
        &self,
        root: &RepoRoot,
    ) -> Result<RepoRoot, ArtifactError> {
        self.as_bound().descriptor_root(root)
    }

    fn as_bound(&self) -> BoundRepository<'_> {
        BoundRepository {
            path: &self.path,
            descriptor: &self.descriptor,
        }
    }
}

pub(in crate::qualification::runtime) struct BoundRepository<'a> {
    pub(super) path: &'a Path,
    pub(super) descriptor: &'a OwnedFd,
}

impl BoundRepository<'_> {
    pub(in crate::qualification::runtime) fn require_current(
        &self,
        root: &RepoRoot,
    ) -> Result<(), ArtifactError> {
        if self.path != root.path {
            return Err(ArtifactError::RepositoryIdentity);
        }
        self.require_path_current()
    }

    pub(in crate::qualification::runtime) fn descriptor_root(
        &self,
        root: &RepoRoot,
    ) -> Result<RepoRoot, ArtifactError> {
        self.require_current(root)?;
        Ok(RepoRoot {
            path: PathBuf::from(format!(
                "/proc/{}/fd/{}",
                std::process::id(),
                self.descriptor.as_raw_fd()
            )),
        })
    }

    pub(super) fn require_path_current(&self) -> Result<(), ArtifactError> {
        let current = open_absolute_directory(self.path)?;
        if same_file(&current, self.descriptor)? {
            Ok(())
        } else {
            Err(ArtifactError::RepositoryIdentity)
        }
    }
}
