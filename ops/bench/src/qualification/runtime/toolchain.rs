use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::process::{ProcessLimits, ProcessRequest, run_bounded_process};
use crate::root::RepoRoot;

const RUST_TOOLCHAIN: &str = "nightly-2026-06-20";
const RUSTUP_PATH: &str = "/usr/bin/rustup";
const MAX_TOOL_BYTES: u64 = 512 << 20;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ToolchainEvidence {
    pub(super) rust_toolchain: String,
    pub(super) cargo_profile: String,
    pub(super) rustup_path: String,
    pub(super) rustup_sha256: String,
    pub(super) cargo_path: String,
    pub(super) cargo_sha256: String,
    pub(super) cargo_verbose_version: String,
    pub(super) rustc_path: String,
    pub(super) rustc_sha256: String,
    pub(super) rustc_verbose_version: String,
    pub(super) target_triple: String,
}

pub(super) fn collect(root: &RepoRoot) -> Result<ToolchainEvidence, ToolchainError> {
    if cfg!(debug_assertions) {
        return Err(ToolchainError::NonReleaseController);
    }
    let rustup = PathBuf::from(RUSTUP_PATH);
    let rustup_sha256 = super::adapter::sha256_regular_file(&rustup, MAX_TOOL_BYTES)?;
    let environment = rustup_environment()?;
    let cargo_path = rustup_tool_path(root, &rustup, "cargo", environment.clone())?;
    let cargo_sha256 = super::adapter::sha256_regular_file(&cargo_path, MAX_TOOL_BYTES)?;
    let cargo_version = checked_output(
        root,
        &cargo_path,
        vec![OsString::from("-Vv")],
        controlled_environment(),
    )?;
    let cargo_verbose_version = parse_tool_version("cargo", &cargo_version)?;
    let rustc_path = rustup_tool_path(root, &rustup, "rustc", environment)?;
    let rustc_sha256 = super::adapter::sha256_regular_file(&rustc_path, MAX_TOOL_BYTES)?;
    let version = checked_output(
        root,
        &rustc_path,
        vec![OsString::from("-vV")],
        controlled_environment(),
    )?;
    let rustc_verbose_version = std::str::from_utf8(&version)
        .map_err(ToolchainError::Utf8)?
        .trim()
        .to_string();
    if rustc_verbose_version.is_empty() || rustc_verbose_version.len() > 64 << 10 {
        return Err(ToolchainError::MalformedVersion);
    }
    let target_triple = rustc_verbose_version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or(ToolchainError::MissingTarget)?
        .to_string();
    if !target_triple.starts_with(std::env::consts::ARCH)
        || !target_triple.contains(std::env::consts::OS)
    {
        return Err(ToolchainError::TargetMismatch(target_triple));
    }
    Ok(ToolchainEvidence {
        rust_toolchain: RUST_TOOLCHAIN.to_string(),
        cargo_profile: "release".to_string(),
        rustup_path: rustup.to_string_lossy().into_owned(),
        rustup_sha256,
        cargo_path: cargo_path.to_string_lossy().into_owned(),
        cargo_sha256,
        cargo_verbose_version,
        rustc_path: rustc_path.to_string_lossy().into_owned(),
        rustc_sha256,
        rustc_verbose_version,
        target_triple,
    })
}

fn rustup_tool_path(
    root: &RepoRoot,
    rustup: &Path,
    tool: &'static str,
    environment: Vec<(OsString, OsString)>,
) -> Result<PathBuf, ToolchainError> {
    let output = checked_output(
        root,
        rustup,
        vec![
            OsString::from("which"),
            OsString::from(tool),
            OsString::from("--toolchain"),
            OsString::from(RUST_TOOLCHAIN),
        ],
        environment,
    )?;
    parse_single_line_path(&output)
}

fn parse_tool_version(tool: &'static str, bytes: &[u8]) -> Result<String, ToolchainError> {
    let value = std::str::from_utf8(bytes)
        .map_err(ToolchainError::Utf8)?
        .trim();
    if value.is_empty() || value.len() > 64 << 10 {
        return Err(ToolchainError::MalformedToolVersion(tool));
    }
    Ok(value.to_string())
}

fn checked_output(
    root: &RepoRoot,
    program: &Path,
    args: Vec<OsString>,
    environment: Vec<(OsString, OsString)>,
) -> Result<Vec<u8>, ToolchainError> {
    let output = run_bounded_process(&ProcessRequest {
        program: program.to_path_buf(),
        args,
        stdin: Vec::new(),
        working_directory: root.path.clone(),
        environment,
        affinity_cpu: None,
        limits: ProcessLimits {
            stdin_bytes: 0,
            stdout_bytes: 64 << 10,
            stderr_bytes: 64 << 10,
            regular_file_bytes: None,
            timeout: Duration::from_secs(30),
        },
    })?;
    if output.status != Some(0) || !output.stderr.is_empty() {
        return Err(ToolchainError::ProcessFailed {
            program: program.to_path_buf(),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(output.stdout)
}

fn parse_single_line_path(bytes: &[u8]) -> Result<PathBuf, ToolchainError> {
    let value = std::str::from_utf8(bytes)
        .map_err(ToolchainError::Utf8)?
        .trim();
    if value.is_empty() || value.lines().count() != 1 {
        return Err(ToolchainError::MalformedRustcPath);
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(ToolchainError::MalformedRustcPath);
    }
    std::fs::canonicalize(&path).map_err(|source| ToolchainError::Canonicalize { path, source })
}

fn rustup_environment() -> Result<Vec<(OsString, OsString)>, ToolchainError> {
    let home = std::env::var_os("HOME").ok_or(ToolchainError::MissingHome)?;
    let home_path = PathBuf::from(&home);
    if !home_path.is_absolute() {
        return Err(ToolchainError::InvalidHome(home_path));
    }
    let mut environment = controlled_environment();
    environment.push((OsString::from("HOME"), home));
    if let Some(rustup_home) = std::env::var_os("RUSTUP_HOME") {
        let path = PathBuf::from(&rustup_home);
        if !path.is_absolute() {
            return Err(ToolchainError::InvalidRustupHome(path));
        }
        environment.push((OsString::from("RUSTUP_HOME"), rustup_home));
    }
    environment.push((
        OsString::from("RUSTUP_TOOLCHAIN"),
        OsString::from(RUST_TOOLCHAIN),
    ));
    Ok(environment)
}

fn controlled_environment() -> Vec<(OsString, OsString)> {
    vec![
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (OsString::from("PATH"), OsString::from("/usr/bin:/bin")),
        (OsString::from("TZ"), OsString::from("UTC")),
    ]
}

#[derive(Debug, Error)]
pub(super) enum ToolchainError {
    #[error("performance qualification must run the release-profile controller")]
    NonReleaseController,
    #[error(transparent)]
    Adapter(#[from] super::adapter::AdapterError),
    #[error(transparent)]
    Process(#[from] super::process::ProcessError),
    #[error("toolchain process {program} failed with status {status:?}: {stderr}")]
    ProcessFailed {
        program: PathBuf,
        status: Option<i32>,
        stderr: String,
    },
    #[error("toolchain output is not UTF-8: {0}")]
    Utf8(std::str::Utf8Error),
    #[error("rustup returned a malformed rustc path")]
    MalformedRustcPath,
    #[error("failed to canonicalize rustc path {path}: {source}")]
    Canonicalize {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("rustc -vV output is malformed")]
    MalformedVersion,
    #[error("{0} verbose version output is malformed")]
    MalformedToolVersion(&'static str),
    #[error("rustc -vV output omits the host target")]
    MissingTarget,
    #[error("rustc target {0} differs from the running qualification host")]
    TargetMismatch(String),
    #[error("HOME is required to resolve the pinned rustup toolchain")]
    MissingHome,
    #[error("HOME is not an absolute path: {0}")]
    InvalidHome(PathBuf),
    #[error("RUSTUP_HOME is not an absolute path: {0}")]
    InvalidRustupHome(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rustc_path_parser_rejects_relative_and_multiline_values() {
        assert!(parse_single_line_path(b"relative/rustc\n").is_err());
        assert!(parse_single_line_path(b"/one\n/two\n").is_err());
    }
}
