//! Typed domain errors for the end-to-end suite, grouped by the area that raises them.
//!
//! Every area error converts into [`E2eError`] and from there into
//! [`BenchError::E2e`], whose rendering keeps the
//! `end-to-end benchmark validation failed:` prefix. Supervisor failures stay
//! [`BenchError::Process`].

use std::fmt;

use thiserror::Error;

use crate::error::BenchError;

mod bundle;
mod data;
mod report;
mod run;
mod suite;

pub(crate) use bundle::*;
pub(crate) use data::*;
pub(crate) use report::*;
pub(crate) use run::*;
pub(crate) use suite::*;

/// Any end-to-end suite failure that is not a supervisor failure.
#[derive(Debug, Error)]
pub(crate) enum E2eError {
    #[error(transparent)]
    Suite(#[from] SuiteError),
    #[error(transparent)]
    Data(#[from] DataError),
    #[error(transparent)]
    Host(#[from] HostError),
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error(transparent)]
    Statistics(#[from] StatisticsError),
    #[error(transparent)]
    Report(#[from] ReportError),
    #[error(transparent)]
    Bundle(#[from] BundleError),
    #[error(transparent)]
    Run(#[from] RunError),
    #[error(transparent)]
    Case(#[from] CaseError),
    #[error(transparent)]
    Pipeline(#[from] PipelineError),
}

macro_rules! into_bench_error {
    ($($area:ty),* $(,)?) => {
        $(impl From<$area> for BenchError {
            fn from(error: $area) -> Self {
                Self::E2e(E2eError::from(error))
            }
        })*
    };
}

into_bench_error!(
    SuiteError,
    DataError,
    HostError,
    StatisticsError,
    ReportError,
    BundleError,
    RunError,
    CaseError,
    PipelineError,
);

/// A supervisor failure keeps its own top-level rendering, exactly as before the typed errors.
impl From<CommandError> for BenchError {
    fn from(error: CommandError) -> Self {
        match error {
            CommandError::Process(source) => Self::Process(source),
            other => Self::E2e(E2eError::Command(other)),
        }
    }
}

fn join_lines(items: &[impl fmt::Display]) -> String {
    join(items, "\n")
}

fn join_comma(items: &[impl fmt::Display]) -> String {
    join(items, ", ")
}

fn join(items: &[impl fmt::Display], separator: &str) -> String {
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(separator)
}
