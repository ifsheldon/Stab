//! Descriptor-walked file access that rejects symlinks in every path component.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rustix::fs::{Mode, OFlags};
use thiserror::Error;

const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::CLOEXEC)
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW);

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
    let (components, file_name) = split_absolute_file_path(path)?;
    let directory = open_directory_components(components)?;
    let descriptor = rustix::fs::openat(
        &directory,
        file_name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    let file = std::fs::File::from(descriptor);
    if !file.metadata()?.is_file() {
        return Err(SafeFileError::NotRegular);
    }
    Ok(file)
}

pub(crate) fn read_regular_file_bounded(
    path: &Path,
    limit: usize,
) -> Result<Vec<u8>, SafeFileError> {
    let mut file = open_regular_file(path)?;
    let limit_u64 = u64::try_from(limit).unwrap_or(u64::MAX);
    if file.metadata()?.len() > limit_u64 {
        return Err(SafeFileError::TooLarge { limit });
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(limit_u64)
        .read_to_end(&mut bytes)?;
    let mut extra = [0_u8; 1];
    if bytes.len() == limit && file.read(&mut extra)? != 0 {
        return Err(SafeFileError::TooLarge { limit });
    }
    Ok(bytes)
}

pub(crate) fn open_directory(path: &Path) -> Result<std::fs::File, SafeFileError> {
    let directory = open_directory_components(absolute_normal_components(path)?)?;
    Ok(std::fs::File::from(directory))
}

pub(crate) fn atomic_write_regular_file(path: &Path, bytes: &[u8]) -> Result<(), SafeFileError> {
    use rustix::fs::AtFlags;
    use std::mem::DropGuard;

    let (components, file_name) = split_absolute_file_path(path)?;
    let directory = open_or_create_directory_components(components)?;
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

fn open_directory_components(components: Vec<&OsStr>) -> Result<OwnedFd, SafeFileError> {
    let mut directory = open_root_directory()?;
    for component in components {
        directory = rustix::fs::openat(&directory, component, DIRECTORY_FLAGS, Mode::empty())
            .map_err(std::io::Error::from)?;
    }
    Ok(directory)
}

fn open_or_create_directory_components(components: Vec<&OsStr>) -> Result<OwnedFd, SafeFileError> {
    let mut directory = open_root_directory()?;
    for component in components {
        let next = rustix::fs::openat(&directory, component, DIRECTORY_FLAGS, Mode::empty());
        directory = match next {
            Ok(next) => next,
            Err(rustix::io::Errno::NOENT) => {
                rustix::fs::mkdirat(
                    &directory,
                    component,
                    Mode::RUSR
                        | Mode::WUSR
                        | Mode::XUSR
                        | Mode::RGRP
                        | Mode::XGRP
                        | Mode::ROTH
                        | Mode::XOTH,
                )
                .map_err(std::io::Error::from)?;
                rustix::fs::openat(&directory, component, DIRECTORY_FLAGS, Mode::empty())
                    .map_err(std::io::Error::from)?
            }
            Err(source) => return Err(SafeFileError::Io(source.into())),
        };
    }
    Ok(directory)
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

fn open_root_directory() -> Result<OwnedFd, SafeFileError> {
    rustix::fs::open("/", DIRECTORY_FLAGS, Mode::empty())
        .map_err(|source| SafeFileError::Io(source.into()))
}

fn split_absolute_file_path(path: &Path) -> Result<(Vec<&OsStr>, &OsStr), SafeFileError> {
    let mut normal = absolute_normal_components(path)?;
    let file_name = normal.pop().ok_or(SafeFileError::UnsafePath)?;
    Ok((normal, file_name))
}

fn absolute_normal_components(path: &Path) -> Result<Vec<&OsStr>, SafeFileError> {
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(SafeFileError::UnsafePath);
    }
    let mut normal = Vec::new();
    for component in components {
        let Component::Normal(component) = component else {
            return Err(SafeFileError::UnsafePath);
        };
        normal.push(component);
    }
    Ok(normal)
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
