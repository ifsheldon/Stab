use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::filesystem::{open_absolute_directory, same_file};
use super::{ArtifactError, ensure_linux};
use crate::root::RepoRoot;

#[derive(Debug)]
pub(in crate::qualification::runtime) struct RepositoryBinding {
    pub(super) path: PathBuf,
    pub(super) descriptor: Arc<OwnedFd>,
}

impl RepositoryBinding {
    pub(in crate::qualification::runtime) fn open(root: &RepoRoot) -> Result<Self, ArtifactError> {
        ensure_linux()?;
        let descriptor = open_absolute_directory(&root.path)?;
        Ok(Self {
            path: root.path.clone(),
            descriptor: Arc::new(descriptor),
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
            descriptor: self.descriptor.as_ref(),
            shared_descriptor: Some(&self.descriptor),
        }
    }
}

pub(in crate::qualification::runtime) struct BoundRepository<'a> {
    pub(super) path: &'a Path,
    pub(super) descriptor: &'a OwnedFd,
    pub(super) shared_descriptor: Option<&'a Arc<OwnedFd>>,
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
        if let Some(descriptor) = self.shared_descriptor {
            return Ok(RepoRoot::from_shared_retained_descriptor(Arc::clone(
                descriptor,
            )));
        }
        let descriptor = rustix::io::dup(self.descriptor).map_err(ArtifactError::Io)?;
        Ok(RepoRoot::from_retained_descriptor(descriptor))
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
