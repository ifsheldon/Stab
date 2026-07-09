use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{
    BenchmarkId, BenchmarkManifestRow, BlockerLedgerError, FixtureId, MAX_MANIFEST_BYTES,
    MAX_MANIFEST_ROWS, MAX_TRACKED_PATH_BYTES, OracleManifestRow, StimSourcePath,
};
use crate::RepoRoot;

pub(super) fn open_regular_file(path: &Path) -> Result<std::fs::File, BlockerLedgerError> {
    #[cfg(not(unix))]
    {
        return Err(BlockerLedgerError::UnsupportedEvidenceIdentity {
            path: path.to_path_buf(),
        });
    }

    #[cfg(unix)]
    {
        open_regular_file_unix(path)
    }
}

#[cfg(unix)]
fn open_regular_file_unix(path: &Path) -> Result<std::fs::File, BlockerLedgerError> {
    open_regular_file_unix_with_pre_open_hook(path, || {})
}

#[cfg(all(test, unix))]
pub(super) fn open_regular_file_with_pre_open_hook(
    path: &Path,
    hook: impl FnOnce(),
) -> Result<std::fs::File, BlockerLedgerError> {
    open_regular_file_unix_with_pre_open_hook(path, hook)
}

#[cfg(unix)]
fn open_regular_file_unix_with_pre_open_hook(
    path: &Path,
    hook: impl FnOnce(),
) -> Result<std::fs::File, BlockerLedgerError> {
    use std::os::unix::fs::MetadataExt;

    use rustix::fs::{Mode, OFlags};

    let expected =
        std::fs::symlink_metadata(path).map_err(|source| BlockerLedgerError::Inspect {
            path: path.to_path_buf(),
            source,
        })?;
    if !expected.file_type().is_file() || expected.file_type().is_symlink() {
        return Err(BlockerLedgerError::EvidenceNotRegular {
            path: path.to_path_buf(),
        });
    }
    hook();
    let descriptor = rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|source| BlockerLedgerError::Inspect {
        path: path.to_path_buf(),
        source: source.into(),
    })?;
    let file = std::fs::File::from(descriptor);
    let opened = file
        .metadata()
        .map_err(|source| BlockerLedgerError::Inspect {
            path: path.to_path_buf(),
            source,
        })?;
    let current =
        std::fs::symlink_metadata(path).map_err(|source| BlockerLedgerError::Inspect {
            path: path.to_path_buf(),
            source,
        })?;
    if !opened.is_file()
        || !current.file_type().is_file()
        || current.file_type().is_symlink()
        || expected.dev() != opened.dev()
        || expected.ino() != opened.ino()
        || current.dev() != opened.dev()
        || current.ino() != opened.ino()
    {
        return Err(BlockerLedgerError::EvidenceNotRegular {
            path: path.to_path_buf(),
        });
    }
    Ok(file)
}

pub(super) fn read_oracle_manifest(
    path: &Path,
) -> Result<BTreeMap<FixtureId, OracleManifestRow>, BlockerLedgerError> {
    read_manifest(path, |row: OracleManifestRow| (row.id.clone(), row))
}

pub(super) fn read_benchmark_manifest(
    path: &Path,
) -> Result<BTreeMap<BenchmarkId, BenchmarkManifestRow>, BlockerLedgerError> {
    read_manifest(path, |row: BenchmarkManifestRow| (row.id.clone(), row))
}

fn read_manifest<K, V, F>(path: &Path, key_value: F) -> Result<BTreeMap<K, V>, BlockerLedgerError>
where
    K: Ord + std::fmt::Debug,
    V: for<'de> Deserialize<'de>,
    F: Fn(V) -> (K, V),
{
    let file = open_regular_file(path)?;
    let mut content = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut content)
        .map_err(|source| BlockerLedgerError::Inspect {
            path: path.to_path_buf(),
            source,
        })?;
    let actual = u64::try_from(content.len()).unwrap_or(u64::MAX);
    if actual > MAX_MANIFEST_BYTES {
        return Err(BlockerLedgerError::ManifestTooLarge {
            path: path.to_path_buf(),
            actual,
            limit: MAX_MANIFEST_BYTES,
        });
    }

    let mut reader = csv::Reader::from_reader(content.as_slice());
    let mut rows = BTreeMap::new();
    for (index, row) in reader.deserialize().enumerate() {
        if index >= MAX_MANIFEST_ROWS {
            return Err(BlockerLedgerError::TooManyManifestRows {
                path: path.to_path_buf(),
                limit: MAX_MANIFEST_ROWS,
            });
        }
        let row = row.map_err(|source| BlockerLedgerError::ReadManifest {
            path: path.to_path_buf(),
            source,
        })?;
        let (key, row) = key_value(row);
        if rows.insert(key, row).is_some() {
            return Err(BlockerLedgerError::Validation(
                format!("manifest {} contains a duplicate id", path.display()).into_boxed_str(),
            ));
        }
    }
    Ok(rows)
}

pub(super) fn read_tracked_stim_paths(
    root: &RepoRoot,
) -> Result<BTreeSet<StimSourcePath>, BlockerLedgerError> {
    let output = crate::run_checked("git", ["ls-files", "-z"], &[], Some(&root.stim_source()))
        .map_err(|source| BlockerLedgerError::ListTrackedStimFiles {
            reason: source.to_string().into_boxed_str(),
        })?;
    if output.stdout.truncated || output.stdout.bytes.len() > MAX_TRACKED_PATH_BYTES {
        return Err(BlockerLedgerError::TrackedStimFilesTooLarge {
            actual: output
                .stdout
                .bytes
                .len()
                .saturating_add(usize::from(output.stdout.truncated)),
            limit: MAX_TRACKED_PATH_BYTES,
        });
    }

    let mut paths = BTreeSet::new();
    for bytes in output.stdout.bytes.split(|byte| *byte == 0) {
        if bytes.is_empty() {
            continue;
        }
        let value =
            std::str::from_utf8(bytes).map_err(|_| BlockerLedgerError::NonUtf8TrackedStimPath)?;
        paths.insert(StimSourcePath(PathBuf::from(value)));
    }
    Ok(paths)
}
