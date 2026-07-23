mod adapter;
mod artifact;
mod calibration;
mod clifford_vectors;
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

pub(super) struct QualificationSession {
    root: crate::root::RepoRoot,
    source_root: crate::root::RepoRoot,
    repository: artifact::RepositoryBinding,
}

impl QualificationSession {
    pub(super) fn open(root: &crate::root::RepoRoot) -> Result<Self, String> {
        let repository =
            artifact::RepositoryBinding::open(root).map_err(|error| error.to_string())?;
        let source_root = repository
            .descriptor_root(root)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            root: root.clone(),
            source_root,
            repository,
        })
    }

    pub(super) fn source_root(&self) -> &crate::root::RepoRoot {
        &self.source_root
    }

    pub(super) fn require_current(&self) -> Result<(), String> {
        self.repository
            .require_current(&self.root)
            .map_err(|error| error.to_string())
    }
}

pub(crate) fn run_worker(args: WorkerArgs) -> Result<(), String> {
    worker::run(args).map_err(|error| error.to_string())
}

pub(super) fn validate_migration_target(
    root: &crate::root::RepoRoot,
    inventory_digest: &str,
    group_id: &str,
    measurement_id: &str,
    scale_id: Option<&str>,
) -> Result<(), String> {
    let resolved =
        group::load_group(root, inventory_digest, group_id).map_err(|error| error.to_string())?;
    let measurement_exists = resolved
        .contract
        .measurement_ids
        .iter()
        .any(|measurement| measurement.to_string() == measurement_id);
    let scale_exists = scale_id.is_none_or(|expected| {
        resolved
            .contract
            .scales
            .iter()
            .any(|scale| scale.id.to_string() == expected)
    });
    if measurement_exists && scale_exists && !resolved.contract.scales.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "threshold migration target {group_id}/{measurement_id} does not exist at the requested scale"
        ))
    }
}

pub(crate) fn run_probe(session: &QualificationSession, args: ProbeArgs) -> Result<(), String> {
    probe::run(&session.source_root, args).map_err(|error| error.to_string())
}

pub(crate) fn regenerate_clifford_vectors(
    root: &crate::root::RepoRoot,
    check: bool,
) -> Result<(), String> {
    clifford_vectors::regenerate(root, check).map_err(|error| error.to_string())
}

pub(crate) fn verify_worker_reproducibility(
    session: &QualificationSession,
) -> Result<(String, String), String> {
    let identity = invocation::verify_private_worker_reproducibility(&session.source_root)
        .map_err(|error| error.to_string())?;
    Ok((identity.stim_binary_sha256, identity.stab_binary_sha256))
}

pub(crate) fn run_qualification(
    session: &QualificationSession,
    inventory_digest: &str,
    correctness_digest: &str,
    args: RunArgs,
) -> Result<std::path::PathBuf, String> {
    run::run_with_repository(
        &session.root,
        &session.source_root,
        &session.repository,
        inventory_digest,
        correctness_digest,
        args,
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn run_report(
    session: &QualificationSession,
    inventory_digest: &str,
    correctness_digest: &str,
    args: ReportArgs,
) -> Result<std::path::PathBuf, String> {
    report::run_args_with_repository(
        &session.root,
        &session.source_root,
        &session.repository,
        inventory_digest,
        correctness_digest,
        args,
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn run_completion(
    session: &QualificationSession,
    inventory_digest: &str,
    correctness_digest: &str,
    args: CompletionArgs,
) -> Result<std::path::PathBuf, String> {
    completion::run_with_repository(
        &session.root,
        &session.repository,
        inventory_digest,
        correctness_digest,
        args,
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn run_completion_report(
    session: &QualificationSession,
    inventory_digest: &str,
    correctness_digest: &str,
    args: CompletionReportArgs,
) -> Result<std::path::PathBuf, String> {
    completion::run_report_with_repository(
        &session.root,
        &session.repository,
        inventory_digest,
        correctness_digest,
        args,
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn run_regression(
    session: &QualificationSession,
    inventory_digest: &str,
    correctness_digest: &str,
    args: RegressionArgs,
) -> Result<regression::RegressionSummary, String> {
    regression::run_args_with_repository(
        &session.root,
        &session.source_root,
        &session.repository,
        inventory_digest,
        correctness_digest,
        args,
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn run_rollup(
    session: &QualificationSession,
    inventory_digest: &str,
    correctness_digest: &str,
    args: RollupArgs,
) -> Result<std::path::PathBuf, String> {
    rollup::run_with_repository(
        &session.root,
        &session.source_root,
        &session.repository,
        inventory_digest,
        correctness_digest,
        args,
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn run_rollup_report(
    session: &QualificationSession,
    inventory_digest: &str,
    correctness_digest: &str,
    args: RollupReportArgs,
) -> Result<std::path::PathBuf, String> {
    rollup::run_report_with_repository(
        &session.root,
        &session.source_root,
        &session.repository,
        inventory_digest,
        correctness_digest,
        args,
    )
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

#[cfg(test)]
mod tests {
    use super::QualificationSession;
    use crate::root::RepoRoot;

    #[cfg(unix)]
    #[test]
    fn qualification_session_keeps_source_reads_on_the_retained_repository() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let repository = parent.path().join("repository");
        std::fs::create_dir(&repository).expect("create repository");
        std::fs::write(repository.join("marker"), b"retained").expect("write retained marker");
        let root = RepoRoot::resolve(&repository).expect("resolve repository");
        let session = QualificationSession::open(&root).expect("open qualification session");
        let detached = parent.path().join("detached");
        std::fs::rename(&repository, &detached).expect("detach repository");
        std::fs::create_dir(&repository).expect("create replacement repository");
        std::fs::write(repository.join("marker"), b"replacement")
            .expect("write replacement marker");
        let marker_path = session.source_root().path.join("marker");

        let marker = crate::source_file::read_repo_regular_file_bounded(
            session.source_root(),
            &marker_path,
            16,
        )
        .expect("read through retained repository descriptor");

        assert_eq!(marker, b"retained");
        assert!(session.require_current().is_err());
    }
}
