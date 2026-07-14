mod adapter;
mod artifact;
mod calibration;
mod correctness;
mod executable;
mod git;
mod group;
mod host;
mod invocation;
mod probe;
mod process;
mod protocol;
mod regression;
mod report;
mod run;
mod stab_build;
mod statistics;
mod toolchain;
mod worker;

pub(crate) use probe::ProbeArgs;
pub(crate) use regression::RegressionArgs;
pub(crate) use report::ReportArgs;
pub(crate) use run::RunArgs;
pub(crate) use worker::WorkerArgs;

pub(crate) fn run_worker(args: WorkerArgs) -> Result<(), String> {
    worker::run(args).map_err(|error| error.to_string())
}

pub(crate) fn run_probe(root: &crate::root::RepoRoot, args: ProbeArgs) -> Result<(), String> {
    probe::run(root, args).map_err(|error| error.to_string())
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
    args: ReportArgs,
) -> Result<std::path::PathBuf, String> {
    report::run(root, args).map_err(|error| error.to_string())
}

pub(crate) fn run_regression(
    root: &crate::root::RepoRoot,
    args: RegressionArgs,
) -> Result<regression::RegressionSummary, String> {
    regression::run(root, args).map_err(|error| error.to_string())
}

pub(crate) fn check_contracts(
    root: &crate::root::RepoRoot,
    inventory_digest: &str,
) -> Result<(), String> {
    host::check_policy(root).map_err(|error| error.to_string())?;
    group::check(root, inventory_digest).map_err(|error| error.to_string())?;
    regression::check_baseline(root, inventory_digest).map_err(|error| error.to_string())
}
