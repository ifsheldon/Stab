use std::io::Read as _;
use std::path::{Component, Path};

use sha2::{Digest, Sha256};

use super::{ArtifactBinding, focused_error};
use crate::error::BenchError;
use crate::root::RepoRoot;
use crate::source_file::open_regular_file_bounded_descriptor;

pub(super) fn verify_binding(
    root: &RepoRoot,
    binding: &ArtifactBinding,
    max_bytes: u64,
) -> Result<Vec<u8>, BenchError> {
    let path = root.resolve_relative(Path::new(&binding.path));
    let bytes = read_bounded(&path, max_bytes)?;
    let actual = hex::encode(Sha256::digest(&bytes));
    if actual != binding.sha256 {
        return Err(focused_error(format!(
            "{} SHA-256 is {actual}, expected {}",
            binding.path, binding.sha256
        )));
    }
    Ok(bytes)
}

pub(super) fn bind_artifact(
    root: &RepoRoot,
    path: &Path,
    max_bytes: u64,
) -> Result<(ArtifactBinding, Vec<u8>), BenchError> {
    let relative = normalize_repo_relative_path(root, path)?;
    let path_text = relative.to_str().ok_or_else(|| {
        focused_error(format!(
            "artifact path {} is not valid UTF-8",
            relative.display()
        ))
    })?;
    if !valid_relative_path(path_text) {
        return Err(focused_error(format!(
            "artifact path {path_text:?} is not safe and relative"
        )));
    }
    let bytes = read_bounded(&root.resolve_relative(&relative), max_bytes)?;
    let binding = ArtifactBinding {
        path: path_text.to_string(),
        sha256: hex::encode(Sha256::digest(&bytes)),
    };
    Ok((binding, bytes))
}

pub(super) fn normalize_repo_relative_path(
    root: &RepoRoot,
    path: &Path,
) -> Result<std::path::PathBuf, BenchError> {
    let relative = if path.is_absolute() {
        path.strip_prefix(&root.path).map_err(|_| {
            focused_error(format!(
                "artifact path {} is outside the repository root",
                path.display()
            ))
        })?
    } else {
        path
    };
    let path_text = relative.to_str().ok_or_else(|| {
        focused_error(format!(
            "artifact path {} is not valid UTF-8",
            relative.display()
        ))
    })?;
    if !valid_relative_path(path_text) {
        return Err(focused_error(format!(
            "artifact path {path_text:?} is not safe and relative"
        )));
    }
    Ok(relative.to_path_buf())
}

pub(super) fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, BenchError> {
    let file = open_regular_file_bounded_descriptor(path, max_bytes)?;
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| focused_error(format!("failed to read {}: {error}", path.display())))?;
    let too_large = match u64::try_from(bytes.len()) {
        Ok(len) => len > max_bytes,
        Err(_) => true,
    };
    if too_large {
        return Err(focused_error(format!(
            "{} grew beyond {max_bytes} bytes while reading",
            path.display()
        )));
    }
    Ok(bytes)
}

pub(super) fn validate_binding(label: &str, binding: &ArtifactBinding, issues: &mut Vec<String>) {
    if !valid_relative_path(&binding.path) {
        issues.push(format!(
            "{label} path {:?} is not safe and relative",
            binding.path
        ));
    }
    if !crate::qualification::Sha256Digest::is_valid_str(&binding.sha256) {
        issues.push(format!("{label} has invalid SHA-256 {:?}", binding.sha256));
    }
}

pub(super) fn validate_compare_report_path(label: &str, value: &str, issues: &mut Vec<String>) {
    let path = Path::new(value);
    if !path.starts_with("target/benchmarks") || path.file_name() != Some("compare.json".as_ref()) {
        issues.push(format!(
            "{label} path {value:?} must name target/benchmarks/.../compare.json"
        ));
    }
}

pub(super) fn validate_baseline_report_path(label: &str, value: &str, issues: &mut Vec<String>) {
    let path = Path::new(value);
    if !path.starts_with("target/benchmarks") || path.file_name() != Some("baseline.json".as_ref())
    {
        issues.push(format!(
            "{label} path {value:?} must name target/benchmarks/.../baseline.json"
        ));
    }
}

pub(super) fn validate_profile_receipt_path(value: &str, issues: &mut Vec<String>) {
    let path = Path::new(value);
    if !path.starts_with("target/benchmarks")
        || path.file_name() != Some("profile-receipt.json".as_ref())
    {
        issues.push(format!(
            "hardware profile receipt path {value:?} must name target/benchmarks/.../profile-receipt.json"
        ));
    }
}

pub(super) fn validate_revision(revision: &str, issues: &mut Vec<String>) {
    if !crate::qualification::GitCommit::is_canonical_str(revision) {
        issues.push("source_revision must be a lowercase 40-byte Git object id".to_string());
    }
}

pub(super) fn path_ends_with(value: &str, expected: &str) -> bool {
    Path::new(value).ends_with(expected)
}

fn valid_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !path.is_absolute()
        && path.components().all(|component| {
            matches!(component, Component::Normal(_))
                && component.as_os_str().to_string_lossy() != "."
        })
}
