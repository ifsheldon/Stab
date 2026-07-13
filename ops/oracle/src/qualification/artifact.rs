use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;

use crate::RepoRoot;

pub(super) const DEFAULT_OUTPUT_DIR: &str = "target/qualification/correctness/latest";
pub(super) const MAX_REPORT_BYTES: usize = 32 << 20;
#[cfg(target_os = "linux")]
const MAX_CLEANUP_DEPTH: usize = 128;
#[cfg(target_os = "linux")]
const MAX_CLEANUP_ENTRIES: usize = 100_000;
#[cfg(target_os = "linux")]
static ARTIFACT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
#[cfg(all(test, target_os = "linux"))]
std::thread_local! {
    static CREATED_PARENT_SYNC_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Debug)]
pub(super) struct QualificationOutputDir {
    repository_root: PathBuf,
    relative: PathBuf,
    absolute: PathBuf,
    publish_target: Option<PathBuf>,
    #[cfg(target_os = "linux")]
    bound_directory: Option<std::fs::File>,
    #[cfg(target_os = "linux")]
    staged: Option<StagedDirectory>,
}

#[derive(Debug)]
pub(super) struct PublishedOutput {
    output: QualificationOutputDir,
    #[cfg(target_os = "linux")]
    repository: std::fs::File,
    #[cfg(target_os = "linux")]
    parent_components: Vec<std::ffi::OsString>,
    #[cfg(target_os = "linux")]
    parent: std::fs::File,
    #[cfg(target_os = "linux")]
    target_name: std::ffi::OsString,
    #[cfg(target_os = "linux")]
    _lock: PublicationLock,
}

#[cfg(target_os = "linux")]
#[derive(Debug)]
struct StagedDirectory {
    repository: std::fs::File,
    parent_components: Vec<std::ffi::OsString>,
    parent: std::fs::File,
    directory: std::fs::File,
    name: std::ffi::OsString,
    target_name: std::ffi::OsString,
}

#[derive(Debug, Error)]
pub(crate) enum ArtifactError {
    #[error("qualification output {path} is invalid: {reason}")]
    InvalidOutput { path: PathBuf, reason: &'static str },

    #[error("qualification artifact {path} is invalid")]
    InvalidArtifact { path: PathBuf },

    #[error("failed to write qualification artifact {path}: {reason}")]
    Write { path: PathBuf, reason: Box<str> },

    #[error("failed to read qualification artifact {path}: {reason}")]
    Read { path: PathBuf, reason: Box<str> },

    #[error("failed to prepare qualification run directory {path}: {reason}")]
    Prepare { path: PathBuf, reason: Box<str> },

    #[error("failed to publish qualification run directory {path}: {reason}")]
    Publish { path: PathBuf, reason: Box<str> },
}

impl QualificationOutputDir {
    pub(super) fn parse(root: &RepoRoot, value: &Path) -> Result<Self, ArtifactError> {
        if value.to_str().is_none() {
            return Err(ArtifactError::InvalidOutput {
                path: value.to_path_buf(),
                reason: "path must be valid UTF-8 so report artifact paths are serializable",
            });
        }
        if value.is_absolute() || value.components().any(unsafe_component) {
            return Err(ArtifactError::InvalidOutput {
                path: value.to_path_buf(),
                reason: "path must be repository-relative and contain only normal components",
            });
        }
        let mut components = value.components();
        if components.next() != Some(Component::Normal("target".as_ref()))
            || components.next() != Some(Component::Normal("qualification".as_ref()))
        {
            return Err(ArtifactError::InvalidOutput {
                path: value.to_path_buf(),
                reason: "path must be under target/qualification",
            });
        }
        if components.next().is_none() {
            return Err(ArtifactError::InvalidOutput {
                path: value.to_path_buf(),
                reason: "path must name a qualification run directory",
            });
        }
        Ok(Self {
            repository_root: root.path.clone(),
            relative: value.to_path_buf(),
            absolute: root.path.join(value),
            publish_target: None,
            #[cfg(target_os = "linux")]
            bound_directory: None,
            #[cfg(target_os = "linux")]
            staged: None,
        })
    }

    pub(super) fn begin_run(&self) -> Result<Self, ArtifactError> {
        #[cfg(not(target_os = "linux"))]
        {
            return Err(ArtifactError::Prepare {
                path: self.absolute.clone(),
                reason: "atomic qualification directory publication is supported only on Linux"
                    .into(),
            });
        }
        #[cfg(target_os = "linux")]
        {
            let repository =
                crate::safe_file::open_directory(&self.repository_root).map_err(|source| {
                    ArtifactError::Prepare {
                        path: self.repository_root.clone(),
                        reason: source.to_string().into_boxed_str(),
                    }
                })?;
            let parent_components =
                directory_components(self.relative.parent().ok_or_else(|| {
                    ArtifactError::Prepare {
                        path: self.absolute.clone(),
                        reason: "output directory has no parent".into(),
                    }
                })?)
                .map_err(|source| ArtifactError::Prepare {
                    path: self.absolute.clone(),
                    reason: source.to_string().into_boxed_str(),
                })?;
            let parent_descriptor = open_or_create_directories_at(&repository, &parent_components)
                .map_err(|source| ArtifactError::Prepare {
                    path: self.absolute.clone(),
                    reason: source.to_string().into_boxed_str(),
                })?;
            let (staging_name, staging_descriptor) =
                create_staging_directory_at(&parent_descriptor).map_err(|source| {
                    ArtifactError::Prepare {
                        path: self.absolute.clone(),
                        reason: source.to_string().into_boxed_str(),
                    }
                })?;
            let target_name = self
                .absolute
                .file_name()
                .ok_or_else(|| ArtifactError::Prepare {
                    path: self.absolute.clone(),
                    reason: "output directory has no file name".into(),
                })?
                .to_os_string();
            let staging = self
                .absolute
                .parent()
                .ok_or_else(|| ArtifactError::Prepare {
                    path: self.absolute.clone(),
                    reason: "output directory has no parent".into(),
                })?
                .join(&staging_name);
            Ok(Self {
                repository_root: self.repository_root.clone(),
                relative: self.relative.clone(),
                absolute: staging,
                publish_target: Some(self.absolute.clone()),
                bound_directory: None,
                staged: Some(StagedDirectory {
                    repository,
                    parent_components,
                    parent: parent_descriptor,
                    directory: staging_descriptor,
                    name: staging_name,
                    target_name,
                }),
            })
        }
    }

    pub(super) fn commit(mut self) -> Result<(), ArtifactError> {
        let Some(target) = self.publish_target.as_ref() else {
            return Err(ArtifactError::Publish {
                path: self.absolute.clone(),
                reason: "only a staged qualification run can be published".into(),
            });
        };
        #[cfg(not(target_os = "linux"))]
        {
            let _ = target;
            return Err(ArtifactError::Publish {
                path: self.absolute,
                reason: "atomic qualification directory publication is supported only on Linux"
                    .into(),
            });
        }
        #[cfg(target_os = "linux")]
        {
            let staged = self.staged.as_ref().ok_or_else(|| ArtifactError::Publish {
                path: self.absolute.clone(),
                reason: "staged directory descriptor is missing".into(),
            })?;
            publish_staged_directory(staged).map_err(|source| ArtifactError::Publish {
                path: target.clone(),
                reason: source.to_string().into_boxed_str(),
            })?;
            self.staged = None;
            self.publish_target = None;
            Ok(())
        }
    }

    pub(super) fn relative(&self) -> &Path {
        &self.relative
    }

    #[cfg(target_os = "linux")]
    pub(super) fn lock_published(&self) -> Result<PublishedOutput, ArtifactError> {
        if self.publish_target.is_some() {
            return Err(ArtifactError::Prepare {
                path: self.absolute.clone(),
                reason: "a staged run cannot be reopened as published evidence".into(),
            });
        }
        let repository =
            crate::safe_file::open_directory(&self.repository_root).map_err(|source| {
                ArtifactError::Prepare {
                    path: self.repository_root.clone(),
                    reason: source.to_string().into_boxed_str(),
                }
            })?;
        let parent_components =
            directory_components(
                self.relative
                    .parent()
                    .ok_or_else(|| ArtifactError::Prepare {
                        path: self.absolute.clone(),
                        reason: "output directory has no parent".into(),
                    })?,
            )
            .map_err(|source| ArtifactError::Prepare {
                path: self.absolute.clone(),
                reason: source.to_string().into_boxed_str(),
            })?;
        let target_name = self
            .absolute
            .file_name()
            .ok_or_else(|| ArtifactError::Prepare {
                path: self.absolute.clone(),
                reason: "output directory has no file name".into(),
            })?
            .to_os_string();
        let parent = open_directories_at(&repository, &parent_components).map_err(|source| {
            ArtifactError::Prepare {
                path: self.absolute.clone(),
                reason: source.to_string().into_boxed_str(),
            }
        })?;
        let publication_lock =
            PublicationLock::acquire(&repository).map_err(|source| ArtifactError::Prepare {
                path: self.absolute.clone(),
                reason: source.to_string().into_boxed_str(),
            })?;
        ensure_directory_chain(&repository, &parent_components, &parent).map_err(|source| {
            ArtifactError::Prepare {
                path: self.absolute.clone(),
                reason: source.to_string().into_boxed_str(),
            }
        })?;
        let directory =
            open_directory_at(&parent, &target_name).map_err(|source| ArtifactError::Prepare {
                path: self.absolute.clone(),
                reason: std::io::Error::from(source).to_string().into_boxed_str(),
            })?;
        let output = Self {
            repository_root: self.repository_root.clone(),
            relative: self.relative.clone(),
            absolute: self.absolute.clone(),
            publish_target: None,
            bound_directory: Some(directory),
            staged: None,
        };
        Ok(PublishedOutput {
            output,
            repository,
            parent_components,
            parent,
            target_name,
            _lock: publication_lock,
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub(super) fn lock_published(&self) -> Result<PublishedOutput, ArtifactError> {
        Err(ArtifactError::Prepare {
            path: self.absolute.clone(),
            reason: "locked qualification evidence is supported only on Linux".into(),
        })
    }

    pub(super) fn write(&self, relative: &Path, bytes: &[u8]) -> Result<PathBuf, ArtifactError> {
        let path = self.artifact_path(relative)?;
        #[cfg(target_os = "linux")]
        if let Some(directory) = self.directory_descriptor() {
            write_regular_file_at(directory, relative, bytes).map_err(|source| {
                ArtifactError::Write {
                    path: path.clone(),
                    reason: source.to_string().into_boxed_str(),
                }
            })?;
            return Ok(self.relative.join(relative));
        }
        crate::safe_file::atomic_write_regular_file(&path, bytes).map_err(|source| {
            ArtifactError::Write {
                path: path.clone(),
                reason: source.to_string().into_boxed_str(),
            }
        })?;
        Ok(self.relative.join(relative))
    }

    pub(super) fn read(&self, relative: &Path, limit: usize) -> Result<Vec<u8>, ArtifactError> {
        let path = self.artifact_path(relative)?;
        #[cfg(target_os = "linux")]
        if let Some(directory) = self.directory_descriptor() {
            return read_regular_file_at(directory, relative, limit).map_err(|source| {
                ArtifactError::Read {
                    path,
                    reason: source.to_string().into_boxed_str(),
                }
            });
        }
        crate::safe_file::read_regular_file_bounded(&path, limit).map_err(|source| {
            ArtifactError::Read {
                path,
                reason: source.to_string().into_boxed_str(),
            }
        })
    }

    fn artifact_path(&self, relative: &Path) -> Result<PathBuf, ArtifactError> {
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative.components().any(unsafe_component)
        {
            return Err(ArtifactError::InvalidArtifact {
                path: relative.to_path_buf(),
            });
        }
        Ok(self.absolute.join(relative))
    }

    #[cfg(target_os = "linux")]
    fn directory_descriptor(&self) -> Option<&std::fs::File> {
        self.bound_directory
            .as_ref()
            .or_else(|| self.staged.as_ref().map(|staged| &staged.directory))
    }
}

impl PublishedOutput {
    pub(super) fn output(&self) -> &QualificationOutputDir {
        &self.output
    }

    #[cfg(target_os = "linux")]
    pub(super) fn finish(self) -> Result<(), ArtifactError> {
        let directory =
            self.output
                .bound_directory
                .as_ref()
                .ok_or_else(|| ArtifactError::Prepare {
                    path: self.output.absolute.clone(),
                    reason: "published output descriptor is missing".into(),
                })?;
        ensure_directory_chain(&self.repository, &self.parent_components, &self.parent).map_err(
            |source| ArtifactError::Prepare {
                path: self.output.absolute.clone(),
                reason: source.to_string().into_boxed_str(),
            },
        )?;
        if !directory_entry_matches(&self.parent, &self.target_name, directory).map_err(
            |source| ArtifactError::Prepare {
                path: self.output.absolute.clone(),
                reason: source.to_string().into_boxed_str(),
            },
        )? {
            return Err(ArtifactError::Prepare {
                path: self.output.absolute.clone(),
                reason: "published output changed while it was locked".into(),
            });
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub(super) fn finish(self) -> Result<(), ArtifactError> {
        Err(ArtifactError::Prepare {
            path: self.output.absolute,
            reason: "locked qualification evidence is supported only on Linux".into(),
        })
    }
}

impl Drop for QualificationOutputDir {
    fn drop(&mut self) {
        if self.publish_target.is_some() {
            #[cfg(target_os = "linux")]
            if let Some(staged) = self.staged.as_ref() {
                drop(cleanup_owned_directory(
                    &staged.parent,
                    &staged.name,
                    &staged.directory,
                ));
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn publish_staged_directory(staged: &StagedDirectory) -> Result<(), std::io::Error> {
    publish_staged_directory_with_check(staged, || {
        crate::process::ensure_qualification_active().map_err(std::io::Error::other)
    })
}

#[cfg(target_os = "linux")]
fn publish_staged_directory_with_check(
    staged: &StagedDirectory,
    check_active: impl FnOnce() -> Result<(), std::io::Error>,
) -> Result<(), std::io::Error> {
    let previous = {
        let _publication_lock = PublicationLock::acquire(&staged.repository)?;
        check_active()?;
        ensure_directory_chain(
            &staged.repository,
            &staged.parent_components,
            &staged.parent,
        )?;
        if !directory_entry_matches(&staged.parent, &staged.name, &staged.directory)? {
            return Err(std::io::Error::other(
                "staging directory changed before publication",
            ));
        }
        let previous = match open_directory_at(&staged.parent, &staged.target_name) {
            Ok(previous) => {
                rustix::fs::renameat_with(
                    &staged.parent,
                    &staged.name,
                    &staged.parent,
                    &staged.target_name,
                    rustix::fs::RenameFlags::EXCHANGE,
                )
                .map_err(std::io::Error::from)?;
                if !directory_entry_matches(&staged.parent, &staged.target_name, &staged.directory)?
                    || !directory_entry_matches(&staged.parent, &staged.name, &previous)?
                {
                    return Err(std::io::Error::other(
                        "qualification directory identities changed during publication",
                    ));
                }
                Some(previous)
            }
            Err(rustix::io::Errno::NOENT) => {
                rustix::fs::renameat_with(
                    &staged.parent,
                    &staged.name,
                    &staged.parent,
                    &staged.target_name,
                    rustix::fs::RenameFlags::NOREPLACE,
                )
                .map_err(std::io::Error::from)?;
                if !directory_entry_matches(&staged.parent, &staged.target_name, &staged.directory)?
                {
                    return Err(std::io::Error::other(
                        "published qualification directory identity changed",
                    ));
                }
                None
            }
            Err(source) => return Err(source.into()),
        };
        rustix::fs::fsync(&staged.parent).map_err(std::io::Error::from)?;
        ensure_directory_chain(
            &staged.repository,
            &staged.parent_components,
            &staged.parent,
        )?;
        previous
    };
    if let Some(previous) = previous
        && cleanup_owned_directory(&staged.parent, &staged.name, &previous).is_err()
    {
        // Publication is already durable. A bounded cleanup failure leaves a quarantined
        // harness-owned directory for a later run instead of invalidating good evidence.
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[derive(Debug)]
struct PublicationLock {
    descriptor: std::fs::File,
}

#[cfg(target_os = "linux")]
impl PublicationLock {
    fn acquire(parent: &std::fs::File) -> Result<Self, std::io::Error> {
        let descriptor = parent.try_clone()?;
        rustix::fs::flock(&descriptor, rustix::fs::FlockOperation::LockExclusive)
            .map_err(std::io::Error::from)?;
        Ok(Self { descriptor })
    }
}

#[cfg(target_os = "linux")]
impl Drop for PublicationLock {
    fn drop(&mut self) {
        let _ = rustix::fs::flock(&self.descriptor, rustix::fs::FlockOperation::Unlock);
    }
}

#[cfg(target_os = "linux")]
fn write_regular_file_at(
    root: &std::fs::File,
    relative: &Path,
    bytes: &[u8],
) -> Result<(), std::io::Error> {
    use rustix::fs::{AtFlags, Mode, OFlags};

    let (components, file_name) = relative_file_parts(relative)?;
    let directory = open_or_create_directories_at(root, &components)?;
    for _ in 0..128 {
        let sequence = ARTIFACT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary_name = std::ffi::OsString::from(format!(
            ".stab-qualification-{}-{sequence}.tmp",
            std::process::id()
        ));
        let descriptor = match rustix::fs::openat(
            &directory,
            &temporary_name,
            OFlags::WRONLY
                | OFlags::CLOEXEC
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK,
            Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH,
        ) {
            Ok(descriptor) => descriptor,
            Err(rustix::io::Errno::EXIST) => continue,
            Err(source) => return Err(source.into()),
        };
        let mut file = std::fs::File::from(descriptor);
        if !file.metadata()?.is_file() {
            let _cleanup_result =
                rustix::fs::unlinkat(&directory, &temporary_name, AtFlags::empty());
            return Err(std::io::Error::other(
                "temporary qualification artifact is not a regular file",
            ));
        }
        let result = (|| -> Result<(), std::io::Error> {
            file.write_all(bytes)?;
            file.sync_all()?;
            rustix::fs::renameat(&directory, &temporary_name, &directory, &file_name)
                .map_err(std::io::Error::from)?;
            rustix::fs::fsync(&directory).map_err(std::io::Error::from)
        })();
        if result.is_err() {
            let _cleanup_result =
                rustix::fs::unlinkat(&directory, &temporary_name, AtFlags::empty());
        }
        return result;
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "failed to reserve a qualification artifact temporary file",
    ))
}

#[cfg(target_os = "linux")]
fn read_regular_file_at(
    root: &std::fs::File,
    relative: &Path,
    limit: usize,
) -> Result<Vec<u8>, std::io::Error> {
    use rustix::fs::{Mode, OFlags};

    let (components, file_name) = relative_file_parts(relative)?;
    let directory = open_directories_at(root, &components)?;
    let descriptor = rustix::fs::openat(
        &directory,
        &file_name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    let mut file = std::fs::File::from(descriptor);
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other(
            "qualification artifact is not a regular file",
        ));
    }
    let limit_u64 = u64::try_from(limit).unwrap_or(u64::MAX);
    if file.metadata()?.len() > limit_u64 {
        return Err(std::io::Error::other(format!(
            "qualification artifact exceeds the {limit}-byte limit"
        )));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(limit_u64)
        .read_to_end(&mut bytes)?;
    let mut extra = [0_u8; 1];
    if bytes.len() == limit && file.read(&mut extra)? != 0 {
        return Err(std::io::Error::other(format!(
            "qualification artifact exceeds the {limit}-byte limit"
        )));
    }
    Ok(bytes)
}

#[cfg(target_os = "linux")]
fn relative_file_parts(
    relative: &Path,
) -> Result<(Vec<std::ffi::OsString>, std::ffi::OsString), std::io::Error> {
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err(std::io::Error::other("invalid qualification artifact path"));
    }
    let mut components = Vec::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(std::io::Error::other(
                "invalid qualification artifact path component",
            ));
        };
        components.push(component.to_os_string());
    }
    let file_name = components
        .pop()
        .ok_or_else(|| std::io::Error::other("qualification artifact has no file name"))?;
    Ok((components, file_name))
}

#[cfg(target_os = "linux")]
fn directory_components(relative: &Path) -> Result<Vec<std::ffi::OsString>, std::io::Error> {
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err(std::io::Error::other(
            "invalid qualification directory path",
        ));
    }
    relative
        .components()
        .map(|component| match component {
            Component::Normal(component) => Ok(component.to_os_string()),
            _ => Err(std::io::Error::other(
                "invalid qualification directory path component",
            )),
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn create_staging_directory_at(
    parent: &std::fs::File,
) -> Result<(std::ffi::OsString, std::fs::File), std::io::Error> {
    use rustix::fs::Mode;

    for _ in 0..128 {
        let sequence = ARTIFACT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = std::ffi::OsString::from(format!(
            ".stab-correctness-{}-{sequence}",
            std::process::id()
        ));
        match rustix::fs::mkdirat(
            parent,
            &name,
            Mode::RUSR
                | Mode::WUSR
                | Mode::XUSR
                | Mode::RGRP
                | Mode::XGRP
                | Mode::ROTH
                | Mode::XOTH,
        ) {
            Ok(()) => {
                let directory = open_directory_at(parent, &name).map_err(std::io::Error::from)?;
                rustix::fs::fsync(parent).map_err(std::io::Error::from)?;
                return Ok((name, directory));
            }
            Err(rustix::io::Errno::EXIST) => continue,
            Err(source) => return Err(source.into()),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "failed to reserve a qualification staging directory",
    ))
}

#[cfg(target_os = "linux")]
fn open_directories_at(
    root: &std::fs::File,
    components: &[std::ffi::OsString],
) -> Result<std::fs::File, std::io::Error> {
    let mut directory = root.try_clone()?;
    for component in components {
        directory = open_directory_at(&directory, component).map_err(std::io::Error::from)?;
    }
    Ok(directory)
}

#[cfg(target_os = "linux")]
fn open_or_create_directories_at(
    root: &std::fs::File,
    components: &[std::ffi::OsString],
) -> Result<std::fs::File, std::io::Error> {
    use rustix::fs::Mode;

    let mut directory = root.try_clone()?;
    for component in components {
        directory = match open_directory_at(&directory, component) {
            Ok(next) => next,
            Err(rustix::io::Errno::NOENT) => {
                let created = match rustix::fs::mkdirat(
                    &directory,
                    component,
                    Mode::RUSR
                        | Mode::WUSR
                        | Mode::XUSR
                        | Mode::RGRP
                        | Mode::XGRP
                        | Mode::ROTH
                        | Mode::XOTH,
                ) {
                    Ok(()) => true,
                    Err(rustix::io::Errno::EXIST) => false,
                    Err(source) => return Err(source.into()),
                };
                if created {
                    sync_created_directory_parent(&directory)?;
                }
                open_directory_at(&directory, component).map_err(std::io::Error::from)?
            }
            Err(source) => return Err(source.into()),
        };
    }
    Ok(directory)
}

#[cfg(target_os = "linux")]
fn sync_created_directory_parent(directory: &std::fs::File) -> Result<(), std::io::Error> {
    rustix::fs::fsync(directory).map_err(std::io::Error::from)?;
    #[cfg(test)]
    CREATED_PARENT_SYNC_COUNT.with(|count| count.set(count.get().saturating_add(1)));
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
fn reset_created_parent_sync_count() {
    CREATED_PARENT_SYNC_COUNT.with(|count| count.set(0));
}

#[cfg(all(test, target_os = "linux"))]
fn created_parent_sync_count() -> usize {
    CREATED_PARENT_SYNC_COUNT.with(std::cell::Cell::get)
}

#[cfg(target_os = "linux")]
pub(crate) fn open_directory_at(
    parent: &std::fs::File,
    name: &std::ffi::OsStr,
) -> Result<std::fs::File, rustix::io::Errno> {
    use rustix::fs::{Mode, OFlags};

    rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map(std::fs::File::from)
}

#[cfg(target_os = "linux")]
fn directory_entry_matches(
    parent: &std::fs::File,
    name: &std::ffi::OsStr,
    expected: &std::fs::File,
) -> Result<bool, std::io::Error> {
    use std::os::unix::fs::MetadataExt as _;

    let actual = match open_directory_at(parent, name) {
        Ok(actual) => actual,
        Err(rustix::io::Errno::NOENT | rustix::io::Errno::NOTDIR | rustix::io::Errno::LOOP) => {
            return Ok(false);
        }
        Err(source) => return Err(source.into()),
    };
    let actual = actual.metadata()?;
    let expected = expected.metadata()?;
    Ok(actual.dev() == expected.dev() && actual.ino() == expected.ino())
}

#[cfg(target_os = "linux")]
fn ensure_directory_chain(
    repository: &std::fs::File,
    components: &[std::ffi::OsString],
    expected_parent: &std::fs::File,
) -> Result<(), std::io::Error> {
    use std::os::unix::fs::MetadataExt as _;

    let actual = open_directories_at(repository, components)?;
    let actual = actual.metadata()?;
    let expected = expected_parent.metadata()?;
    if actual.dev() == expected.dev() && actual.ino() == expected.ino() {
        Ok(())
    } else {
        Err(std::io::Error::other(
            "qualification output parent chain changed while repository lock was held",
        ))
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn cleanup_owned_directory(
    parent: &std::fs::File,
    name: &std::ffi::OsStr,
    directory: &std::fs::File,
) -> Result<(), std::io::Error> {
    remove_directory_contents(directory)?;
    if directory_entry_matches(parent, name, directory)? {
        rustix::fs::unlinkat(parent, name, rustix::fs::AtFlags::REMOVEDIR)
            .map_err(std::io::Error::from)?;
        rustix::fs::fsync(parent).map_err(std::io::Error::from)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn remove_directory_contents(directory: &std::fs::File) -> Result<(), std::io::Error> {
    use std::os::unix::ffi::OsStrExt as _;

    enum CleanupTask {
        Scan {
            directory: std::fs::File,
            depth: usize,
        },
        RemoveDirectory {
            parent: std::fs::File,
            name: std::ffi::OsString,
            directory: std::fs::File,
        },
    }

    let mut tasks = vec![CleanupTask::Scan {
        directory: directory.try_clone()?,
        depth: 0,
    }];
    let mut visited_entries = 0_usize;
    while let Some(task) = tasks.pop() {
        match task {
            CleanupTask::Scan { directory, depth } => {
                if depth > MAX_CLEANUP_DEPTH {
                    return Err(std::io::Error::other(format!(
                        "qualification cleanup exceeded depth {MAX_CLEANUP_DEPTH}"
                    )));
                }
                rustix::fs::fchmod(
                    &directory,
                    rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR | rustix::fs::Mode::XUSR,
                )
                .map_err(std::io::Error::from)?;
                let entries =
                    rustix::fs::Dir::read_from(&directory).map_err(std::io::Error::from)?;
                let mut names = Vec::new();
                for entry in entries {
                    let entry = entry.map_err(std::io::Error::from)?;
                    let bytes = entry.file_name().to_bytes();
                    if bytes == b"." || bytes == b".." {
                        continue;
                    }
                    visited_entries = visited_entries.checked_add(1).ok_or_else(|| {
                        std::io::Error::other("qualification cleanup entry count overflowed")
                    })?;
                    if visited_entries > MAX_CLEANUP_ENTRIES {
                        return Err(std::io::Error::other(format!(
                            "qualification cleanup exceeded {MAX_CLEANUP_ENTRIES} entries"
                        )));
                    }
                    names.push(std::ffi::OsString::from(std::ffi::OsStr::from_bytes(bytes)));
                }
                for name in names {
                    match open_directory_at(&directory, &name) {
                        Ok(child) => {
                            tasks.push(CleanupTask::RemoveDirectory {
                                parent: directory.try_clone()?,
                                name,
                                directory: child.try_clone()?,
                            });
                            tasks.push(CleanupTask::Scan {
                                directory: child,
                                depth: depth + 1,
                            });
                        }
                        Err(rustix::io::Errno::NOENT) => {}
                        Err(rustix::io::Errno::NOTDIR | rustix::io::Errno::LOOP) => {
                            match rustix::fs::unlinkat(
                                &directory,
                                &name,
                                rustix::fs::AtFlags::empty(),
                            ) {
                                Ok(()) | Err(rustix::io::Errno::NOENT) => {}
                                Err(source) => return Err(source.into()),
                            }
                        }
                        Err(source) => return Err(source.into()),
                    }
                }
            }
            CleanupTask::RemoveDirectory {
                parent,
                name,
                directory,
            } => {
                if directory_entry_matches(&parent, &name, &directory)? {
                    match rustix::fs::unlinkat(&parent, &name, rustix::fs::AtFlags::REMOVEDIR) {
                        Ok(()) | Err(rustix::io::Errno::NOENT) => {}
                        Err(source) => return Err(source.into()),
                    }
                }
            }
        }
    }
    Ok(())
}

fn unsafe_component(component: Component<'_>) -> bool {
    !matches!(component, Component::Normal(_))
}

#[cfg(test)]
#[path = "artifact/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "artifact/concurrency_tests.rs"]
mod concurrency_tests;
