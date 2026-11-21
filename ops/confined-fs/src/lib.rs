//! No-follow descriptor-walked file primitives shared by Stab's operational tools.
//!
//! Every open walks descriptors from `/` with `O_NOFOLLOW` on every component, so a symlink
//! anywhere in a path is rejected rather than followed, and only absolute paths made of normal
//! components are accepted. These primitives do not revalidate device and inode identity after
//! an open; callers that need identity guarantees (such as `stab-release`) layer them on top.

#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact diagnostics"
    )
)]

use std::ffi::OsStr;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::AsFd;
use std::path::{Component, Path, PathBuf};

use rustix::fs::{Mode, OFlags};
use thiserror::Error;

const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::CLOEXEC)
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW);
const REGULAR_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::CLOEXEC)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK);

#[derive(Debug, Error)]
pub enum ConfinedFsError {
    /// The path is not absolute or has a component other than a normal name.
    #[error("{0} is not an absolute path containing only normal components")]
    InvalidPath(PathBuf),
    /// A component of the path is a symbolic link.
    #[error("{0} contains a symbolic link")]
    Symlink(PathBuf),
    #[error("{0} is not a regular file")]
    NotRegularFile(PathBuf),
    #[error("{path} exceeds the {limit}-byte limit")]
    TooLarge { path: PathBuf, limit: u64 },
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl ConfinedFsError {
    fn io(path: &Path, source: impl Into<std::io::Error>) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source: source.into(),
        }
    }

    fn from_open(path: &Path, source: rustix::io::Errno) -> Self {
        if source == rustix::io::Errno::LOOP {
            Self::Symlink(path.to_path_buf())
        } else {
            Self::io(path, source)
        }
    }
}

/// Splits an absolute path into its parent components and final name.
pub fn split_absolute_path(path: &Path) -> Result<(Vec<&OsStr>, &OsStr), ConfinedFsError> {
    let mut components = absolute_components(path)?;
    let name = components
        .pop()
        .ok_or_else(|| ConfinedFsError::InvalidPath(path.to_path_buf()))?;
    Ok((components, name))
}

/// The normal components of an absolute path, rejecting `.`, `..`, and prefixes.
pub fn absolute_components(path: &Path) -> Result<Vec<&OsStr>, ConfinedFsError> {
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(ConfinedFsError::InvalidPath(path.to_path_buf()));
    }
    components
        .map(|component| match component {
            Component::Normal(component) => Ok(component),
            _ => Err(ConfinedFsError::InvalidPath(path.to_path_buf())),
        })
        .collect()
}

/// Opens the directory `name` directly below `directory` without following a symlink.
pub fn open_directory_at(
    directory: &impl AsFd,
    name: &OsStr,
    display_path: &Path,
) -> Result<File, ConfinedFsError> {
    rustix::fs::openat(directory, name, DIRECTORY_FLAGS, Mode::empty())
        .map(File::from)
        .map_err(|source| ConfinedFsError::from_open(display_path, source))
}

/// Opens the regular file `name` directly below `directory` without following a symlink and
/// without blocking on a FIFO or device.
pub fn open_regular_at(
    directory: &impl AsFd,
    name: &OsStr,
    display_path: &Path,
) -> Result<File, ConfinedFsError> {
    let file = rustix::fs::openat(directory, name, REGULAR_FLAGS, Mode::empty())
        .map(File::from)
        .map_err(|source| ConfinedFsError::from_open(display_path, source))?;
    if !file
        .metadata()
        .map_err(|source| ConfinedFsError::io(display_path, source))?
        .is_file()
    {
        return Err(ConfinedFsError::NotRegularFile(display_path.to_path_buf()));
    }
    Ok(file)
}

/// Opens `/` as the start of a descriptor walk.
pub fn open_root(display_path: &Path) -> Result<File, ConfinedFsError> {
    rustix::fs::open("/", DIRECTORY_FLAGS, Mode::empty())
        .map(File::from)
        .map_err(|source| ConfinedFsError::io(display_path, source))
}

/// Walks `components` from `/`, optionally creating missing directories with mode 0755.
pub fn open_directory_components(
    components: &[&OsStr],
    create_missing: bool,
    display_path: &Path,
) -> Result<File, ConfinedFsError> {
    let mut directory = open_root(display_path)?;
    for component in components {
        directory = match open_directory_at(&directory, component, display_path) {
            Ok(next) => next,
            Err(ConfinedFsError::Io { source, .. })
                if create_missing && source.kind() == std::io::ErrorKind::NotFound =>
            {
                rustix::fs::mkdirat(
                    &directory,
                    *component,
                    Mode::RUSR
                        | Mode::WUSR
                        | Mode::XUSR
                        | Mode::RGRP
                        | Mode::XGRP
                        | Mode::ROTH
                        | Mode::XOTH,
                )
                .map_err(|source| ConfinedFsError::io(display_path, source))?;
                open_directory_at(&directory, component, display_path)?
            }
            Err(error) => return Err(error),
        };
    }
    Ok(directory)
}

/// Opens the absolute directory `path` through a no-follow walk from `/`.
pub fn open_directory(path: &Path) -> Result<File, ConfinedFsError> {
    open_directory_components(&absolute_components(path)?, false, path)
}

/// Opens the absolute regular file `path` through a no-follow walk from `/`.
pub fn open_regular_file(path: &Path) -> Result<File, ConfinedFsError> {
    let (parents, name) = split_absolute_path(path)?;
    let directory = open_directory_components(&parents, false, path)?;
    open_regular_at(&directory, name, path)
}

/// Reads all of `file` from its start, failing if it holds more than `limit` bytes.
pub fn read_bounded(mut file: File, path: &Path, limit: u64) -> Result<Vec<u8>, ConfinedFsError> {
    let too_large = || ConfinedFsError::TooLarge {
        path: path.to_path_buf(),
        limit,
    };
    let metadata = file
        .metadata()
        .map_err(|source| ConfinedFsError::io(path, source))?;
    if !metadata.is_file() {
        return Err(ConfinedFsError::NotRegularFile(path.to_path_buf()));
    }
    if metadata.len() > limit {
        return Err(too_large());
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|source| ConfinedFsError::io(path, source))?;
    let capacity = usize::try_from(metadata.len()).map_err(|_| too_large())?;
    let mut bytes = Vec::with_capacity(capacity);
    Read::by_ref(&mut file)
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| ConfinedFsError::io(path, source))?;
    if !matches!(u64::try_from(bytes.len()), Ok(size) if size <= limit) {
        return Err(too_large());
    }
    Ok(bytes)
}

/// Opens and reads the absolute regular file `path`, bounded at `limit` bytes.
pub fn read_regular_file_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, ConfinedFsError> {
    read_bounded(open_regular_file(path)?, path, limit)
}

#[cfg(test)]
mod tests {
    use std::assert_matches;
    use std::os::unix::fs::symlink;

    use super::{ConfinedFsError, open_directory, open_regular_file, read_regular_file_bounded};

    #[test]
    fn every_component_is_walked_without_following_symlinks() {
        let directory = tempfile::tempdir().expect("directory");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("file"), b"outside").expect("outside file");
        symlink(outside.path(), directory.path().join("parent")).expect("parent symlink");
        symlink(outside.path().join("file"), directory.path().join("leaf")).expect("leaf link");

        // Linux reports a no-follow directory open of a symlink as ENOTDIR and a no-follow
        // regular open of one as ELOOP.
        assert_matches!(
            open_regular_file(&directory.path().join("parent/file")),
            Err(ConfinedFsError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotADirectory
        );
        assert_matches!(
            open_regular_file(&directory.path().join("leaf")),
            Err(ConfinedFsError::Symlink(_))
        );
        assert!(open_directory(&directory.path().join("parent")).is_err());
        assert_eq!(
            std::fs::read(outside.path().join("file")).expect("outside file"),
            b"outside"
        );
    }

    #[test]
    fn relative_and_dot_paths_are_rejected_before_any_open() {
        for path in ["relative/file", "/tmp/../etc/passwd", "/"] {
            assert_matches!(
                open_regular_file(std::path::Path::new(path)),
                Err(ConfinedFsError::InvalidPath(_)),
                "{path}"
            );
        }
    }

    #[test]
    fn bounded_reads_accept_the_limit_and_reject_one_more_byte() {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("file");
        std::fs::write(&path, b"1234").expect("file");

        assert_eq!(
            read_regular_file_bounded(&path, 4).expect("at limit"),
            b"1234"
        );
        assert_eq!(
            read_regular_file_bounded(&path, u64::MAX).expect("maximum"),
            b"1234"
        );
        assert_matches!(
            read_regular_file_bounded(&path, 3),
            Err(ConfinedFsError::TooLarge { limit: 3, .. })
        );
        assert_matches!(
            read_regular_file_bounded(directory.path(), 10),
            Err(ConfinedFsError::NotRegularFile(_))
        );
    }
}
