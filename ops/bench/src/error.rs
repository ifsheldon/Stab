use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum BenchError {
    #[error("failed to resolve repository root {path}: {source}")]
    ResolveRoot {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read benchmark manifest {path}: {source}")]
    ReadManifest {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read benchmark baseline {path}: {source}")]
    ReadBaseline {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse benchmark manifest: {0}")]
    ParseManifest(#[from] csv::Error),

    #[error("benchmark manifest validation failed:\n{0}")]
    ManifestValidation(Box<str>),

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

    #[error("failed to write benchmark output {path}: {source}")]
    WriteOutput {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read benchmark stdin {path}: {source}")]
    ReadStdin {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to process benchmark JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("failed to start {program}: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },

    #[error("failed to write stdin for {program}: {source}")]
    WriteStdin {
        program: String,
        source: std::io::Error,
    },

    #[error("failed to wait for {program}: {source}")]
    Wait {
        program: String,
        source: std::io::Error,
    },

    #[error("{program} timed out after {seconds}s")]
    TimedOut { program: String, seconds: u64 },

    #[error("{program} failed with status {status}\nstderr:\n{stderr}")]
    CommandFailed {
        program: String,
        status: String,
        stderr: Box<str>,
    },

    #[error("{row_id} produced no parseable stim_perf measurements")]
    MissingPerfMeasurements { row_id: String },

    #[error("{row_id} Stab comparison runner failed: {message}")]
    StabRunner { row_id: String, message: String },

    #[error("benchmark filter matched no rows: {0}")]
    UnmatchedFilter(String),

    #[error("cli_iterations does not fit in usize: {0}")]
    CliIterationsOverflow(u32),

    #[error("baseline target seconds must be positive")]
    InvalidTargetSeconds,

    #[error("cli_iterations must be at least 1")]
    InvalidCliIterations,

    #[error("benchmark output path {path} is invalid: {reason}")]
    InvalidBenchmarkOutputDir { path: PathBuf, reason: String },

    #[error("benchmark output path {path} escaped {root}")]
    BenchmarkOutputEscaped { path: PathBuf, root: PathBuf },

    #[error("benchmark comparison is incomplete:\n{details}")]
    CompareIncomplete { details: Box<str> },
}
