use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::io::{Read as _, Seek as _, Write as _};
use std::os::fd::OwnedFd;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use crate::root::RepoRoot;

mod filesystem;
mod repository;

pub(in crate::qualification::runtime) use repository::{BoundRepository, RepositoryBinding};

use filesystem::{
    artifact_entry_matches, create_staging_directory, directory_entry_is_missing,
    directory_entry_matches, directory_names, ensure_directory_chain, open_directory_at,
    open_existing_directories, open_existing_directories_if_present, open_or_create_directories,
    same_file, sync_directory_chain,
};

const OUTPUT_PREFIX: [&str; 3] = ["target", "benchmarks", "qualification"];
const PUBLICATION_LOCK: &str = ".publication.lock";
const MAX_ARTIFACT_BYTES: usize = 64 << 20;
const MAX_DIRECTORY_ENTRIES: usize = 16;
const MAX_DIRECT_ARTIFACT_NAME_BYTES: usize = 128;
const ARTIFACT_NAMES: [&str; 3] = ["preflight.json", "report.json", "report.md"];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct DirectQualificationArtifactPath(PathBuf);

impl DirectQualificationArtifactPath {
    pub(crate) fn try_new(path: &Path) -> Result<Self, ArtifactError> {
        let components = validate_output(path)?;
        if components.len() != OUTPUT_PREFIX.len() + 1 {
            return Err(ArtifactError::NonDirectArtifact(path.to_path_buf()));
        }
        let Some(name) = components.last().and_then(|component| component.to_str()) else {
            return Err(ArtifactError::NonDirectArtifact(path.to_path_buf()));
        };
        if name.len() > MAX_DIRECT_ARTIFACT_NAME_BYTES
            || !name
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(ArtifactError::NonDirectArtifact(path.to_path_buf()));
        }
        Ok(Self(components.into_iter().collect()))
    }

    pub(crate) fn as_path(&self) -> &Path {
        &self.0
    }

    pub(crate) fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

#[derive(Debug)]
pub(crate) struct QualificationOutput {
    relative: PathBuf,
    repository_path: PathBuf,
    repository: OwnedFd,
    parent_components: Vec<OsString>,
    parent: OwnedFd,
    staging: OwnedFd,
    staging_name: OsString,
    staged_children: BTreeMap<&'static str, OwnedFd>,
    staged_artifacts: BTreeMap<&'static str, BoundArtifact>,
    target_name: OsString,
    bound_target: Option<BoundDirectory>,
    bound_siblings: BTreeMap<OsString, BoundDirectory>,
    staging_active: bool,
    _lock: OwnedFd,
}

#[derive(Debug)]
struct BoundDirectory {
    descriptor: OwnedFd,
    artifacts: BTreeMap<&'static str, BoundArtifact>,
}

impl BoundDirectory {
    fn new(descriptor: OwnedFd) -> Self {
        Self {
            descriptor,
            artifacts: BTreeMap::new(),
        }
    }

    fn bind_exact(
        &mut self,
        name: &'static str,
        expected: &[u8],
        maximum_bytes: usize,
    ) -> Result<(), ArtifactError> {
        let expected_digest = super::run::sha256_hex(expected);
        self.bind(
            name,
            &expected_digest,
            expected.len(),
            maximum_bytes,
            Some(expected),
        )
    }

    fn bind_digest(
        &mut self,
        name: &'static str,
        expected_sha256: &str,
        maximum_bytes: usize,
    ) -> Result<(), ArtifactError> {
        let current = read_artifact_from_directory(&self.descriptor, name, maximum_bytes)?;
        self.bind(name, expected_sha256, current.len(), maximum_bytes, None)
    }

    fn bind(
        &mut self,
        name: &'static str,
        expected_sha256: &str,
        expected_len: usize,
        maximum_bytes: usize,
        expected_bytes: Option<&[u8]>,
    ) -> Result<(), ArtifactError> {
        if let Some(bound) = self.artifacts.get(name) {
            bound.require_current(&self.descriptor, name)?;
            if bound.sha256 == expected_sha256 && bound.len == expected_len {
                return Ok(());
            }
            return Err(ArtifactError::ConcurrentReplacement(name));
        }
        let descriptor = open_artifact_at(&self.descriptor, name)?;
        let current = read_artifact_from_descriptor(&descriptor, name, maximum_bytes)?;
        if expected_bytes.is_some_and(|expected| current != expected)
            || current.len() != expected_len
            || super::run::sha256_hex(&current) != expected_sha256
        {
            return Err(ArtifactError::ConcurrentReplacement(name));
        }
        self.artifacts.insert(
            name,
            BoundArtifact {
                descriptor,
                sha256: expected_sha256.to_string(),
                len: expected_len,
                maximum_bytes,
            },
        );
        Ok(())
    }

    fn require_current(&self) -> Result<(), ArtifactError> {
        let expected = self
            .artifacts
            .keys()
            .map(|name| OsString::from(*name))
            .collect::<BTreeSet<_>>();
        let actual = directory_names(&self.descriptor, MAX_DIRECTORY_ENTRIES)?;
        if actual != expected {
            return Err(ArtifactError::BoundArtifactSetChanged { expected, actual });
        }
        for (name, artifact) in &self.artifacts {
            artifact.require_current(&self.descriptor, name)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct BoundArtifact {
    descriptor: OwnedFd,
    sha256: String,
    len: usize,
    maximum_bytes: usize,
}

#[derive(Debug)]
pub(super) struct BoundExistingDirectory {
    descriptor: OwnedFd,
    children: BTreeMap<OsString, OwnedFd>,
}

impl BoundExistingDirectory {
    fn require_current(&self, parent: &OwnedFd, name: &OsStr) -> Result<(), ArtifactError> {
        if !directory_entry_matches(parent, name, &self.descriptor)? {
            return Err(ArtifactError::DirectoryIdentity(
                "replaced qualification directory changed before cleanup",
            ));
        }
        let expected = self.children.keys().cloned().collect::<BTreeSet<_>>();
        let actual = directory_names(&self.descriptor, MAX_DIRECTORY_ENTRIES)?;
        if actual != expected {
            return Err(ArtifactError::UnexpectedExistingArtifacts(actual));
        }
        for (child_name, descriptor) in &self.children {
            if !artifact_entry_matches(&self.descriptor, child_name, descriptor)? {
                return Err(ArtifactError::DirectoryIdentity(
                    "replaced qualification artifact changed before cleanup",
                ));
            }
        }
        Ok(())
    }
}

impl BoundArtifact {
    fn require_current(
        &self,
        directory: &OwnedFd,
        name: &'static str,
    ) -> Result<(), ArtifactError> {
        let current = open_artifact_at(directory, name)?;
        if !same_file(&current, &self.descriptor)? {
            return Err(ArtifactError::ConcurrentReplacement(name));
        }
        let bytes = read_artifact_from_descriptor(&current, name, self.maximum_bytes)?;
        if bytes.len() != self.len || super::run::sha256_hex(&bytes) != self.sha256 {
            return Err(ArtifactError::ConcurrentReplacement(name));
        }
        Ok(())
    }
}

impl QualificationOutput {
    #[cfg(test)]
    pub(crate) fn require_absent(
        root: &RepoRoot,
        relative: &DirectQualificationArtifactPath,
    ) -> Result<(), ArtifactError> {
        let repository = RepositoryBinding::open(root)?;
        Self::require_absent_with_repository(root, &repository, relative)
    }

    pub(crate) fn require_absent_with_repository(
        root: &RepoRoot,
        repository: &RepositoryBinding,
        relative: &DirectQualificationArtifactPath,
    ) -> Result<(), ArtifactError> {
        repository.require_current(root)?;
        let components = validate_output(relative.as_path())?;
        let (target_name, parent_components) = components
            .split_last()
            .ok_or_else(|| ArtifactError::InvalidOutput(relative.as_path().to_path_buf()))?;
        let Some(parent) =
            open_existing_directories_if_present(&repository.descriptor, parent_components)?
        else {
            repository.require_current(root)?;
            return Ok(());
        };
        let result = require_missing_target(&parent, target_name, relative.as_path());
        repository.require_current(root)?;
        result
    }

    #[cfg(test)]
    pub(crate) fn begin(
        root: &RepoRoot,
        relative: &DirectQualificationArtifactPath,
    ) -> Result<Self, ArtifactError> {
        let repository = RepositoryBinding::open(root)?;
        Self::begin_with_repository(root, &repository, relative)
    }

    pub(crate) fn begin_with_repository(
        root: &RepoRoot,
        repository: &RepositoryBinding,
        relative: &DirectQualificationArtifactPath,
    ) -> Result<Self, ArtifactError> {
        repository.require_current(root)?;
        let components = validate_output(relative.as_path())?;
        let target_name = components
            .last()
            .ok_or_else(|| ArtifactError::InvalidOutput(relative.as_path().to_path_buf()))?
            .to_os_string();
        let parent_components = components
            .get(..components.len().saturating_sub(1))
            .ok_or_else(|| ArtifactError::InvalidOutput(relative.as_path().to_path_buf()))?;
        let repository_descriptor =
            rustix::io::dup(&repository.descriptor).map_err(ArtifactError::Io)?;
        let parent = open_or_create_directories(&repository_descriptor, parent_components)?;
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
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockExclusive)
            .map_err(ArtifactError::Io)?;
        let (staging_name, staging) = create_staging_directory(&parent)?;
        let output = Self {
            relative: relative.as_path().to_path_buf(),
            repository_path: repository.path.clone(),
            repository: repository_descriptor,
            parent_components: parent_components
                .iter()
                .map(|component| (*component).to_os_string())
                .collect(),
            parent,
            staging,
            staging_name,
            staged_children: BTreeMap::new(),
            staged_artifacts: BTreeMap::new(),
            target_name,
            bound_target: None,
            bound_siblings: BTreeMap::new(),
            staging_active: true,
            _lock: lock,
        };
        repository.require_current(root)?;
        Ok(output)
    }

    #[cfg(test)]
    pub(crate) fn begin_new(
        root: &RepoRoot,
        relative: &DirectQualificationArtifactPath,
    ) -> Result<Self, ArtifactError> {
        let repository = RepositoryBinding::open(root)?;
        Self::begin_new_with_repository(root, &repository, relative)
    }

    pub(crate) fn begin_new_with_repository(
        root: &RepoRoot,
        repository: &RepositoryBinding,
        relative: &DirectQualificationArtifactPath,
    ) -> Result<Self, ArtifactError> {
        let output = Self::begin_with_repository(root, repository, relative)?;
        require_missing_target(&output.parent, &output.target_name, relative.as_path())?;
        repository.require_current(root)?;
        Ok(output)
    }

    pub(crate) fn write(&mut self, name: &'static str, bytes: &[u8]) -> Result<(), ArtifactError> {
        if !ARTIFACT_NAMES.contains(&name) {
            return Err(ArtifactError::InvalidArtifactName(name));
        }
        if self.staged_children.contains_key(name) {
            return Err(ArtifactError::DuplicateStagedArtifact(name));
        }
        if bytes.len() > MAX_ARTIFACT_BYTES {
            return Err(ArtifactError::ArtifactTooLarge {
                name,
                actual: bytes.len(),
                maximum: MAX_ARTIFACT_BYTES,
            });
        }
        let descriptor = rustix::fs::openat(
            &self.staging,
            name,
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::RUSR
                | rustix::fs::Mode::WUSR
                | rustix::fs::Mode::RGRP
                | rustix::fs::Mode::ROTH,
        )
        .map_err(ArtifactError::Io)?;
        self.staged_children.insert(name, descriptor);
        let write_result = (|| {
            let descriptor =
                self.staged_children
                    .get(name)
                    .ok_or(ArtifactError::DirectoryIdentity(
                        "new staged artifact descriptor is missing",
                    ))?;
            let duplicate = rustix::io::dup(descriptor).map_err(ArtifactError::Io)?;
            let mut file = std::fs::File::from(duplicate);
            file.write_all(bytes).map_err(ArtifactError::Write)?;
            file.sync_all().map_err(ArtifactError::Write)
        })();
        if let Err(source) = write_result {
            return Err(self.handle_write_failure(source));
        }
        let descriptor = self
            .staged_children
            .get(name)
            .ok_or(ArtifactError::DirectoryIdentity(
                "written staged artifact descriptor is missing",
            ))?;
        self.staged_artifacts.insert(
            name,
            BoundArtifact {
                descriptor: rustix::io::dup(descriptor).map_err(ArtifactError::Io)?,
                sha256: super::run::sha256_hex(bytes),
                len: bytes.len(),
                maximum_bytes: MAX_ARTIFACT_BYTES,
            },
        );
        Ok(())
    }

    fn handle_write_failure(&mut self, source: ArtifactError) -> ArtifactError {
        match self.abort_staging() {
            Ok(()) => source,
            Err(cleanup) => ArtifactError::WriteCleanup {
                write: Box::new(source),
                cleanup: Box::new(cleanup),
            },
        }
    }

    fn abort_staging(&mut self) -> Result<(), ArtifactError> {
        self.require_repository_current()?;
        ensure_directory_chain(&self.repository, &self.parent_components, &self.parent)?;
        if !directory_entry_matches(&self.parent, &self.staging_name, &self.staging)? {
            return Err(ArtifactError::DirectoryIdentity(
                "failed qualification staging directory changed before cleanup",
            ));
        }
        let expected = self
            .staged_children
            .keys()
            .map(|name| OsString::from(*name))
            .collect::<BTreeSet<_>>();
        let actual = directory_names(&self.staging, MAX_DIRECTORY_ENTRIES)?;
        if actual != expected {
            return Err(ArtifactError::UnexpectedStagedArtifacts(actual));
        }
        for (name, descriptor) in &self.staged_children {
            if !artifact_entry_matches(&self.staging, OsStr::new(name), descriptor)? {
                return Err(ArtifactError::DirectoryIdentity(
                    "failed qualification artifact changed before staging cleanup",
                ));
            }
            rustix::fs::unlinkat(&self.staging, *name, rustix::fs::AtFlags::empty())
                .map_err(ArtifactError::Io)?;
        }
        rustix::fs::fsync(&self.staging).map_err(ArtifactError::Io)?;
        if !directory_entry_matches(&self.parent, &self.staging_name, &self.staging)? {
            return Err(ArtifactError::DirectoryIdentity(
                "failed qualification staging directory changed during cleanup",
            ));
        }
        rustix::fs::unlinkat(
            &self.parent,
            &self.staging_name,
            rustix::fs::AtFlags::REMOVEDIR,
        )
        .map_err(ArtifactError::Io)?;
        sync_directory_chain(&self.repository, &self.parent_components, &self.parent)?;
        self.staging_active = false;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn commit(self) -> Result<(), ArtifactError> {
        self.commit_with_cleanup(remove_known_output)
    }

    #[cfg(test)]
    pub(crate) fn commit_new(self) -> Result<(), ArtifactError> {
        self.commit_with_cleanup_mode(false, remove_known_output)
    }

    pub(crate) fn commit_with_source_validation<F>(
        self,
        validate_source: F,
    ) -> Result<(), ArtifactError>
    where
        F: FnMut(&BoundRepository<'_>) -> Result<(), ArtifactError>,
    {
        self.commit_with_validation_and_hooks_mode(
            true,
            validate_source,
            |_| Ok(()),
            remove_known_output,
        )
    }

    pub(crate) fn commit_new_with_source_validation<F>(
        self,
        validate_source: F,
    ) -> Result<(), ArtifactError>
    where
        F: FnMut(&BoundRepository<'_>) -> Result<(), ArtifactError>,
    {
        self.commit_with_validation_and_hooks_mode(
            false,
            validate_source,
            |_| Ok(()),
            remove_known_output,
        )
    }

    #[cfg(test)]
    pub(super) fn commit_with_cleanup<F>(self, cleanup: F) -> Result<(), ArtifactError>
    where
        F: FnOnce(&OwnedFd, &BoundExistingDirectory, &OsStr) -> Result<(), ArtifactError>,
    {
        self.commit_with_cleanup_mode(true, cleanup)
    }

    #[cfg(test)]
    fn commit_with_cleanup_mode<F>(
        self,
        replace_existing: bool,
        cleanup: F,
    ) -> Result<(), ArtifactError>
    where
        F: FnOnce(&OwnedFd, &BoundExistingDirectory, &OsStr) -> Result<(), ArtifactError>,
    {
        self.commit_with_validation_and_hooks_mode(
            replace_existing,
            |_| Ok(()),
            |_| Ok(()),
            cleanup,
        )
    }

    #[cfg(test)]
    pub(super) fn commit_with_after_exchange<F>(
        self,
        replace_existing: bool,
        after_exchange: F,
    ) -> Result<(), ArtifactError>
    where
        F: FnOnce(Option<&OwnedFd>) -> Result<(), ArtifactError>,
    {
        self.commit_with_validation_and_hooks_mode(
            replace_existing,
            |_| Ok(()),
            after_exchange,
            remove_known_output,
        )
    }

    fn commit_with_validation_and_hooks_mode<ValidateSource, AfterExchange, Cleanup>(
        mut self,
        replace_existing: bool,
        mut validate_source: ValidateSource,
        after_exchange: AfterExchange,
        cleanup: Cleanup,
    ) -> Result<(), ArtifactError>
    where
        ValidateSource: FnMut(&BoundRepository<'_>) -> Result<(), ArtifactError>,
        AfterExchange: FnOnce(Option<&OwnedFd>) -> Result<(), ArtifactError>,
        Cleanup: FnOnce(&OwnedFd, &BoundExistingDirectory, &OsStr) -> Result<(), ArtifactError>,
    {
        rustix::fs::fsync(&self.staging).map_err(ArtifactError::Io)?;
        self.require_repository_current()?;
        ensure_directory_chain(&self.repository, &self.parent_components, &self.parent)?;
        self.require_staging_current(&self.staging_name)?;
        self.require_bound_target_current(&self.target_name)?;
        self.require_bound_siblings_current()?;
        let previous = if let Some(bound_target) = &self.bound_target {
            let previous = rustix::io::dup(&bound_target.descriptor).map_err(ArtifactError::Io)?;
            Some(bind_existing_output(previous)?)
        } else {
            match open_directory_at(&self.parent, &self.target_name) {
                Ok(previous) => {
                    let previous = bind_existing_output(previous)?;
                    if !directory_entry_matches(
                        &self.parent,
                        &self.target_name,
                        &previous.descriptor,
                    )? {
                        return Err(ArtifactError::DirectoryIdentity(
                            "published directory changed before replacement",
                        ));
                    }
                    Some(previous)
                }
                Err(rustix::io::Errno::NOENT) => None,
                Err(source) => return Err(ArtifactError::Io(source)),
            }
        };
        if previous.is_some() && !replace_existing {
            return Err(ArtifactError::OutputAlreadyExists(self.relative.clone()));
        }
        validate_source(&self.bound_repository())?;
        self.require_exchange_operands_current(previous.as_ref())?;
        if previous.is_some() {
            rustix::fs::renameat_with(
                &self.parent,
                &self.staging_name,
                &self.parent,
                &self.target_name,
                rustix::fs::RenameFlags::EXCHANGE,
            )
            .map_err(ArtifactError::Io)?;
            self.staging_active = false;
        } else {
            rustix::fs::renameat_with(
                &self.parent,
                &self.staging_name,
                &self.parent,
                &self.target_name,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(ArtifactError::Io)?;
            self.staging_active = false;
        }
        let previous_directory = previous.as_ref().map(|previous| &previous.descriptor);
        let publication_result = (|| {
            after_exchange(previous_directory)?;
            validate_source(&self.bound_repository())?;
            self.require_published_state(previous_directory)?;
            sync_directory_chain(&self.repository, &self.parent_components, &self.parent)?;
            validate_source(&self.bound_repository())?;
            self.require_published_state(previous_directory)
        })();
        if let Err(source) = publication_result {
            if self.rollback_publication(previous_directory).is_err() {
                return Err(ArtifactError::PublicationRollback);
            }
            return Err(source);
        }
        if let Some(previous) = previous {
            cleanup(&self.parent, &previous, &self.staging_name)?;
            sync_directory_chain(&self.repository, &self.parent_components, &self.parent)?;
            validate_source(&self.bound_repository())?;
            self.require_final_published_state()?;
        }
        Ok(())
    }

    fn require_exchange_operands_current(
        &self,
        previous: Option<&BoundExistingDirectory>,
    ) -> Result<(), ArtifactError> {
        self.require_repository_current()?;
        ensure_directory_chain(&self.repository, &self.parent_components, &self.parent)?;
        self.require_staging_current(&self.staging_name)?;
        self.require_bound_siblings_current()?;
        match previous {
            Some(previous) => {
                previous.require_current(&self.parent, &self.target_name)?;
                self.require_bound_target_current(&self.target_name)
            }
            None => require_missing_target(&self.parent, &self.target_name, &self.relative),
        }
    }

    fn require_final_published_state(&self) -> Result<(), ArtifactError> {
        self.require_repository_current()?;
        ensure_directory_chain(&self.repository, &self.parent_components, &self.parent)?;
        self.require_staging_current(&self.target_name)?;
        if !directory_entry_is_missing(&self.parent, &self.staging_name)? {
            return Err(ArtifactError::DirectoryIdentity(
                "displaced qualification directory remained after cleanup",
            ));
        }
        self.require_bound_siblings_current()
    }

    fn require_published_state(&self, previous: Option<&OwnedFd>) -> Result<(), ArtifactError> {
        self.require_repository_current()?;
        ensure_directory_chain(&self.repository, &self.parent_components, &self.parent)?;
        self.require_staging_current(&self.target_name)?;
        if let Some(previous) = previous {
            if !directory_entry_matches(&self.parent, &self.staging_name, previous)? {
                return Err(ArtifactError::DirectoryIdentity(
                    "displaced qualification directory identity changed",
                ));
            }
            self.require_bound_target_current(&self.staging_name)?;
        }
        self.require_bound_siblings_current()
    }

    fn rollback_publication(&mut self, previous: Option<&OwnedFd>) -> Result<(), ArtifactError> {
        self.require_repository_current()
            .map_err(|_| ArtifactError::PublicationRollback)?;
        ensure_directory_chain(&self.repository, &self.parent_components, &self.parent)
            .map_err(|_| ArtifactError::PublicationRollback)?;
        if !directory_entry_matches(&self.parent, &self.target_name, &self.staging)? {
            return Err(ArtifactError::PublicationRollback);
        }
        if let Some(previous) = previous {
            if !directory_entry_matches(&self.parent, &self.staging_name, previous)? {
                return Err(ArtifactError::PublicationRollback);
            }
            rustix::fs::renameat_with(
                &self.parent,
                &self.target_name,
                &self.parent,
                &self.staging_name,
                rustix::fs::RenameFlags::EXCHANGE,
            )
            .map_err(ArtifactError::Io)?;
        } else {
            if !directory_entry_is_missing(&self.parent, &self.staging_name)? {
                return Err(ArtifactError::PublicationRollback);
            }
            rustix::fs::renameat_with(
                &self.parent,
                &self.target_name,
                &self.parent,
                &self.staging_name,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(ArtifactError::Io)?;
        }
        self.staging_active = true;
        if !directory_entry_matches(&self.parent, &self.staging_name, &self.staging)? {
            return Err(ArtifactError::PublicationRollback);
        }
        if let Some(previous) = previous
            && !directory_entry_matches(&self.parent, &self.target_name, previous)?
        {
            return Err(ArtifactError::PublicationRollback);
        }
        sync_directory_chain(&self.repository, &self.parent_components, &self.parent)
            .map_err(|_| ArtifactError::PublicationRollback)
    }

    pub(crate) fn relative(&self) -> &Path {
        &self.relative
    }

    fn bound_repository(&self) -> BoundRepository<'_> {
        BoundRepository {
            path: &self.repository_path,
            descriptor: &self.repository,
            shared_descriptor: None,
        }
    }

    fn require_repository_current(&self) -> Result<(), ArtifactError> {
        self.bound_repository().require_path_current()
    }

    pub(crate) fn require_current_artifact(
        &mut self,
        name: &'static str,
        expected: &[u8],
    ) -> Result<(), ArtifactError> {
        if !ARTIFACT_NAMES.contains(&name) {
            return Err(ArtifactError::InvalidArtifactName(name));
        }
        if self.bound_target.is_none() {
            let target =
                open_directory_at(&self.parent, &self.target_name).map_err(ArtifactError::Io)?;
            if !directory_entry_matches(&self.parent, &self.target_name, &target)? {
                return Err(ArtifactError::DirectoryIdentity(
                    "qualification directory changed while it was being validated",
                ));
            }
            self.bound_target = Some(BoundDirectory::new(target));
        }
        let target = self
            .bound_target
            .as_mut()
            .ok_or(ArtifactError::DirectoryIdentity(
                "validated qualification directory identity is missing",
            ))?;
        if !directory_entry_matches(&self.parent, &self.target_name, &target.descriptor)? {
            return Err(ArtifactError::DirectoryIdentity(
                "validated qualification directory changed",
            ));
        }
        target.bind_exact(name, expected, MAX_ARTIFACT_BYTES)
    }

    pub(crate) fn require_sibling_artifact_digest(
        &mut self,
        source_path: &DirectQualificationArtifactPath,
        name: &'static str,
        expected_sha256: &str,
        maximum_bytes: usize,
    ) -> Result<(), ArtifactError> {
        if !ARTIFACT_NAMES.contains(&name) {
            return Err(ArtifactError::InvalidArtifactName(name));
        }
        DirectQualificationArtifactPath::try_new(&self.relative)?;
        let source_components = validate_output(source_path.as_path())?;
        let source_name = *source_components
            .last()
            .ok_or_else(|| ArtifactError::NonDirectArtifact(source_path.as_path().to_path_buf()))?;
        if source_name == self.target_name {
            return Err(ArtifactError::NonSiblingArtifact(
                source_path.as_path().to_path_buf(),
            ));
        }
        if !self.bound_siblings.contains_key(source_name) {
            let source = open_directory_at(&self.parent, source_name).map_err(ArtifactError::Io)?;
            if !directory_entry_matches(&self.parent, source_name, &source)? {
                return Err(ArtifactError::DirectoryIdentity(
                    "qualification source changed while it was being validated",
                ));
            }
            self.bound_siblings
                .insert(source_name.to_os_string(), BoundDirectory::new(source));
        }
        let source =
            self.bound_siblings
                .get_mut(source_name)
                .ok_or(ArtifactError::DirectoryIdentity(
                    "validated qualification source identity is missing",
                ))?;
        if !directory_entry_matches(&self.parent, source_name, &source.descriptor)? {
            return Err(ArtifactError::DirectoryIdentity(
                "validated qualification source changed",
            ));
        }
        source.bind_digest(name, expected_sha256, maximum_bytes)
    }

    fn require_bound_siblings_current(&self) -> Result<(), ArtifactError> {
        for (name, directory) in &self.bound_siblings {
            if !directory_entry_matches(&self.parent, name, &directory.descriptor)? {
                return Err(ArtifactError::DirectoryIdentity(
                    "validated qualification source changed before publication",
                ));
            }
            directory.require_current()?;
        }
        Ok(())
    }

    fn require_bound_target_current(&self, target_name: &OsStr) -> Result<(), ArtifactError> {
        let Some(target) = &self.bound_target else {
            return Ok(());
        };
        if !directory_entry_matches(&self.parent, target_name, &target.descriptor)? {
            return Err(ArtifactError::DirectoryIdentity(
                "validated qualification directory changed before publication",
            ));
        }
        target.require_current()
    }

    fn require_staging_current(&self, entry_name: &OsStr) -> Result<(), ArtifactError> {
        if !directory_entry_matches(&self.parent, entry_name, &self.staging)? {
            return Err(ArtifactError::DirectoryIdentity(
                "staged qualification directory changed",
            ));
        }
        let expected_names = self
            .staged_artifacts
            .keys()
            .map(|name| OsString::from(*name))
            .collect::<BTreeSet<_>>();
        let actual_names = directory_names(&self.staging, MAX_DIRECTORY_ENTRIES)?;
        if actual_names != expected_names {
            return Err(ArtifactError::UnexpectedStagedArtifacts(actual_names));
        }
        for (name, artifact) in &self.staged_artifacts {
            artifact.require_current(&self.staging, name)?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn read_artifact(
    root: &RepoRoot,
    relative: &DirectQualificationArtifactPath,
    name: &'static str,
) -> Result<Vec<u8>, ArtifactError> {
    read_artifact_bounded(root, relative, name, MAX_ARTIFACT_BYTES)
}

#[cfg(test)]
pub(crate) fn read_artifact_bounded(
    root: &RepoRoot,
    relative: &DirectQualificationArtifactPath,
    name: &'static str,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ArtifactError> {
    let repository = RepositoryBinding::open(root)?;
    read_artifact_bounded_with_repository(root, &repository, relative, name, maximum_bytes)
}

pub(crate) fn read_artifact_bounded_with_repository(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    relative: &DirectQualificationArtifactPath,
    name: &'static str,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ArtifactError> {
    repository.require_current(root)?;
    if !ARTIFACT_NAMES.contains(&name) {
        return Err(ArtifactError::InvalidArtifactName(name));
    }
    if maximum_bytes > MAX_ARTIFACT_BYTES {
        return Err(ArtifactError::InvalidReadLimit(maximum_bytes));
    }
    let components = validate_output(relative.as_path())?;
    let (target_name, parent_components) = components
        .split_last()
        .ok_or_else(|| ArtifactError::InvalidOutput(relative.as_path().to_path_buf()))?;
    let parent = open_existing_directories(&repository.descriptor, parent_components)?;
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
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockShared).map_err(ArtifactError::Io)?;
    let target = open_directory_at(&parent, target_name).map_err(ArtifactError::Io)?;
    let bytes = read_artifact_from_directory(&target, name, maximum_bytes)?;
    repository.require_current(root)?;
    Ok(bytes)
}

fn read_artifact_from_directory(
    directory: &OwnedFd,
    name: &'static str,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ArtifactError> {
    let descriptor = open_artifact_at(directory, name)?;
    read_artifact_from_descriptor(&descriptor, name, maximum_bytes)
}

fn open_artifact_at(directory: &OwnedFd, name: &'static str) -> Result<OwnedFd, ArtifactError> {
    rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(ArtifactError::Io)
}

fn read_artifact_from_descriptor(
    descriptor: &OwnedFd,
    name: &'static str,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ArtifactError> {
    let duplicate = rustix::io::dup(descriptor).map_err(ArtifactError::Io)?;
    let mut file = std::fs::File::from(duplicate);
    file.rewind().map_err(ArtifactError::Write)?;
    let metadata = file.metadata().map_err(ArtifactError::Write)?;
    let maximum = u64::try_from(maximum_bytes).map_err(|_| ArtifactError::SizeOverflow)?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(ArtifactError::UnsafeArtifact(name));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len()).map_err(|_| ArtifactError::SizeOverflow)?,
    );
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(ArtifactError::Write)?;
    if bytes.len() > maximum_bytes {
        return Err(ArtifactError::ArtifactTooLarge {
            name,
            actual: bytes.len(),
            maximum: maximum_bytes,
        });
    }
    Ok(bytes)
}

impl Drop for QualificationOutput {
    fn drop(&mut self) {
        if self.staging_active
            && directory_entry_matches(&self.parent, &self.staging_name, &self.staging)
                .is_ok_and(|matches| matches)
        {
            for (name, descriptor) in &self.staged_children {
                if artifact_entry_matches(&self.staging, OsStr::new(name), descriptor)
                    .is_ok_and(|matches| matches)
                {
                    let _ =
                        rustix::fs::unlinkat(&self.staging, *name, rustix::fs::AtFlags::empty());
                }
            }
            if directory_entry_matches(&self.parent, &self.staging_name, &self.staging)
                .is_ok_and(|matches| matches)
            {
                let _ = rustix::fs::unlinkat(
                    &self.parent,
                    &self.staging_name,
                    rustix::fs::AtFlags::REMOVEDIR,
                );
            }
        }
    }
}

fn validate_output(path: &Path) -> Result<Vec<&OsStr>, ArtifactError> {
    if path.is_absolute() || path.to_str().is_none() {
        return Err(ArtifactError::InvalidOutput(path.to_path_buf()));
    }
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(ArtifactError::InvalidOutput(path.to_path_buf()));
        };
        components.push(component);
    }
    if components.len() <= OUTPUT_PREFIX.len()
        || !OUTPUT_PREFIX
            .iter()
            .zip(&components)
            .all(|(expected, actual)| OsStr::new(expected) == *actual)
    {
        return Err(ArtifactError::InvalidOutput(path.to_path_buf()));
    }
    Ok(components)
}

fn require_missing_target(
    parent: &OwnedFd,
    name: &OsStr,
    relative: &Path,
) -> Result<(), ArtifactError> {
    match open_directory_at(parent, name) {
        Ok(_) | Err(rustix::io::Errno::NOTDIR | rustix::io::Errno::LOOP) => {
            Err(ArtifactError::OutputAlreadyExists(relative.to_path_buf()))
        }
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Err(source) => Err(ArtifactError::Io(source)),
    }
}

fn bind_existing_output(directory: OwnedFd) -> Result<BoundExistingDirectory, ArtifactError> {
    let names = directory_names(&directory, MAX_DIRECTORY_ENTRIES)?;
    let allowed = ARTIFACT_NAMES
        .iter()
        .map(|name| OsString::from(*name))
        .collect::<BTreeSet<_>>();
    if !names.is_subset(&allowed) {
        return Err(ArtifactError::UnexpectedExistingArtifacts(names));
    }
    let mut children = BTreeMap::new();
    for name in &names {
        let descriptor = rustix::fs::openat(
            &directory,
            name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )
        .map_err(ArtifactError::Io)?;
        let metadata_descriptor = rustix::io::dup(&descriptor).map_err(ArtifactError::Io)?;
        if !std::fs::File::from(metadata_descriptor)
            .metadata()
            .map_err(ArtifactError::Write)?
            .is_file()
        {
            return Err(ArtifactError::UnexpectedExistingArtifacts(names));
        }
        if !artifact_entry_matches(&directory, name, &descriptor)? {
            return Err(ArtifactError::DirectoryIdentity(
                "qualification artifact changed while cleanup was bound",
            ));
        }
        children.insert(name.clone(), descriptor);
    }
    Ok(BoundExistingDirectory {
        descriptor: directory,
        children,
    })
}

fn remove_known_output(
    parent: &OwnedFd,
    previous: &BoundExistingDirectory,
    previous_name: &OsStr,
) -> Result<(), ArtifactError> {
    previous.require_current(parent, previous_name)?;
    for (name, descriptor) in &previous.children {
        if !artifact_entry_matches(&previous.descriptor, name, descriptor)? {
            return Err(ArtifactError::DirectoryIdentity(
                "replaced qualification artifact changed during cleanup",
            ));
        }
        rustix::fs::unlinkat(&previous.descriptor, name, rustix::fs::AtFlags::empty())
            .map_err(ArtifactError::Io)?;
        rustix::fs::fsync(&previous.descriptor).map_err(ArtifactError::Io)?;
    }
    if !directory_names(&previous.descriptor, MAX_DIRECTORY_ENTRIES)?.is_empty() {
        return Err(ArtifactError::DirectoryIdentity(
            "replaced qualification directory gained artifacts during cleanup",
        ));
    }
    if !directory_entry_matches(parent, previous_name, &previous.descriptor)? {
        return Err(ArtifactError::DirectoryIdentity(
            "replaced qualification directory changed during cleanup",
        ));
    }
    rustix::fs::unlinkat(parent, previous_name, rustix::fs::AtFlags::REMOVEDIR)
        .map_err(ArtifactError::Io)
}

fn ensure_linux() -> Result<(), ArtifactError> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(ArtifactError::UnsupportedHost)
    }
}

#[derive(Debug, Error)]
pub(crate) enum ArtifactError {
    #[error("performance qualification artifact publication requires Linux")]
    UnsupportedHost,
    #[error(
        "qualification output must be a normal repository-relative directory below target/benchmarks/qualification: {0}"
    )]
    InvalidOutput(PathBuf),
    #[error("qualification artifact name is not source-owned: {0}")]
    InvalidArtifactName(&'static str),
    #[error("qualification artifact {name} is {actual} bytes, exceeding {maximum}")]
    ArtifactTooLarge {
        name: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("qualification output contains unexpected existing artifacts: {0:?}")]
    UnexpectedExistingArtifacts(BTreeSet<OsString>),
    #[error("qualification staging output contains unexpected artifacts: {0:?}")]
    UnexpectedStagedArtifacts(BTreeSet<OsString>),
    #[error("qualification staging output contains a duplicate artifact: {0}")]
    DuplicateStagedArtifact(&'static str),
    #[error(
        "qualification bound directory artifact set changed: expected {expected:?}, actual {actual:?}"
    )]
    BoundArtifactSetChanged {
        expected: BTreeSet<OsString>,
        actual: BTreeSet<OsString>,
    },
    #[error("qualification output contains too many existing artifacts")]
    TooManyExistingArtifacts,
    #[error("qualification producer output already exists and cannot be replaced: {0}")]
    OutputAlreadyExists(PathBuf),
    #[error("failed to reserve a unique qualification staging directory")]
    NoStagingName,
    #[error("qualification artifact filesystem operation failed: {0}")]
    Io(rustix::io::Errno),
    #[error("qualification artifact directory identity check failed: {0}")]
    DirectoryIdentity(&'static str),
    #[error("qualification repository root is not the bound live absolute directory")]
    RepositoryIdentity,
    #[error("qualification artifact publication changed state and could not be rolled back")]
    PublicationRollback,
    #[error("qualification artifact write failed: {0}")]
    Write(std::io::Error),
    #[error("{write}; qualification staging cleanup also failed: {cleanup}")]
    WriteCleanup {
        write: Box<ArtifactError>,
        cleanup: Box<ArtifactError>,
    },
    #[error("qualification artifact is not a bounded regular file: {0}")]
    UnsafeArtifact(&'static str),
    #[error("qualification artifact {0} changed while its derived report was being validated")]
    ConcurrentReplacement(&'static str),
    #[error("qualification source changed while its derived report was being published: {0}")]
    ExternalSourceChanged(&'static str),
    #[error("qualification rollup source is not a direct sibling artifact: {0}")]
    NonSiblingArtifact(PathBuf),
    #[error("qualification artifact is not a direct child of its source-owned root: {0}")]
    NonDirectArtifact(PathBuf),
    #[error("qualification artifact size cannot be represented on this host")]
    SizeOverflow,
    #[error("qualification artifact read limit exceeds the source-owned maximum: {0}")]
    InvalidReadLimit(usize),
}

#[cfg(test)]
#[path = "artifact/tests.rs"]
mod tests;
