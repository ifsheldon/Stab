//! Data materialization, output validation, host control, and command errors.

use std::fmt;
use std::path::PathBuf;

use thiserror::Error;

use stab_process::{ProcessError, ProcessExit};

use super::join_comma;
use crate::e2e::model::WorkUnit;

/// An output-key spelling that names no witnessed output; it holds the rejected text.
#[derive(Debug, Eq, Error, PartialEq)]
#[error("unknown output key {0:?}")]
pub(crate) struct InvalidOutputKey(pub(crate) String);

/// A Stab record census that disagrees with pinned Stim's, or cannot be compared with it.
#[derive(Debug, Error, PartialEq)]
pub(crate) enum CensusMismatch {
    #[error(
        "one-bit comparison needs equal positive record counts, got Stim {stim_records} and Stab {stab_records}"
    )]
    RecordCounts {
        stim_records: u64,
        stab_records: u64,
    },
    #[error(
        "Stab one bits {stab_ones} differ from pinned Stim's {stim_ones} over {records} records by more than {sigma} standard errors ({standard_error:.6} per record)",
        sigma = crate::e2e::data::ONE_BIT_TWO_SAMPLE_SIGMA
    )]
    OneBits {
        stab_ones: u64,
        stim_ones: u64,
        records: u64,
        standard_error: f64,
    },
}

/// Materializing case inputs and validating case outputs.
#[derive(Debug, Error)]
pub(crate) enum DataError {
    #[error(transparent)]
    Pinned(#[from] CommandError),
    #[error("input byte count does not fit in u64")]
    InputTooLarge,
    #[error("semantic work declares {declared} {unit:?}, actual value is {actual}")]
    WorkMismatch {
        declared: u64,
        unit: WorkUnit,
        actual: u64,
    },
    #[error("output has {actual} bytes, expected at least {minimum}")]
    OutputTooShort { actual: usize, minimum: usize },
    #[error("record output has {actual} records, expected {expected}")]
    RecordCount { actual: u64, expected: usize },
    #[error("record output has {ones} one bits, expected at least {minimum}")]
    TooFewOnes { ones: u64, minimum: u64 },
    #[error("record bit count overflow")]
    BitCountOverflow,
    #[error("record one fraction {fraction:.6} exceeds {maximum:.6}")]
    OneFraction { fraction: f64, maximum: f64 },
    #[error("output byte count does not fit in u64")]
    OutputTooLarge,
    #[error("record output is invalid: {0}")]
    InvalidRecords(#[source] stab_records::FormatError),
    #[error("record output is invalid: {0}")]
    InvalidRecordStream(#[source] stab_records::RecordStreamReadError),
    #[error("record census overflow")]
    CensusOverflow,
    #[error("generic records cannot infer typed DETS namespaces")]
    DetsFormat,
    #[error("record recipes require positive records and bits")]
    RecordDimensions,
    #[error("cannot encode record fixture: {0}")]
    Encode(#[source] stab_records::FormatError),
    #[error("cannot encode record fixture: {0}")]
    EncodeWrite(#[source] stab_records::RecordWriteError),
    #[error("cannot reserve record fixture batch: {0}")]
    ReserveBatch(#[source] stab_records::FormatError),
    #[error("cannot encode record fixture: ptb64 record recipes require a multiple of 64 records")]
    PartialPtb64Group,
    #[error("typed DETS recipes require positive dimensions")]
    TypedDetsDimensions,
    #[error("typed DETS hit count exceeds its namespace")]
    TypedDetsHits,
    #[error("folded circuit dimensions must be positive")]
    FoldedDimensions,
    #[error("m2d circuit width must be positive")]
    M2dWidth,
    #[error("DEM recipe dimensions must be positive")]
    DemDimensions,
    #[error("invalid probability {0}")]
    Probability(f64),
    #[error("cannot format generated fixture text: {0}")]
    Format(#[from] fmt::Error),
}

/// Capturing and validating the measurement host.
#[derive(Debug, Error)]
pub(crate) enum HostError {
    #[error("cannot determine logical CPU count: {0}")]
    LogicalCpus(#[source] std::io::Error),
    #[error("cannot read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot scan /proc: {0}")]
    ScanProc(#[source] std::io::Error),
    #[error("competing benchmark processes are active: {}", join_comma(.0))]
    Competitors(Vec<CompetingProcess>),
    #[error("host identity or swap configuration changed during the run")]
    IdentityChanged,
    #[error(
        "swap I/O changed during timing: in {pages_in_before}->{pages_in_after}, out {pages_out_before}->{pages_out_after}"
    )]
    SwapIo {
        pages_in_before: u64,
        pages_in_after: u64,
        pages_out_before: u64,
        pages_out_after: u64,
    },
    #[error("formal timing requires an explicit affinity CPU")]
    MissingAffinity,
    #[error("formal timing requires at least one readable thermal zone")]
    NoThermalZone,
    #[error("thermal zone {zone} is {millidegrees} millidegrees, above {maximum}")]
    Hot {
        zone: String,
        millidegrees: u64,
        maximum: u64,
    },
    #[error("host exposes neither a CPU model nor a DMI product name")]
    NoCpuModel,
    #[error("/proc/vmstat has no {0}")]
    MissingVmstat(&'static str),
    #[error("{} has malformed process stat data", path.display())]
    MalformedStat { path: PathBuf },
    #[error("{} omits process state", path.display())]
    StatOmitsState { path: PathBuf },
    #[error("{} omits parent process id", path.display())]
    StatOmitsParent { path: PathBuf },
    #[error("{} has invalid parent process id: {source}", path.display())]
    StatParent {
        path: PathBuf,
        source: std::num::ParseIntError,
    },
}

/// A process found by the competitor scan; `command` is its raw command line.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CompetingProcess {
    pub(crate) pid: u32,
    pub(crate) command: String,
}

impl fmt::Display for CompetingProcess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.pid, self.command)
    }
}

/// Running a supervised command and checking its result.
#[derive(Debug, Error)]
pub(crate) enum CommandError {
    #[error("locate measurement launcher: {0}")]
    Launcher(#[source] std::io::Error),
    #[error(transparent)]
    Process(#[from] ProcessError),
    /// `stderr` is the child's own diagnostic text.
    #[error("{} failed with status {status} and stderr {stderr}", program.display())]
    Unclean {
        program: PathBuf,
        status: ProcessExit,
        stderr: Box<str>,
    },
    /// `stderr` is the child's own diagnostic text.
    #[error("{} failed: {stderr}", program.display())]
    Failed { program: PathBuf, stderr: Box<str> },
    #[error("{what} differs from pinned Stim")]
    Disagreement { what: AgreedOutput },
}

/// A deterministic input that pinned Stim and Stab must produce byte for byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgreedOutput {
    GeneratedCircuit,
    AnalyzedDem,
}

impl fmt::Display for AgreedOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::GeneratedCircuit => "generated circuit",
            Self::AnalyzedDem => "analyzed detector error model",
        })
    }
}
