use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
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

    pub(crate) fn source_path(&self) -> &Path {
        &self.source_path
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
    file: File,
    path: &Path,
    limit: u64,
) -> Result<Vec<u8>, ReleaseError> {
    stab_confined_fs::read_bounded(file, path, limit).map_err(ReleaseError::from)
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
        #[cfg(target_os = "linux")]
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
        // The program is executed through `/proc/self/fd/N`. For an ELF binary
        // the kernel resolves that path before exec closes CLOEXEC
        // descriptors, but for a `#!` script the kernel hands the interpreter
        // the `/proc/self/fd/N` path to open after exec, so the descriptor
        // must survive exec. Clearing CLOEXEC has a cost: every child spawned
        // while this program is retained inherits one read-only descriptor of
        // a regular file it could already execute.
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

/// An in-memory copy of exactly `bytes`, sealed so that no process, including
/// this one, can write, grow, shrink, or reseal it. Uploads read from it so
/// the transmitted bytes are the bytes that were validated.
pub(crate) fn sealed_copy(name: &str, bytes: &[u8]) -> Result<File, ReleaseError> {
    #[cfg(target_os = "linux")]
    {
        use rustix::fs::{MemfdFlags, SealFlags};

        let display = PathBuf::from(format!("sealed copy of {name}"));
        let io_error = |source: std::io::Error| ReleaseError::io(&display, source);
        let descriptor =
            rustix::fs::memfd_create(name, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)
                .map_err(|source| io_error(source.into()))?;
        let mut file = File::from(descriptor);
        file.write_all(bytes).map_err(io_error)?;
        let seals = SealFlags::WRITE | SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL;
        rustix::fs::fcntl_add_seals(&file, seals).map_err(|source| io_error(source.into()))?;
        if !rustix::fs::fcntl_get_seals(&file)
            .map_err(|source| io_error(source.into()))?
            .contains(seals)
        {
            return Err(io_error(std::io::Error::other(
                "memfd seals were not applied",
            )));
        }
        file.seek(SeekFrom::Start(0)).map_err(io_error)?;
        Ok(file)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (name, bytes);
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
    stab_confined_fs::open_directory_components(&components, create_missing, display_path)
        .map_err(ReleaseError::from)
}

#[cfg(unix)]
fn open_directory_at(
    directory: &File,
    name: &OsStr,
    display_path: &Path,
) -> Result<File, ReleaseError> {
    stab_confined_fs::open_directory_at(directory, name, display_path).map_err(ReleaseError::from)
}

#[cfg(unix)]
fn open_regular_at(
    directory: &File,
    name: &OsStr,
    display_path: &Path,
) -> Result<File, ReleaseError> {
    stab_confined_fs::open_regular_at(directory, name, display_path).map_err(ReleaseError::from)
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

pub(crate) fn validate_relative(path: &Path) -> Result<(), ReleaseError> {
    if path.is_empty()
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
    stab_confined_fs::split_absolute_path(path).map_err(ReleaseError::from)
}

/// Maps the shared primitives' errors onto the release error variants they always produced.
impl From<stab_confined_fs::ConfinedFsError> for ReleaseError {
    fn from(error: stab_confined_fs::ConfinedFsError) -> Self {
        use stab_confined_fs::ConfinedFsError;
        match error {
            ConfinedFsError::InvalidPath(path) => Self::InvalidPath(path),
            ConfinedFsError::Symlink(path) => Self::SymlinkPath(path),
            ConfinedFsError::NotRegularFile(path) => Self::NotRegularFile(path),
            ConfinedFsError::TooLarge { path, limit } => Self::FileTooLarge { path, limit },
            ConfinedFsError::Io { path, source } => Self::io(&path, source),
        }
    }
}

#[cfg(test)]
mod tests;
