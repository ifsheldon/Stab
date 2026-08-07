//! Shared typed identities for SHA-256 digests and Git commits.
//!
//! This module is the single owner of the digest and commit validation rules
//! used across stab-bench: the worker protocol deserializes into these types,
//! and every ledger, receipt, report, and manifest validator delegates to the
//! same rules through [`Sha256Digest::is_valid_str`] and
//! [`GitCommit::is_canonical_str`]. It also owns the one `sha256_hex` helper
//! for computing lowercase hexadecimal SHA-256 digests.

use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest as _, Sha256};

use super::protocol::ProtocolError;

/// Computes the lowercase hexadecimal SHA-256 digest of `bytes`.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex_lower(&Sha256::digest(bytes))
}

/// Encodes raw digest bytes as lowercase hexadecimal.
pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(hex_digit(byte >> 4));
        output.push(hex_digit(byte & 0x0f));
    }
    output
}

fn hex_digit(value: u8) -> char {
    char::from(if value < 10 {
        b'0' + value
    } else {
        b'a' + (value - 10)
    })
}

/// A canonical lowercase 64-character hexadecimal SHA-256 digest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(crate) struct Sha256Digest(Box<str>);

impl Sha256Digest {
    pub(crate) fn try_new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        if !Self::is_valid_str(&value) {
            return Err(ProtocolError::InvalidSha256(value));
        }
        Ok(Self(value.into_boxed_str()))
    }

    /// Returns true when `value` is a lowercase 64-character hexadecimal
    /// SHA-256 digest. Uppercase hexadecimal digits are rejected.
    pub(crate) fn is_valid_str(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

/// A 40-character hexadecimal Git object id, normalized to lowercase.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(crate) struct GitCommit(Box<str>);

impl GitCommit {
    /// Accepts a 40-character hexadecimal object id in any case and
    /// normalizes it to the canonical lowercase form.
    pub(crate) fn try_new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ProtocolError::InvalidGitCommit(value));
        }
        Ok(Self(value.to_ascii_lowercase().into_boxed_str()))
    }

    /// Returns true when `value` is already a canonical lowercase
    /// 40-character hexadecimal Git object id.
    pub(crate) fn is_canonical_str(value: &str) -> bool {
        value.len() == 40
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for GitCommit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_rule_accepts_only_lowercase_64_character_hex() {
        assert!(Sha256Digest::is_valid_str(&"0".repeat(64)));
        assert!(Sha256Digest::try_new("a".repeat(64)).is_ok());
        let uppercase = format!("{}A", "0".repeat(63));
        assert!(!Sha256Digest::is_valid_str(&uppercase));
        assert!(Sha256Digest::try_new(uppercase).is_err());
        assert!(!Sha256Digest::is_valid_str(&"0".repeat(63)));
        assert!(!Sha256Digest::is_valid_str(&"g".repeat(64)));
    }

    #[test]
    fn commit_constructor_normalizes_and_canonical_rule_stays_lowercase() {
        let mixed = format!("{}F", "a".repeat(39));
        let commit = GitCommit::try_new(mixed.clone()).expect("mixed-case commit");
        assert_eq!(commit.as_str(), format!("{}f", "a".repeat(39)));
        assert!(!GitCommit::is_canonical_str(&mixed));
        assert!(GitCommit::is_canonical_str(commit.as_str()));
        assert!(GitCommit::try_new("z".repeat(40)).is_err());
        assert!(!GitCommit::is_canonical_str(&"a".repeat(39)));
    }

    #[test]
    fn sha256_hex_produces_the_canonical_empty_digest() {
        let digest = sha256_hex(b"");
        assert_eq!(
            digest,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert!(Sha256Digest::is_valid_str(&digest));
    }
}
