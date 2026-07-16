mod adapter;
mod artifact;
mod calibration;
mod completion;
mod contract;
mod correctness;
mod executable;
mod git;
mod group;
mod host;
mod invocation;
mod markdown;
mod probe;
mod process;
mod protocol;
mod regression;
mod report;
mod rollup;
mod run;
mod stab_build;
mod statistics;
mod toolchain;
mod worker;

pub(crate) use completion::{CompletionArgs, CompletionReportArgs};
pub(crate) use probe::ProbeArgs;
pub(crate) use regression::RegressionArgs;
pub(crate) use report::ReportArgs;
pub(crate) use rollup::{RollupArgs, RollupReportArgs};
pub(crate) use run::RunArgs;
pub(crate) use worker::WorkerArgs;

pub(crate) fn run_worker(args: WorkerArgs) -> Result<(), String> {
    worker::run(args).map_err(|error| error.to_string())
}

pub(crate) fn run_probe(root: &crate::root::RepoRoot, args: ProbeArgs) -> Result<(), String> {
    probe::run(root, args).map_err(|error| error.to_string())
}

pub(crate) fn verify_worker_reproducibility(
    root: &crate::root::RepoRoot,
) -> Result<(String, String), String> {
    let identity = invocation::verify_private_worker_reproducibility(root)
        .map_err(|error| error.to_string())?;
    Ok((identity.stim_binary_sha256, identity.stab_binary_sha256))
}

pub(crate) fn run_qualification(
    root: &crate::root::RepoRoot,
    inventory_digest: &str,
    correctness_digest: &str,
    args: RunArgs,
) -> Result<std::path::PathBuf, String> {
    run::run(root, inventory_digest, correctness_digest, args).map_err(|error| error.to_string())
}

pub(crate) fn run_report(
    root: &crate::root::RepoRoot,
    inventory_digest: &str,
    correctness_digest: &str,
    args: ReportArgs,
) -> Result<std::path::PathBuf, String> {
    report::run(root, inventory_digest, correctness_digest, args).map_err(|error| error.to_string())
}

pub(crate) fn run_completion(
    root: &crate::root::RepoRoot,
    inventory_digest: &str,
    correctness_digest: &str,
    args: CompletionArgs,
) -> Result<std::path::PathBuf, String> {
    completion::run(root, inventory_digest, correctness_digest, args)
        .map_err(|error| error.to_string())
}

pub(crate) fn run_completion_report(
    root: &crate::root::RepoRoot,
    inventory_digest: &str,
    correctness_digest: &str,
    args: CompletionReportArgs,
) -> Result<std::path::PathBuf, String> {
    completion::run_report(root, inventory_digest, correctness_digest, args)
        .map_err(|error| error.to_string())
}

pub(crate) fn run_regression(
    root: &crate::root::RepoRoot,
    inventory_digest: &str,
    correctness_digest: &str,
    args: RegressionArgs,
) -> Result<regression::RegressionSummary, String> {
    regression::run(root, inventory_digest, correctness_digest, args)
        .map_err(|error| error.to_string())
}

pub(crate) fn run_rollup(
    root: &crate::root::RepoRoot,
    inventory_digest: &str,
    correctness_digest: &str,
    args: RollupArgs,
) -> Result<std::path::PathBuf, String> {
    rollup::run(root, inventory_digest, correctness_digest, args).map_err(|error| error.to_string())
}

pub(crate) fn run_rollup_report(
    root: &crate::root::RepoRoot,
    inventory_digest: &str,
    correctness_digest: &str,
    args: RollupReportArgs,
) -> Result<std::path::PathBuf, String> {
    rollup::run_report(root, inventory_digest, correctness_digest, args)
        .map_err(|error| error.to_string())
}

pub(crate) fn check_contracts(
    root: &crate::root::RepoRoot,
    inventory_digest: &str,
    suite: &super::model::QualificationSuite,
) -> Result<(), String> {
    host::check_policy(root).map_err(|error| error.to_string())?;
    group::check(root, inventory_digest, suite).map_err(|error| error.to_string())?;
    regression::check_baseline(root, inventory_digest).map_err(|error| error.to_string())
}
