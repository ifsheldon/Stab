use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::io::{Read as _, Write as _};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;

use crate::root::RepoRoot;

const OUTPUT_PREFIX: [&str; 3] = ["target", "benchmarks", "qualification"];
const PUBLICATION_LOCK: &str = ".publication.lock";
const MAX_ARTIFACT_BYTES: usize = 64 << 20;
const MAX_DIRECTORY_ENTRIES: usize = 16;
const MAX_DIRECT_ARTIFACT_NAME_BYTES: usize = 128;
const ARTIFACT_NAMES: [&str; 3] = ["preflight.json", "report.json", "report.md"];
static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct DirectQualificationArtifactPath(PathBuf);

impl DirectQualificationArtifactPath {
    pub(crate) fn try_new(path: &Path) -> Result<Self, ArtifactError> {
        let components = validate_output(path)?;
        if components.len() != OUTPUT_PREFIX.len() + 1 {
            return Err(ArtifactError::NonDirectArtifact(path.to_path_buf()));
        }
        let Some(name) = components.last().and_then(|component| component.to_str()) else {
            return Err(ArtifactError::NonDirectArtifact(path.to_path_buf()));
        };
        if name.len() > MAX_DIRECT_ARTIFACT_NAME_BYTES
            || !name
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(ArtifactError::NonDirectArtifact(path.to_path_buf()));
        }
        Ok(Self(components.into_iter().collect()))
    }

    pub(crate) fn as_path(&self) -> &Path {
        &self.0
    }

    pub(crate) fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

#[derive(Debug)]
pub(crate) struct QualificationOutput {
    relative: PathBuf,
    parent: OwnedFd,
    staging: OwnedFd,
    staging_name: OsString,
    target_name: OsString,
    staging_active: bool,
    _lock: OwnedFd,
}

impl QualificationOutput {
    pub(crate) fn begin(root: &RepoRoot, relative: &Path) -> Result<Self, ArtifactError> {
        ensure_linux()?;
        let components = validate_output(relative)?;
        let target_name = components
            .last()
            .ok_or_else(|| ArtifactError::InvalidOutput(relative.to_path_buf()))?
            .to_os_string();
        let parent_components = components
            .get(..components.len().saturating_sub(1))
            .ok_or_else(|| ArtifactError::InvalidOutput(relative.to_path_buf()))?;
        let repository = open_directory(&root.path)?;
        let parent = open_or_create_directories(&repository, parent_components)?;
        let lock = rustix::fs::openat(
            &parent,
            PUBLICATION_LOCK,
            rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .map_err(ArtifactError::Io)?;
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockExclusive)
            .map_err(ArtifactError::Io)?;
        let (staging_name, staging) = create_staging_directory(&parent)?;
        Ok(Self {
            relative: relative.to_path_buf(),
            parent,
            staging,
            staging_name,
            target_name,
            staging_active: true,
            _lock: lock,
        })
    }

    pub(crate) fn write(&self, name: &'static str, bytes: &[u8]) -> Result<(), ArtifactError> {
        if !ARTIFACT_NAMES.contains(&name) {
            return Err(ArtifactError::InvalidArtifactName(name));
        }
        if bytes.len() > MAX_ARTIFACT_BYTES {
            return Err(ArtifactError::ArtifactTooLarge {
                name,
                actual: bytes.len(),
                maximum: MAX_ARTIFACT_BYTES,
            });
        }
        let descriptor = rustix::fs::openat(
            &self.staging,
            name,
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::RUSR
                | rustix::fs::Mode::WUSR
                | rustix::fs::Mode::RGRP
                | rustix::fs::Mode::ROTH,
        )
        .map_err(ArtifactError::Io)?;
        let mut file = std::fs::File::from(descriptor);
        file.write_all(bytes).map_err(ArtifactError::Write)?;
        file.sync_all().map_err(ArtifactError::Write)
    }

    pub(crate) fn commit(mut self) -> Result<(), ArtifactError> {
        rustix::fs::fsync(&self.staging).map_err(ArtifactError::Io)?;
        match open_directory_at(&self.parent, &self.target_name) {
            Ok(previous) => {
                let previous_names = validate_existing_output(&previous)?;
                rustix::fs::renameat_with(
                    &self.parent,
                    &self.staging_name,
                    &self.parent,
                    &self.target_name,
                    rustix::fs::RenameFlags::EXCHANGE,
                )
                .map_err(ArtifactError::Io)?;
                self.staging_active = false;
                remove_known_output(&self.parent, &previous, &self.staging_name, &previous_names)?;
            }
            Err(rustix::io::Errno::NOENT) => {
                rustix::fs::renameat(
                    &self.parent,
                    &self.staging_name,
                    &self.parent,
                    &self.target_name,
                )
                .map_err(ArtifactError::Io)?;
                self.staging_active = false;
            }
            Err(source) => return Err(ArtifactError::Io(source)),
        }
        rustix::fs::fsync(&self.parent).map_err(ArtifactError::Io)?;
        Ok(())
    }

    pub(crate) fn relative(&self) -> &Path {
        &self.relative
    }

    pub(crate) fn require_current_artifact(
        &self,
        name: &'static str,
        expected: &[u8],
    ) -> Result<(), ArtifactError> {
        if !ARTIFACT_NAMES.contains(&name) {
            return Err(ArtifactError::InvalidArtifactName(name));
        }
        let target =
            open_directory_at(&self.parent, &self.target_name).map_err(ArtifactError::Io)?;
        let current = read_artifact_from_directory(&target, name, MAX_ARTIFACT_BYTES)?;
        if current == expected {
            Ok(())
        } else {
            Err(ArtifactError::ConcurrentReplacement(name))
        }
    }

    pub(crate) fn require_sibling_artifact_digest(
        &self,
        source_path: &DirectQualificationArtifactPath,
        name: &'static str,
        expected_sha256: &str,
        maximum_bytes: usize,
    ) -> Result<(), ArtifactError> {
        if !ARTIFACT_NAMES.contains(&name) {
            return Err(ArtifactError::InvalidArtifactName(name));
        }
        DirectQualificationArtifactPath::try_new(&self.relative)?;
        let source_components = validate_output(source_path.as_path())?;
        let source_name = source_components
            .last()
            .ok_or_else(|| ArtifactError::NonDirectArtifact(source_path.as_path().to_path_buf()))?;
        if *source_name == self.target_name {
            return Err(ArtifactError::NonSiblingArtifact(
                source_path.as_path().to_path_buf(),
            ));
        }
        let source = open_directory_at(&self.parent, source_name).map_err(ArtifactError::Io)?;
        let current = read_artifact_from_directory(&source, name, maximum_bytes)?;
        if super::run::sha256_hex(&current) == expected_sha256 {
            Ok(())
        } else {
            Err(ArtifactError::ConcurrentReplacement(name))
        }
    }
}

#[cfg(test)]
pub(crate) fn read_artifact(
    root: &RepoRoot,
    relative: &Path,
    name: &'static str,
) -> Result<Vec<u8>, ArtifactError> {
    read_artifact_bounded(root, relative, name, MAX_ARTIFACT_BYTES)
}

pub(crate) fn read_artifact_bounded(
    root: &RepoRoot,
    relative: &Path,
    name: &'static str,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ArtifactError> {
    ensure_linux()?;
    if !ARTIFACT_NAMES.contains(&name) {
        return Err(ArtifactError::InvalidArtifactName(name));
    }
    if maximum_bytes > MAX_ARTIFACT_BYTES {
        return Err(ArtifactError::InvalidReadLimit(maximum_bytes));
    }
    let components = validate_output(relative)?;
    let (target_name, parent_components) = components
        .split_last()
        .ok_or_else(|| ArtifactError::InvalidOutput(relative.to_path_buf()))?;
    let repository = open_directory(&root.path)?;
    let parent = open_existing_directories(&repository, parent_components)?;
    let lock = rustix::fs::openat(
        &parent,
        PUBLICATION_LOCK,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .map_err(ArtifactError::Io)?;
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockShared).map_err(ArtifactError::Io)?;
    let target = open_directory_at(&parent, target_name).map_err(ArtifactError::Io)?;
    read_artifact_from_directory(&target, name, maximum_bytes)
}

fn read_artifact_from_directory(
    directory: &OwnedFd,
    name: &'static str,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ArtifactError> {
    let descriptor = rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(ArtifactError::Io)?;
    let mut file = std::fs::File::from(descriptor);
    let metadata = file.metadata().map_err(ArtifactError::Write)?;
    let maximum = u64::try_from(maximum_bytes).map_err(|_| ArtifactError::SizeOverflow)?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(ArtifactError::UnsafeArtifact(name));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len()).map_err(|_| ArtifactError::SizeOverflow)?,
    );
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(ArtifactError::Write)?;
    if bytes.len() > maximum_bytes {
        return Err(ArtifactError::ArtifactTooLarge {
            name,
            actual: bytes.len(),
            maximum: maximum_bytes,
        });
    }
    Ok(bytes)
}

impl Drop for QualificationOutput {
    fn drop(&mut self) {
        if self.staging_active && open_directory_at(&self.parent, &self.staging_name).is_ok() {
            for name in ARTIFACT_NAMES {
                let _ = rustix::fs::unlinkat(&self.staging, name, rustix::fs::AtFlags::empty());
            }
            let _ = rustix::fs::unlinkat(
                &self.parent,
                &self.staging_name,
                rustix::fs::AtFlags::REMOVEDIR,
            );
        }
    }
}

fn validate_output(path: &Path) -> Result<Vec<&OsStr>, ArtifactError> {
    if path.is_absolute() || path.to_str().is_none() {
        return Err(ArtifactError::InvalidOutput(path.to_path_buf()));
    }
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(ArtifactError::InvalidOutput(path.to_path_buf()));
        };
        components.push(component);
    }
    if components.len() <= OUTPUT_PREFIX.len()
        || !OUTPUT_PREFIX
            .iter()
            .zip(&components)
            .all(|(expected, actual)| OsStr::new(expected) == *actual)
    {
        return Err(ArtifactError::InvalidOutput(path.to_path_buf()));
    }
    Ok(components)
}

fn open_directory(path: &Path) -> Result<OwnedFd, ArtifactError> {
    rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(ArtifactError::Io)
}

fn open_directory_at(parent: &OwnedFd, name: &OsStr) -> Result<OwnedFd, rustix::io::Errno> {
    rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
}

fn open_or_create_directories(
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

fn open_existing_directories(
    root: &OwnedFd,
    components: &[&OsStr],
) -> Result<OwnedFd, ArtifactError> {
    let mut current = rustix::io::dup(root).map_err(ArtifactError::Io)?;
    for component in components {
        current = open_directory_at(&current, component).map_err(ArtifactError::Io)?;
    }
    Ok(current)
}

fn create_staging_directory(parent: &OwnedFd) -> Result<(OsString, OwnedFd), ArtifactError> {
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

fn validate_existing_output(directory: &OwnedFd) -> Result<BTreeSet<OsString>, ArtifactError> {
    let names = directory_names(directory)?;
    let allowed = ARTIFACT_NAMES
        .iter()
        .map(|name| OsString::from(*name))
        .collect::<BTreeSet<_>>();
    if !names.is_subset(&allowed) {
        return Err(ArtifactError::UnexpectedExistingArtifacts(names));
    }
    for name in &names {
        let descriptor = rustix::fs::openat(
            directory,
            name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )
        .map_err(ArtifactError::Io)?;
        if !std::fs::File::from(descriptor)
            .metadata()
            .map_err(ArtifactError::Write)?
            .is_file()
        {
            return Err(ArtifactError::UnexpectedExistingArtifacts(names));
        }
    }
    Ok(names)
}

fn directory_names(directory: &OwnedFd) -> Result<BTreeSet<OsString>, ArtifactError> {
    let descriptor = rustix::io::dup(directory).map_err(ArtifactError::Io)?;
    let mut buffer = [MaybeUninit::uninit(); 8192];
    let mut entries = rustix::fs::RawDir::new(descriptor, &mut buffer);
    let mut names = BTreeSet::new();
    while let Some(entry) = entries.next() {
        let entry = entry.map_err(ArtifactError::Io)?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        if names.len() == MAX_DIRECTORY_ENTRIES {
            return Err(ArtifactError::TooManyExistingArtifacts);
        }
        use std::os::unix::ffi::OsStringExt as _;
        names.insert(OsString::from_vec(name.to_vec()));
    }
    Ok(names)
}

fn remove_known_output(
    parent: &OwnedFd,
    previous: &OwnedFd,
    previous_name: &OsStr,
    names: &BTreeSet<OsString>,
) -> Result<(), ArtifactError> {
    for name in names {
        rustix::fs::unlinkat(previous, name, rustix::fs::AtFlags::empty())
            .map_err(ArtifactError::Io)?;
    }
    rustix::fs::unlinkat(parent, previous_name, rustix::fs::AtFlags::REMOVEDIR)
        .map_err(ArtifactError::Io)
}

fn ensure_linux() -> Result<(), ArtifactError> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(ArtifactError::UnsupportedHost)
    }
}

#[derive(Debug, Error)]
pub(crate) enum ArtifactError {
    #[error("performance qualification artifact publication requires Linux")]
    UnsupportedHost,
    #[error(
        "qualification output must be a normal repository-relative directory below target/benchmarks/qualification: {0}"
    )]
    InvalidOutput(PathBuf),
    #[error("qualification artifact name is not source-owned: {0}")]
    InvalidArtifactName(&'static str),
    #[error("qualification artifact {name} is {actual} bytes, exceeding {maximum}")]
    ArtifactTooLarge {
        name: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("qualification output contains unexpected existing artifacts: {0:?}")]
    UnexpectedExistingArtifacts(BTreeSet<OsString>),
    #[error("qualification output contains too many existing artifacts")]
    TooManyExistingArtifacts,
    #[error("failed to reserve a unique qualification staging directory")]
    NoStagingName,
    #[error("qualification artifact filesystem operation failed: {0}")]
    Io(rustix::io::Errno),
    #[error("qualification artifact write failed: {0}")]
    Write(std::io::Error),
    #[error("qualification artifact is not a bounded regular file: {0}")]
    UnsafeArtifact(&'static str),
    #[error("qualification artifact {0} changed while its derived report was being validated")]
    ConcurrentReplacement(&'static str),
    #[error("qualification rollup source is not a direct sibling artifact: {0}")]
    NonSiblingArtifact(PathBuf),
    #[error("qualification artifact is not a direct child of its source-owned root: {0}")]
    NonDirectArtifact(PathBuf),
    #[error("qualification artifact size cannot be represented on this host")]
    SizeOverflow,
    #[error("qualification artifact read limit exceeds the source-owned maximum: {0}")]
    InvalidReadLimit(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_path_rejects_escape_and_shallow_targets() {
        assert!(validate_output(Path::new("target/benchmarks/qualification/pr")).is_ok());
        assert!(validate_output(Path::new("target/benchmarks/qualification")).is_err());
        assert!(validate_output(Path::new("target/benchmarks/qualification/../outside")).is_err());
        assert!(validate_output(Path::new("/tmp/qualification")).is_err());
        assert!(
            DirectQualificationArtifactPath::try_new(Path::new(
                "target/benchmarks/qualification/pr"
            ))
            .is_ok()
        );
        for unsafe_name in [
            "nested/pr",
            ".publication.lock",
            ".run-1-0.staging",
            "bad|row",
            "bad`code",
            "bad\nrow",
        ] {
            let path = PathBuf::from("target/benchmarks/qualification").join(unsafe_name);
            assert!(DirectQualificationArtifactPath::try_new(&path).is_err());
        }
    }

    #[test]
    fn atomically_replaces_known_artifacts_without_leaving_staging_directories() {
        let repository = tempfile::tempdir().expect("temporary repository");
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        let output = Path::new("target/benchmarks/qualification/test-run");

        let first = QualificationOutput::begin(&root, output).expect("begin first publication");
        first
            .write("report.json", b"first\n")
            .expect("write first report");
        first.commit().expect("publish first report");

        let second = QualificationOutput::begin(&root, output).expect("begin replacement");
        second
            .write("report.json", b"second\n")
            .expect("write replacement report");
        second.commit().expect("publish replacement");

        assert_eq!(
            read_artifact(&root, output, "report.json").expect("read replacement"),
            b"second\n"
        );
        let parent = repository.path().join("target/benchmarks/qualification");
        let staging = std::fs::read_dir(parent)
            .expect("read publication parent")
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".staging"));
        assert!(!staging, "successful replacement left a staging directory");
    }

    #[test]
    fn unexpected_existing_artifact_blocks_replacement_without_damaging_evidence() {
        let repository = tempfile::tempdir().expect("temporary repository");
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        let output = Path::new("target/benchmarks/qualification/test-run");
        let first = QualificationOutput::begin(&root, output).expect("begin first publication");
        first
            .write("report.json", b"first\n")
            .expect("write first report");
        first.commit().expect("publish first report");
        std::fs::write(
            repository
                .path()
                .join("target/benchmarks/qualification/test-run/unexpected"),
            b"hostile",
        )
        .expect("write unexpected artifact");

        let replacement =
            QualificationOutput::begin(&root, output).expect("begin blocked replacement");
        replacement
            .write("report.json", b"second\n")
            .expect("write staged replacement");
        assert!(replacement.commit().is_err());
        assert_eq!(
            read_artifact(&root, output, "report.json").expect("read preserved report"),
            b"first\n"
        );
    }

    #[test]
    fn stale_refresh_cannot_replace_newer_published_evidence() {
        let repository = tempfile::tempdir().expect("temporary repository");
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        let output = Path::new("target/benchmarks/qualification/test-run");

        let first = QualificationOutput::begin(&root, output).expect("begin first publication");
        first
            .write("report.json", b"first\n")
            .expect("write first report");
        first.commit().expect("publish first report");
        let stale = read_artifact(&root, output, "report.json").expect("read stale report");

        let second = QualificationOutput::begin(&root, output).expect("begin newer publication");
        second
            .write("report.json", b"second\n")
            .expect("write newer report");
        second.commit().expect("publish newer report");

        let refresh = QualificationOutput::begin(&root, output).expect("begin stale refresh");
        assert!(matches!(
            refresh.require_current_artifact("report.json", &stale),
            Err(ArtifactError::ConcurrentReplacement("report.json"))
        ));
        drop(refresh);
        assert_eq!(
            read_artifact(&root, output, "report.json").expect("read newer report"),
            b"second\n"
        );
    }

    #[test]
    fn sibling_binding_rejects_changed_or_nested_sources() {
        let repository = tempfile::tempdir().expect("temporary repository");
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        let source_path = Path::new("target/benchmarks/qualification/source");
        let source = QualificationOutput::begin(&root, source_path).expect("begin source");
        source
            .write("report.json", b"current\n")
            .expect("write source");
        source.commit().expect("publish source");

        let rollup_path = Path::new("target/benchmarks/qualification/rollup");
        let rollup = QualificationOutput::begin(&root, rollup_path).expect("begin rollup");
        let source_path =
            DirectQualificationArtifactPath::try_new(source_path).expect("direct source path");
        let current_digest = super::super::run::sha256_hex(b"current\n");
        let stale_digest = super::super::run::sha256_hex(b"stale\n");
        rollup
            .require_sibling_artifact_digest(&source_path, "report.json", &current_digest, 64)
            .expect("bind current source");
        assert!(matches!(
            rollup.require_sibling_artifact_digest(&source_path, "report.json", &stale_digest, 64,),
            Err(ArtifactError::ConcurrentReplacement("report.json"))
        ));
        assert!(matches!(
            DirectQualificationArtifactPath::try_new(Path::new(
                "target/benchmarks/qualification/nested/source"
            )),
            Err(ArtifactError::NonDirectArtifact(_))
        ));
    }

    #[test]
    fn bounded_reads_reject_oversized_artifacts_and_invalid_limits() {
        let repository = tempfile::tempdir().expect("temporary repository");
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        let output = Path::new("target/benchmarks/qualification/bounded-read");
        let publication = QualificationOutput::begin(&root, output).expect("begin publication");
        publication
            .write("report.json", &[b'x'; 65])
            .expect("write oversized-for-test report");
        publication.commit().expect("publish report");

        assert!(matches!(
            read_artifact_bounded(&root, output, "report.json", 64),
            Err(ArtifactError::UnsafeArtifact("report.json"))
        ));
        assert!(matches!(
            read_artifact_bounded(
                &root,
                output,
                "report.json",
                MAX_ARTIFACT_BYTES.saturating_add(1),
            ),
            Err(ArtifactError::InvalidReadLimit(_))
        ));
    }
}
