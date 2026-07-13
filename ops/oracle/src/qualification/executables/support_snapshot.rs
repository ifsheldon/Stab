use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::io::{Read as _, Write as _};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use super::{ExecutableError, render_sha256};

const MAX_SNAPSHOT_BYTES: u64 = 1 << 30;
const MAX_SNAPSHOT_ENTRIES: u64 = 250_000;

#[derive(Debug)]
pub(super) struct SupportSnapshot {
    _cleanup: SnapshotRootGuard,
    root: PathBuf,
    sources: Vec<PathBuf>,
    paths: Vec<PathBuf>,
    digest: String,
}

impl SupportSnapshot {
    pub(super) fn create(
        parent: &Path,
        name: &'static str,
        sources: &[PathBuf],
    ) -> Result<Self, ExecutableError> {
        if sources.is_empty() {
            return Err(snapshot_error(name, "support snapshot has no source roots"));
        }
        let (cleanup, root) = SnapshotRootGuard::create(parent, name)?;
        let mut canonical_sources = Vec::with_capacity(sources.len());
        let mut paths = Vec::with_capacity(sources.len());
        let mut budget = CopyBudget::default();
        for (index, source) in sources.iter().enumerate() {
            let canonical = std::fs::canonicalize(source).map_err(|error| {
                snapshot_error(
                    name,
                    format!("failed to resolve {}: {error}", source.display()),
                )
            })?;
            let destination = root.join(format!("root-{index:04}"));
            copy_source(
                name,
                &canonical,
                &destination,
                &mut BTreeSet::new(),
                &mut budget,
            )?;
            canonical_sources.push(canonical);
            paths.push(destination);
        }
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o500))
            .map_err(|source| snapshot_error(name, source))?;
        let digest = hash_snapshot(name, &root, &canonical_sources)?;
        let snapshot = Self {
            _cleanup: cleanup,
            root,
            sources: canonical_sources,
            paths,
            digest,
        };
        Ok(snapshot)
    }

    pub(super) fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    pub(super) fn digest(&self) -> &str {
        &self.digest
    }

    pub(super) fn verify(&self, name: &'static str) -> Result<(), ExecutableError> {
        let actual = hash_snapshot(name, &self.root, &self.sources)?;
        if actual == self.digest {
            Ok(())
        } else {
            Err(snapshot_error(
                name,
                format!(
                    "support snapshot changed after preparation: expected {}, found {actual}",
                    self.digest
                ),
            ))
        }
    }
}

#[derive(Debug)]
struct SnapshotRootGuard {
    parent: std::fs::File,
    directory: std::fs::File,
    name: OsString,
}

impl SnapshotRootGuard {
    fn create(parent_path: &Path, name: &'static str) -> Result<(Self, PathBuf), ExecutableError> {
        let parent = crate::safe_file::open_directory(parent_path)
            .map_err(|source| snapshot_error(name, source))?;
        let name_os = OsString::from(name);
        rustix::fs::mkdirat(
            &parent,
            &name_os,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR | rustix::fs::Mode::XUSR,
        )
        .map_err(|source| snapshot_error(name, source))?;
        let directory = match crate::qualification::artifact::open_directory_at(&parent, &name_os) {
            Ok(directory) => directory,
            Err(source) => {
                let _ = rustix::fs::unlinkat(&parent, &name_os, rustix::fs::AtFlags::REMOVEDIR);
                return Err(snapshot_error(name, source));
            }
        };
        let guard = Self {
            parent,
            directory,
            name: name_os,
        };
        rustix::fs::fsync(&guard.parent).map_err(|source| snapshot_error(name, source))?;
        Ok((guard, parent_path.join(name)))
    }
}

impl Drop for SnapshotRootGuard {
    fn drop(&mut self) {
        drop(crate::qualification::artifact::cleanup_owned_directory(
            &self.parent,
            &self.name,
            &self.directory,
        ));
    }
}

#[derive(Default)]
struct CopyBudget {
    entries: u64,
    bytes: u64,
}

fn copy_source(
    name: &'static str,
    source: &Path,
    destination: &Path,
    active_directories: &mut BTreeSet<PathBuf>,
    budget: &mut CopyBudget,
) -> Result<(), ExecutableError> {
    budget.entries = budget
        .entries
        .checked_add(1)
        .ok_or_else(|| snapshot_error(name, "support snapshot entry count overflowed"))?;
    if budget.entries > MAX_SNAPSHOT_ENTRIES {
        return Err(snapshot_error(
            name,
            format!("support snapshot exceeds {MAX_SNAPSHOT_ENTRIES} entries"),
        ));
    }
    let metadata = std::fs::metadata(source).map_err(|error| {
        snapshot_error(
            name,
            format!("failed to inspect {}: {error}", source.display()),
        )
    })?;
    if metadata.is_dir() {
        let canonical = std::fs::canonicalize(source).map_err(|error| {
            snapshot_error(
                name,
                format!("failed to resolve directory {}: {error}", source.display()),
            )
        })?;
        if !active_directories.insert(canonical.clone()) {
            return Err(snapshot_error(
                name,
                format!(
                    "support snapshot contains a directory cycle at {}",
                    source.display()
                ),
            ));
        }
        std::fs::create_dir(destination).map_err(|source| snapshot_error(name, source))?;
        let mut entries = std::fs::read_dir(source)
            .map_err(|error| {
                snapshot_error(
                    name,
                    format!("failed to read {}: {error}", source.display()),
                )
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| snapshot_error(name, source))?;
        entries.sort_by(|left, right| {
            left.file_name()
                .as_bytes()
                .cmp(right.file_name().as_bytes())
        });
        for entry in entries {
            copy_source(
                name,
                &entry.path(),
                &destination.join(entry.file_name()),
                active_directories,
                budget,
            )?;
        }
        active_directories.remove(&canonical);
        std::fs::set_permissions(destination, std::fs::Permissions::from_mode(0o500))
            .map_err(|source| snapshot_error(name, source))?;
        return Ok(());
    }
    if !metadata.is_file() {
        return Err(snapshot_error(
            name,
            format!(
                "support snapshot source {} is not a regular file or directory",
                source.display()
            ),
        ));
    }
    budget.bytes = budget
        .bytes
        .checked_add(metadata.len())
        .ok_or_else(|| snapshot_error(name, "support snapshot byte count overflowed"))?;
    if budget.bytes > MAX_SNAPSHOT_BYTES {
        return Err(snapshot_error(
            name,
            format!("support snapshot exceeds {MAX_SNAPSHOT_BYTES} bytes"),
        ));
    }
    let mut input = std::fs::File::open(source).map_err(|source| snapshot_error(name, source))?;
    if !input
        .metadata()
        .map_err(|source| snapshot_error(name, source))?
        .is_file()
    {
        return Err(snapshot_error(name, "support source changed while opening"));
    }
    let mut output = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|source| snapshot_error(name, source))?;
    let copied =
        std::io::copy(&mut input, &mut output).map_err(|source| snapshot_error(name, source))?;
    if copied != metadata.len() {
        return Err(snapshot_error(name, "support source changed while copying"));
    }
    output
        .flush()
        .map_err(|source| snapshot_error(name, source))?;
    let mode = if metadata.permissions().mode() & 0o111 == 0 {
        0o400
    } else {
        0o500
    };
    output
        .set_permissions(std::fs::Permissions::from_mode(mode))
        .map_err(|source| snapshot_error(name, source))?;
    Ok(())
}

fn hash_snapshot(
    name: &'static str,
    root: &Path,
    sources: &[PathBuf],
) -> Result<String, ExecutableError> {
    let mut hasher = Sha256::new();
    hasher.update(b"stab-cq1/support-snapshot/v1\0");
    let mut budget = CopyBudget::default();
    for (index, source) in sources.iter().enumerate() {
        hash_bytes(&mut hasher, source.as_os_str());
        hash_path(
            name,
            root,
            &root.join(format!("root-{index:04}")),
            &mut budget,
            &mut hasher,
        )?;
    }
    Ok(render_sha256(&hasher.finalize()))
}

fn hash_path(
    name: &'static str,
    root: &Path,
    path: &Path,
    budget: &mut CopyBudget,
    hasher: &mut Sha256,
) -> Result<(), ExecutableError> {
    budget.entries = budget
        .entries
        .checked_add(1)
        .ok_or_else(|| snapshot_error(name, "support verification entry count overflowed"))?;
    if budget.entries > MAX_SNAPSHOT_ENTRIES {
        return Err(snapshot_error(
            name,
            format!("support verification exceeds {MAX_SNAPSHOT_ENTRIES} entries"),
        ));
    }
    let metadata =
        std::fs::symlink_metadata(path).map_err(|source| snapshot_error(name, source))?;
    if metadata.file_type().is_symlink() {
        return Err(snapshot_error(name, "support snapshot contains a symlink"));
    }
    let relative = path
        .strip_prefix(root)
        .map_err(|_| snapshot_error(name, "support snapshot path escaped its root"))?;
    hash_bytes(hasher, relative.as_os_str());
    if metadata.is_dir() {
        hasher.update(b"d");
        let mut entries = std::fs::read_dir(path)
            .map_err(|source| snapshot_error(name, source))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| snapshot_error(name, source))?;
        entries.sort_by(|left, right| {
            left.file_name()
                .as_bytes()
                .cmp(right.file_name().as_bytes())
        });
        for entry in entries {
            hash_path(name, root, &entry.path(), budget, hasher)?;
        }
        return Ok(());
    }
    if !metadata.is_file() {
        return Err(snapshot_error(
            name,
            "support snapshot contains a non-regular entry",
        ));
    }
    hasher.update(b"f");
    hasher.update((metadata.permissions().mode() & 0o111).to_le_bytes());
    hasher.update(metadata.len().to_le_bytes());
    budget.bytes = budget
        .bytes
        .checked_add(metadata.len())
        .ok_or_else(|| snapshot_error(name, "support verification byte count overflowed"))?;
    if budget.bytes > MAX_SNAPSHOT_BYTES {
        return Err(snapshot_error(
            name,
            format!("support verification exceeds {MAX_SNAPSHOT_BYTES} bytes"),
        ));
    }
    let mut file = std::fs::File::open(path).map_err(|source| snapshot_error(name, source))?;
    let mut buffer = [0_u8; 64 << 10];
    let mut read_bytes = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| snapshot_error(name, source))?;
        if read == 0 {
            break;
        }
        hasher.update(
            buffer
                .get(..read)
                .ok_or_else(|| snapshot_error(name, "support hash read exceeded its buffer"))?,
        );
        read_bytes = read_bytes
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| snapshot_error(name, "support hash byte count overflowed"))?;
    }
    if read_bytes != metadata.len() {
        return Err(snapshot_error(
            name,
            "support snapshot changed while hashing",
        ));
    }
    Ok(())
}

fn hash_bytes(hasher: &mut Sha256, value: &OsStr) {
    let bytes = value.as_bytes();
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}

fn snapshot_error(name: &'static str, reason: impl ToString) -> ExecutableError {
    ExecutableError::Build {
        step: name,
        reason: reason.to_string().into_boxed_str(),
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    use super::SupportSnapshot;

    #[test]
    fn support_snapshot_copies_symlink_targets_and_detects_mutation() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let source = temporary.path().join("source");
        let target = temporary.path().join("target");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::create_dir(&target).expect("target directory");
        std::fs::write(source.join("real"), b"content").expect("source file");
        symlink("real", source.join("alias")).expect("source symlink");

        let snapshot = SupportSnapshot::create(&target, "test support snapshot", &[source])
            .expect("support snapshot");
        let copied_root = snapshot.paths().first().expect("copied root");

        assert_eq!(
            std::fs::read(copied_root.join("alias")).expect("copied alias"),
            b"content"
        );
        assert!(
            !std::fs::symlink_metadata(copied_root.join("alias"))
                .expect("alias metadata")
                .file_type()
                .is_symlink()
        );
        snapshot
            .verify("test support snapshot")
            .expect("unchanged snapshot");

        let copied = copied_root.join("real");
        std::fs::set_permissions(&copied, std::fs::Permissions::from_mode(0o600))
            .expect("make copied file writable");
        std::fs::write(&copied, b"changed").expect("mutate copied file");
        assert!(snapshot.verify("test support snapshot").is_err());
    }

    #[test]
    fn support_snapshot_drop_removes_read_only_tree() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let source = temporary.path().join("source");
        let target = temporary.path().join("target");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::create_dir(&target).expect("target directory");
        std::fs::create_dir(source.join("nested")).expect("nested source directory");
        std::fs::write(source.join("nested/content"), b"content").expect("source file");

        let snapshot = SupportSnapshot::create(&target, "drop-test-snapshot", &[source])
            .expect("support snapshot");
        let snapshot_root = snapshot.root.clone();
        assert!(snapshot_root.exists());

        drop(snapshot);

        assert!(!snapshot_root.exists());
    }

    #[test]
    fn support_snapshot_construction_failure_removes_partial_tree() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let source = temporary.path().join("source");
        let missing = temporary.path().join("missing");
        let target = temporary.path().join("target");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::create_dir(&target).expect("target directory");
        std::fs::create_dir(source.join("nested")).expect("nested source directory");
        std::fs::write(source.join("nested/content"), b"content").expect("source file");
        let snapshot_root = target.join("partial-snapshot");

        SupportSnapshot::create(&target, "partial-snapshot", &[source, missing])
            .expect_err("missing later source must fail construction");

        assert!(!snapshot_root.exists());
    }

    #[test]
    fn support_snapshot_cleanup_does_not_remove_a_replacement_root() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let source = temporary.path().join("source");
        let target = temporary.path().join("target");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::create_dir(&target).expect("target directory");
        std::fs::write(source.join("content"), b"owned").expect("source file");

        let snapshot = SupportSnapshot::create(&target, "swapped-snapshot", &[source])
            .expect("support snapshot");
        let snapshot_root = snapshot.root.clone();
        let moved_root = target.join("moved-snapshot");
        std::fs::rename(&snapshot_root, &moved_root).expect("move owned snapshot");
        std::fs::create_dir(&snapshot_root).expect("replacement snapshot root");
        std::fs::write(snapshot_root.join("replacement"), b"keep").expect("replacement marker");

        drop(snapshot);

        assert_eq!(
            std::fs::read(snapshot_root.join("replacement")).expect("replacement marker"),
            b"keep"
        );
        assert_eq!(
            std::fs::read_dir(&moved_root)
                .expect("detached owned root")
                .count(),
            0,
            "descriptor cleanup should empty the owned tree without touching its replacement"
        );
    }
}
