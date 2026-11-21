//! Bundle, replay, baseline-candidate, and release-source errors.

use std::ffi::OsString;
use std::path::PathBuf;

use thiserror::Error;

use super::{HostError, ReportError, SuiteViolations};
use crate::e2e::model::CaseId;
use crate::e2e::report::CORRECTNESS_SCHEMA_VERSION;

/// Publishing, replaying, and pairing bundles, and checking the release evidence and its source.
#[derive(Debug, Error)]
pub(crate) enum BundleError {
    #[error("cannot serialize the baseline candidate: {0}")]
    SerializeCandidate(#[source] toml::ser::Error),
    #[error("evidence pointer and measured source identities differ")]
    PointerSourceMismatch,
    #[error("release evidence must come from controlled AArch64")]
    NotAarch64,
    #[error("release evidence does not cover every release case in source order")]
    IncompleteCoverage,
    #[error("cannot digest the E2E workload contract: {0}")]
    ContractDigest(#[source] serde_json::Error),
    #[error("current and measured E2E workload contracts differ")]
    ContractMismatch,
    #[error("release evidence fails Stim parity or memory policy")]
    ReleaseGates,
    #[error("first release evidence must report self-regression as unseeded")]
    FirstEvidenceSeeded,
    #[error("current suite does not contain the reviewed first self-regression baseline")]
    MissingFirstBaseline,
    #[error("seeded release evidence fails or changes its accepted self baseline")]
    SeededEvidence,
    #[error("release evidence requires a clean current source tree")]
    DirtySource,
    /// `path` is the repository-relative spelling Git reported.
    #[error("release descendant changes non-evidence path {path}")]
    NonEvidencePath { path: String },
    #[error("git diff path is not UTF-8: {0}")]
    NonUtf8DiffPath(#[source] std::string::FromUtf8Error),
    #[error(
        "evidence bundle path {} is not a normal repository-relative path",
        path.display()
    )]
    AbnormalEvidencePath { path: PathBuf },
    #[error("evidence bundle {} is outside benchmarks/evidence/aarch64", path.display())]
    EvidenceOutsideRoot { path: PathBuf },
    #[error("cannot inspect {}: {source}", path.display())]
    Inspect {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("evidence path {} is a symbolic link", path.display())]
    EvidenceSymlink { path: PathBuf },
    #[error("soak omits {case}")]
    SoakOmits { case: CaseId },
    #[error("case digest differs for {case}")]
    CandidateDigest { case: CaseId },
    #[error("full and soak bundles contain different case sets")]
    CandidateCaseSets,
    #[error("failed to create {}: {source}", path.display())]
    Create {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write {}: {source}", path.display())]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read bundled suite: {0}")]
    ReadSuite(#[source] std::io::Error),
    #[error("bundled suite is not UTF-8: {0}")]
    SuiteUtf8(#[source] std::str::Utf8Error),
    #[error("bundled suite is invalid: {0}")]
    SuiteToml(#[source] toml::de::Error),
    #[error(transparent)]
    SuiteViolations(#[from] SuiteViolations),
    #[error("bundled suite digest does not match run metadata")]
    SuiteDigest,
    #[error(transparent)]
    Derivation(#[from] ReportError),
    #[error("offline report derivation differs from report.json")]
    ReportDiffers,
    #[error("failed to read {name}: {source}")]
    ReadMember {
        name: &'static str,
        source: std::io::Error,
    },
    #[error("report.json bytes are not canonical replay output")]
    NonCanonicalReport,
    #[error("report.md differs from deterministic replay")]
    MarkdownDiffers,
    #[error("run metadata uses the wrong Stim commit")]
    WrongStimCommit,
    #[error("run tier, formal status, and source cleanliness disagree")]
    FormalStatus,
    #[error("run metadata has an invalid {0} identity")]
    InvalidIdentity(&'static str),
    #[error("run metadata omits the Rust toolchain or target")]
    MissingToolchain,
    #[error(transparent)]
    Host(#[from] HostError),
    #[error("baseline candidate requires one full and one soak bundle")]
    CandidateTiers,
    #[error("baseline candidate requires formal clean-source bundles")]
    CandidateFormal,
    #[error("full and soak {0} identities differ")]
    CandidateIdentity(&'static str),
    #[error("full and soak affinity or selected case sets differ")]
    CandidateSelection,
    #[error("baseline candidate requires passing parity and memory with no failed self-regression")]
    CandidateGates,
    #[error("cannot read {}: {source}", path.display())]
    ReadDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot inspect a member of {}: {source}", path.display())]
    DirectoryEntry {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("bundle member {} is not a regular file", path.display())]
    IrregularMember { path: PathBuf },
    #[error("bundle member {name:?} is not valid UTF-8")]
    NonUtf8Member { name: OsString },
    #[error("bundle directory has missing or extra entries")]
    DirectoryEntries,
    #[error("replay schema is {actual}, expected {expected}")]
    ReplaySchema { actual: u16, expected: u16 },
    #[error("replay manifest has missing or extra files")]
    ManifestFiles,
    #[error("replay file {file} has the wrong digest")]
    ReplayDigest { file: String },
    #[error("correctness schema is {actual}, expected {CORRECTNESS_SCHEMA_VERSION}")]
    CorrectnessSchema { actual: u16 },
    #[error("correctness report case set differs from run selection")]
    CorrectnessCases,
    #[error("correctness case {case} has no Stab output witness")]
    NoStabWitness { case: CaseId },
    #[error("failed to read {}: {source}", path.display())]
    ReadJson {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse {}: {source}", path.display())]
    ParseJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("cannot read {}: {source}", path.display())]
    ReadForDigest {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("current evidence pointer {pointer} is a symbolic link")]
    PointerSymlink { pointer: &'static str },
    #[error("current evidence pointer {pointer} is not a regular file")]
    PointerNotFile { pointer: &'static str },
    #[error("current AArch64 evidence is unavailable at {pointer}: {source}")]
    PointerUnavailable {
        pointer: &'static str,
        source: std::io::Error,
    },
    #[error("current evidence pointer is invalid: {0}")]
    PointerToml(#[source] toml::de::Error),
    #[error("current evidence pointer violates schema 1")]
    PointerSchema,
}
