use std::path::PathBuf;

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
}

impl ReleaseError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
