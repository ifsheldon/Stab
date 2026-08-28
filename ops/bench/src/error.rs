use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum BenchError {
    #[error("failed to resolve repository root {path}: {source}")]
    ResolveRoot {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Stim source directory does not exist at {0}")]
    MissingStimSource(PathBuf),

    #[error("Stim source is at commit {actual}, expected {expected}")]
    WrongStimCommit { actual: String, expected: String },

    #[error("Stim source is at tag {actual}, expected {expected}")]
    WrongStimTag { actual: String, expected: String },

    #[error("Stim source has tracked local modifications:\n{status}")]
    DirtyStimSource { status: Box<str> },

    #[error("CMake build finished without producing {0}")]
    MissingStimBinary(PathBuf),

    #[error("failed to create benchmark output directory {path}: {source}")]
    CreateOutputDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("benchmark output path {path} is invalid: {reason}")]
    InvalidBenchmarkOutputDir { path: PathBuf, reason: String },

    #[error("benchmark output path {path} escaped {root}")]
    BenchmarkOutputEscaped { path: PathBuf, root: PathBuf },

    #[error("end-to-end benchmark validation failed:\n{0}")]
    E2e(String),

    #[error("failed to process benchmark JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Process(#[from] crate::process::ProcessError),

    #[error("{program} failed with status {status}\nstderr:\n{stderr}")]
    CommandFailed {
        program: String,
        status: String,
        stderr: Box<str>,
    },
}
