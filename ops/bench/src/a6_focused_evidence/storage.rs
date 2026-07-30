use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest as _, Sha256};

use super::focused_error;
use crate::error::BenchError;
use crate::process::{check_success, run_process};
use crate::root::RepoRoot;
use crate::source_file::{atomic_create_repo_regular_file, read_repo_regular_file_bounded};

const LEGACY_LEDGER_PATH: &str = "benchmarks/a6-focused-evidence.json";
const CANONICAL_FILE_PREFIX: &str = "a6-focused-evidence-";
const CANONICAL_FILE_SUFFIX: &str = ".json";
const REVISION_HEX_BYTES: usize = 40;
const SHA256_HEX_BYTES: usize = 64;
const MAX_SERIALIZED_LEDGER_BYTES: usize = 1 << 20;
const MAX_TRACKED_OBJECTS: usize = 256;
const MAX_GIT_PATH_LIST_BYTES: usize = 1 << 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum EvidenceObjectName {
    Canonical {
        source_revision: String,
        sha256: String,
    },
    Legacy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PreparedEvidenceObject {
    pub(super) relative_path: PathBuf,
    pub(super) source_revision: String,
    pub(super) sha256: String,
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TrackedEvidenceObject {
    pub(super) relative_path: PathBuf,
    pub(super) name: EvidenceObjectName,
    pub(super) bytes: Vec<u8>,
}

pub(super) fn prepare<T: Serialize>(
    source_revision: &str,
    ledger: &T,
) -> Result<PreparedEvidenceObject, BenchError> {
    require_lower_hex("source revision", source_revision, REVISION_HEX_BYTES)?;
    let bytes = serialize_bounded(ledger)?;
    let sha256 = hex::encode(Sha256::digest(&bytes));
    let relative_path = canonical_path(source_revision, &sha256);
    Ok(PreparedEvidenceObject {
        relative_path,
        source_revision: source_revision.to_string(),
        sha256,
        bytes,
    })
}

pub(super) fn publish(
    root: &RepoRoot,
    prepared: &PreparedEvidenceObject,
) -> Result<PathBuf, BenchError> {
    let expected_path = canonical_path(&prepared.source_revision, &prepared.sha256);
    if prepared.relative_path != expected_path {
        return Err(focused_error(format!(
            "prepared A6 evidence path {} does not match its revision and digest",
            prepared.relative_path.display()
        )));
    }
    if prepared.bytes.len() > MAX_SERIALIZED_LEDGER_BYTES {
        return Err(focused_error(format!(
            "serialized A6 evidence exceeds {MAX_SERIALIZED_LEDGER_BYTES} bytes"
        )));
    }
    let actual_sha256 = hex::encode(Sha256::digest(&prepared.bytes));
    if actual_sha256 != prepared.sha256 {
        return Err(focused_error(format!(
            "prepared A6 evidence digest is {}, expected {}",
            prepared.sha256, actual_sha256
        )));
    }

    let existing_paths = worktree_objects_for_revision(root, &prepared.source_revision)?;
    if let Some(existing_path) = existing_paths
        .iter()
        .find(|path| **path != prepared.relative_path)
    {
        return Err(focused_error(format!(
            "source revision {} already has a different A6 evidence object at {}",
            prepared.source_revision,
            existing_path.display()
        )));
    }
    if let Some(existing_path) = existing_paths.first() {
        let existing = read_repo_regular_file_bounded(
            root,
            &root.resolve_relative(existing_path),
            MAX_SERIALIZED_LEDGER_BYTES,
        )?;
        if existing == prepared.bytes {
            return Ok(existing_path.clone());
        }
        return Err(focused_error(format!(
            "A6 evidence object {} already exists with different bytes",
            existing_path.display()
        )));
    }

    let output = root.resolve_relative(&prepared.relative_path);
    atomic_create_repo_regular_file(root, &output, &prepared.bytes)?;
    let reopened = read_repo_regular_file_bounded(root, &output, MAX_SERIALIZED_LEDGER_BYTES)?;
    if reopened != prepared.bytes {
        return Err(focused_error(format!(
            "published A6 evidence object {} does not contain the prepared bytes",
            prepared.relative_path.display()
        )));
    }
    Ok(prepared.relative_path.clone())
}

pub(super) fn discover_tracked(root: &RepoRoot) -> Result<Vec<TrackedEvidenceObject>, BenchError> {
    let mut paths = tracked_evidence_paths(root)?;
    if paths.len() > MAX_TRACKED_OBJECTS {
        return Err(focused_error(format!(
            "tracked A6 evidence contains {} objects, exceeding {MAX_TRACKED_OBJECTS}",
            paths.len()
        )));
    }
    paths.sort();
    paths
        .into_iter()
        .map(|path| read_explicit_tracked(root, &path))
        .collect()
}

pub(super) fn read_explicit_tracked(
    root: &RepoRoot,
    relative_path: &Path,
) -> Result<TrackedEvidenceObject, BenchError> {
    let name = recognize_path(relative_path)?;
    let head_bytes = read_head_blob(root, relative_path)?;
    let working_bytes = read_repo_regular_file_bounded(
        root,
        &root.resolve_relative(relative_path),
        MAX_SERIALIZED_LEDGER_BYTES,
    )?;
    if working_bytes != head_bytes {
        return Err(focused_error(format!(
            "working A6 evidence object {} differs from HEAD",
            relative_path.display()
        )));
    }
    if let EvidenceObjectName::Canonical {
        source_revision: _,
        sha256,
    } = &name
    {
        let actual = hex::encode(Sha256::digest(&working_bytes));
        if actual != *sha256 {
            return Err(focused_error(format!(
                "A6 evidence object {} has SHA-256 {actual}, expected {sha256}",
                relative_path.display()
            )));
        }
    }
    Ok(TrackedEvidenceObject {
        relative_path: relative_path.to_path_buf(),
        name,
        bytes: working_bytes,
    })
}

pub(super) fn recognize_path(path: &Path) -> Result<EvidenceObjectName, BenchError> {
    if path == Path::new(LEGACY_LEDGER_PATH) {
        return Ok(EvidenceObjectName::Legacy);
    }
    if path.parent() != Some(Path::new("benchmarks")) {
        return Err(focused_error(format!(
            "A6 evidence path {} is neither canonical nor the legacy path",
            path.display()
        )));
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            focused_error(format!(
                "A6 evidence path {} has no UTF-8 file name",
                path.display()
            ))
        })?;
    let body = file_name
        .strip_prefix(CANONICAL_FILE_PREFIX)
        .and_then(|value| value.strip_suffix(CANONICAL_FILE_SUFFIX))
        .ok_or_else(|| {
            focused_error(format!(
                "A6 evidence path {} is neither canonical nor the legacy path",
                path.display()
            ))
        })?;
    let expected_body_bytes = REVISION_HEX_BYTES + 1 + SHA256_HEX_BYTES;
    if body.len() != expected_body_bytes {
        return Err(focused_error(format!(
            "A6 evidence file name {file_name:?} has an invalid length"
        )));
    }
    let (source_revision, remainder) = body.split_at(REVISION_HEX_BYTES);
    let sha256 = remainder.strip_prefix('-').ok_or_else(|| {
        focused_error(format!(
            "A6 evidence file name {file_name:?} omits the revision/digest separator"
        ))
    })?;
    require_lower_hex("source revision", source_revision, REVISION_HEX_BYTES)?;
    require_lower_hex("ledger SHA-256", sha256, SHA256_HEX_BYTES)?;
    Ok(EvidenceObjectName::Canonical {
        source_revision: source_revision.to_string(),
        sha256: sha256.to_string(),
    })
}

fn canonical_path(source_revision: &str, sha256: &str) -> PathBuf {
    Path::new("benchmarks").join(format!(
        "{CANONICAL_FILE_PREFIX}{source_revision}-{sha256}{CANONICAL_FILE_SUFFIX}"
    ))
}

fn serialize_bounded<T: Serialize>(value: &T) -> Result<Vec<u8>, BenchError> {
    let mut writer = BoundedWriter::new(MAX_SERIALIZED_LEDGER_BYTES);
    let result = serde_json::to_writer_pretty(&mut writer, value);
    if writer.exceeded {
        return Err(focused_error(format!(
            "serialized A6 evidence exceeds {MAX_SERIALIZED_LEDGER_BYTES} bytes"
        )));
    }
    result?;
    if writer.bytes.len() == MAX_SERIALIZED_LEDGER_BYTES {
        return Err(focused_error(format!(
            "serialized A6 evidence plus its terminal newline exceeds {MAX_SERIALIZED_LEDGER_BYTES} bytes"
        )));
    }
    writer.bytes.push(b'\n');
    Ok(writer.bytes)
}

#[derive(Debug)]
struct BoundedWriter {
    bytes: Vec<u8>,
    maximum: usize,
    exceeded: bool,
}

impl BoundedWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            exceeded: false,
        }
    }
}

impl std::io::Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if buffer.len() > self.maximum.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(std::io::Error::other("bounded JSON output exceeded"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn tracked_evidence_paths(root: &RepoRoot) -> Result<Vec<PathBuf>, BenchError> {
    let output = run_git(
        root,
        &[
            "ls-tree",
            "-r",
            "-z",
            "--name-only",
            "HEAD",
            "--",
            "benchmarks",
        ],
    )?;
    parse_git_paths(&output)?
        .into_iter()
        .filter(|path| is_evidence_namespace(path))
        .map(|path| recognize_path(&path).map(|_| path))
        .collect()
}

fn worktree_objects_for_revision(
    root: &RepoRoot,
    source_revision: &str,
) -> Result<Vec<PathBuf>, BenchError> {
    let pathspec =
        format!("benchmarks/{CANONICAL_FILE_PREFIX}{source_revision}-*{CANONICAL_FILE_SUFFIX}");
    let mut paths = BTreeSet::new();
    for arguments in [
        vec![
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            pathspec.as_str(),
        ],
        vec![
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--",
            pathspec.as_str(),
        ],
    ] {
        let output = run_git(root, &arguments)?;
        paths.extend(parse_git_paths(&output)?);
    }
    if paths.len() > MAX_TRACKED_OBJECTS {
        return Err(focused_error(format!(
            "source revision {source_revision} has more than {MAX_TRACKED_OBJECTS} candidate A6 evidence objects"
        )));
    }
    paths
        .into_iter()
        .map(|path| match recognize_path(&path)? {
            EvidenceObjectName::Canonical {
                source_revision: actual,
                sha256: _,
            } if actual == source_revision => Ok(path),
            EvidenceObjectName::Canonical {
                source_revision: actual,
                sha256: _,
            } => Err(focused_error(format!(
                "Git returned A6 evidence for revision {actual} while selecting {source_revision}"
            ))),
            EvidenceObjectName::Legacy => Err(focused_error(
                "legacy A6 evidence cannot satisfy a revision-keyed publication",
            )),
        })
        .collect()
}

fn read_head_blob(root: &RepoRoot, relative_path: &Path) -> Result<Vec<u8>, BenchError> {
    let path_text = relative_path.to_str().ok_or_else(|| {
        focused_error(format!(
            "A6 evidence path {} is not UTF-8",
            relative_path.display()
        ))
    })?;
    let object = format!("HEAD:{path_text}");
    let bytes = run_git(root, &["cat-file", "blob", object.as_str()])?;
    if bytes.len() > MAX_SERIALIZED_LEDGER_BYTES {
        return Err(focused_error(format!(
            "tracked A6 evidence object {} exceeds {MAX_SERIALIZED_LEDGER_BYTES} bytes",
            relative_path.display()
        )));
    }
    Ok(bytes)
}

fn run_git(root: &RepoRoot, arguments: &[&str]) -> Result<Vec<u8>, BenchError> {
    let arguments = arguments.iter().map(OsString::from).collect::<Vec<_>>();
    let output = run_process(
        Path::new("git"),
        &arguments,
        b"",
        &root.process_working_dir(),
        true,
    )?;
    check_success(Path::new("git"), &output)?;
    if output.stdout.len() > MAX_GIT_PATH_LIST_BYTES {
        return Err(focused_error(format!(
            "Git output for A6 evidence exceeds {MAX_GIT_PATH_LIST_BYTES} bytes"
        )));
    }
    Ok(output.stdout)
}

fn parse_git_paths(bytes: &[u8]) -> Result<Vec<PathBuf>, BenchError> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            let text = std::str::from_utf8(path)
                .map_err(|error| focused_error(format!("Git emitted a non-UTF-8 path: {error}")))?;
            Ok(PathBuf::from(text))
        })
        .collect()
}

fn is_evidence_namespace(path: &Path) -> bool {
    path == Path::new(LEGACY_LEDGER_PATH)
        || (path.parent() == Some(Path::new("benchmarks"))
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(CANONICAL_FILE_PREFIX)))
}

fn require_lower_hex(label: &str, value: &str, expected_bytes: usize) -> Result<(), BenchError> {
    if value.len() == expected_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(focused_error(format!(
            "{label} must be exactly {expected_bytes} lowercase hexadecimal bytes"
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde::Serialize;

    use super::*;

    #[derive(Debug, Serialize)]
    struct FixtureLedger<'a> {
        schema_version: u32,
        source_revision: &'a str,
        note: &'a str,
    }

    fn test_git(repository: &Path, arguments: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(arguments)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .output()
            .expect("run test Git command");
        assert!(
            output.status.success(),
            "Git {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("Git output UTF-8")
            .trim()
            .to_string()
    }

    fn initialized_repository() -> (tempfile::TempDir, RepoRoot, String) {
        let repository = tempfile::tempdir().expect("temporary repository");
        test_git(repository.path(), &["init", "--quiet"]);
        test_git(repository.path(), &["config", "user.name", "Stab Test"]);
        test_git(
            repository.path(),
            &["config", "user.email", "stab@example.invalid"],
        );
        fs::create_dir(repository.path().join("benchmarks")).expect("create benchmarks directory");
        fs::write(repository.path().join("benchmarks/seed"), b"seed\n")
            .expect("write tracked seed");
        test_git(repository.path(), &["add", "--all"]);
        test_git(repository.path(), &["commit", "--quiet", "-m", "source"]);
        let revision = test_git(repository.path(), &["rev-parse", "HEAD"]);
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        (repository, root, revision)
    }

    fn fixture<'a>(revision: &'a str, note: &'a str) -> FixtureLedger<'a> {
        FixtureLedger {
            schema_version: 3,
            source_revision: revision,
            note,
        }
    }

    #[test]
    fn canonical_name_round_trips_and_legacy_path_remains_recognized() {
        let revision = "a".repeat(REVISION_HEX_BYTES);
        let digest = "b".repeat(SHA256_HEX_BYTES);
        let path = canonical_path(&revision, &digest);
        assert_eq!(
            recognize_path(&path).expect("canonical path"),
            EvidenceObjectName::Canonical {
                source_revision: revision,
                sha256: digest,
            }
        );
        assert_eq!(
            recognize_path(Path::new(LEGACY_LEDGER_PATH)).expect("legacy path"),
            EvidenceObjectName::Legacy
        );

        let uppercase = canonical_path(&"A".repeat(REVISION_HEX_BYTES), &"b".repeat(64));
        assert!(
            recognize_path(&uppercase)
                .expect_err("uppercase revision")
                .to_string()
                .contains("lowercase hexadecimal")
        );
    }

    #[test]
    fn preparation_hashes_exact_newline_terminated_bytes() {
        let revision = "a".repeat(REVISION_HEX_BYTES);
        let prepared = prepare(&revision, &fixture(&revision, "bounded")).expect("prepare ledger");
        assert_eq!(prepared.bytes.last(), Some(&b'\n'));
        assert_eq!(
            prepared.sha256,
            hex::encode(Sha256::digest(&prepared.bytes))
        );
        assert_eq!(
            prepared.relative_path,
            canonical_path(&revision, &prepared.sha256)
        );
    }

    #[test]
    fn preparation_rejects_serialization_above_one_mibibyte() {
        let revision = "a".repeat(REVISION_HEX_BYTES);
        let oversized = "x".repeat(MAX_SERIALIZED_LEDGER_BYTES);
        let error = prepare(&revision, &fixture(&revision, &oversized))
            .expect_err("oversized ledger must fail");
        assert!(error.to_string().contains("exceeds 1048576 bytes"));
    }

    #[test]
    fn publication_and_tracked_discovery_follow_a_real_git_lifecycle() {
        let (repository, root, revision) = initialized_repository();
        let prepared = prepare(&revision, &fixture(&revision, "first")).expect("prepare ledger");
        let published = publish(&root, &prepared).expect("publish ledger");
        assert_eq!(published, prepared.relative_path);
        assert_eq!(
            fs::read(repository.path().join(&published)).expect("read published ledger"),
            prepared.bytes
        );
        assert!(
            discover_tracked(&root)
                .expect("discover before commit")
                .is_empty(),
            "untracked files must not be discovered as source-owned evidence"
        );

        test_git(repository.path(), &["add", "--all"]);
        test_git(repository.path(), &["commit", "--quiet", "-m", "evidence"]);
        let discovered = discover_tracked(&root).expect("discover committed evidence");
        assert_eq!(discovered.len(), 1);
        let object = discovered.first().expect("one discovered object");
        assert_eq!(object.relative_path, prepared.relative_path);
        assert_eq!(object.bytes, prepared.bytes);
        assert_eq!(
            publish(&root, &prepared).expect("idempotent publication"),
            prepared.relative_path
        );
    }

    #[test]
    fn publication_rejects_a_different_object_for_the_same_revision() {
        let (repository, root, revision) = initialized_repository();
        let first = prepare(&revision, &fixture(&revision, "first")).expect("first object");
        publish(&root, &first).expect("publish first object");
        test_git(repository.path(), &["add", "--all"]);
        test_git(
            repository.path(),
            &["commit", "--quiet", "-m", "first evidence"],
        );

        let second = prepare(&revision, &fixture(&revision, "second")).expect("second object");
        let error = publish(&root, &second).expect_err("second object for revision must fail");
        assert!(error.to_string().contains("already has a different"));
        assert!(!repository.path().join(second.relative_path).exists());
        assert_eq!(
            fs::read(repository.path().join(first.relative_path)).expect("first object remains"),
            first.bytes
        );
    }

    #[test]
    fn publication_never_replaces_an_existing_target() {
        let (repository, root, revision) = initialized_repository();
        let prepared =
            prepare(&revision, &fixture(&revision, "collision")).expect("prepare ledger");
        let output = repository.path().join(&prepared.relative_path);
        fs::write(&output, b"sentinel\n").expect("write collision");

        publish(&root, &prepared).expect_err("existing target must not be replaced");
        assert_eq!(fs::read(output).expect("read collision"), b"sentinel\n");
    }

    #[test]
    fn discovery_rejects_working_bytes_that_differ_from_head() {
        let (repository, root, revision) = initialized_repository();
        let prepared = prepare(&revision, &fixture(&revision, "tracked")).expect("prepare ledger");
        publish(&root, &prepared).expect("publish ledger");
        test_git(repository.path(), &["add", "--all"]);
        test_git(repository.path(), &["commit", "--quiet", "-m", "evidence"]);
        fs::write(
            repository.path().join(&prepared.relative_path),
            b"working-tree drift\n",
        )
        .expect("mutate working evidence");

        let error = discover_tracked(&root).expect_err("working drift must fail");
        assert!(error.to_string().contains("differs from HEAD"));
    }

    #[test]
    fn tracked_legacy_path_remains_explicitly_readable() {
        let (repository, root, _) = initialized_repository();
        fs::write(
            repository.path().join(LEGACY_LEDGER_PATH),
            b"{\"schema_version\":3}\n",
        )
        .expect("write legacy ledger");
        test_git(repository.path(), &["add", "--all"]);
        test_git(
            repository.path(),
            &["commit", "--quiet", "-m", "legacy evidence"],
        );

        let explicit = read_explicit_tracked(&root, Path::new(LEGACY_LEDGER_PATH))
            .expect("read tracked legacy ledger");
        assert_eq!(explicit.name, EvidenceObjectName::Legacy);
        assert_eq!(explicit.bytes, b"{\"schema_version\":3}\n");
        assert_eq!(
            discover_tracked(&root)
                .expect("discover legacy ledger")
                .len(),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn discovery_rejects_a_tracked_symlink_object() {
        use std::os::unix::fs::symlink;

        let (repository, root, revision) = initialized_repository();
        let prepared = prepare(&revision, &fixture(&revision, "symlink")).expect("prepare ledger");
        symlink("seed", repository.path().join(&prepared.relative_path))
            .expect("create evidence symlink");
        test_git(repository.path(), &["add", "--all"]);
        test_git(
            repository.path(),
            &["commit", "--quiet", "-m", "symlink evidence"],
        );

        let error = discover_tracked(&root).expect_err("tracked symlink must fail");
        assert!(
            error.to_string().contains("symbolic link")
                || error.to_string().contains("regular nonsymlink file")
        );
    }
}
