use std::ffi::OsStr;
use std::io::{self, Cursor, Read};
use std::path::{Component, Path};

use flate2::read::MultiGzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{ReleaseError, safe_fs};

pub(crate) const MAX_CRATE_ARCHIVE_BYTES: u64 = 64 << 20;
const MAX_CRATE_EXPANDED_BYTES: u64 = 80 << 20;
const MAX_CRATE_DECLARED_BYTES: u64 = 64 << 20;
const MAX_CRATE_ENTRY_COUNT: u64 = 4_096;
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
    let decoder = MultiGzDecoder::new(Cursor::new(bytes));
    let expanded = ExpandedReader::new(decoder, MAX_CRATE_EXPANDED_BYTES);
    let mut archive = tar::Archive::new(expanded);
    let entries = archive
        .entries()
        .map_err(|source| archive_error(path, source.to_string()))?;
    let mut vcs = None;
    let mut entry_count = 0_u64;
    let mut declared_bytes = 0_u64;
    for entry in entries {
        let mut entry = entry.map_err(|source| archive_error(path, source.to_string()))?;
        entry_count = entry_count
            .checked_add(1)
            .ok_or_else(|| archive_error(path, "archive entry count overflowed"))?;
        if entry_count > MAX_CRATE_ENTRY_COUNT {
            return Err(archive_error(
                path,
                format!("archive exceeds its {MAX_CRATE_ENTRY_COUNT}-entry entry-count limit"),
            ));
        }
        let declared_size = entry
            .header()
            .size()
            .map_err(|source| archive_error(path, source.to_string()))?;
        declared_bytes = declared_bytes
            .checked_add(declared_size)
            .ok_or_else(|| archive_error(path, "archive cumulative declared size overflowed"))?;
        if declared_bytes > MAX_CRATE_DECLARED_BYTES {
            return Err(archive_error(
                path,
                format!(
                    "archive exceeds its {MAX_CRATE_DECLARED_BYTES}-byte cumulative declared-size limit"
                ),
            ));
        }
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
            let size = declared_size;
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
    let mut expanded = archive.into_inner();
    io::copy(&mut expanded, &mut io::sink())
        .map_err(|source| archive_error(path, source.to_string()))?;
    vcs.ok_or_else(|| archive_error(path, "archive has no .cargo_vcs_info.json"))
}

struct ExpandedReader<R> {
    inner: R,
    consumed: u64,
    limit: u64,
}

impl<R> ExpandedReader<R> {
    fn new(inner: R, limit: u64) -> Self {
        Self {
            inner,
            consumed: 0,
            limit,
        }
    }
}

impl<R: Read> Read for ExpandedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let remaining = self.limit.saturating_sub(self.consumed);
        let permitted = remaining
            .saturating_add(1)
            .min(u64::try_from(buffer.len()).unwrap_or(u64::MAX));
        let permitted = usize::try_from(permitted)
            .map_err(|_| io::Error::other("expanded archive read bound does not fit usize"))?;
        let bounded = buffer
            .get_mut(..permitted)
            .ok_or_else(|| io::Error::other("expanded archive read exceeded its buffer"))?;
        let read = self.inner.read(bounded)?;
        self.consumed =
            self.consumed
                .checked_add(u64::try_from(read).map_err(|_| {
                    io::Error::other("expanded archive read length does not fit u64")
                })?)
                .ok_or_else(|| io::Error::other("expanded archive byte count overflowed"))?;
        if self.consumed > self.limit {
            return Err(io::Error::other(format!(
                "archive exceeds its {}-byte expanded byte limit",
                self.limit
            )));
        }
        Ok(read)
    }
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
    use std::io::{self, Write as _};

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

    fn expanded_bomb() -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
        io::copy(
            &mut io::repeat(0).take(MAX_CRATE_EXPANDED_BYTES + 1),
            &mut encoder,
        )
        .expect("compress expanded payload");
        encoder.finish().expect("finish gzip")
    }

    fn excessive_entries() -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::best());
        let mut archive = tar::Builder::new(encoder);
        for index in 0..=MAX_CRATE_ENTRY_COUNT {
            let mut header = tar::Header::new_gnu();
            header.set_size(0);
            header.set_mode(0o644);
            header.set_cksum();
            archive
                .append_data(
                    &mut header,
                    format!("stab-core-0.2.0/entry-{index}"),
                    io::empty(),
                )
                .expect("append empty entry");
        }
        let encoder = archive.into_inner().expect("finish tar");
        encoder.finish().expect("finish gzip")
    }

    fn excessive_declared_size() -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
        let mut header = tar::Header::new_gnu();
        header
            .set_path("stab-core-0.2.0/oversized")
            .expect("entry path");
        header.set_size(MAX_CRATE_DECLARED_BYTES + 1);
        header.set_mode(0o644);
        header.set_cksum();
        encoder
            .write_all(header.as_bytes())
            .expect("write oversized header");
        encoder.finish().expect("finish gzip")
    }

    fn archive_detail(error: ReleaseError) -> Option<String> {
        match error {
            ReleaseError::ArchiveContract { detail, .. } => Some(detail),
            _ => None,
        }
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

    #[test]
    fn archive_expansion_is_bounded_even_after_tar_termination() {
        let detail = archive_detail(
            validate_bytes(
                Path::new("stab-core-0.2.0.crate"),
                expanded_bomb(),
                "stab-core",
                "0.2.0",
                "1",
            )
            .expect_err("expanded archive must fail"),
        )
        .expect("archive contract error");
        assert!(detail.contains("expanded byte limit"), "{detail}");
    }

    #[test]
    fn archive_entry_count_is_bounded() {
        let detail = archive_detail(
            validate_bytes(
                Path::new("stab-core-0.2.0.crate"),
                excessive_entries(),
                "stab-core",
                "0.2.0",
                "1",
            )
            .expect_err("excess entries must fail"),
        )
        .expect("archive contract error");
        assert!(detail.contains("entry-count limit"), "{detail}");
    }

    #[test]
    fn archive_cumulative_declared_size_is_bounded() {
        let detail = archive_detail(
            validate_bytes(
                Path::new("stab-core-0.2.0.crate"),
                excessive_declared_size(),
                "stab-core",
                "0.2.0",
                "1",
            )
            .expect_err("declared size must fail"),
        )
        .expect("archive contract error");
        assert!(detail.contains("declared-size limit"), "{detail}");
    }
}
