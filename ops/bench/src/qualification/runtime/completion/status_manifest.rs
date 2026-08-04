use std::path::Path;

use crate::root::RepoRoot;

use super::super::artifact::RepositoryBinding;
use super::super::protocol::TimingBoundary;
use super::super::self_regression::SelfRegressionOutcome;
use super::super::statistics::GateOutcome;
use super::{
    COMPLETION_SCHEMA_VERSION, CompletionCheckpointArgs, CompletionError, CompletionManifest,
    CompletionReportArgs, CompletionReportValidation, MemoryScalingStatus, RELEASE_SCOPE_ID,
    STIM_COMMIT, STIM_TAG, parse_canonical, run_report_with_repository, scope, validate_manifest,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompletionStatusRegression {
    Passed,
    Unseeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompletionStatusSummary {
    pub(crate) scope_id: String,
    pub(crate) artifact_path: String,
    pub(crate) stab_commit: String,
    pub(crate) architecture: String,
    pub(crate) performance_inventory_sha256: String,
    pub(crate) correctness_inventory_sha256: String,
    pub(crate) regression: CompletionStatusRegression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InspectedCompletionStatus {
    pub(crate) summary: CompletionStatusSummary,
    pub(crate) matches_current_contract: bool,
}

pub(in crate::qualification::runtime) fn checkpoint_manifest_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: CompletionCheckpointArgs,
) -> Result<Vec<u8>, CompletionError> {
    let input = run_report_with_repository(
        root,
        source_root,
        repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        CompletionReportArgs { input: args.input },
    )?;
    replayed_report_bytes(input)
}

fn replayed_report_bytes(
    validation: CompletionReportValidation,
) -> Result<Vec<u8>, CompletionError> {
    let CompletionReportValidation::Replayed(replayed) = validation else {
        return Err(CompletionError::Boundary);
    };
    Ok(replayed.report_json().to_vec())
}

pub(super) fn inspect(
    source_root: &RepoRoot,
    report_json: &[u8],
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    expected_parity_policy_sha256: &str,
    expected_regression_policy_sha256: &str,
    expected_regression_baselines_sha256: &str,
) -> Result<InspectedCompletionStatus, CompletionError> {
    let manifest: CompletionManifest = parse_canonical(report_json)?;
    if manifest.schema_version != COMPLETION_SCHEMA_VERSION
        || manifest.scope_id != RELEASE_SCOPE_ID
        || manifest.stim_tag != STIM_TAG
        || manifest.stim_commit != STIM_COMMIT
        || manifest.repository.commit_before != manifest.repository.commit_after
        || manifest.repository.local_modifications_before
        || manifest.repository.local_modifications_after
        || manifest.parity_outcome != GateOutcome::Passed
        || !manifest.environment_valid
        || manifest.memory_scaling_status != MemoryScalingStatus::Recorded
        || manifest.timing_boundary != TimingBoundary::RawWorkV2
        || !valid_sha256(&manifest.performance_inventory_sha256)
        || !valid_sha256(&manifest.correctness_inventory_sha256)
        || !valid_sha256(&manifest.parity_policy_sha256)
        || !valid_sha256(&manifest.regression_policy_sha256)
        || !valid_sha256(&manifest.regression_baselines_sha256)
        || !valid_git_commit(&manifest.repository.commit_after)
        || manifest.environment.architecture.is_empty()
        || super::super::validate_status_artifact_path(Path::new(&manifest.output)).is_err()
    {
        return Err(CompletionError::Boundary);
    }
    let regression = if manifest
        .regression_outcomes
        .iter()
        .any(|outcome| outcome.outcome == SelfRegressionOutcome::Unseeded)
    {
        CompletionStatusRegression::Unseeded
    } else {
        CompletionStatusRegression::Passed
    };
    let matches_current_contract = manifest.performance_inventory_sha256
        == expected_performance_inventory_sha256
        && manifest.correctness_inventory_sha256 == expected_correctness_inventory_sha256
        && manifest.parity_policy_sha256 == expected_parity_policy_sha256
        && manifest.regression_policy_sha256 == expected_regression_policy_sha256
        && manifest.regression_baselines_sha256 == expected_regression_baselines_sha256;
    if matches_current_contract {
        let scope = scope::load(
            source_root,
            expected_performance_inventory_sha256,
            RELEASE_SCOPE_ID,
        )?;
        validate_manifest(&manifest, &scope)?;
    }
    Ok(InspectedCompletionStatus {
        summary: CompletionStatusSummary {
            scope_id: manifest.scope_id,
            artifact_path: manifest.output,
            stab_commit: manifest.repository.commit_after,
            architecture: manifest.environment.architecture,
            performance_inventory_sha256: manifest.performance_inventory_sha256,
            correctness_inventory_sha256: manifest.correctness_inventory_sha256,
            regression,
        },
        matches_current_contract,
    })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_git_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qualification::runtime::artifact::{
        DirectQualificationArtifactPath, RetainedArtifactContext,
    };
    use crate::qualification::runtime::completion::ReplayedCompletion;

    #[test]
    fn checkpoint_uses_replayed_bytes_after_path_substitution() {
        let repository = tempfile::tempdir().expect("temporary repository");
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        let relative = Path::new("target/benchmarks/qualification/checkpoint-race");
        let directory = root.path.join(relative);
        std::fs::create_dir_all(&directory).expect("create completion fixture");
        std::fs::write(directory.join("report.json"), b"validated report\n")
            .expect("write completion report");
        std::fs::write(directory.join("preflight.json"), b"preflight\n")
            .expect("write completion preflight");
        std::fs::write(directory.join("report.md"), b"markdown\n")
            .expect("write completion markdown");
        let live_repository = RepositoryBinding::open(&root).expect("bind repository");
        let context =
            RetainedArtifactContext::open(&root, &live_repository).expect("open artifact context");
        let direct = DirectQualificationArtifactPath::try_new(relative).expect("direct artifact");
        let (binding, _) = context
            .read_and_bind(
                &root,
                &direct,
                &[
                    ("report.json", 1024),
                    ("preflight.json", 1024),
                    ("report.md", 1024),
                ],
            )
            .expect("bind completion artifacts");
        let replayed = CompletionReportValidation::Replayed(ReplayedCompletion {
            path: relative.to_path_buf(),
            report_json: b"validated report\n".to_vec(),
            _artifact_binding: binding,
        });

        let displaced = directory.with_extension("displaced");
        std::fs::rename(&directory, &displaced).expect("displace replayed completion");
        std::fs::create_dir(&directory).expect("create replacement completion");
        std::fs::write(directory.join("report.json"), b"unreplayed report\n")
            .expect("write replacement report");

        assert_eq!(
            replayed_report_bytes(replayed).expect("checkpoint bytes"),
            b"validated report\n"
        );
    }
}
