//! Suite loading, validation, and prerequisite errors.

use std::fmt;
use std::path::PathBuf;

use thiserror::Error;

use super::join_lines;
use crate::e2e::model::{
    CaseId, MAXIMUM_CASE_RSS_BYTES, PARITY_MAX_RATIO, SELF_REGRESSION_MAX_RATIO, STIM_COMMIT,
    STIM_VERSION, SUITE_SCHEMA_VERSION, TIMING_BOUNDARY, Tier,
};

/// Loading, validating, and documenting the source-owned suite and its prerequisites.
#[derive(Debug, Error)]
pub(crate) enum SuiteError {
    #[error("failed to read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error(transparent)]
    Invalid(#[from] SuiteViolations),
    #[error("failed to write {}: {source}", path.display())]
    WriteDocs {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "failed to read generated {}: {source}; run just bench::e2e-check --write-docs",
        path.display()
    )]
    ReadDocs {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{} is stale; run just bench::e2e-check --write-docs", path.display())]
    StaleDocs { path: PathBuf },
    #[error("duplicate parity family {family}")]
    DuplicateParityFamily { family: String },
    #[error("{}", join_lines(.0))]
    Prerequisites(Vec<PrerequisiteIssue>),
}

/// One correctness prerequisite that the parity ledger does not satisfy.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PrerequisiteIssue {
    Unsatisfied {
        prerequisite: String,
        status: &'static str,
    },
    NotInLedger {
        prerequisite: String,
    },
}

impl fmt::Display for PrerequisiteIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsatisfied {
                prerequisite,
                status,
            } => write!(
                formatter,
                "correctness prerequisite {prerequisite} has status {status}"
            ),
            Self::NotInLedger { prerequisite } => write!(
                formatter,
                "correctness prerequisite {prerequisite} is not in the parity ledger"
            ),
        }
    }
}

/// Every violation one suite validation found, rendered one per line.
#[derive(Debug, Error)]
#[error("{}", join_lines(.0))]
pub(crate) struct SuiteViolations(pub(crate) Vec<SuiteViolation>);

/// One violated suite invariant.
#[derive(Debug, Error)]
pub(crate) enum SuiteViolation {
    #[error("schema_version is {actual}, expected {SUITE_SCHEMA_VERSION}")]
    SchemaVersion { actual: u16 },
    #[error("Stim identity must remain {STIM_VERSION} at {STIM_COMMIT}")]
    StimIdentity,
    #[error("{0}")]
    Policy(PolicyField),
    #[error(
        "tier {tier} must use {expected_warmups} warmups and {expected_samples} samples, found {warmups} and {samples}"
    )]
    TierShape {
        tier: Tier,
        expected_warmups: usize,
        expected_samples: usize,
        warmups: usize,
        samples: usize,
    },
    #[error("tier {tier} is missing")]
    TierMissing { tier: Tier },
    #[error("release family count {count} exceeds {maximum}")]
    FamilyCap { count: usize, maximum: usize },
    #[error("expected_release_families is {declared}, actual count is {actual}")]
    FamilyCount { declared: usize, actual: usize },
    #[error("release case count {count} exceeds {maximum}")]
    CaseCap { count: usize, maximum: usize },
    #[error("expected_release_cases is {declared}, actual count is {actual}")]
    CaseCount { declared: usize, actual: usize },
    /// The rejected identifier is the raw suite text that failed validation.
    #[error("{kind} id {value:?} is not lowercase kebab-case")]
    InvalidId { kind: IdKind, value: String },
    #[error("duplicate family id {family}")]
    DuplicateFamily { family: String },
    #[error("family {family} {problem}")]
    Family {
        family: String,
        problem: FamilyProblem,
    },
    #[error("duplicate case id {case}")]
    DuplicateCase { case: CaseId },
    #[error("case {case} {problem}")]
    Case { case: CaseId, problem: CaseProblem },
    #[error("duplicate self baseline for {case} on {architecture}")]
    DuplicateBaseline { case: CaseId, architecture: String },
    #[error("self baseline {case} {problem}")]
    Baseline {
        case: CaseId,
        problem: BaselineProblem,
    },
}

/// A policy field that must keep its reviewed value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PolicyField {
    TimingBoundary,
    ParityMaxRatio,
    SelfRegressionMaxRatio,
    BootstrapSeed,
    BootstrapResamples,
    ConfidenceLevel,
    MaximumTemperature,
    MaximumOutputBytes,
    CommandTimeout,
}

impl fmt::Display for PolicyField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        use crate::e2e::model::{
            BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED, COMMAND_TIMEOUT_SECONDS, CONFIDENCE_LEVEL,
            MAXIMUM_OUTPUT_BYTES, MAXIMUM_TEMPERATURE_MILLIDEGREES,
        };
        match self {
            Self::TimingBoundary => write!(formatter, "timing_boundary must be {TIMING_BOUNDARY}"),
            Self::ParityMaxRatio => {
                write!(formatter, "parity_max_ratio must remain {PARITY_MAX_RATIO}")
            }
            Self::SelfRegressionMaxRatio => write!(
                formatter,
                "self_regression_max_ratio must remain {SELF_REGRESSION_MAX_RATIO}"
            ),
            Self::BootstrapSeed => write!(formatter, "bootstrap_seed must remain {BOOTSTRAP_SEED}"),
            Self::BootstrapResamples => write!(
                formatter,
                "bootstrap_resamples must remain {BOOTSTRAP_RESAMPLES}"
            ),
            Self::ConfidenceLevel => {
                write!(formatter, "confidence_level must remain {CONFIDENCE_LEVEL}")
            }
            Self::MaximumTemperature => write!(
                formatter,
                "maximum_temperature_millidegrees must remain {MAXIMUM_TEMPERATURE_MILLIDEGREES}"
            ),
            Self::MaximumOutputBytes => write!(
                formatter,
                "maximum_output_bytes must remain {MAXIMUM_OUTPUT_BYTES}"
            ),
            Self::CommandTimeout => write!(
                formatter,
                "command_timeout_seconds must remain {COMMAND_TIMEOUT_SECONDS}"
            ),
        }
    }
}

/// The namespace of a validated suite identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IdKind {
    Family,
    Case,
    File,
}

impl fmt::Display for IdKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Family => "family",
            Self::Case => "case",
            Self::File => "file",
        })
    }
}

/// A family-level suite violation.
#[derive(Debug, Error)]
pub(crate) enum FamilyProblem {
    #[error("has an empty description")]
    EmptyDescription,
    #[error("has no correctness prerequisites")]
    NoPrerequisites,
    #[error("repeats prerequisite {0}")]
    RepeatedPrerequisite(String),
    #[error("has inconsistent runner and comparator")]
    InconsistentRunner,
    #[error("has no cases")]
    NoCases,
}

/// A data recipe whose parameters are out of range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecipeKind {
    FoldedCircuit,
    Records,
    TypedDets,
    Dem,
}

impl fmt::Display for RecipeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::FoldedCircuit => "folded-circuit",
            Self::Records => "record",
            Self::TypedDets => "typed-DETS",
            Self::Dem => "DEM",
        })
    }
}

/// A case-level suite violation.
#[derive(Debug, Error)]
pub(crate) enum CaseProblem {
    #[error("has a zero batch")]
    ZeroBatch,
    #[error("has zero semantic work")]
    ZeroWork,
    #[error("has a zero RSS limit")]
    ZeroRss,
    #[error("exceeds the {MAXIMUM_CASE_RSS_BYTES}-byte RSS policy cap")]
    RssCap,
    #[error("pipeline needs at least two steps")]
    ShortPipeline,
    #[error("first pipeline step needs empty stdin")]
    FirstStepStdin,
    #[error("later pipeline steps need previous stdout")]
    LaterStepStdin,
    #[error("Rust pipeline needs matching positive work and a nontrivial logical-failure band")]
    RustPipelineBand,
    #[error("workload does not match its family runner")]
    RunnerMismatch,
    #[error("DEM recipe must run analyze_errors")]
    DemWithoutAnalysis,
    #[error("sampled-record recipe needs seeded sample with --shots equal to records")]
    SampledRecords,
    #[error("has an invalid {0} recipe")]
    InvalidRecipe(RecipeKind),
    #[error("has a zero-width m2d circuit")]
    ZeroWidthM2d,
    #[error("has a vacuous exact-output contract")]
    VacuousExactOutput,
    #[error("has an invalid record-output contract")]
    InvalidRecordOutput,
    #[error("record work does not equal its input recipe")]
    RecordWork,
    #[error("shot work does not equal its --shots argument")]
    ShotArgument,
    #[error("shot work does not equal its output record count")]
    ShotRecords,
    #[error("command args need a named subcommand")]
    MissingSubcommand,
    #[error("command args contain NUL")]
    NulArgument,
    #[error("repeats file {0}")]
    RepeatedFile(String),
    #[error("input file {0} needs data and no output contract")]
    InputFileShape(String),
    #[error("output file {0} needs an output contract and no data")]
    OutputFileShape(String),
    #[error("has an unknown or partial file placeholder")]
    UnknownPlaceholder,
    #[error("does not route file {0} into its command")]
    UnroutedFile(String),
}

/// A self-regression baseline that does not match the suite it belongs to.
#[derive(Debug, Error)]
pub(crate) enum BaselineProblem {
    #[error("has a stale case digest")]
    StaleDigest,
    #[error("cannot digest its case: {0}")]
    Digest(#[source] serde_json::Error),
    #[error("has no active release case")]
    NoCase,
    #[error("has an incomplete identity")]
    IncompleteIdentity,
    #[error("has invalid {0}")]
    InvalidBound(BaselineBound),
    #[error("upper bound is below its median")]
    UpperBelowMedian,
}

/// One of a baseline's two timing bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BaselineBound {
    Median,
    Upper,
}

impl fmt::Display for BaselineBound {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Median => "median",
            Self::Upper => "upper",
        })
    }
}
