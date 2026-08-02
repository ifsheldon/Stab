use std::ffi::OsStr;
use std::io::{Cursor, Read};
use std::path::{Component, Path};

use flate2::read::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{ReleaseError, safe_fs};

pub(crate) const MAX_CRATE_ARCHIVE_BYTES: u64 = 64 << 20;
const MAX_VCS_INFO_BYTES: u64 = 64 << 10;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewedArchive {
    pub(crate) bytes: Vec<u8>,
    pub(crate) sha256: String,
    pub(crate) vcs_commit: String,
}

pub(crate) fn read_file_and_validate(
    file: std::fs::File,
    path: &Path,
    package: &str,
    version: &str,
    expected_commit: &str,
) -> Result<ReviewedArchive, ReleaseError> {
    let bytes = safe_fs::read_bounded_file(file, path, MAX_CRATE_ARCHIVE_BYTES)?;
    validate_bytes(path, bytes, package, version, expected_commit)
}

pub(crate) fn validate_bytes(
    path: &Path,
    bytes: Vec<u8>,
    package: &str,
    version: &str,
    expected_commit: &str,
) -> Result<ReviewedArchive, ReleaseError> {
    let vcs_commit = archive_vcs_commit(path, &bytes, package, version)?;
    if vcs_commit != expected_commit {
        return Err(ReleaseError::ArchiveContract {
            path: path.to_path_buf(),
            detail: format!(
                ".cargo_vcs_info.json names commit {vcs_commit}, expected {expected_commit}"
            ),
        });
    }
    Ok(ReviewedArchive {
        sha256: sha256_bytes(&bytes),
        bytes,
        vcs_commit,
    })
}

pub(crate) fn write_immutable_copy(
    directory: &safe_fs::RetainedDirectory,
    name: &OsStr,
    archive: &ReviewedArchive,
) -> Result<(), ReleaseError> {
    let file = directory.write_new(name, &archive.bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        file.set_permissions(std::fs::Permissions::from_mode(0o444))
            .map_err(|source| ReleaseError::io(directory.path().join(name), source))?;
        file.sync_all()
            .map_err(|source| ReleaseError::io(directory.path().join(name), source))?;
    }
    #[cfg(not(unix))]
    return Err(ReleaseError::UnsupportedPlatform);
    Ok(())
}

pub(crate) fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn archive_vcs_commit(
    path: &Path,
    bytes: &[u8],
    package: &str,
    version: &str,
) -> Result<String, ReleaseError> {
    let expected_root = format!("{package}-{version}");
    let expected_vcs = format!("{expected_root}/.cargo_vcs_info.json");
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|source| archive_error(path, source.to_string()))?;
    let mut vcs = None;
    for entry in entries {
        let mut entry = entry.map_err(|source| archive_error(path, source.to_string()))?;
        let entry_path = entry
            .path()
            .map_err(|source| archive_error(path, source.to_string()))?;
        if entry_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
            || !entry_path.starts_with(&expected_root)
        {
            return Err(archive_error(
                path,
                format!(
                    "unsafe or unexpected archive entry {}",
                    entry_path.display()
                ),
            ));
        }
        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(archive_error(
                path,
                format!("archive entry {} is a link", entry_path.display()),
            ));
        }
        if entry_path == Path::new(&expected_vcs) {
            if vcs.is_some() {
                return Err(archive_error(path, "duplicate .cargo_vcs_info.json"));
            }
            let size = entry
                .header()
                .size()
                .map_err(|source| archive_error(path, source.to_string()))?;
            if size > MAX_VCS_INFO_BYTES {
                return Err(archive_error(path, ".cargo_vcs_info.json is oversized"));
            }
            let mut bytes = Vec::with_capacity(
                usize::try_from(size)
                    .map_err(|_| archive_error(path, ".cargo_vcs_info.json is oversized"))?,
            );
            entry
                .by_ref()
                .take(MAX_VCS_INFO_BYTES.saturating_add(1))
                .read_to_end(&mut bytes)
                .map_err(|source| archive_error(path, source.to_string()))?;
            if !matches!(u64::try_from(bytes.len()), Ok(length) if length <= MAX_VCS_INFO_BYTES) {
                return Err(archive_error(path, ".cargo_vcs_info.json is oversized"));
            }
            let parsed: CargoVcsInfo = serde_json::from_slice(&bytes)?;
            if parsed.git.dirty == Some(true) {
                return Err(archive_error(path, "archive records dirty source"));
            }
            vcs = Some(parsed.git.sha1);
        }
    }
    vcs.ok_or_else(|| archive_error(path, "archive has no .cargo_vcs_info.json"))
}

fn archive_error(path: &Path, detail: impl Into<String>) -> ReleaseError {
    ReleaseError::ArchiveContract {
        path: path.to_path_buf(),
        detail: detail.into(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CargoVcsInfo {
    git: CargoGitIdentity,
    #[serde(rename = "path_in_vcs")]
    _path_in_vcs: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CargoGitIdentity {
    sha1: String,
    #[serde(default)]
    dirty: Option<bool>,
}

#[cfg(test)]
mod tests {
    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    fn crate_bytes(commit: &str) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut archive = tar::Builder::new(encoder);
        let vcs =
            format!("{{\"git\":{{\"sha1\":\"{commit}\"}},\"path_in_vcs\":\"crates/stab-core\"}}");
        let mut header = tar::Header::new_gnu();
        header.set_size(u64::try_from(vcs.len()).expect("VCS length"));
        header.set_mode(0o644);
        header.set_cksum();
        archive
            .append_data(
                &mut header,
                "stab-core-0.2.0/.cargo_vcs_info.json",
                vcs.as_bytes(),
            )
            .expect("append VCS info");
        let encoder = archive.into_inner().expect("finish tar");
        encoder.finish().expect("finish gzip")
    }

    #[test]
    fn archive_vcs_identity_must_match_reviewed_commit() {
        let expected = "1111111111111111111111111111111111111111";
        let stale = "2222222222222222222222222222222222222222";
        let path = Path::new("stab-core-0.2.0.crate");
        let error = validate_bytes(path, crate_bytes(stale), "stab-core", "0.2.0", expected)
            .expect_err("stale archive must fail");
        assert!(matches!(error, ReleaseError::ArchiveContract { .. }));

        let reviewed = validate_bytes(path, crate_bytes(expected), "stab-core", "0.2.0", expected)
            .expect("current archive");
        assert_eq!(reviewed.vcs_commit, expected);
        assert_eq!(reviewed.sha256, sha256_bytes(&reviewed.bytes));
    }
}
