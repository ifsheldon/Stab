//! Run controller, case execution, and Rust pipeline errors.

use std::fmt;
use std::path::PathBuf;

use thiserror::Error;

use super::{CensusError, Implementation};
use crate::e2e::data::OutputKey;
use crate::e2e::model::{CaseId, Tier};

/// Selecting cases, building and guarding the measured programs, and closing a run.
#[derive(Debug, Error)]
pub(crate) enum RunError {
    #[error("internal subprocess dispatch reached the repository controller")]
    InternalDispatch,
    /// The payload is the signal-installation message from `stab-process`.
    #[error("cannot install signal handlers: {0}")]
    SignalHandlers(String),
    #[error("interrupted by SIGINT or SIGTERM; no bundle was published")]
    Interrupted,
    #[error("--allow-dirty is available only for smoke diagnostics")]
    AllowDirtyFormal,
    #[error("source tree is dirty; commit it or use --allow-dirty for smoke")]
    DirtySource,
    #[error("suite omits tier {tier}")]
    MissingTier { tier: Tier },
    #[error("source commit or cleanliness changed during the run")]
    SourceChanged,
    #[error("E2E {tier} report failed; retained bundle {}", bundle.display())]
    ReportFailed { tier: Tier, bundle: PathBuf },
    /// `filter` is the raw `--only` argument.
    #[error("E2E filter {filter} matched no family or case")]
    UnmatchedFilter { filter: String },
    #[error("E2E selection is empty")]
    EmptySelection,
    #[error("{program} output is not UTF-8: {source}")]
    NonUtf8Output {
        program: &'static str,
        source: std::string::FromUtf8Error,
    },
    #[error("cannot resolve benchmark binary: {0}")]
    ResolveBench(#[source] std::io::Error),
    #[error("benchmark binary is missing at {}", path.display())]
    MissingBench { path: PathBuf },
    #[error("failed to create {}: {source}", path.display())]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot create measured binary directory: {0}")]
    BinaryDirectory(#[source] std::io::Error),
    #[error("{implementation} binary {} changed during the run", path.display())]
    BinaryChanged {
        implementation: MeasuredProgram,
        path: PathBuf,
    },
    #[error("cargo wrote an unreadable message: {0}")]
    CargoMessage(#[source] std::io::Error),
    #[error("cargo build reported no executable for the stab binary")]
    NoStabExecutable,
    #[error("cannot read {}: {source}", path.display())]
    ReadBinary {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot write {}: {source}", path.display())]
    WriteBinary {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("rustc -vV omitted host target")]
    RustcHost,
    #[error("rustc -vV is empty")]
    RustcEmpty,
}

/// A program whose bytes the run hashes before preflight and after the last sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MeasuredProgram {
    Stim,
    Stab,
    Benchmark,
}

impl fmt::Display for MeasuredProgram {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Stim => "Stim",
            Self::Stab => "Stab",
            Self::Benchmark => "benchmark",
        })
    }
}

/// Preparing, preflighting, and measuring one case.
#[derive(Debug, Error)]
pub(crate) enum CaseError {
    #[error("cannot create case scratch: {0}")]
    Scratch(#[source] std::io::Error),
    #[error("input file {file} has no data")]
    InputWithoutData { file: String },
    #[error("cannot write {}: {source}", path.display())]
    WriteInput {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot clear {}: {source}", path.display())]
    ClearOutput {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot read {}: {source}", path.display())]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{side} stdout witness is missing")]
    MissingStdoutWitness { side: Implementation },
    #[error("{case} pipeline intermediate outputs differ from pinned Stim")]
    IntermediatesDiffer { case: CaseId },
    #[error("{case} preflight did not capture stdout")]
    StdoutNotCaptured { case: CaseId },
    #[error("{case} omitted output file {file}")]
    OmittedOutputFile { case: CaseId, file: String },
    #[error("{case} output file {file} has no contract")]
    OutputWithoutContract { case: CaseId, file: String },
    #[error("cannot digest case {case}: {source}")]
    Digest {
        case: CaseId,
        source: serde_json::Error,
    },
    #[error("{case} has no Stim output expectation")]
    NoStimExpectation { case: CaseId },
    #[error("{case} output size changed during paired sample {index}")]
    OutputSizeChanged { case: CaseId, index: usize },
    #[error("{case} semantic output changed during Stab sample {index}")]
    SemanticOutputChanged { case: CaseId, index: usize },
    #[error("{0}")]
    ByteCount(ByteCountOverflow),
    #[error("{case} pipeline wall time overflowed")]
    PipelineTime { case: CaseId },
    #[error("{case} pipeline RSS was not observed")]
    PipelineRss { case: CaseId },
    #[error("{case} pipeline has no final step")]
    NoFinalStep { case: CaseId },
    #[error("{case} Rust workload reached the CLI runner")]
    RustOnCliRunner { case: CaseId },
    #[error("{case} non-Rust workload reached Rust runner")]
    CliOnRustRunner { case: CaseId },
    #[error("{case} process RSS was not observed")]
    ProcessRss { case: CaseId },
    #[error("{case} Rust batch output changed")]
    RustBatchChanged { case: CaseId },
    #[error("Rust batch is empty")]
    EmptyRustBatch,
    #[error("{case} Rust worker receipt is invalid: {source}")]
    WorkerReceipt {
        case: CaseId,
        source: serde_json::Error,
    },
    #[error("{case} Rust worker receipt violates its semantic contract")]
    ReceiptContract { case: CaseId },
    #[error("{case} Rust worker RSS was not observed")]
    WorkerRss { case: CaseId },
    #[error("{case} batch output byte count overflow")]
    BatchBytes { case: CaseId },
    #[error("{case} semantic work overflow")]
    WorkOverflow { case: CaseId },
    #[error("{case} output {key}: {source}")]
    Census {
        case: CaseId,
        key: OutputKey,
        source: CensusError,
    },
    #[error("exact output {key} differs from pinned Stim")]
    ExactOutputDiffers { key: OutputKey },
    /// `argument` is the raw suite argument.
    #[error("unknown or partial file placeholder {argument}")]
    Placeholder { argument: String },
}

/// A byte counter that left its integer range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ByteCountOverflow {
    CliBatch,
    SideOutput,
    TotalOutput,
    RustBatch,
    RustOutput,
}

impl fmt::Display for ByteCountOverflow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CliBatch => "CLI output byte count overflow",
            Self::SideOutput => "side-output byte count does not fit in u64",
            Self::TotalOutput => "total output byte count overflow",
            Self::RustBatch => "Rust output byte count overflow",
            Self::RustOutput => "Rust output size does not fit in u64",
        })
    }
}

/// A stage of the reusable Rust pipeline whose component API can fail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PipelineStage {
    CodeParameters,
    CircuitGeneration,
    ErrorAnalysis,
    DecoderCompilation,
    SamplingCompilation,
    DetectionCompilation,
    SamplingSession,
    DetectionSession,
    PredictionBatch,
    Transaction,
    Sampling,
    Decoding,
}

impl fmt::Display for PipelineStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CodeParameters => "code parameters",
            Self::CircuitGeneration => "circuit generation",
            Self::ErrorAnalysis => "error analysis",
            Self::DecoderCompilation => "decoder compilation",
            Self::SamplingCompilation => "sampling compilation",
            Self::DetectionCompilation => "detection compilation",
            Self::SamplingSession => "sampling session",
            Self::DetectionSession => "detection session",
            Self::PredictionBatch => "prediction batch",
            Self::Transaction => "detection transaction",
            Self::Sampling => "sampling",
            Self::Decoding => "decoding",
        })
    }
}

/// The reusable Rust sample, detect, and decode worker.
#[derive(Debug, Error)]
pub(crate) enum PipelineError {
    #[error("cannot read start barrier: {0}")]
    Barrier(#[source] std::io::Error),
    #[error("start barrier must be one newline")]
    BarrierByte,
    #[error("{stage}: {source}")]
    Stage {
        stage: PipelineStage,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("compiled sampling and decoder layouts disagree")]
    LayoutMismatch,
    #[error("sampling committed {committed} of {shots} shots")]
    CommittedShots { committed: u64, shots: u64 },
    #[error("detection sink did not finish")]
    SinkUnfinished,
    #[error(
        "pipeline produced {failures} failures in {observed_shots} shots, expected {minimum}..={maximum} in {shots}"
    )]
    FailureBand {
        failures: u64,
        observed_shots: u64,
        minimum: u64,
        maximum: u64,
        shots: u64,
    },
    #[error("cannot write worker receipt: {0}")]
    ReceiptJson(#[source] serde_json::Error),
    #[error("cannot write worker receipt: {0}")]
    ReceiptWrite(#[source] std::io::Error),
    #[error("decoder completed {completed} of {records} records")]
    DecoderIncomplete { completed: usize, records: usize },
    #[error("prediction bit is missing for shot {shot}")]
    MissingPrediction { shot: usize },
    #[error("observable bit is missing for shot {shot}")]
    MissingObservable { shot: usize },
    #[error("logical failure count overflow")]
    FailureOverflow,
    #[error("batch shot count does not fit in u64")]
    BatchShots,
    #[error("pipeline shot count overflow")]
    ShotOverflow,
}

impl PipelineError {
    /// Wraps a component API failure with the stage that raised it.
    pub(crate) fn stage<E>(stage: PipelineStage) -> impl FnOnce(E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        move |source| Self::Stage {
            stage,
            source: Box::new(source),
        }
    }
}
