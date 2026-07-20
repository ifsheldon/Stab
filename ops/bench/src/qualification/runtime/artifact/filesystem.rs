use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};

use super::ArtifactError;

static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) fn open_absolute_directory(path: &Path) -> Result<OwnedFd, ArtifactError> {
    if !path.is_absolute() {
        return Err(ArtifactError::RepositoryIdentity);
    }
    let mut current = rustix::fs::open("/", directory_flags(), rustix::fs::Mode::empty())
        .map_err(ArtifactError::Io)?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                current = open_directory_at(&current, name).map_err(ArtifactError::Io)?;
            }
            _ => return Err(ArtifactError::RepositoryIdentity),
        }
    }
    Ok(current)
}

pub(super) fn open_directory_at(
    parent: &OwnedFd,
    name: &OsStr,
) -> Result<OwnedFd, rustix::io::Errno> {
    rustix::fs::openat(parent, name, directory_flags(), rustix::fs::Mode::empty())
}

pub(super) fn open_or_create_directories(
    root: &OwnedFd,
    components: &[&OsStr],
) -> Result<OwnedFd, ArtifactError> {
    let mut current = rustix::io::dup(root).map_err(ArtifactError::Io)?;
    for component in components {
        match open_directory_at(&current, component) {
            Ok(next) => current = next,
            Err(rustix::io::Errno::NOENT) => {
                match rustix::fs::mkdirat(
                    &current,
                    *component,
                    rustix::fs::Mode::RUSR
                        | rustix::fs::Mode::WUSR
                        | rustix::fs::Mode::XUSR
                        | rustix::fs::Mode::RGRP
                        | rustix::fs::Mode::XGRP
                        | rustix::fs::Mode::ROTH
                        | rustix::fs::Mode::XOTH,
                ) {
                    Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                    Err(source) => return Err(ArtifactError::Io(source)),
                }
                current = open_directory_at(&current, component).map_err(ArtifactError::Io)?;
            }
            Err(source) => return Err(ArtifactError::Io(source)),
        }
    }
    Ok(current)
}

pub(super) fn open_existing_directories(
    root: &OwnedFd,
    components: &[&OsStr],
) -> Result<OwnedFd, ArtifactError> {
    let mut current = rustix::io::dup(root).map_err(ArtifactError::Io)?;
    for component in components {
        current = open_directory_at(&current, component).map_err(ArtifactError::Io)?;
    }
    Ok(current)
}

pub(super) fn open_existing_directories_if_present(
    root: &OwnedFd,
    components: &[&OsStr],
) -> Result<Option<OwnedFd>, ArtifactError> {
    let mut current = rustix::io::dup(root).map_err(ArtifactError::Io)?;
    for component in components {
        match open_directory_at(&current, component) {
            Ok(next) => current = next,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(source) => return Err(ArtifactError::Io(source)),
        }
    }
    Ok(Some(current))
}

pub(super) fn ensure_directory_chain(
    repository: &OwnedFd,
    components: &[OsString],
    expected: &OwnedFd,
) -> Result<(), ArtifactError> {
    let components = components
        .iter()
        .map(OsString::as_os_str)
        .collect::<Vec<_>>();
    let actual = open_existing_directories(repository, &components)?;
    if same_directory(&actual, expected)? {
        Ok(())
    } else {
        Err(ArtifactError::DirectoryIdentity(
            "qualification output parent chain changed",
        ))
    }
}

pub(super) fn sync_directory_chain(
    repository: &OwnedFd,
    components: &[OsString],
    expected: &OwnedFd,
) -> Result<(), ArtifactError> {
    let mut directories = vec![rustix::io::dup(repository).map_err(ArtifactError::Io)?];
    for component in components {
        let parent = directories.last().ok_or(ArtifactError::DirectoryIdentity(
            "qualification output directory chain is empty",
        ))?;
        directories.push(open_directory_at(parent, component).map_err(ArtifactError::Io)?);
    }
    let actual = directories.last().ok_or(ArtifactError::DirectoryIdentity(
        "qualification output directory chain is empty",
    ))?;
    if !same_directory(actual, expected)? {
        return Err(ArtifactError::DirectoryIdentity(
            "qualification output parent chain changed",
        ));
    }
    for directory in directories.iter().rev() {
        rustix::fs::fsync(directory).map_err(ArtifactError::Io)?;
    }
    Ok(())
}

pub(super) fn directory_entry_is_missing(
    parent: &OwnedFd,
    name: &OsStr,
) -> Result<bool, ArtifactError> {
    match open_directory_at(parent, name) {
        Ok(_) | Err(rustix::io::Errno::NOTDIR | rustix::io::Errno::LOOP) => Ok(false),
        Err(rustix::io::Errno::NOENT) => Ok(true),
        Err(source) => Err(ArtifactError::Io(source)),
    }
}

pub(super) fn directory_entry_matches(
    parent: &OwnedFd,
    name: &OsStr,
    expected: &OwnedFd,
) -> Result<bool, ArtifactError> {
    let actual = match open_directory_at(parent, name) {
        Ok(actual) => actual,
        Err(rustix::io::Errno::NOENT | rustix::io::Errno::NOTDIR | rustix::io::Errno::LOOP) => {
            return Ok(false);
        }
        Err(source) => return Err(ArtifactError::Io(source)),
    };
    same_directory(&actual, expected)
}

pub(super) fn artifact_entry_matches(
    directory: &OwnedFd,
    name: &OsStr,
    expected: &OwnedFd,
) -> Result<bool, ArtifactError> {
    let actual = match rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    ) {
        Ok(actual) => actual,
        Err(rustix::io::Errno::NOENT | rustix::io::Errno::NOTDIR | rustix::io::Errno::LOOP) => {
            return Ok(false);
        }
        Err(source) => return Err(ArtifactError::Io(source)),
    };
    same_file(&actual, expected)
}

pub(super) fn same_directory(left: &OwnedFd, right: &OwnedFd) -> Result<bool, ArtifactError> {
    same_file(left, right)
}

pub(super) fn same_file(left: &OwnedFd, right: &OwnedFd) -> Result<bool, ArtifactError> {
    use std::os::unix::fs::MetadataExt as _;

    let left = std::fs::File::from(rustix::io::dup(left).map_err(ArtifactError::Io)?)
        .metadata()
        .map_err(ArtifactError::Write)?;
    let right = std::fs::File::from(rustix::io::dup(right).map_err(ArtifactError::Io)?)
        .metadata()
        .map_err(ArtifactError::Write)?;
    Ok(left.dev() == right.dev() && left.ino() == right.ino())
}

pub(super) fn create_staging_directory(
    parent: &OwnedFd,
) -> Result<(OsString, OwnedFd), ArtifactError> {
    for _ in 0..128 {
        let sequence = STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = OsString::from(format!(".run-{}-{sequence}.staging", std::process::id()));
        match rustix::fs::mkdirat(
            parent,
            &name,
            rustix::fs::Mode::RUSR
                | rustix::fs::Mode::WUSR
                | rustix::fs::Mode::XUSR
                | rustix::fs::Mode::RGRP
                | rustix::fs::Mode::XGRP,
        ) {
            Ok(()) => {
                let directory = open_directory_at(parent, &name).map_err(ArtifactError::Io)?;
                return Ok((name, directory));
            }
            Err(rustix::io::Errno::EXIST) => continue,
            Err(source) => return Err(ArtifactError::Io(source)),
        }
    }
    Err(ArtifactError::NoStagingName)
}

pub(super) fn directory_names(
    directory: &OwnedFd,
    maximum_entries: usize,
) -> Result<BTreeSet<OsString>, ArtifactError> {
    let descriptor = open_directory_at(directory, OsStr::new(".")).map_err(ArtifactError::Io)?;
    let mut buffer = [MaybeUninit::uninit(); 8192];
    let mut entries = rustix::fs::RawDir::new(descriptor, &mut buffer);
    let mut names = BTreeSet::new();
    while let Some(entry) = entries.next() {
        let entry = entry.map_err(ArtifactError::Io)?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        if names.len() == maximum_entries {
            return Err(ArtifactError::TooManyExistingArtifacts);
        }
        use std::os::unix::ffi::OsStringExt as _;
        names.insert(OsString::from_vec(name.to_vec()));
    }
    Ok(names)
}

fn directory_flags() -> rustix::fs::OFlags {
    rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::CLOEXEC
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
}
