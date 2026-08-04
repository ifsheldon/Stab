use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("failed to resolve repository root {path}: {source}")]
    ResolveRoot {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("release repository is not clean: {0}")]
    DirtyRepository(String),
    #[error("release repository changed from {before} to {after}")]
    RepositoryChanged { before: String, after: String },
    #[error("release command {program} failed with status {status}: {stderr}")]
    CommandFailed {
        program: String,
        status: String,
        stderr: String,
    },
    #[error("release command {program} produced more than {limit} bytes")]
    CommandOutputLimit { program: String, limit: usize },
    #[error("release command {program} timed out after {timeout:?}")]
    CommandTimeout { program: String, timeout: Duration },
    #[error("release command {program} was interrupted by SIGINT or SIGTERM")]
    CommandInterrupted { program: String },
    #[error("release operation {operation} was interrupted by SIGINT or SIGTERM")]
    OperationInterrupted { operation: String },
    #[error("failed to install release cancellation handlers: {0}")]
    CommandSignalHandlers(String),
    #[error("failed to capture release command {program}: {source}")]
    CommandCapture {
        program: String,
        source: std::io::Error,
    },
    #[error("release command {program} has an invalid process identity")]
    InvalidProcessIdentity { program: String },
    #[error("release command output is not UTF-8: {0}")]
    CommandUtf8(#[from] std::string::FromUtf8Error),
    #[error("failed to execute release command {program}: {source}")]
    CommandIo {
        program: String,
        source: std::io::Error,
    },
    #[error("architecture preflight failed: {0}")]
    Architecture(String),
    #[error("failed to read Cargo workspace metadata: {0}")]
    Metadata(#[from] cargo_metadata::Error),
    #[error("release package contract violation: {0}")]
    PackageContract(String),
    #[error("release archive contract violation for {path}: {detail}")]
    ArchiveContract { path: PathBuf, detail: String },
    #[error("release toolchain changed or differs from the reviewed preflight: {0}")]
    ToolchainIdentity(String),
    #[error("crates.io request failed: {0}")]
    Registry(String),
    #[error("crates.io checksum for {package} {version} is {actual}, expected {expected}")]
    RegistryChecksum {
        package: String,
        version: String,
        expected: String,
        actual: String,
    },
    #[error("crates.io version {package} {version} is yanked and cannot satisfy release recovery")]
    RegistryYanked { package: String, version: String },
    #[error(
        "crates.io did not expose {package} {version} with checksum {checksum} before the visibility deadline"
    )]
    RegistryVisibility {
        package: String,
        version: String,
        checksum: String,
    },
    #[error("release publication confirmation must be {expected}, found {actual}")]
    PublicationConfirmation { expected: String, actual: String },
    #[error("release publication state conflicts with the reviewed preflight: {0}")]
    PublicationState(String),
    #[error("release credential environment violates the {operation} boundary; unset: {variables}")]
    CredentialEnvironment {
        operation: &'static str,
        variables: String,
    },
    #[error("release path is invalid: {0}")]
    InvalidPath(PathBuf),
    #[error("release target label is invalid: {0:?}")]
    InvalidTarget(String),
    #[error("release output already exists: {0}")]
    OutputExists(PathBuf),
    #[error("release path traverses a symbolic link: {0}")]
    SymlinkPath(PathBuf),
    #[error("release input is not a regular file: {0}")]
    NotRegularFile(PathBuf),
    #[error("release input exceeds its {limit}-byte bound: {path}")]
    FileTooLarge { path: PathBuf, limit: u64 },
    #[error("release directory exceeds its {limit}-entry bound: {path}")]
    DirectoryTooLarge { path: PathBuf, limit: usize },
    #[error("release directory exceeds its {limit}-level depth bound: {path}")]
    DirectoryDepth { path: PathBuf, limit: usize },
    #[error("release input changed while it was being used: {0}")]
    FileIdentityChanged(PathBuf),
    #[error("release binary contract violation: {0}")]
    BinaryContract(String),
    #[error("GitHub release contract violation: {0}")]
    GitHubRelease(String),
    #[error("release tag must be {expected}, found {actual}")]
    TagName { expected: String, actual: String },
    #[error("release tag {tag} is not an annotated tag object")]
    TagKind { tag: String },
    #[error("release tag {tag} resolves to {tag_commit}, not current commit {head}")]
    TagCommit {
        tag: String,
        tag_commit: String,
        head: String,
    },
    #[error("failed to access release path {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to serialize release report: {0}")]
    Json(#[from] serde_json::Error),
    #[error("release operations require a Unix host with no-follow descriptor support")]
    UnsupportedPlatform,
}

impl ReleaseError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
