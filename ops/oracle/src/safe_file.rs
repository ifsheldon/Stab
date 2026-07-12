use std::io::{Read, Write};
use std::path::{Component, Path};
#[cfg(not(windows))]
use std::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;

#[cfg(not(windows))]
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

pub(crate) fn open_regular_file(path: &Path) -> Result<std::fs::File, SafeFileError> {
    #[cfg(unix)]
    {
        open_regular_file_unix(path)
    }
    #[cfg(not(unix))]
    {
        open_regular_file_fallback(path)
    }
}

pub(crate) fn read_regular_file_bounded(
    path: &Path,
    limit: usize,
) -> Result<Vec<u8>, SafeFileError> {
    let mut file = open_regular_file(path)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(u64::try_from(limit).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(SafeFileError::TooLarge { limit });
    }
    Ok(bytes)
}

pub(crate) fn open_directory(path: &Path) -> Result<std::fs::File, SafeFileError> {
    #[cfg(unix)]
    {
        use rustix::fs::{Mode, OFlags};

        let components = absolute_normal_components(path)?;
        let mut directory = open_root_directory()?;
        for component in components {
            directory = rustix::fs::openat(
                &directory,
                component,
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)?;
        }
        Ok(std::fs::File::from(directory))
    }
    #[cfg(not(unix))]
    {
        validate_existing_components(path, true)?;
        Ok(std::fs::File::open(path)?)
    }
}

pub(crate) fn atomic_write_regular_file(path: &Path, bytes: &[u8]) -> Result<(), SafeFileError> {
    #[cfg(unix)]
    {
        atomic_write_regular_file_unix(path, bytes)
    }
    #[cfg(not(unix))]
    {
        atomic_write_regular_file_fallback(path, bytes)
    }
}

#[cfg(unix)]
fn open_regular_file_unix(path: &Path) -> Result<std::fs::File, SafeFileError> {
    use rustix::fs::{Mode, OFlags};

    let (components, file_name) = split_absolute_file_path(path)?;
    let mut directory = open_root_directory()?;
    for component in components {
        directory = rustix::fs::openat(
            &directory,
            component,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)?;
    }
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

#[cfg(unix)]
fn atomic_write_regular_file_unix(path: &Path, bytes: &[u8]) -> Result<(), SafeFileError> {
    use rustix::fs::AtFlags;

    let (directory, file_name) = open_or_create_parent_unix(path)?;
    let (temporary_name, descriptor) = create_temporary_file_unix(&directory)?;
    let mut file = std::fs::File::from(descriptor);
    if !file.metadata()?.is_file() {
        return Err(SafeFileError::NotRegular);
    }
    let result = (|| -> Result<(), SafeFileError> {
        file.write_all(bytes)?;
        file.sync_all()?;
        rustix::fs::renameat(&directory, &temporary_name, &directory, file_name)
            .map_err(std::io::Error::from)?;
        rustix::fs::fsync(&directory).map_err(std::io::Error::from)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = rustix::fs::unlinkat(&directory, &temporary_name, AtFlags::empty());
    }
    result
}

#[cfg(unix)]
fn open_or_create_parent_unix(
    path: &Path,
) -> Result<(std::os::fd::OwnedFd, &std::ffi::OsStr), SafeFileError> {
    let (components, file_name) = split_absolute_file_path(path)?;
    let directory = open_or_create_directory_components(components)?;
    Ok((directory, file_name))
}

#[cfg(unix)]
fn open_or_create_directory_components(
    components: Vec<&std::ffi::OsStr>,
) -> Result<std::os::fd::OwnedFd, SafeFileError> {
    use rustix::fs::{Mode, OFlags};

    let mut directory = open_root_directory()?;
    for component in components {
        let next = rustix::fs::openat(
            &directory,
            component,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        );
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
                rustix::fs::openat(
                    &directory,
                    component,
                    OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
                    Mode::empty(),
                )
                .map_err(std::io::Error::from)?
            }
            Err(source) => return Err(SafeFileError::Io(source.into())),
        };
    }
    Ok(directory)
}

#[cfg(unix)]
fn create_temporary_file_unix(
    directory: &std::os::fd::OwnedFd,
) -> Result<(std::ffi::OsString, std::os::fd::OwnedFd), SafeFileError> {
    use rustix::fs::{Mode, OFlags};

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

#[cfg(unix)]
fn open_root_directory() -> Result<std::os::fd::OwnedFd, SafeFileError> {
    use rustix::fs::{Mode, OFlags};

    rustix::fs::open(
        "/",
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|source| SafeFileError::Io(source.into()))
}

#[cfg(unix)]
fn split_absolute_file_path(
    path: &Path,
) -> Result<(Vec<&std::ffi::OsStr>, &std::ffi::OsStr), SafeFileError> {
    let mut normal = absolute_normal_components(path)?;
    let file_name = normal.pop().ok_or(SafeFileError::UnsafePath)?;
    Ok((normal, file_name))
}

#[cfg(unix)]
fn absolute_normal_components(path: &Path) -> Result<Vec<&std::ffi::OsStr>, SafeFileError> {
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

#[cfg(not(unix))]
fn open_regular_file_fallback(path: &Path) -> Result<std::fs::File, SafeFileError> {
    validate_existing_components(path, false)?;
    let file = std::fs::OpenOptions::new().read(true).open(path)?;
    if !file.metadata()?.is_file() {
        return Err(SafeFileError::NotRegular);
    }
    Ok(file)
}

#[cfg(windows)]
fn atomic_write_regular_file_fallback(path: &Path, bytes: &[u8]) -> Result<(), SafeFileError> {
    let parent = path.parent().ok_or(SafeFileError::UnsafePath)?;
    ensure_directory_fallback(parent)?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".stab-oracle-")
        .tempfile_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| SafeFileError::Io(error.error))?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn atomic_write_regular_file_fallback(path: &Path, bytes: &[u8]) -> Result<(), SafeFileError> {
    let parent = path.parent().ok_or(SafeFileError::UnsafePath)?;
    ensure_directory_fallback(parent)?;
    let temporary = parent.join(format!(
        ".stab-oracle-{}-{}.tmp",
        std::process::id(),
        TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| -> Result<(), SafeFileError> {
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        drop(std::fs::remove_file(&temporary));
    }
    result
}

#[cfg(not(unix))]
fn ensure_directory_fallback(path: &Path) -> Result<(), SafeFileError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(SafeFileError::UnsafePath);
    }
    let mut ancestor = Some(path);
    loop {
        let candidate = ancestor.ok_or(SafeFileError::UnsafePath)?;
        match std::fs::symlink_metadata(candidate) {
            Ok(_) => {
                validate_existing_components(candidate, true)?;
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ancestor = candidate.parent();
            }
            Err(error) => return Err(SafeFileError::Io(error)),
        }
    }
    std::fs::create_dir_all(path)?;
    validate_existing_components(path, true)
}

#[cfg(not(unix))]
fn validate_existing_components(path: &Path, directory: bool) -> Result<(), SafeFileError> {
    let mut current = std::path::PathBuf::new();
    for component in path.components() {
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() {
            return Err(SafeFileError::NotRegular);
        }
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        return Err(SafeFileError::NotRegular);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{atomic_write_regular_file, open_regular_file};

    #[cfg(unix)]
    #[test]
    fn descriptor_walk_rejects_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("temporary directory");
        let outside = tempfile::tempdir().expect("outside directory");
        std::fs::write(outside.path().join("fixture"), b"outside").expect("outside fixture");
        symlink(outside.path(), directory.path().join("fixtures")).expect("fixture root symlink");

        assert!(open_regular_file(&directory.path().join("fixtures/fixture")).is_err());
    }

    #[cfg(unix)]
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
