//! Descriptor-walked file access that rejects symlinks in every path component, built on the
//! shared `stab-confined-fs` primitives. It does not revalidate device and inode identity.

use std::ffi::OsStr;
use std::io::Write;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rustix::fs::{Mode, OFlags};
use thiserror::Error;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error)]
pub(crate) enum SafeFileError {
    #[error("path is not an absolute path containing only normal components")]
    UnsafePath,
    #[error("path does not identify a regular file")]
    NotRegular,
    #[error("file exceeds the {limit}-byte limit")]
    TooLarge { limit: usize },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug)]
pub(crate) struct SafeFileLocation {
    display_path: PathBuf,
}

impl SafeFileLocation {
    pub(crate) fn path(path: PathBuf) -> Self {
        Self { display_path: path }
    }

    pub(crate) fn display_path(&self) -> &Path {
        &self.display_path
    }

    pub(crate) fn open_regular_file(&self) -> Result<std::fs::File, SafeFileError> {
        open_regular_file(&self.display_path)
    }
}

pub(crate) fn open_regular_file(path: &Path) -> Result<std::fs::File, SafeFileError> {
    stab_confined_fs::open_regular_file(path).map_err(SafeFileError::from)
}

pub(crate) fn read_regular_file_bounded(
    path: &Path,
    limit: usize,
) -> Result<Vec<u8>, SafeFileError> {
    let limit = u64::try_from(limit).unwrap_or(u64::MAX);
    stab_confined_fs::read_regular_file_bounded(path, limit).map_err(SafeFileError::from)
}

pub(crate) fn open_directory(path: &Path) -> Result<std::fs::File, SafeFileError> {
    stab_confined_fs::open_directory(path).map_err(SafeFileError::from)
}

/// Opens the directory `name` directly below `parent` without following a symlink.
pub(crate) fn open_child_directory(
    parent: &impl std::os::fd::AsFd,
    name: &OsStr,
) -> Result<OwnedFd, SafeFileError> {
    stab_confined_fs::open_directory_at(parent, name, Path::new(name))
        .map(OwnedFd::from)
        .map_err(SafeFileError::from)
}

pub(crate) fn atomic_write_regular_file(path: &Path, bytes: &[u8]) -> Result<(), SafeFileError> {
    use rustix::fs::AtFlags;
    use std::mem::DropGuard;

    let (components, file_name) =
        stab_confined_fs::split_absolute_path(path).map_err(SafeFileError::from)?;
    let directory = OwnedFd::from(
        stab_confined_fs::open_directory_components(&components, true, path)
            .map_err(SafeFileError::from)?,
    );
    let (temporary_name, descriptor) = create_temporary_file(&directory)?;
    let mut file = std::fs::File::from(descriptor);
    if !file.metadata()?.is_file() {
        return Err(SafeFileError::NotRegular);
    }
    // Any failure from here on removes the temporary file; success dismisses the cleanup.
    let cleanup = DropGuard::new(&temporary_name, |name| {
        let _ = rustix::fs::unlinkat(&directory, name, AtFlags::empty());
    });
    file.write_all(bytes)?;
    file.sync_all()?;
    rustix::fs::renameat(&directory, &temporary_name, &directory, file_name)
        .map_err(std::io::Error::from)?;
    rustix::fs::fsync(&directory).map_err(std::io::Error::from)?;
    DropGuard::dismiss(cleanup);
    Ok(())
}

fn create_temporary_file(
    directory: &OwnedFd,
) -> Result<(std::ffi::OsString, OwnedFd), SafeFileError> {
    for _ in 0..128 {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = std::ffi::OsString::from(format!(
            ".stab-oracle-{}-{sequence}.tmp",
            std::process::id()
        ));
        match rustix::fs::openat(
            directory,
            &name,
            OFlags::WRONLY
                | OFlags::CLOEXEC
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK,
            Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH,
        ) {
            Ok(descriptor) => return Ok((name, descriptor)),
            Err(rustix::io::Errno::EXIST) => continue,
            Err(source) => return Err(SafeFileError::Io(source.into())),
        }
    }
    Err(SafeFileError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "failed to reserve a unique temporary output file",
    )))
}

/// Keeps each caller's error variants: the shared walk reports a symlink as its own error, which
/// the oracle has always surfaced as the underlying `ELOOP`.
impl From<stab_confined_fs::ConfinedFsError> for SafeFileError {
    fn from(error: stab_confined_fs::ConfinedFsError) -> Self {
        use stab_confined_fs::ConfinedFsError;
        match error {
            ConfinedFsError::InvalidPath(_) => Self::UnsafePath,
            ConfinedFsError::Symlink(_) => Self::Io(rustix::io::Errno::LOOP.into()),
            ConfinedFsError::NotRegularFile(_) => Self::NotRegular,
            ConfinedFsError::TooLarge { limit, .. } => Self::TooLarge {
                limit: usize::try_from(limit).unwrap_or(usize::MAX),
            },
            ConfinedFsError::Io { source, .. } => Self::Io(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{atomic_write_regular_file, open_regular_file, read_regular_file_bounded};

    #[test]
    fn bounded_read_handles_the_maximum_usize_without_overflow() {
        let file = tempfile::NamedTempFile::new().expect("temporary file");
        std::fs::write(file.path(), b"bounded").expect("write temporary file");

        assert_eq!(
            read_regular_file_bounded(file.path(), usize::MAX).expect("bounded read"),
            b"bounded"
        );
    }

    #[test]
    fn descriptor_walk_rejects_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("temporary directory");
        let outside = tempfile::tempdir().expect("outside directory");
        std::fs::write(outside.path().join("fixture"), b"outside").expect("outside fixture");
        symlink(outside.path(), directory.path().join("fixtures")).expect("fixture root symlink");

        assert!(open_regular_file(&directory.path().join("fixtures/fixture")).is_err());
    }

    #[test]
    fn atomic_write_replaces_hard_link_without_mutating_outside_inode() {
        let directory = tempfile::tempdir().expect("fixture directory");
        let outside = tempfile::NamedTempFile::new().expect("outside file");
        std::fs::write(outside.path(), b"outside").expect("outside contents");
        let golden = directory.path().join("golden.stdout");
        std::fs::hard_link(outside.path(), &golden).expect("hard-linked golden");

        atomic_write_regular_file(&golden, b"recorded").expect("atomic golden write");

        assert_eq!(std::fs::read(outside.path()).unwrap(), b"outside");
        assert_eq!(std::fs::read(golden).unwrap(), b"recorded");
    }
}
