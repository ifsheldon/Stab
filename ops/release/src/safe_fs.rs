use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ReleaseError;

const MAX_REMOVE_DEPTH: usize = 128;
const MAX_REMOVE_ENTRIES: usize = 1_000_000;
const MAX_TOMBSTONE_NAME_ATTEMPTS: usize = 1024;

static NEXT_TOMBSTONE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(crate) struct RetainedDirectory {
    path: PathBuf,
    file: File,
    identity: FileIdentity,
    parent: File,
    name: OsString,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
impl FileIdentity {
    // Match MetadataExt's portable u64 representation of platform dev_t and ino_t.
    #[allow(
        clippy::cast_sign_loss,
        clippy::unnecessary_cast,
        reason = "std::os::unix::fs::MetadataExt normalizes dev_t and ino_t to u64 the same way"
    )]
    fn from_stat(metadata: &rustix::fs::Stat) -> Self {
        Self {
            device: metadata.st_dev as u64,
            inode: metadata.st_ino as u64,
        }
    }
}

#[derive(Debug)]
pub(crate) struct DescriptorProgram {
    path: PathBuf,
    source_path: PathBuf,
    source_identity: FileIdentity,
    _descriptor: std::os::fd::OwnedFd,
}

impl DescriptorProgram {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn revalidate(&self) -> Result<(), ReleaseError> {
        require_same_path_identity(&self.source_path, self.source_identity)
    }
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
            let parent = self
                .file
                .try_clone()
                .map_err(|source| ReleaseError::io(&self.path, source))?;
            Ok(Self {
                path,
                file,
                identity,
                parent,
                name: name.to_os_string(),
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
            let parent = self
                .file
                .try_clone()
                .map_err(|source| ReleaseError::io(&self.path, source))?;
            Ok(Self {
                path,
                file,
                identity,
                parent,
                name: name.to_os_string(),
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

    pub(crate) fn entry_names(&self, limit: usize) -> Result<Vec<OsString>, ReleaseError> {
        self.revalidate()?;
        read_directory_names(&self.file, &self.path, limit)
    }

    pub(crate) fn revalidate(&self) -> Result<(), ReleaseError> {
        let reopened = open_directory_at(&self.parent, &self.name, &self.path)?;
        if file_identity(&reopened, &self.path)? != self.identity {
            return Err(ReleaseError::FileIdentityChanged(self.path.clone()));
        }
        let path_reopened = open_directory(&self.path)?;
        if path_reopened.identity != self.identity {
            return Err(ReleaseError::FileIdentityChanged(self.path.clone()));
        }
        Ok(())
    }

    pub(crate) fn remove_tree(self) -> Result<(), ReleaseError> {
        self.revalidate()?;
        #[cfg(unix)]
        {
            #[cfg(test)]
            run_remove_tree_hook(RemoveTreeHookEvent {
                point: RemoveTreeHookPoint::BeforeRoot,
                original_path: self.path.clone(),
                tombstone_path: None,
            });
            let (tombstone_name, tombstone_path) = move_entry_to_private_tombstone(
                &self.parent,
                &self.name,
                parent_display_path(&self.path),
                &self.path,
                self.identity,
            )?;
            #[cfg(test)]
            run_remove_tree_hook(RemoveTreeHookEvent {
                point: RemoveTreeHookPoint::RootMoved,
                original_path: self.path.clone(),
                tombstone_path: Some(tombstone_path.clone()),
            });
            let mut removed_entries = 0;
            remove_directory_contents(&self.file, &tombstone_path, 0, &mut removed_entries)?;
            remove_moved_directory(
                &self.parent,
                &tombstone_name,
                &tombstone_path,
                self.identity,
            )?;
            self.parent
                .sync_all()
                .map_err(|source| ReleaseError::io(parent_display_path(&self.path), source))?;
            Ok(())
        }
        #[cfg(not(unix))]
        Err(ReleaseError::UnsupportedPlatform)
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

pub(crate) fn descriptor_program(
    file: &File,
    display_path: &Path,
) -> Result<DescriptorProgram, ReleaseError> {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd as _;

        if !file
            .metadata()
            .map_err(|source| ReleaseError::io(display_path, source))?
            .is_file()
        {
            return Err(ReleaseError::NotRegularFile(display_path.to_path_buf()));
        }
        let descriptor = rustix::io::fcntl_dupfd_cloexec(file, 3)
            .map_err(|source| ReleaseError::io(display_path, source.into()))?;
        rustix::io::fcntl_setfd(&descriptor, rustix::io::FdFlags::empty())
            .map_err(|source| ReleaseError::io(display_path, source.into()))?;
        let source_identity = file_identity(file, display_path)?;
        #[cfg(target_os = "linux")]
        let path = PathBuf::from(format!("/proc/self/fd/{}", descriptor.as_raw_fd()));
        #[cfg(not(target_os = "linux"))]
        let path = display_path.to_path_buf();
        Ok(DescriptorProgram {
            path,
            source_path: display_path.to_path_buf(),
            source_identity,
            _descriptor: descriptor,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = (file, display_path);
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
            parent,
            name: name.to_os_string(),
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
        let (parents, name) = split_absolute_file(path)?;
        let parent = open_directory_components(parents, false, path)?;
        let file = open_directory_at(&parent, name, path)?;
        let identity = file_identity(&file, path)?;
        Ok(RetainedDirectory {
            path: path.to_path_buf(),
            file,
            identity,
            parent,
            name: name.to_os_string(),
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

#[cfg(unix)]
fn read_directory_names(
    directory: &File,
    display_path: &Path,
    limit: usize,
) -> Result<Vec<OsString>, ReleaseError> {
    use std::os::unix::ffi::OsStringExt as _;

    let mut entries = rustix::fs::Dir::read_from(directory)
        .map_err(|source| ReleaseError::io(display_path, source.into()))?;
    let mut names = Vec::new();
    while let Some(entry) = entries.read() {
        let entry = entry.map_err(|source| ReleaseError::io(display_path, source.into()))?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        if names.len() == limit {
            return Err(ReleaseError::DirectoryTooLarge {
                path: display_path.to_path_buf(),
                limit,
            });
        }
        names.push(OsString::from_vec(bytes.to_vec()));
    }
    Ok(names)
}

#[cfg(not(unix))]
fn read_directory_names(
    _directory: &File,
    _display_path: &Path,
    _limit: usize,
) -> Result<Vec<OsString>, ReleaseError> {
    Err(ReleaseError::UnsupportedPlatform)
}

#[cfg(unix)]
fn move_entry_to_private_tombstone(
    parent: &File,
    original_name: &OsStr,
    parent_display_path: &Path,
    original_display_path: &Path,
    expected_identity: FileIdentity,
) -> Result<(OsString, PathBuf), ReleaseError> {
    for _ in 0..MAX_TOMBSTONE_NAME_ATTEMPTS {
        let tombstone_name = private_tombstone_name();
        let tombstone_path = parent_display_path.join(&tombstone_name);
        let renamed = rustix::fs::renameat_with(
            parent,
            original_name,
            parent,
            &tombstone_name,
            rustix::fs::RenameFlags::NOREPLACE,
        );
        match renamed {
            Ok(()) => {
                let moved_identity = match identity_at(parent, &tombstone_name, &tombstone_path) {
                    Ok(identity) => identity,
                    Err(error) => {
                        drop(restore_mismatched_tombstone(
                            parent,
                            &tombstone_name,
                            original_name,
                            parent_display_path,
                            &tombstone_path,
                        ));
                        return Err(error);
                    }
                };
                if moved_identity == expected_identity {
                    return Ok((tombstone_name, tombstone_path));
                }
                restore_mismatched_tombstone(
                    parent,
                    &tombstone_name,
                    original_name,
                    parent_display_path,
                    &tombstone_path,
                )?;
                return Err(ReleaseError::FileIdentityChanged(
                    original_display_path.to_path_buf(),
                ));
            }
            Err(rustix::io::Errno::EXIST) => continue,
            Err(rustix::io::Errno::NOENT) => {
                return Err(ReleaseError::FileIdentityChanged(
                    original_display_path.to_path_buf(),
                ));
            }
            Err(source) => return Err(ReleaseError::io(original_display_path, source.into())),
        }
    }
    Err(ReleaseError::OutputExists(
        parent_display_path.join(".stab-release-delete-*"),
    ))
}

#[cfg(unix)]
fn restore_mismatched_tombstone(
    parent: &File,
    tombstone_name: &OsStr,
    original_name: &OsStr,
    parent_display_path: &Path,
    tombstone_path: &Path,
) -> Result<(), ReleaseError> {
    match rustix::fs::renameat_with(
        parent,
        tombstone_name,
        parent,
        original_name,
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) | Err(rustix::io::Errno::EXIST) | Err(rustix::io::Errno::NOENT) => {
            parent
                .sync_all()
                .map_err(|source| ReleaseError::io(parent_display_path, source))?;
            Ok(())
        }
        Err(source) => Err(ReleaseError::io(tombstone_path, source.into())),
    }
}

#[cfg(unix)]
fn remove_moved_directory(
    parent: &File,
    name: &OsStr,
    display_path: &Path,
    expected_identity: FileIdentity,
) -> Result<(), ReleaseError> {
    if identity_at(parent, name, display_path)? != expected_identity {
        return Err(ReleaseError::FileIdentityChanged(
            display_path.to_path_buf(),
        ));
    }
    rustix::fs::unlinkat(parent, name, rustix::fs::AtFlags::REMOVEDIR)
        .map_err(|source| ReleaseError::io(display_path, source.into()))
}

#[cfg(unix)]
fn remove_moved_non_directory(
    parent: &File,
    name: &OsStr,
    display_path: &Path,
    expected_identity: FileIdentity,
) -> Result<(), ReleaseError> {
    if identity_at(parent, name, display_path)? != expected_identity {
        return Err(ReleaseError::FileIdentityChanged(
            display_path.to_path_buf(),
        ));
    }
    rustix::fs::unlinkat(parent, name, rustix::fs::AtFlags::empty())
        .map_err(|source| ReleaseError::io(display_path, source.into()))
}

#[cfg(unix)]
fn private_tombstone_name() -> OsString {
    let id = NEXT_TOMBSTONE_ID.fetch_add(1, Ordering::Relaxed);
    OsString::from(format!(
        ".stab-release-delete-{}-{id:016x}",
        std::process::id()
    ))
}

#[cfg(unix)]
fn identity_at(
    directory: &File,
    name: &OsStr,
    display_path: &Path,
) -> Result<FileIdentity, ReleaseError> {
    let metadata = rustix::fs::statat(directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|source| ReleaseError::io(display_path, source.into()))?;
    Ok(FileIdentity::from_stat(&metadata))
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RemoveTreeHookPoint {
    BeforeRoot,
    RootMoved,
    BeforeEntry,
    EntryMoved,
}

#[cfg(test)]
#[derive(Debug)]
struct RemoveTreeHookEvent {
    point: RemoveTreeHookPoint,
    original_path: PathBuf,
    tombstone_path: Option<PathBuf>,
}

#[cfg(test)]
type RemoveTreeHook = Box<dyn FnMut(RemoveTreeHookEvent)>;

#[cfg(test)]
thread_local! {
    static REMOVE_TREE_HOOK: std::cell::RefCell<Option<RemoveTreeHook>> =
        std::cell::RefCell::new(None);
}

#[cfg(test)]
fn run_remove_tree_hook(event: RemoveTreeHookEvent) {
    REMOVE_TREE_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow_mut().as_mut() {
            hook(event);
        }
    });
}

#[cfg(test)]
fn with_remove_tree_hook<T>(
    hook: impl FnMut(RemoveTreeHookEvent) + 'static,
    body: impl FnOnce() -> T,
) -> T {
    REMOVE_TREE_HOOK.with(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(Box::new(hook));
    });
    let result = body();
    REMOVE_TREE_HOOK.with(|slot| {
        *slot.borrow_mut() = None;
    });
    result
}

#[cfg(unix)]
fn remove_directory_contents(
    directory: &File,
    display_path: &Path,
    depth: usize,
    removed_entries: &mut usize,
) -> Result<(), ReleaseError> {
    if depth >= MAX_REMOVE_DEPTH {
        return Err(ReleaseError::DirectoryDepth {
            path: display_path.to_path_buf(),
            limit: MAX_REMOVE_DEPTH,
        });
    }
    let remaining = MAX_REMOVE_ENTRIES.saturating_sub(*removed_entries);
    let names = read_directory_names(directory, display_path, remaining)?;
    *removed_entries = removed_entries.checked_add(names.len()).ok_or_else(|| {
        ReleaseError::DirectoryTooLarge {
            path: display_path.to_path_buf(),
            limit: MAX_REMOVE_ENTRIES,
        }
    })?;
    for name in names {
        let path = display_path.join(&name);
        let metadata = rustix::fs::statat(directory, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|source| ReleaseError::io(&path, source.into()))?;
        let expected_identity = FileIdentity::from_stat(&metadata);
        #[cfg(test)]
        run_remove_tree_hook(RemoveTreeHookEvent {
            point: RemoveTreeHookPoint::BeforeEntry,
            original_path: path.clone(),
            tombstone_path: None,
        });
        let (tombstone_name, tombstone_path) = move_entry_to_private_tombstone(
            directory,
            &name,
            display_path,
            &path,
            expected_identity,
        )?;
        #[cfg(test)]
        run_remove_tree_hook(RemoveTreeHookEvent {
            point: RemoveTreeHookPoint::EntryMoved,
            original_path: path,
            tombstone_path: Some(tombstone_path.clone()),
        });
        if rustix::fs::FileType::from_raw_mode(metadata.st_mode).is_dir() {
            let child = open_directory_at(directory, &tombstone_name, &tombstone_path)?;
            remove_directory_contents(&child, &tombstone_path, depth + 1, removed_entries)?;
            remove_moved_directory(
                directory,
                &tombstone_name,
                &tombstone_path,
                expected_identity,
            )?;
        } else {
            remove_moved_non_directory(
                directory,
                &tombstone_name,
                &tombstone_path,
                expected_identity,
            )?;
        }
    }
    directory
        .sync_all()
        .map_err(|source| ReleaseError::io(display_path, source))?;
    Ok(())
}

fn parent_display_path(path: &Path) -> &Path {
    path.parent().unwrap_or(path)
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
    use std::cell::Cell;
    use std::fs;
    use std::rc::Rc;

    use super::*;

    #[cfg(unix)]
    #[test]
    fn descriptor_and_path_stats_share_file_identity_normalization() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("artifact");
        fs::write(&path, b"artifact").expect("artifact");
        let file = File::open(&path).expect("file");
        let directory = File::open(root.path()).expect("directory");

        let descriptor_identity = file_identity(&file, &path).expect("descriptor identity");
        let path_identity =
            identity_at(&directory, OsStr::new("artifact"), &path).expect("path identity");

        assert_eq!(descriptor_identity, path_identity);
    }

    #[cfg(unix)]
    #[test]
    fn retained_program_rejects_source_path_replacement() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("program");
        fs::write(&path, b"original").expect("program");
        let file = open_regular_file(&path).expect("file");
        let program = descriptor_program(&file, &path).expect("retained program");

        fs::rename(&path, root.path().join("displaced")).expect("displace program");
        fs::write(&path, b"replacement").expect("replacement program");

        assert!(matches!(
            program.revalidate(),
            Err(ReleaseError::FileIdentityChanged(_))
        ));
    }

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

    #[test]
    fn descriptor_enumeration_is_bounded_and_exact() {
        let root = tempfile::tempdir().expect("root");
        fs::create_dir(root.path().join("target")).expect("target");
        let directory = RetainedDirectory::create_new_under(
            root.path(),
            Path::new("target/release-review"),
            None,
        )
        .expect("retained directory");
        directory
            .write_new(OsStr::new("alpha"), b"a")
            .expect("alpha");
        directory.write_new(OsStr::new("beta"), b"b").expect("beta");

        let mut names = directory.entry_names(2).expect("entry names");
        names.sort();
        assert_eq!(names, [OsString::from("alpha"), OsString::from("beta")]);
        assert!(matches!(
            directory.entry_names(1),
            Err(ReleaseError::DirectoryTooLarge { limit: 1, .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn descriptor_cleanup_removes_nested_entries_without_following_symlinks() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("root");
        fs::create_dir(root.path().join("target")).expect("target");
        let work = RetainedDirectory::create_new_under(
            root.path(),
            Path::new("target/release-work"),
            None,
        )
        .expect("work");
        let nested = work.create_directory(OsStr::new("nested")).expect("nested");
        nested
            .write_new(OsStr::new("artifact"), b"bytes")
            .expect("artifact");
        let outside = root.path().join("outside");
        fs::write(&outside, b"sentinel").expect("outside");
        symlink(&outside, work.path().join("alias")).expect("symlink");
        let work_path = work.path().to_path_buf();

        work.remove_tree().expect("remove tree");
        assert!(!work_path.exists());
        assert_eq!(fs::read(&outside).expect("outside bytes"), b"sentinel");
    }

    #[test]
    fn descriptor_cleanup_rejects_replaced_root_without_deleting_it() {
        let root = tempfile::tempdir().expect("root");
        fs::create_dir(root.path().join("target")).expect("target");
        let work = RetainedDirectory::create_new_under(
            root.path(),
            Path::new("target/release-work"),
            None,
        )
        .expect("work");
        let work_path = work.path().to_path_buf();
        fs::rename(&work_path, root.path().join("target/displaced")).expect("displace");
        fs::create_dir(&work_path).expect("replacement");
        fs::write(work_path.join("sentinel"), b"replacement").expect("sentinel");

        assert!(matches!(
            work.remove_tree(),
            Err(ReleaseError::FileIdentityChanged(_))
        ));
        assert_eq!(
            fs::read(work_path.join("sentinel")).expect("replacement survives"),
            b"replacement"
        );
    }

    #[cfg(unix)]
    #[test]
    fn descriptor_cleanup_preserves_root_replacement_installed_before_tombstone_move() {
        let root = tempfile::tempdir().expect("root");
        fs::create_dir(root.path().join("target")).expect("target");
        let work = RetainedDirectory::create_new_under(
            root.path(),
            Path::new("target/release-work"),
            None,
        )
        .expect("work");
        work.write_new(OsStr::new("artifact"), b"original")
            .expect("artifact");
        let work_path = work.path().to_path_buf();
        let displaced_path = root.path().join("target/displaced-original");
        let replacement_seen = Rc::new(Cell::new(false));
        let hook_seen = Rc::clone(&replacement_seen);
        let hook_work_path = work_path.clone();
        let hook_displaced_path = displaced_path.clone();

        let result = with_remove_tree_hook(
            move |event| {
                if event.point == RemoveTreeHookPoint::BeforeRoot
                    && event.original_path == hook_work_path
                    && !hook_seen.replace(true)
                {
                    fs::rename(&hook_work_path, &hook_displaced_path).expect("displace retained");
                    fs::create_dir(&hook_work_path).expect("replacement root");
                    fs::write(hook_work_path.join("sentinel"), b"replacement")
                        .expect("replacement sentinel");
                }
            },
            || work.remove_tree(),
        );

        assert!(matches!(result, Err(ReleaseError::FileIdentityChanged(_))));
        assert!(replacement_seen.get());
        assert_eq!(
            fs::read(work_path.join("sentinel")).expect("replacement survives"),
            b"replacement"
        );
        assert_eq!(
            fs::read(displaced_path.join("artifact")).expect("retained tree survives"),
            b"original"
        );
    }

    #[cfg(unix)]
    #[test]
    fn descriptor_cleanup_preserves_root_replacement_installed_after_tombstone_move() {
        let root = tempfile::tempdir().expect("root");
        fs::create_dir(root.path().join("target")).expect("target");
        let work = RetainedDirectory::create_new_under(
            root.path(),
            Path::new("target/release-work"),
            None,
        )
        .expect("work");
        work.write_new(OsStr::new("artifact"), b"original")
            .expect("artifact");
        let work_path = work.path().to_path_buf();
        let replacement_seen = Rc::new(Cell::new(false));
        let hook_seen = Rc::clone(&replacement_seen);
        let hook_work_path = work_path.clone();

        with_remove_tree_hook(
            move |event| {
                if event.point == RemoveTreeHookPoint::RootMoved
                    && event.original_path == hook_work_path
                    && !hook_seen.replace(true)
                {
                    let tombstone_path = event.tombstone_path.expect("tombstone path");
                    assert!(tombstone_path.exists());
                    fs::create_dir(&hook_work_path).expect("replacement root");
                    fs::write(hook_work_path.join("sentinel"), b"replacement")
                        .expect("replacement sentinel");
                }
            },
            || work.remove_tree(),
        )
        .expect("remove retained tree");

        assert!(replacement_seen.get());
        assert_eq!(
            fs::read(work_path.join("sentinel")).expect("replacement survives"),
            b"replacement"
        );
        assert!(!work_path.join("artifact").exists());
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
