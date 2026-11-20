//! Typed crate identities and the crates.io sparse-index file format.
//!
//! Cargo resolves registry dependencies through the sparse index, not the
//! crates.io web API, and the index can lag the API after an upload. The
//! release visibility wait therefore reads the same index file Cargo reads.

use std::fmt;

use cargo_metadata::semver::Version;
use serde::Deserialize;

use super::{RegistryVersion, is_sha256_hex};
use crate::ReleaseError;

const MAX_CRATE_NAME_BYTES: usize = 64;

/// A crates.io package name, validated against the registry's naming rules.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CrateName(String);

impl CrateName {
    pub(crate) fn try_new(value: &str) -> Result<Self, ReleaseError> {
        let mut bytes = value.bytes();
        let valid = value.len() <= MAX_CRATE_NAME_BYTES
            && bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic())
            && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if !valid {
            return Err(ReleaseError::Registry(format!(
                "invalid crates.io package name {value:?}"
            )));
        }
        Ok(Self(value.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// The index file path Cargo's sparse protocol uses for this name.
    pub(super) fn sparse_index_path(&self) -> String {
        let name = self.0.to_ascii_lowercase();
        let prefix =
            |skip: usize, take: usize| name.chars().skip(skip).take(take).collect::<String>();
        match name.len() {
            1 => format!("1/{name}"),
            2 => format!("2/{name}"),
            3 => format!("3/{}/{name}", prefix(0, 1)),
            _ => format!("{}/{}/{name}", prefix(0, 2), prefix(2, 2)),
        }
    }
}

impl fmt::Display for CrateName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A package version in canonical semantic-version form, as registries store it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CrateVersion(String);

impl CrateVersion {
    pub(crate) fn try_new(value: &str) -> Result<Self, ReleaseError> {
        let canonical = Version::parse(value)
            .ok()
            .map(|version| version.to_string())
            .filter(|canonical| canonical == value)
            .ok_or_else(|| {
                ReleaseError::Registry(format!("invalid canonical crate version {value:?}"))
            })?;
        Ok(Self(canonical))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CrateVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Deserialize)]
struct IndexRecord {
    name: String,
    vers: String,
    cksum: String,
    yanked: bool,
}

/// Finds one version in a sparse-index file; `None` means Cargo cannot see it yet.
pub(super) fn parse_index_file(
    body: &str,
    name: &CrateName,
    version: &CrateVersion,
) -> Result<Option<RegistryVersion>, ReleaseError> {
    let mut found = None;
    for line in body.lines().filter(|line| !line.trim().is_empty()) {
        let record: IndexRecord = serde_json::from_str(line).map_err(|error| {
            ReleaseError::Registry(format!("invalid sparse index entry for {name}: {error}"))
        })?;
        if record.name != name.as_str() {
            return Err(ReleaseError::Registry(format!(
                "sparse index file for {name} contains an entry for {}",
                record.name
            )));
        }
        if record.vers != version.as_str() {
            continue;
        }
        if found.is_some() {
            return Err(ReleaseError::Registry(format!(
                "sparse index lists {name} {version} more than once"
            )));
        }
        if !is_sha256_hex(&record.cksum) {
            return Err(ReleaseError::Registry(format!(
                "sparse index returned an invalid checksum for {name} {version}"
            )));
        }
        found = Some(RegistryVersion {
            checksum: record.cksum,
            yanked: record.yanked,
        });
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;

    fn name(value: &str) -> CrateName {
        CrateName::try_new(value).expect("crate name")
    }

    fn version(value: &str) -> CrateVersion {
        CrateVersion::try_new(value).expect("crate version")
    }

    fn entry(name: &str, vers: &str, cksum: &str, yanked: bool) -> String {
        format!(
            "{{\"name\":\"{name}\",\"vers\":\"{vers}\",\"deps\":[],\"cksum\":\"{cksum}\",\"features\":{{}},\"yanked\":{yanked},\"v\":2}}"
        )
    }

    #[test]
    fn sparse_index_paths_follow_cargo_layout() {
        assert_eq!(name("a").sparse_index_path(), "1/a");
        assert_eq!(name("ab").sparse_index_path(), "2/ab");
        assert_eq!(name("abc").sparse_index_path(), "3/a/abc");
        assert_eq!(name("stab-core").sparse_index_path(), "st/ab/stab-core");
        assert_eq!(name("Stab_CLI").sparse_index_path(), "st/ab/stab_cli");
    }

    #[test]
    fn crate_identities_reject_unregistrable_values() {
        for invalid in ["", "1stab", "-stab", "stab/core", "stab core", "stab.core"] {
            assert!(CrateName::try_new(invalid).is_err(), "{invalid:?}");
        }
        assert!(CrateName::try_new(&"a".repeat(MAX_CRATE_NAME_BYTES + 1)).is_err());
        for invalid in ["", "0.2", "v0.2.0", "00.2.0", " 0.2.0"] {
            assert!(CrateVersion::try_new(invalid).is_err(), "{invalid:?}");
        }
        assert_eq!(version("0.2.0-rc.1").as_str(), "0.2.0-rc.1");
    }

    #[test]
    fn index_lookup_selects_the_requested_version() {
        let wanted = "b".repeat(64);
        let body = [
            entry("stab-core", "0.1.0", &"a".repeat(64), false),
            entry("stab-core", "0.2.0", &wanted, true),
            String::new(),
        ]
        .join("\n");
        assert_eq!(
            parse_index_file(&body, &name("stab-core"), &version("0.2.0")).expect("index"),
            Some(RegistryVersion {
                checksum: wanted,
                yanked: true,
            })
        );
        assert_eq!(
            parse_index_file(&body, &name("stab-core"), &version("0.3.0")).expect("index"),
            None
        );
    }

    #[test]
    fn index_lookup_rejects_inconsistent_files() {
        let checksum = "a".repeat(64);
        let foreign = entry("stab-cli", "0.2.0", &checksum, false);
        let duplicate = [
            entry("stab-core", "0.2.0", &checksum, false),
            entry("stab-core", "0.2.0", &checksum, false),
        ]
        .join("\n");
        let bad_checksum = entry("stab-core", "0.2.0", "not-a-checksum", false);
        for body in [foreign.as_str(), &duplicate, &bad_checksum, "{not json"] {
            assert_matches!(
                parse_index_file(body, &name("stab-core"), &version("0.2.0")),
                Err(ReleaseError::Registry(_)),
                "{body}"
            );
        }
    }
}
