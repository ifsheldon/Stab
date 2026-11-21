//! Statistics and report derivation errors.

use std::fmt;

use thiserror::Error;

use super::{CensusMismatch, join_comma};
use crate::e2e::data::OutputKey;
use crate::e2e::model::{CaseId, Tier};
use crate::e2e::report::{RUN_SCHEMA_VERSION, SAMPLES_SCHEMA_VERSION};

/// Which implementation a timing statement is about.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Implementation {
    Stim,
    Stab,
}

impl fmt::Display for Implementation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Stim => "Stim",
            Self::Stab => "Stab",
        })
    }
}

/// Summarizing raw timing samples.
#[derive(Debug, Error, PartialEq)]
pub(crate) enum StatisticsError {
    #[error("paired timing has no samples")]
    NoPairedSamples,
    #[error("paired sample {position} has the wrong identity or order")]
    PairIdentity { position: usize },
    #[error("paired sample {position} has unequal semantic work {stim} versus {stab}")]
    UnequalWork {
        position: usize,
        stim: u64,
        stab: u64,
    },
    #[error("paired timing has no Stab RSS")]
    NoPairedRss,
    #[error("paired timing has no Stab output size")]
    NoPairedOutput,
    #[error("Stab timing has no samples")]
    NoStabSamples,
    #[error("Stab sample {position} has the wrong index")]
    StabIndex { position: usize },
    #[error("Stab timing has no RSS")]
    NoStabRss,
    #[error("Stab timing has no output size")]
    NoStabOutput,
    #[error("invalid elapsed seconds {0}")]
    Seconds(f64),
    #[error("semantic work must be positive")]
    ZeroWork,
    #[error("peak RSS must be positive")]
    ZeroRss,
    #[error("bootstrap resample count must be positive")]
    ZeroResamples,
    #[error("bootstrap confidence must be between zero and one")]
    Confidence,
    #[error("bootstrap index escaped values")]
    BootstrapIndex,
    #[error("bootstrap {0} quantile is missing")]
    MissingQuantile(QuantileSide),
    #[error("distribution has no values")]
    NoValues,
    #[error("distribution contains a nonpositive or nonfinite value")]
    InvalidValue,
    #[error("median {0}value is missing")]
    MissingMedian(MedianPart),
    #[error("{0} output byte count changed across samples")]
    OutputDrift(Implementation),
}

/// The lower or upper bootstrap quantile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QuantileSide {
    Lower,
    Upper,
}

impl fmt::Display for QuantileSide {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Lower => "lower",
            Self::Upper => "upper",
        })
    }
}

/// The sorted value a median reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MedianPart {
    Lower,
    Upper,
    Middle,
}

impl fmt::Display for MedianPart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Lower => "lower ",
            Self::Upper => "upper ",
            Self::Middle => "",
        })
    }
}

/// Deriving a report from a run's metadata, witnesses, and raw samples.
#[derive(Debug, Error)]
pub(crate) enum ReportError {
    #[error("run schema is {actual}, expected {RUN_SCHEMA_VERSION}")]
    RunSchema { actual: u16 },
    #[error("sample schema is {actual}, expected {SAMPLES_SCHEMA_VERSION}")]
    SampleSchema { actual: u16 },
    #[error("suite omits tier {tier}")]
    MissingTier { tier: Tier },
    #[error("run recorded {recorded:?} warmups, tier {tier} requires {required}")]
    Warmups {
        recorded: Option<usize>,
        tier: Tier,
        required: usize,
    },
    #[error("raw samples repeat a case id")]
    RepeatedSampleCase,
    #[error("run selection is empty or repeats a case id")]
    Selection,
    #[error("run selection names unknown case {case}")]
    UnknownCase { case: CaseId },
    #[error("raw samples omit {case}")]
    MissingSamples { case: CaseId },
    #[error("cannot digest case {case}: {source}")]
    Digest {
        case: CaseId,
        source: serde_json::Error,
    },
    #[error("case digest mismatch for {case}")]
    DigestMismatch { case: CaseId },
    #[error("correctness report omits {case}")]
    MissingWitness { case: CaseId },
    #[error("paired case {case} has Stab-only rows")]
    StabOnlyRows { case: CaseId },
    #[error("self-only case {case} has paired rows")]
    PairedRows { case: CaseId },
    #[error("{case} expected sample shape overflows")]
    ShapeOverflow { case: CaseId },
    #[error("{case} has {actual} samples, its tier requires {required}")]
    SampleCount {
        case: CaseId,
        actual: usize,
        required: usize,
    },
    #[error("{case} paired samples have no Stim witness")]
    NoStimWitness { case: CaseId },
    #[error(
        "{case} paired sample {index} records work {stim_work}/{stab_work}, the case requires {required}"
    )]
    PairedWork {
        case: CaseId,
        index: usize,
        stim_work: u64,
        stab_work: u64,
        required: u64,
    },
    #[error(
        "{case} paired sample {index} records output bytes {stim_bytes}/{stab_bytes}, the witnesses require {required_stim}/{required_stab}"
    )]
    PairedOutput {
        case: CaseId,
        index: usize,
        stim_bytes: u64,
        stab_bytes: u64,
        required_stim: u64,
        required_stab: u64,
    },
    #[error(
        "{case} Stab sample {index} records work {work} and output bytes {output_bytes}, the case and witness require {required_work} and {required_output_bytes}"
    )]
    StabSample {
        case: CaseId,
        index: usize,
        work: u64,
        output_bytes: u64,
        required_work: u64,
        required_output_bytes: u64,
    },
    #[error("raw samples contain unselected cases: {}", join_comma(.0))]
    UnselectedSamples(Vec<CaseId>),
    #[error("{case} output {key}: {source}")]
    Census {
        case: CaseId,
        key: OutputKey,
        source: CensusError,
    },
    #[error(transparent)]
    Statistics(#[from] StatisticsError),
}

/// A recorded record census that cannot replay the preflight comparison.
#[derive(Debug, Error, PartialEq)]
pub(crate) enum CensusError {
    #[error("{0} witness records a census but its one_bits disagree")]
    OneBits(Implementation),
    #[error("only the {0} witness records a census")]
    OneSided(Implementation),
    #[error(transparent)]
    Mismatch(#[from] CensusMismatch),
}
