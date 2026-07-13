use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum BenchError {
    #[error("failed to resolve repository root {path}: {source}")]
    ResolveRoot {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read benchmark baseline {path}: {source}")]
    ReadBaseline {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read benchmark compare report {path}: {source}")]
    ReadCompareReport {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read benchmark threshold file {path}: {source}")]
    ReadThresholds {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read benchmark beta-waiver file {path}: {source}")]
    ReadBetaWaivers {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read benchmark regression-waiver file {path}: {source}")]
    ReadRegressionWaivers {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse benchmark manifest: {0}")]
    ParseManifest(#[from] csv::Error),

    #[error("failed to parse benchmark threshold file {path}: {source}")]
    ParseThresholds {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("failed to parse benchmark beta-waiver file {path}: {source}")]
    ParseBetaWaivers {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("failed to parse benchmark regression-waiver file {path}: {source}")]
    ParseRegressionWaivers {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("failed to parse benchmark compare report {path}: {source}")]
    ParseCompareReport {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("benchmark manifest validation failed:\n{0}")]
    ManifestValidation(Box<str>),

    #[error("benchmark threshold file {path} is invalid:\n{details}")]
    ThresholdValidation { path: PathBuf, details: Box<str> },

    #[error("benchmark beta-waiver file {path} is invalid:\n{details}")]
    BetaWaiverValidation { path: PathBuf, details: Box<str> },

    #[error("benchmark regression-waiver file {path} is invalid:\n{details}")]
    RegressionWaiverValidation { path: PathBuf, details: Box<str> },

    #[error("memory baseline compare report {path} is invalid:\n{details}")]
    MemoryBaselineValidation { path: PathBuf, details: Box<str> },

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

    #[error("failed to process benchmark JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("source input validation failed: {0}")]
    SourceInput(String),

    #[error("failed to access source input {path}: {source}")]
    SourceInputIo {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("performance qualification validation failed:\n{0}")]
    Qualification(String),

    #[error("failed to access performance qualification input {path}: {source}")]
    QualificationIo {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error(
        "performance qualification inventory differs from deterministic regeneration; run just bench::qualification-regenerate"
    )]
    QualificationDrift,

    #[error("performance qualification inventory digest is not frozen in source code")]
    QualificationUnfrozen,

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

    #[error("measurement_runs must be at least 1")]
    InvalidMeasurementRuns,

    #[error("benchmark output path {path} is invalid: {reason}")]
    InvalidBenchmarkOutputDir { path: PathBuf, reason: String },

    #[error("benchmark output path {path} escaped {root}")]
    BenchmarkOutputEscaped { path: PathBuf, root: PathBuf },

    #[error("benchmark comparison is incomplete:\n{details}")]
    CompareIncomplete { details: Box<str> },

    #[error("benchmark baseline metadata does not match pinned Stim v1.16.0:\n{details}")]
    BaselineMetadataMismatch { details: Box<str> },

    #[error(
        "--require-profiler-notes requires --report so profiler notes can be read beside the report"
    )]
    ProfilerNotesRequireReport,

    #[error("required profiler notes are missing or invalid:\n{details}")]
    ProfilerNotesMissing { details: Box<str> },

    #[error("beta performance gate failed:\n{details}")]
    BetaGateFailed { details: Box<str> },

    #[error("--beta-waivers requires --require-beta-gate")]
    BetaWaiversRequireGate,

    #[error("--require-memory-gate requires --track-allocations")]
    MemoryGateRequiresAllocationTracking,

    #[error("--require-memory-gate requires --memory-baseline")]
    MemoryGateRequiresBaseline,

    #[error("--memory-baseline requires --require-memory-gate")]
    MemoryBaselineRequiresGate,

    #[error("beta memory gate failed:\n{details}")]
    MemoryGateFailed { details: Box<str> },

    #[error("regression threshold gate failed:\n{details}")]
    RegressionThresholdFailed { details: Box<str> },

    #[error("--regression-waivers requires --thresholds")]
    RegressionWaiversRequireThresholds,

    #[error("--track-allocations requires building stab-bench with --features count-allocations")]
    AllocationTrackingUnavailable,
}
