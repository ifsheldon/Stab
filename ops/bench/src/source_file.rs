#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::path::Component;
use std::path::Path;
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::BenchError;
use crate::root::RepoRoot;

#[cfg(unix)]
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn atomic_write_repo_regular_file(
    root: &RepoRoot,
    path: &Path,
    bytes: &[u8],
) -> Result<(), BenchError> {
    #[cfg(unix)]
    {
        atomic_write_repo_regular_file_unix(root, path, bytes)
    }
    #[cfg(not(unix))]
    {
        let _ = (root, path, bytes);
        Err(non_unix_unsupported("atomic source output"))
    }
}

pub(crate) fn read_repo_regular_file_bounded(
    root: &RepoRoot,
    path: &Path,
    max_bytes: usize,
) -> Result<Vec<u8>, BenchError> {
    #[cfg(unix)]
    {
        validate_repo_regular_file(root, path)?;
        read_regular_file_bounded(path, max_bytes)
    }
    #[cfg(not(unix))]
    {
        let _ = (root, path, max_bytes);
        Err(non_unix_unsupported("bounded repository input"))
    }
}

pub(crate) fn validate_repo_regular_file(root: &RepoRoot, path: &Path) -> Result<(), BenchError> {
    #[cfg(unix)]
    {
        validate_repo_regular_file_unix(root, path)
    }
    #[cfg(not(unix))]
    {
        let _ = (root, path);
        Err(non_unix_unsupported("repository input validation"))
    }
}

#[cfg(unix)]
fn validate_repo_regular_file_unix(root: &RepoRoot, path: &Path) -> Result<(), BenchError> {
    let relative = path.strip_prefix(&root.path).map_err(|_| {
        BenchError::SourceInput(format!(
            "source input {} is outside the repository root",
            path.display()
        ))
    })?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(BenchError::SourceInput(format!(
            "source input {} is not a normal repository-relative file path",
            path.display()
        )));
    }
    let component_count = relative.components().count();
    let mut current = root.path.clone();
    for (index, component) in relative.components().enumerate() {
        current.push(component.as_os_str());
        let metadata = metadata(&current)?;
        let final_component = index + 1 == component_count;
        if metadata.file_type().is_symlink()
            || final_component && !metadata.is_file()
            || !final_component && !metadata.is_dir()
        {
            return Err(BenchError::SourceInput(format!(
                "source input component {} must be a nonsymlink {}",
                current.display(),
                if final_component { "file" } else { "directory" }
            )));
        }
    }
    Ok(())
}

pub(crate) fn read_regular_file_bounded(
    path: &Path,
    max_bytes: usize,
) -> Result<Vec<u8>, BenchError> {
    #[cfg(not(unix))]
    {
        let _ = (path, max_bytes);
        return Err(non_unix_unsupported("bounded source input"));
    }
    #[cfg(unix)]
    {
        read_regular_file_bounded_unix(path, max_bytes)
    }
}

pub(crate) fn open_regular_file_bounded_descriptor(
    path: &Path,
    max_bytes: u64,
) -> Result<std::fs::File, BenchError> {
    #[cfg(unix)]
    {
        let file = open_regular_file_unix(path)?;
        let metadata = file
            .metadata()
            .map_err(|source| BenchError::SourceInputIo {
                path: path.to_path_buf(),
                source,
            })?;
        if !metadata.is_file() || metadata.len() > max_bytes {
            return Err(BenchError::SourceInput(format!(
                "source input {} must be a regular file no larger than {max_bytes} bytes",
                path.display()
            )));
        }
        Ok(file)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, max_bytes);
        Err(non_unix_unsupported("bounded descriptor input"))
    }
}

#[cfg(unix)]
fn read_regular_file_bounded_unix(path: &Path, max_bytes: usize) -> Result<Vec<u8>, BenchError> {
    let file = open_regular_file_unix(path)?;
    let opened = file
        .metadata()
        .map_err(|source| BenchError::SourceInputIo {
            path: path.to_path_buf(),
            source,
        })?;
    let max_bytes_u64 = u64::try_from(max_bytes)
        .map_err(|_| BenchError::SourceInput("source input byte limit exceeds u64".to_string()))?;
    if opened.len() > max_bytes_u64 {
        return Err(BenchError::SourceInput(format!(
            "source input {} exceeds {max_bytes} bytes",
            path.display()
        )));
    }
    let limit = max_bytes_u64.saturating_add(1);
    let capacity = usize::try_from(opened.len()).map_err(|_| {
        BenchError::SourceInput(format!(
            "source input {} size does not fit in usize",
            path.display()
        ))
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|source| BenchError::SourceInputIo {
            path: path.to_path_buf(),
            source,
        })?;
    if bytes.len() > max_bytes {
        return Err(BenchError::SourceInput(format!(
            "source input {} grew beyond {max_bytes} bytes while reading",
            path.display()
        )));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn open_regular_file_unix(path: &Path) -> Result<std::fs::File, BenchError> {
    use rustix::fs::{Mode, OFlags};

    let mut components = absolute_normal_components(path)?;
    let file_name = components.pop().ok_or_else(|| {
        BenchError::SourceInput(format!("source input {} has no file name", path.display()))
    })?;
    let mut directory = rustix::fs::open(
        "/",
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(source_io(path))?;
    for component in components {
        directory = rustix::fs::openat(
            &directory,
            component,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(source_io(path))?;
    }
    let descriptor = rustix::fs::openat(
        &directory,
        file_name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(source_io(path))?;
    let file = std::fs::File::from(descriptor);
    if !file
        .metadata()
        .map_err(|source| BenchError::SourceInputIo {
            path: path.to_path_buf(),
            source,
        })?
        .is_file()
    {
        return Err(BenchError::SourceInput(format!(
            "source input {} must be a regular nonsymlink file",
            path.display()
        )));
    }
    Ok(file)
}

#[cfg(unix)]
fn metadata(path: &Path) -> Result<std::fs::Metadata, BenchError> {
    std::fs::symlink_metadata(path).map_err(|source| BenchError::SourceInputIo {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn atomic_write_repo_regular_file_unix(
    root: &RepoRoot,
    path: &Path,
    bytes: &[u8],
) -> Result<(), BenchError> {
    use rustix::fs::{AtFlags, Mode, OFlags};

    let (parents, file_name) = split_repo_file_path(root, path)?;
    let mut directory = rustix::fs::open(
        &root.path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(source_io(path))?;
    for component in parents {
        directory = rustix::fs::openat(
            &directory,
            component,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(source_io(path))?;
    }
    match rustix::fs::openat(
        &directory,
        file_name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(descriptor) => {
            if !std::fs::File::from(descriptor)
                .metadata()
                .map_err(|source| BenchError::SourceInputIo {
                    path: path.to_path_buf(),
                    source,
                })?
                .is_file()
            {
                return Err(BenchError::SourceInput(format!(
                    "source output {} must replace only a regular file",
                    path.display()
                )));
            }
        }
        Err(rustix::io::Errno::NOENT) => {}
        Err(source) => {
            return Err(BenchError::SourceInputIo {
                path: path.to_path_buf(),
                source: source.into(),
            });
        }
    }
    let (temporary_name, descriptor) = create_temporary_file(&directory, path)?;
    let mut file = std::fs::File::from(descriptor);
    file.set_permissions(unix_source_permissions())
        .map_err(|source| BenchError::SourceInputIo {
            path: path.to_path_buf(),
            source,
        })?;
    let result = (|| -> Result<(), BenchError> {
        file.write_all(bytes)
            .map_err(|source| BenchError::SourceInputIo {
                path: path.to_path_buf(),
                source,
            })?;
        file.sync_all()
            .map_err(|source| BenchError::SourceInputIo {
                path: path.to_path_buf(),
                source,
            })?;
        rustix::fs::renameat(&directory, &temporary_name, &directory, file_name)
            .map_err(source_io(path))?;
        rustix::fs::fsync(&directory).map_err(source_io(path))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = rustix::fs::unlinkat(&directory, &temporary_name, AtFlags::empty());
    }
    result
}

#[cfg(unix)]
fn create_temporary_file(
    directory: &std::os::fd::OwnedFd,
    output: &Path,
) -> Result<(std::ffi::OsString, std::os::fd::OwnedFd), BenchError> {
    use rustix::fs::{Mode, OFlags};

    for _ in 0..128 {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let name =
            std::ffi::OsString::from(format!(".stab-bench-{}-{sequence}.tmp", std::process::id()));
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
            Err(source) => {
                return Err(BenchError::SourceInputIo {
                    path: output.to_path_buf(),
                    source: source.into(),
                });
            }
        }
    }
    Err(BenchError::SourceInput(format!(
        "failed to reserve a unique temporary file for {}",
        output.display()
    )))
}

#[cfg(unix)]
fn split_repo_file_path<'a>(
    root: &RepoRoot,
    path: &'a Path,
) -> Result<(Vec<&'a std::ffi::OsStr>, &'a std::ffi::OsStr), BenchError> {
    let relative = path.strip_prefix(&root.path).map_err(|_| {
        BenchError::SourceInput(format!(
            "source output {} is outside the repository root",
            path.display()
        ))
    })?;
    let mut components = Vec::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(BenchError::SourceInput(format!(
                "source output {} is not a normal repository-relative path",
                path.display()
            )));
        };
        components.push(component);
    }
    let file_name = components.pop().ok_or_else(|| {
        BenchError::SourceInput(format!("source output {} has no file name", path.display()))
    })?;
    Ok((components, file_name))
}

#[cfg(unix)]
fn absolute_normal_components(path: &Path) -> Result<Vec<&std::ffi::OsStr>, BenchError> {
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(BenchError::SourceInput(format!(
            "source input {} is not absolute",
            path.display()
        )));
    }
    let mut normal = Vec::new();
    for component in components {
        let Component::Normal(component) = component else {
            return Err(BenchError::SourceInput(format!(
                "source input {} contains a non-normal path component",
                path.display()
            )));
        };
        normal.push(component);
    }
    Ok(normal)
}

#[cfg(unix)]
fn source_io(path: &Path) -> impl FnOnce(rustix::io::Errno) -> BenchError + '_ {
    |source| BenchError::SourceInputIo {
        path: path.to_path_buf(),
        source: source.into(),
    }
}

#[cfg(unix)]
fn unix_source_permissions() -> std::fs::Permissions {
    use std::os::unix::fs::PermissionsExt;

    std::fs::Permissions::from_mode(0o644)
}

#[cfg(not(unix))]
fn non_unix_unsupported(operation: &str) -> BenchError {
    BenchError::SourceInput(format!(
        "{operation} requires race-resistant Unix descriptor-relative filesystem operations"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn bounded_reader_rejects_oversized_regular_file() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("oversized.json");
        std::fs::write(&path, b"0123456789").expect("write oversized input");

        let error = read_regular_file_bounded(&path, 8).expect_err("oversized input must fail");

        assert!(error.to_string().contains("exceeds 8 bytes"));
    }

    #[cfg(unix)]
    #[test]
    fn repository_reader_rejects_symlink_ancestors() {
        let directory = tempfile::tempdir().expect("temporary repository");
        let outside = tempfile::tempdir().expect("outside directory");
        let root = RepoRoot::resolve(directory.path()).expect("resolve root");
        std::fs::write(outside.path().join("manifest.csv"), b"data").expect("write outside source");
        std::os::unix::fs::symlink(outside.path(), directory.path().join("benchmarks"))
            .expect("create source ancestor symlink");

        let error = read_repo_regular_file_bounded(
            &root,
            &directory.path().join("benchmarks/manifest.csv"),
            1024,
        )
        .expect_err("symlink ancestor must fail");

        assert!(error.to_string().contains("nonsymlink directory"));
    }
}
