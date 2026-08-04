use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::fd::OwnedFd;
use std::sync::Arc;

use super::filesystem::{
    directory_entry_matches, ensure_directory_chain, open_directory_at, open_existing_directories,
};
use super::{
    ArtifactError, BoundDirectory, BoundRepository, DirectQualificationArtifactPath,
    MAX_ARTIFACT_BYTES, OUTPUT_PREFIX, PUBLICATION_LOCK, RepositoryBinding,
    read_artifact_from_directory, validate_output,
};
use crate::root::RepoRoot;

type RetainedArtifactBytes = BTreeMap<&'static str, Vec<u8>>;

#[derive(Debug)]
pub(crate) struct RetainedArtifactContext {
    repository_path: std::path::PathBuf,
    repository: Arc<OwnedFd>,
    parent_components: Vec<OsString>,
    parent: OwnedFd,
    _lock: OwnedFd,
}

impl RetainedArtifactContext {
    pub(crate) fn open(
        root: &RepoRoot,
        repository: &RepositoryBinding,
    ) -> Result<Arc<Self>, ArtifactError> {
        repository.require_current(root)?;
        let parent_components = OUTPUT_PREFIX.iter().map(OsString::from).collect::<Vec<_>>();
        let component_refs = parent_components
            .iter()
            .map(OsString::as_os_str)
            .collect::<Vec<_>>();
        let parent = open_existing_directories(&repository.descriptor, &component_refs)?;
        let lock = rustix::fs::openat(
            &parent,
            PUBLICATION_LOCK,
            rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .map_err(ArtifactError::Io)?;
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockShared)
            .map_err(ArtifactError::Io)?;
        let context = Arc::new(Self {
            repository_path: repository.path.clone(),
            repository: Arc::clone(&repository.descriptor),
            parent_components,
            parent,
            _lock: lock,
        });
        context.require_current(root)?;
        Ok(context)
    }

    pub(crate) fn read_and_bind(
        self: &Arc<Self>,
        root: &RepoRoot,
        path: &DirectQualificationArtifactPath,
        artifacts: &[(&'static str, usize)],
    ) -> Result<(Arc<RetainedArtifactDirectory>, RetainedArtifactBytes), ArtifactError> {
        let (target_name, mut target) = self.open_target(root, path)?;
        let mut bytes = BTreeMap::new();
        for &(name, maximum_bytes) in artifacts {
            validate_artifact_request(name, maximum_bytes)?;
            let current = read_artifact_from_directory(&target.descriptor, name, maximum_bytes)?;
            target.bind_exact(name, &current, maximum_bytes)?;
            if bytes.insert(name, current).is_some() {
                return Err(ArtifactError::InvalidArtifactName(name));
            }
        }
        target.require_current()?;
        let binding = Arc::new(RetainedArtifactDirectory {
            context: Arc::clone(self),
            target_name,
            target,
        });
        binding.require_current(root)?;
        Ok((binding, bytes))
    }

    pub(crate) fn bind_digests(
        self: &Arc<Self>,
        root: &RepoRoot,
        path: &DirectQualificationArtifactPath,
        artifacts: &[(&'static str, &str, usize)],
    ) -> Result<Arc<RetainedArtifactDirectory>, ArtifactError> {
        let (target_name, mut target) = self.open_target(root, path)?;
        for &(name, expected_sha256, maximum_bytes) in artifacts {
            validate_artifact_request(name, maximum_bytes)?;
            target.bind_digest(name, expected_sha256, maximum_bytes)?;
        }
        target.require_current()?;
        let binding = Arc::new(RetainedArtifactDirectory {
            context: Arc::clone(self),
            target_name,
            target,
        });
        binding.require_current(root)?;
        Ok(binding)
    }

    fn open_target(
        &self,
        root: &RepoRoot,
        path: &DirectQualificationArtifactPath,
    ) -> Result<(OsString, BoundDirectory), ArtifactError> {
        self.require_current(root)?;
        let components = validate_output(path.as_path())?;
        if components.len() != OUTPUT_PREFIX.len() + 1 {
            return Err(ArtifactError::NonDirectArtifact(
                path.as_path().to_path_buf(),
            ));
        }
        let target_name = components
            .last()
            .ok_or_else(|| ArtifactError::NonDirectArtifact(path.as_path().to_path_buf()))?
            .to_os_string();
        self.require_current(root)?;
        let target = open_directory_at(&self.parent, &target_name).map_err(ArtifactError::Io)?;
        if !directory_entry_matches(&self.parent, &target_name, &target)? {
            return Err(ArtifactError::DirectoryIdentity(
                "qualification artifact changed while it was being retained",
            ));
        }
        Ok((target_name, BoundDirectory::new(target)))
    }

    fn require_current(&self, root: &RepoRoot) -> Result<(), ArtifactError> {
        BoundRepository {
            path: &self.repository_path,
            descriptor: &self.repository,
            shared_descriptor: Some(&self.repository),
        }
        .require_current(root)?;
        ensure_directory_chain(&self.repository, &self.parent_components, &self.parent)
    }
}

#[derive(Debug)]
pub(crate) struct RetainedArtifactDirectory {
    context: Arc<RetainedArtifactContext>,
    target_name: OsString,
    target: BoundDirectory,
}

impl RetainedArtifactDirectory {
    pub(crate) fn require_current(&self, root: &RepoRoot) -> Result<(), ArtifactError> {
        self.context.require_current(root)?;
        if !directory_entry_matches(
            &self.context.parent,
            &self.target_name,
            &self.target.descriptor,
        )? {
            return Err(ArtifactError::DirectoryIdentity(
                "retained qualification artifact directory changed",
            ));
        }
        self.target.require_current()?;
        self.context.require_current(root)
    }
}

fn validate_artifact_request(
    name: &'static str,
    maximum_bytes: usize,
) -> Result<(), ArtifactError> {
    if !super::ARTIFACT_NAMES.contains(&name) {
        return Err(ArtifactError::InvalidArtifactName(name));
    }
    if maximum_bytes > MAX_ARTIFACT_BYTES {
        return Err(ArtifactError::InvalidReadLimit(maximum_bytes));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
