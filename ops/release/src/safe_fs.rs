use std::ffi::OsStr;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use crate::ReleaseError;

#[derive(Debug)]
pub(crate) struct RetainedDirectory {
    path: PathBuf,
    file: File,
    identity: FileIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileIdentity {
    device: u64,
    inode: u64,
}

impl RetainedDirectory {
    pub(crate) fn create_new_under(
        root: &Path,
        relative: &Path,
        required_prefix: Option<&Path>,
    ) -> Result<Self, ReleaseError> {
        validate_relative(relative)?;
        if required_prefix.is_some_and(|prefix| !relative.starts_with(prefix)) {
            return Err(ReleaseError::InvalidPath(relative.to_path_buf()));
        }
        let absolute = root.join(relative);
        create_new_directory(&absolute)
    }

    pub(crate) fn open_under(
        root: &Path,
        relative: &Path,
        required_prefix: Option<&Path>,
    ) -> Result<Self, ReleaseError> {
        validate_relative(relative)?;
        if required_prefix.is_some_and(|prefix| !relative.starts_with(prefix)) {
            return Err(ReleaseError::InvalidPath(relative.to_path_buf()));
        }
        open_directory(&root.join(relative))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn create_directory(&self, name: &OsStr) -> Result<Self, ReleaseError> {
        validate_name(name, &self.path)?;
        self.revalidate()?;
        #[cfg(unix)]
        {
            use rustix::fs::Mode;

            rustix::fs::mkdirat(
                &self.file,
                name,
                Mode::RUSR
                    | Mode::WUSR
                    | Mode::XUSR
                    | Mode::RGRP
                    | Mode::XGRP
                    | Mode::ROTH
                    | Mode::XOTH,
            )
            .map_err(|source| {
                let path = self.path.join(name);
                if source == rustix::io::Errno::EXIST {
                    ReleaseError::OutputExists(path)
                } else {
                    ReleaseError::io(path, source.into())
                }
            })?;
            let path = self.path.join(name);
            let file = open_directory_at(&self.file, name, &path)?;
            let identity = file_identity(&file, &path)?;
            Ok(Self {
                path,
                file,
                identity,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = name;
            Err(ReleaseError::UnsupportedPlatform)
        }
    }

    pub(crate) fn open_directory(&self, name: &OsStr) -> Result<Self, ReleaseError> {
        validate_name(name, &self.path)?;
        self.revalidate()?;
        #[cfg(unix)]
        {
            let path = self.path.join(name);
            let file = open_directory_at(&self.file, name, &path)?;
            let identity = file_identity(&file, &path)?;
            Ok(Self {
                path,
                file,
                identity,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = name;
            Err(ReleaseError::UnsupportedPlatform)
        }
    }

    pub(crate) fn write_new(&self, name: &OsStr, bytes: &[u8]) -> Result<File, ReleaseError> {
        let mut file = self.create_new_file(name)?;
        file.write_all(bytes)
            .map_err(|source| ReleaseError::io(self.path.join(name), source))?;
        file.sync_all()
            .map_err(|source| ReleaseError::io(self.path.join(name), source))?;
        self.sync()?;
        Ok(file)
    }

    pub(crate) fn open_regular(&self, name: &OsStr) -> Result<File, ReleaseError> {
        validate_name(name, &self.path)?;
        self.revalidate()?;
        open_regular_at(&self.file, name, &self.path.join(name))
    }

    pub(crate) fn read_bounded(&self, name: &OsStr, limit: u64) -> Result<Vec<u8>, ReleaseError> {
        let file = self.open_regular(name)?;
        read_bounded_file(file, &self.path.join(name), limit)
    }

    pub(crate) fn revalidate(&self) -> Result<(), ReleaseError> {
        let reopened = open_directory(&self.path)?;
        if reopened.identity != self.identity {
            return Err(ReleaseError::FileIdentityChanged(self.path.clone()));
        }
        Ok(())
    }

    pub(crate) fn sync(&self) -> Result<(), ReleaseError> {
        self.file
            .sync_all()
            .map_err(|source| ReleaseError::io(&self.path, source))
    }

    pub(crate) fn make_read_only(&self) -> Result<(), ReleaseError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            self.file
                .set_permissions(std::fs::Permissions::from_mode(0o555))
                .map_err(|source| ReleaseError::io(&self.path, source))?;
            self.sync()
        }
        #[cfg(not(unix))]
        Err(ReleaseError::UnsupportedPlatform)
    }

    fn create_new_file(&self, name: &OsStr) -> Result<File, ReleaseError> {
        validate_name(name, &self.path)?;
        self.revalidate()?;
        #[cfg(unix)]
        {
            use rustix::fs::{Mode, OFlags};

            let path = self.path.join(name);
            let descriptor = rustix::fs::openat(
                &self.file,
                name,
                OFlags::WRONLY
                    | OFlags::CLOEXEC
                    | OFlags::CREATE
                    | OFlags::EXCL
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK,
                Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH,
            )
            .map_err(|source| {
                if source == rustix::io::Errno::EXIST {
                    ReleaseError::OutputExists(path.clone())
                } else {
                    ReleaseError::io(&path, source.into())
                }
            })?;
            let file = File::from(descriptor);
            if !file
                .metadata()
                .map_err(|source| ReleaseError::io(&path, source))?
                .is_file()
            {
                return Err(ReleaseError::NotRegularFile(path));
            }
            Ok(file)
        }
        #[cfg(not(unix))]
        {
            let _ = name;
            Err(ReleaseError::UnsupportedPlatform)
        }
    }
}

pub(crate) fn open_regular_file(path: &Path) -> Result<File, ReleaseError> {
    #[cfg(unix)]
    {
        let (parents, name) = split_absolute_file(path)?;
        let directory = open_directory_components(parents, false, path)?;
        open_regular_at(&directory, name, path)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(ReleaseError::UnsupportedPlatform)
    }
}

pub(crate) fn read_bounded_file(
    mut file: File,
    path: &Path,
    limit: u64,
) -> Result<Vec<u8>, ReleaseError> {
    let metadata = file
        .metadata()
        .map_err(|source| ReleaseError::io(path, source))?;
    if !metadata.is_file() {
        return Err(ReleaseError::NotRegularFile(path.to_path_buf()));
    }
    if metadata.len() > limit {
        return Err(ReleaseError::FileTooLarge {
            path: path.to_path_buf(),
            limit,
        });
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|source| ReleaseError::io(path, source))?;
    let capacity = usize::try_from(metadata.len()).map_err(|_| ReleaseError::FileTooLarge {
        path: path.to_path_buf(),
        limit,
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    Read::by_ref(&mut file)
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| ReleaseError::io(path, source))?;
    if !matches!(u64::try_from(bytes.len()), Ok(size) if size <= limit) {
        return Err(ReleaseError::FileTooLarge {
            path: path.to_path_buf(),
            limit,
        });
    }
    Ok(bytes)
}

pub(crate) fn file_identity(file: &File, path: &Path) -> Result<FileIdentity, ReleaseError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        let metadata = file
            .metadata()
            .map_err(|source| ReleaseError::io(path, source))?;
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    {
        let _ = (file, path);
        Err(ReleaseError::UnsupportedPlatform)
    }
}

pub(crate) fn require_same_path_identity(
    path: &Path,
    expected: FileIdentity,
) -> Result<(), ReleaseError> {
    let reopened = open_regular_file(path)?;
    if file_identity(&reopened, path)? != expected {
        return Err(ReleaseError::FileIdentityChanged(path.to_path_buf()));
    }
    Ok(())
}

fn create_new_directory(path: &Path) -> Result<RetainedDirectory, ReleaseError> {
    #[cfg(unix)]
    {
        use rustix::fs::Mode;

        let (parents, name) = split_absolute_file(path)?;
        let parent = open_directory_components(parents, true, path)?;
        rustix::fs::mkdirat(
            &parent,
            name,
            Mode::RUSR
                | Mode::WUSR
                | Mode::XUSR
                | Mode::RGRP
                | Mode::XGRP
                | Mode::ROTH
                | Mode::XOTH,
        )
        .map_err(|source| {
            if source == rustix::io::Errno::EXIST {
                ReleaseError::OutputExists(path.to_path_buf())
            } else {
                ReleaseError::io(path, source.into())
            }
        })?;
        let file = open_directory_at(&parent, name, path)?;
        let identity = file_identity(&file, path)?;
        Ok(RetainedDirectory {
            path: path.to_path_buf(),
            file,
            identity,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(ReleaseError::UnsupportedPlatform)
    }
}

fn open_directory(path: &Path) -> Result<RetainedDirectory, ReleaseError> {
    #[cfg(unix)]
    {
        let components = absolute_components(path)?;
        let file = open_directory_components(components, false, path)?;
        let identity = file_identity(&file, path)?;
        Ok(RetainedDirectory {
            path: path.to_path_buf(),
            file,
            identity,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(ReleaseError::UnsupportedPlatform)
    }
}

#[cfg(unix)]
fn open_directory_components(
    components: Vec<&OsStr>,
    create_missing: bool,
    display_path: &Path,
) -> Result<File, ReleaseError> {
    use rustix::fs::{Mode, OFlags};

    let root = rustix::fs::open(
        "/",
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|source| ReleaseError::io(display_path, source.into()))?;
    let mut directory = File::from(root);
    for component in components {
        match open_directory_at(&directory, component, display_path) {
            Ok(next) => directory = next,
            Err(ReleaseError::Io { source, .. })
                if create_missing && source.kind() == std::io::ErrorKind::NotFound =>
            {
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
                .map_err(|source| ReleaseError::io(display_path, source.into()))?;
                directory = open_directory_at(&directory, component, display_path)?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(directory)
}

#[cfg(unix)]
fn open_directory_at(
    directory: &File,
    name: &OsStr,
    display_path: &Path,
) -> Result<File, ReleaseError> {
    use rustix::fs::{Mode, OFlags};

    let descriptor = rustix::fs::openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|source| {
        if source == rustix::io::Errno::LOOP {
            ReleaseError::SymlinkPath(display_path.to_path_buf())
        } else {
            ReleaseError::io(display_path, source.into())
        }
    })?;
    Ok(File::from(descriptor))
}

#[cfg(unix)]
fn open_regular_at(
    directory: &File,
    name: &OsStr,
    display_path: &Path,
) -> Result<File, ReleaseError> {
    use rustix::fs::{Mode, OFlags};

    let descriptor = rustix::fs::openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|source| {
        if source == rustix::io::Errno::LOOP {
            ReleaseError::SymlinkPath(display_path.to_path_buf())
        } else {
            ReleaseError::io(display_path, source.into())
        }
    })?;
    let file = File::from(descriptor);
    if !file
        .metadata()
        .map_err(|source| ReleaseError::io(display_path, source))?
        .is_file()
    {
        return Err(ReleaseError::NotRegularFile(display_path.to_path_buf()));
    }
    Ok(file)
}

fn validate_relative(path: &Path) -> Result<(), ReleaseError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ReleaseError::InvalidPath(path.to_path_buf()));
    }
    Ok(())
}

fn validate_name(name: &OsStr, parent: &Path) -> Result<(), ReleaseError> {
    let path = Path::new(name);
    if name.is_empty()
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(ReleaseError::InvalidPath(parent.join(path)));
    }
    Ok(())
}

#[cfg(unix)]
fn split_absolute_file(path: &Path) -> Result<(Vec<&OsStr>, &OsStr), ReleaseError> {
    let mut components = absolute_components(path)?;
    let name = components
        .pop()
        .ok_or_else(|| ReleaseError::InvalidPath(path.to_path_buf()))?;
    Ok((components, name))
}

#[cfg(unix)]
fn absolute_components(path: &Path) -> Result<Vec<&OsStr>, ReleaseError> {
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(ReleaseError::InvalidPath(path.to_path_buf()));
    }
    let mut normal = Vec::new();
    for component in components {
        let Component::Normal(component) = component else {
            return Err(ReleaseError::InvalidPath(path.to_path_buf()));
        };
        normal.push(component);
    }
    Ok(normal)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn retained_directory_rejects_replacement_and_symlink_entries() {
        let root = tempfile::tempdir().expect("root");
        fs::create_dir(root.path().join("target")).expect("target");
        let directory = RetainedDirectory::create_new_under(
            root.path(),
            Path::new("target/release-review"),
            None,
        )
        .expect("retained directory");
        directory
            .write_new(OsStr::new("reviewed.crate"), b"reviewed")
            .expect("reviewed file");

        fs::rename(
            root.path().join("target/release-review"),
            root.path().join("target/displaced"),
        )
        .expect("displace directory");
        fs::create_dir(root.path().join("target/release-review")).expect("replacement");
        assert!(matches!(
            directory.revalidate(),
            Err(ReleaseError::FileIdentityChanged(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn regular_file_open_rejects_symlink_substitution() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("root");
        let real = root.path().join("real");
        fs::write(&real, b"bytes").expect("real file");
        let alias = root.path().join("alias");
        symlink(&real, &alias).expect("symlink");
        assert!(matches!(
            open_regular_file(&alias),
            Err(ReleaseError::SymlinkPath(_))
        ));
    }
}
