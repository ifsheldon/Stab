use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::artifact::{
    DirectQualificationArtifactPath, QualificationOutput, RepositoryBinding,
    RetainedArtifactContext, RetainedArtifactDirectory,
};
use super::invocation::WorkerIdentityEvidence;
use super::protocol::TimingBoundary;
use super::rollup::{RollupReplayEvidence, RollupSourceEvidence};
use super::run::{QualificationTier, RepositoryEvidence, sha256_hex};
use super::self_regression::{SelfRegressionOutcome, SelfRegressionSummary};
use super::statistics::GateOutcome;
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::qualification::model::{SizeClass, TimingBatchPolicy};
use crate::root::RepoRoot;
use clap::Args;
use serde::{Deserialize, Serialize};

mod bindings;
mod error;
mod group_correctness;
mod legacy;
mod memory;
mod replay;
mod scope;
mod status_manifest;
#[cfg(test)]
mod tests;

pub(super) use error::CompletionError;
use group_correctness::{CompletionCorrectness, collect as completion_correctness};
#[cfg(test)]
use memory::validate_nofile_limit as validate_completion_nofile_limit;
use memory::{
    ACCEPTED_MAXIMUM_MEMORY_GROUPS, CompletionAcceptedMaximumMemoryReceipt,
    RELEASE_SOFT_NOFILE_LIMIT, admit_paths as admit_memory_receipt_paths,
    collect as collect_memory_receipts, require_nofile_limit as require_completion_nofile_limit,
};
use scope::{CompletionScope, MAX_ROLLUPS, RELEASE_SCOPE_ID, expected_rollup_keys};
#[cfg(test)]
use scope::{DEM_PARSE_GROUP, DEM_PRINT_GROUP, DEM_SCOPE_ID};
pub(super) use status_manifest::checkpoint_manifest_with_repository;
pub(crate) use status_manifest::{CompletionStatusRegression, InspectedCompletionStatus};

const COMPLETION_SCHEMA_VERSION: u32 = 4;
const PREFLIGHT_SCHEMA_VERSION: u32 = 4;
const LEGACY_COMPLETION_SCHEMA_VERSIONS: [u32; 3] = [1, 2, 3];
const DEFAULT_OUTPUT: &str = "target/benchmarks/qualification/completion-latest";
const MAX_COMPLETION_REPORT_BYTES: usize = 16 << 20;
const MAX_COMPLETION_PREFLIGHT_BYTES: usize = 4 << 20;
const MAX_COMPLETION_MARKDOWN_BYTES: usize = 4 << 20;

#[derive(Clone, Debug, Args)]
pub(crate) struct CompletionArgs {
    /// Source-owned architecture/revision completion scope.
    #[arg(long, default_value = RELEASE_SCOPE_ID)]
    scope: String,

    /// Full or soak scale-family rollup; repeat once per required group and tier.
    #[arg(long, required = true)]
    rollup: Vec<PathBuf>,

    /// Accepted-maximum DEM memory receipt; provide parse and print receipts.
    #[arg(long = "memory-receipt", required = true)]
    memory_receipt: Vec<PathBuf>,

    /// New immutable completion-manifest directory.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    out: PathBuf,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct CompletionReportArgs {
    /// Published completion manifest to reconstruct offline.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    input: PathBuf,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct CompletionCheckpointArgs {
    /// Replayed schema-version-4 completion whose exact manifest will be checked in.
    #[arg(long)]
    input: PathBuf,
}

#[derive(Debug)]
pub(crate) enum CompletionReportValidation {
    Replayed(ReplayedCompletion),
    HistoricalReadable { path: PathBuf, schema_version: u32 },
}

#[derive(Debug)]
pub(crate) struct ReplayedCompletion {
    path: PathBuf,
    report_json: Vec<u8>,
    _artifact_binding: Arc<RetainedArtifactDirectory>,
    source_evidence: Box<ReconstructedCompletion>,
}

impl ReplayedCompletion {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn report_json(&self) -> &[u8] {
        &self.report_json
    }

    pub(super) fn require_current(
        &self,
        root: &RepoRoot,
        repository: &RepositoryBinding,
    ) -> Result<(), CompletionError> {
        let source_evidence = &self.source_evidence;
        source_evidence.require_sources_current(root)?;
        self._artifact_binding.require_current(root)?;
        require_completion_repository_state(root, repository, &source_evidence.manifest.repository)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionEnvironment {
    host_policy_sha256: String,
    host_profile_id: String,
    operating_system: String,
    architecture: String,
    cpu_identity: String,
    rust_toolchain: String,
    target_triple: String,
    toolchain_sha256: String,
    soft_nofile_limit: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionArtifact {
    path: String,
    report_sha256: String,
    preflight_sha256: String,
    markdown_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionRollup {
    group_id: String,
    group_contract_sha256: String,
    tier: QualificationTier,
    workload_id: String,
    timing_batch_policy: TimingBatchPolicy,
    comparator_sources: Vec<(String, String)>,
    artifact: CompletionArtifact,
    source_report_count: usize,
    parity_checked_measurements: usize,
    overall_outcome: GateOutcome,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionSourceReport {
    group_id: String,
    tier: QualificationTier,
    scale_id: String,
    artifact: CompletionArtifact,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionMemory {
    group_id: String,
    tier: QualificationTier,
    scale_id: String,
    family_id: String,
    size_class: SizeClass,
    stim_setup_rss_bytes: u64,
    stim_peak_rss_bytes: u64,
    stim_parent_observed_peak_rss_bytes: Option<u64>,
    stab_setup_rss_bytes: u64,
    stab_peak_rss_bytes: u64,
    stab_parent_observed_peak_rss_bytes: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionRegression {
    group_id: String,
    outcome: SelfRegressionOutcome,
    checked_measurements: usize,
    unseeded_measurements: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum MemoryScalingStatus {
    Recorded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionManifest {
    schema_version: u32,
    output: String,
    generated_unix_epoch_seconds: u64,
    scope_id: String,
    performance_inventory_sha256: String,
    correctness_inventory_sha256: String,
    parity_policy_sha256: String,
    regression_policy_sha256: String,
    regression_baselines_sha256: String,
    stim_tag: String,
    stim_commit: String,
    repository: RepositoryEvidence,
    environment: CompletionEnvironment,
    workers: WorkerIdentityEvidence,
    timing_boundary: TimingBoundary,
    correctness_preflights: Vec<CompletionCorrectness>,
    rollups: Vec<CompletionRollup>,
    source_reports: Vec<CompletionSourceReport>,
    memory: Vec<CompletionMemory>,
    accepted_maximum_memory_receipts: Vec<CompletionAcceptedMaximumMemoryReceipt>,
    parity_outcome: GateOutcome,
    regression_outcomes: Vec<CompletionRegression>,
    environment_valid: bool,
    memory_scaling_status: MemoryScalingStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionPreflight {
    schema_version: u32,
    report_sha256: String,
    output: String,
    scope_id: String,
    performance_inventory_sha256: String,
    correctness_inventory_sha256: String,
    stab_commit: String,
    parity_policy_sha256: String,
    regression_policy_sha256: String,
    regression_baselines_sha256: String,
    correctness_preflights: Vec<CompletionCorrectness>,
    rollups: Vec<CompletionArtifact>,
    source_report_count: usize,
    memory_record_count: usize,
    accepted_maximum_memory_receipts: Vec<CompletionAcceptedMaximumMemoryReceipt>,
    parity_outcome: GateOutcome,
    regression_outcomes: Vec<CompletionRegression>,
}

#[derive(Debug)]
struct ReconstructedCompletion {
    manifest: CompletionManifest,
    rollup_evidence: Vec<RollupReplayEvidence>,
    correctness_bindings: Vec<Arc<super::correctness::CorrectnessArtifactBinding>>,
    artifact_bindings: Vec<bindings::RetainedRollupArtifacts>,
    memory_receipt_evidence: Vec<super::probe::DemMemoryReceiptEvidence>,
    memory_receipt_bindings: Vec<Arc<RetainedArtifactDirectory>>,
}

impl ReconstructedCompletion {
    fn bind_source_artifacts(
        &mut self,
        root: &RepoRoot,
        context: &Arc<RetainedArtifactContext>,
    ) -> Result<(), CompletionError> {
        if !self.artifact_bindings.is_empty() {
            return Err(CompletionError::SourceMutation);
        }
        self.artifact_bindings = self
            .rollup_evidence
            .iter()
            .map(|rollup| bindings::RetainedRollupArtifacts::bind(root, context, rollup))
            .collect::<Result<Vec<_>, _>>()?;
        self.memory_receipt_bindings = self
            .memory_receipt_evidence
            .iter()
            .map(|receipt| {
                let path = DirectQualificationArtifactPath::try_new(&receipt.path)?;
                Ok(context.bind_digests(
                    root,
                    &path,
                    &[(
                        "report.json",
                        receipt.report_sha256.as_str(),
                        super::probe::MAX_MEMORY_RECEIPT_BYTES,
                    )],
                )?)
            })
            .collect::<Result<Vec<_>, CompletionError>>()?;
        Ok(())
    }

    fn require_sources_current(&self, root: &RepoRoot) -> Result<(), CompletionError> {
        for binding in &self.artifact_bindings {
            binding.require_current(root)?;
        }
        for binding in &self.correctness_bindings {
            binding.require_current()?;
        }
        for binding in &self.memory_receipt_bindings {
            binding.require_current(root)?;
        }
        Ok(())
    }
}

pub(super) fn run_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: CompletionArgs,
) -> Result<PathBuf, CompletionError> {
    let output = DirectQualificationArtifactPath::try_new(&args.out)?;
    QualificationOutput::require_absent_with_repository(root, repository, &output)?;
    let rollup_paths = admit_paths(&output, &args.rollup)?;
    let memory_receipt_paths =
        admit_memory_receipt_paths(&output, &rollup_paths, &args.memory_receipt)?;
    let reconstructed = reconstruct(
        root,
        source_root,
        repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        &args.scope,
        &output,
        &rollup_paths,
        &memory_receipt_paths,
        current_unix_epoch_seconds()?,
    )?;
    publish(
        root,
        repository,
        expected_performance_inventory_sha256,
        &output,
        &reconstructed,
    )?;
    Ok(output.into_path_buf())
}

pub(in crate::qualification::runtime) use replay::run_report_with_repository;

pub(super) fn inspect_status_manifest(
    source_root: &RepoRoot,
    report_json: &[u8],
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    expected_parity_policy_sha256: &str,
    expected_regression_policy_sha256: &str,
    expected_regression_baselines_sha256: &str,
) -> Result<InspectedCompletionStatus, CompletionError> {
    status_manifest::inspect(
        source_root,
        report_json,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        expected_parity_policy_sha256,
        expected_regression_policy_sha256,
        expected_regression_baselines_sha256,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "completion reconstruction binds every source identity explicitly"
)]
fn reconstruct(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    scope_id: &str,
    output: &DirectQualificationArtifactPath,
    rollup_paths: &[DirectQualificationArtifactPath],
    memory_receipt_paths: &[DirectQualificationArtifactPath],
    generated_unix_epoch_seconds: u64,
) -> Result<ReconstructedCompletion, CompletionError> {
    let scope = scope::load(source_root, expected_performance_inventory_sha256, scope_id)?;
    let soft_nofile_limit = require_completion_nofile_limit(&scope)?;
    require_scope(&scope, rollup_paths.len())?;
    let repository_before = completion_repository_state(root, repository)?;
    require_clean_repository(&repository_before)?;
    let mut rollups = Vec::with_capacity(rollup_paths.len());
    let mut correctness_bindings = bindings::RetainedBindings::default();
    for path in rollup_paths {
        let mut rollup = super::rollup::replay_with_repository(
            root,
            source_root,
            repository,
            expected_performance_inventory_sha256,
            expected_correctness_inventory_sha256,
            path.clone(),
        )?;
        correctness_bindings.admit(&mut rollup)?;
        rollups.push(rollup);
    }
    order_and_validate_scope(&scope, &mut rollups)?;
    let shared = shared_identity(&rollups)?;
    let memory_receipt_evidence = collect_memory_receipts(
        root,
        source_root,
        repository,
        memory_receipt_paths,
        shared,
        &repository_before,
    )?;
    let correctness_preflights =
        completion_correctness(&rollups, &scope, expected_correctness_inventory_sha256)?;
    let parity_policy_sha256 =
        super::parity::policy_sha256(source_root, expected_performance_inventory_sha256)?;
    let regression_sources = super::self_regression::source_identities(
        source_root,
        expected_performance_inventory_sha256,
    )?;
    let parity_counts = validate_source_parity(
        root,
        source_root,
        repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        &rollups,
        &scope,
    )?;
    let regression_outcomes = evaluate_regression(
        source_root,
        expected_performance_inventory_sha256,
        &rollups,
        &scope,
    )?;
    let repository_after = completion_repository_state(root, repository)?;
    require_same_clean_repository(&repository_before, &repository_after)?;

    let completion_rollups = rollups
        .iter()
        .map(|rollup| {
            Ok(CompletionRollup {
                group_id: rollup.group_id.clone(),
                group_contract_sha256: rollup.group_contract_sha256.clone(),
                tier: rollup.tier,
                workload_id: rollup.workload_id.clone(),
                timing_batch_policy: rollup.timing_batch_policy,
                comparator_sources: rollup.comparator_sources.clone(),
                artifact: rollup_artifact(rollup)?,
                source_report_count: rollup.sources.len(),
                parity_checked_measurements: parity_counts
                    .get(&rollup_key(&rollup.group_id, rollup.tier))
                    .copied()
                    .ok_or_else(|| {
                        CompletionError::MissingRollup(rollup_key(&rollup.group_id, rollup.tier))
                    })?,
                overall_outcome: rollup.overall_outcome,
            })
        })
        .collect::<Result<Vec<_>, CompletionError>>()?;
    let source_reports = completion_source_reports(&rollups, &scope)?;
    let memory = completion_memory(&rollups);
    let manifest = CompletionManifest {
        schema_version: COMPLETION_SCHEMA_VERSION,
        output: path_text(output.as_path())?,
        generated_unix_epoch_seconds,
        scope_id: scope_id.to_string(),
        performance_inventory_sha256: expected_performance_inventory_sha256.to_string(),
        correctness_inventory_sha256: expected_correctness_inventory_sha256.to_string(),
        parity_policy_sha256,
        regression_policy_sha256: regression_sources.policy_sha256,
        regression_baselines_sha256: regression_sources.baselines_sha256,
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        repository: RepositoryEvidence {
            commit_before: repository_before.commit,
            commit_after: repository_after.commit,
            local_modifications_before: false,
            local_modifications_after: false,
        },
        environment: CompletionEnvironment {
            host_policy_sha256: shared.host_policy_sha256.clone(),
            host_profile_id: shared.host_profile_id.clone(),
            operating_system: shared.operating_system.clone(),
            architecture: shared.architecture.clone(),
            cpu_identity: shared.cpu_identity.clone(),
            rust_toolchain: shared.rust_toolchain.clone(),
            target_triple: shared.target_triple.clone(),
            toolchain_sha256: shared.toolchain_sha256.clone(),
            soft_nofile_limit,
        },
        workers: shared.workers.clone(),
        timing_boundary: shared.timing_boundary,
        correctness_preflights,
        rollups: completion_rollups,
        source_reports,
        memory,
        accepted_maximum_memory_receipts: memory_receipt_evidence
            .iter()
            .map(|receipt| {
                Ok(CompletionAcceptedMaximumMemoryReceipt {
                    group_id: receipt.runtime_group_id.clone(),
                    path: path_text(&receipt.path)?,
                    report_sha256: receipt.report_sha256.clone(),
                })
            })
            .collect::<Result<Vec<_>, CompletionError>>()?,
        parity_outcome: GateOutcome::Passed,
        regression_outcomes,
        environment_valid: true,
        memory_scaling_status: MemoryScalingStatus::Recorded,
    };
    validate_manifest(&manifest, &scope)?;
    Ok(ReconstructedCompletion {
        manifest,
        rollup_evidence: rollups,
        correctness_bindings: correctness_bindings.into_values(),
        artifact_bindings: Vec::new(),
        memory_receipt_evidence,
        memory_receipt_bindings: Vec::new(),
    })
}

fn admit_paths(
    output: &DirectQualificationArtifactPath,
    paths: &[PathBuf],
) -> Result<Vec<DirectQualificationArtifactPath>, CompletionError> {
    if paths.len() > MAX_ROLLUPS {
        return Err(CompletionError::RollupCount(paths.len()));
    }
    let mut unique = BTreeSet::new();
    let mut admitted = Vec::with_capacity(paths.len());
    for path in paths {
        let path = DirectQualificationArtifactPath::try_new(path)?;
        if path == *output {
            return Err(CompletionError::OutputCollision(path.into_path_buf()));
        }
        if !unique.insert(path.clone()) {
            return Err(CompletionError::DuplicatePath(path.into_path_buf()));
        }
        admitted.push(path);
    }
    Ok(admitted)
}

fn require_scope(scope: &CompletionScope, rollup_count: usize) -> Result<(), CompletionError> {
    if rollup_count != expected_rollup_keys(scope).len() {
        return Err(CompletionError::RollupCount(rollup_count));
    }
    Ok(())
}

fn order_and_validate_scope(
    scope: &CompletionScope,
    rollups: &mut Vec<RollupReplayEvidence>,
) -> Result<(), CompletionError> {
    let expected_keys = expected_rollup_keys(scope);
    let mut by_key = BTreeMap::new();
    for rollup in rollups.drain(..) {
        let key = rollup_key(&rollup.group_id, rollup.tier);
        if !expected_keys.contains(&key) {
            return Err(CompletionError::UnknownRollup(key));
        }
        if by_key.insert(key.clone(), rollup).is_some() {
            return Err(CompletionError::DuplicateRollup(key));
        }
    }
    for key in expected_keys {
        rollups.push(
            by_key
                .remove(&key)
                .ok_or_else(|| CompletionError::MissingRollup(key.clone()))?,
        );
    }
    if !by_key.is_empty() {
        return Err(CompletionError::UnknownRollup(
            by_key.into_keys().next().unwrap_or_default(),
        ));
    }
    Ok(())
}

fn rollup_key(group_id: &str, tier: QualificationTier) -> String {
    format!("{group_id}:{}", tier_name(tier))
}

const fn tier_name(tier: QualificationTier) -> &'static str {
    match tier {
        QualificationTier::Pr => "pr",
        QualificationTier::Full => "full",
        QualificationTier::Soak => "soak",
    }
}

fn shared_identity(
    rollups: &[RollupReplayEvidence],
) -> Result<&RollupReplayEvidence, CompletionError> {
    let first = rollups.first().ok_or(CompletionError::RollupCount(0))?;
    for rollup in rollups {
        if rollup.performance_inventory_sha256 != first.performance_inventory_sha256
            || rollup.stab_commit != first.stab_commit
            || rollup.stim_commit != first.stim_commit
            || rollup.host_policy_sha256 != first.host_policy_sha256
            || rollup.host_profile_id != first.host_profile_id
            || rollup.operating_system != first.operating_system
            || rollup.architecture != first.architecture
            || rollup.cpu_identity != first.cpu_identity
            || rollup.rust_toolchain != first.rust_toolchain
            || rollup.target_triple != first.target_triple
            || rollup.toolchain_sha256 != first.toolchain_sha256
            || rollup.workers != first.workers
            || rollup.timing_boundary != first.timing_boundary
            || rollup.overall_outcome != GateOutcome::Passed
        {
            return Err(CompletionError::MixedIdentity);
        }
    }
    Ok(first)
}

#[allow(
    clippy::too_many_arguments,
    reason = "source-report parity binds both inventories and the retained repository"
)]
fn validate_source_parity(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    rollups: &[RollupReplayEvidence],
    scope: &CompletionScope,
) -> Result<BTreeMap<String, usize>, CompletionError> {
    let mut paths = BTreeSet::new();
    let mut counts = BTreeMap::new();
    for rollup in rollups {
        let expected_measurements = rollup
            .scales
            .first()
            .map(|scale| scale.measurements.len())
            .ok_or(CompletionError::SourceReportCount {
                actual: 0,
                expected: scope.expected_source_reports,
            })?;
        let mut checked = 0;
        for source in &rollup.sources {
            if !paths.insert(source.path.clone()) {
                return Err(CompletionError::DuplicatePath(source.path.clone()));
            }
            let path = DirectQualificationArtifactPath::try_new(&source.path)?;
            let summary = super::parity::run_with_repository(
                root,
                source_root,
                repository,
                expected_performance_inventory_sha256,
                expected_correctness_inventory_sha256,
                &path,
            )?;
            if summary.group_id != rollup.group_id
                || summary.report_only
                || summary.checked_measurements != expected_measurements
            {
                return Err(CompletionError::FailedParity(source.path.clone()));
            }
            checked += summary.checked_measurements;
        }
        counts.insert(rollup_key(&rollup.group_id, rollup.tier), checked);
    }
    if paths.len() != scope.expected_source_reports {
        return Err(CompletionError::SourceReportCount {
            actual: paths.len(),
            expected: scope.expected_source_reports,
        });
    }
    Ok(counts)
}

fn evaluate_regression(
    source_root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
    rollups: &[RollupReplayEvidence],
    scope: &CompletionScope,
) -> Result<Vec<CompletionRegression>, CompletionError> {
    scope
        .group_ids
        .iter()
        .map(|group_id| {
            let full = find_rollup(rollups, group_id, QualificationTier::Full)?;
            let soak = find_rollup(rollups, group_id, QualificationTier::Soak)?;
            let summary = super::self_regression::evaluate_evidence(
                source_root,
                expected_performance_inventory_sha256,
                full,
                soak,
            )?;
            Ok(completion_regression(summary))
        })
        .collect()
}

fn completion_regression(summary: SelfRegressionSummary) -> CompletionRegression {
    CompletionRegression {
        group_id: summary.group_id,
        outcome: summary.outcome,
        checked_measurements: summary.checked_measurements,
        unseeded_measurements: summary.unseeded_measurements,
    }
}

fn find_rollup<'a>(
    rollups: &'a [RollupReplayEvidence],
    group_id: &str,
    tier: QualificationTier,
) -> Result<&'a RollupReplayEvidence, CompletionError> {
    rollups
        .iter()
        .find(|rollup| rollup.group_id == group_id && rollup.tier == tier)
        .ok_or_else(|| CompletionError::MissingRollup(rollup_key(group_id, tier)))
}

fn completion_source_reports(
    rollups: &[RollupReplayEvidence],
    scope: &CompletionScope,
) -> Result<Vec<CompletionSourceReport>, CompletionError> {
    let reports = rollups
        .iter()
        .flat_map(|rollup| {
            rollup.sources.iter().map(|source| {
                Ok(CompletionSourceReport {
                    group_id: rollup.group_id.clone(),
                    tier: rollup.tier,
                    scale_id: source.scale_id.clone(),
                    artifact: source_artifact(source)?,
                })
            })
        })
        .collect::<Result<Vec<_>, CompletionError>>()?;
    if reports.len() != scope.expected_source_reports {
        return Err(CompletionError::SourceReportCount {
            actual: reports.len(),
            expected: scope.expected_source_reports,
        });
    }
    Ok(reports)
}

fn completion_memory(rollups: &[RollupReplayEvidence]) -> Vec<CompletionMemory> {
    rollups
        .iter()
        .flat_map(|rollup| {
            rollup.scales.iter().map(|scale| CompletionMemory {
                group_id: rollup.group_id.clone(),
                tier: rollup.tier,
                scale_id: scale.scale_id.clone(),
                family_id: scale.family_id.clone(),
                size_class: scale.size_class,
                stim_setup_rss_bytes: scale.memory.stim_setup_rss_bytes,
                stim_peak_rss_bytes: scale.memory.stim_peak_rss_bytes,
                stim_parent_observed_peak_rss_bytes: scale
                    .memory
                    .stim_parent_observed_peak_rss_bytes,
                stab_setup_rss_bytes: scale.memory.stab_setup_rss_bytes,
                stab_peak_rss_bytes: scale.memory.stab_peak_rss_bytes,
                stab_parent_observed_peak_rss_bytes: scale
                    .memory
                    .stab_parent_observed_peak_rss_bytes,
            })
        })
        .collect()
}

fn rollup_artifact(rollup: &RollupReplayEvidence) -> Result<CompletionArtifact, CompletionError> {
    Ok(CompletionArtifact {
        path: path_text(&rollup.output)?,
        report_sha256: rollup.report_sha256.clone(),
        preflight_sha256: rollup.preflight_sha256.clone(),
        markdown_sha256: rollup.markdown_sha256.clone(),
    })
}

fn source_artifact(source: &RollupSourceEvidence) -> Result<CompletionArtifact, CompletionError> {
    Ok(CompletionArtifact {
        path: path_text(&source.path)?,
        report_sha256: source.report_sha256.clone(),
        preflight_sha256: source.preflight_sha256.clone(),
        markdown_sha256: source.markdown_sha256.clone(),
    })
}

fn publish(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    output_path: &DirectQualificationArtifactPath,
    reconstructed: &ReconstructedCompletion,
) -> Result<(), CompletionError> {
    let report_json = canonical_json(&reconstructed.manifest)?;
    let preflight = completion_preflight(&reconstructed.manifest, &report_json);
    let preflight_json = canonical_json(&preflight)?;
    let markdown = render_markdown(&reconstructed.manifest, &sha256_hex(&report_json));
    let mut output = QualificationOutput::begin_new_with_repository(root, repository, output_path)?;
    output.write("report.json", &report_json)?;
    output.write("preflight.json", &preflight_json)?;
    output.write("report.md", markdown.as_bytes())?;
    bind_evidence(
        &mut output,
        &reconstructed.rollup_evidence,
        &reconstructed.memory_receipt_evidence,
    )?;

    let expected_commit = reconstructed.manifest.repository.commit_after.clone();
    let expected_parity = reconstructed.manifest.parity_policy_sha256.clone();
    let expected_regression_policy = reconstructed.manifest.regression_policy_sha256.clone();
    let expected_regression_baselines = reconstructed.manifest.regression_baselines_sha256.clone();
    let correctness_bindings = &reconstructed.correctness_bindings;
    output.commit_new_with_source_validation(|bound_repository| {
        bound_repository.require_current(root)?;
        let retained_root = bound_repository.descriptor_root(root)?;
        let state = super::git::repository_state(&retained_root).map_err(|_| {
            super::artifact::ArtifactError::ExternalSourceChanged("completion repository")
        })?;
        if state.commit != expected_commit || state.local_modifications {
            return Err(super::artifact::ArtifactError::ExternalSourceChanged(
                "completion repository",
            ));
        }
        let parity =
            super::parity::policy_sha256(&retained_root, expected_performance_inventory_sha256)
                .map_err(|_| {
                    super::artifact::ArtifactError::ExternalSourceChanged("parity policy")
                })?;
        let regression = super::self_regression::source_identities(
            &retained_root,
            expected_performance_inventory_sha256,
        )
        .map_err(|_| {
            super::artifact::ArtifactError::ExternalSourceChanged("self-regression policy")
        })?;
        if parity != expected_parity
            || regression.policy_sha256 != expected_regression_policy
            || regression.baselines_sha256 != expected_regression_baselines
        {
            return Err(super::artifact::ArtifactError::ExternalSourceChanged(
                "completion policy identities",
            ));
        }
        for binding in correctness_bindings {
            binding
                .require_current()
                .map_err(super::correctness::publication_error)?;
        }
        Ok(())
    })?;
    Ok(())
}

fn bind_evidence(
    output: &mut QualificationOutput,
    rollups: &[RollupReplayEvidence],
    memory_receipts: &[super::probe::DemMemoryReceiptEvidence],
) -> Result<(), CompletionError> {
    for rollup in rollups {
        let path = DirectQualificationArtifactPath::try_new(&rollup.output)?;
        bind_artifact_set(
            output,
            &path,
            &rollup.report_sha256,
            &rollup.preflight_sha256,
            &rollup.markdown_sha256,
            (
                super::rollup::MAX_ROLLUP_REPORT_BYTES,
                super::rollup::MAX_ROLLUP_PREFLIGHT_BYTES,
                super::rollup::MAX_ROLLUP_MARKDOWN_BYTES,
            ),
        )?;
        for source in &rollup.sources {
            let path = DirectQualificationArtifactPath::try_new(&source.path)?;
            bind_artifact_set(
                output,
                &path,
                &source.report_sha256,
                &source.preflight_sha256,
                &source.markdown_sha256,
                (
                    super::report::MAX_PUBLISHED_REPORT_BYTES,
                    super::report::MAX_PUBLISHED_PREFLIGHT_BYTES,
                    super::report::MAX_PUBLISHED_MARKDOWN_BYTES,
                ),
            )?;
        }
    }
    for receipt in memory_receipts {
        let path = DirectQualificationArtifactPath::try_new(&receipt.path)?;
        output.require_sibling_artifact_digest(
            &path,
            "report.json",
            &receipt.report_sha256,
            super::probe::MAX_MEMORY_RECEIPT_BYTES,
        )?;
    }
    Ok(())
}

fn bind_artifact_set(
    output: &mut QualificationOutput,
    path: &DirectQualificationArtifactPath,
    report_sha256: &str,
    preflight_sha256: &str,
    markdown_sha256: &str,
    limits: (usize, usize, usize),
) -> Result<(), CompletionError> {
    output.require_sibling_artifact_digest(path, "report.json", report_sha256, limits.0)?;
    output.require_sibling_artifact_digest(path, "preflight.json", preflight_sha256, limits.1)?;
    output.require_sibling_artifact_digest(path, "report.md", markdown_sha256, limits.2)?;
    Ok(())
}

fn validate_manifest(
    manifest: &CompletionManifest,
    scope: &CompletionScope,
) -> Result<(), CompletionError> {
    if manifest.schema_version != COMPLETION_SCHEMA_VERSION
        || manifest.scope_id != scope.id
        || manifest.stim_tag != STIM_TAG
        || manifest.stim_commit != STIM_COMMIT
        || manifest.repository.commit_before != manifest.repository.commit_after
        || manifest.repository.local_modifications_before
        || manifest.repository.local_modifications_after
        || !group_correctness::valid_manifest(
            &manifest.correctness_preflights,
            scope,
            &manifest.correctness_inventory_sha256,
        )
        || manifest.rollups.len() != expected_rollup_keys(scope).len()
        || manifest.source_reports.len() != scope.expected_source_reports
        || manifest.memory.len() != scope.expected_source_reports
        || manifest.environment.soft_nofile_limit == 0
        || (scope.id == RELEASE_SCOPE_ID
            && manifest.environment.soft_nofile_limit != RELEASE_SOFT_NOFILE_LIMIT)
        || manifest.accepted_maximum_memory_receipts.len() != ACCEPTED_MAXIMUM_MEMORY_GROUPS.len()
        || manifest.parity_outcome != GateOutcome::Passed
        || !manifest.environment_valid
        || manifest.timing_boundary != TimingBoundary::RawWorkV2
    {
        return Err(CompletionError::Boundary);
    }
    let expected_keys = expected_rollup_keys(scope);
    let actual_keys = manifest
        .rollups
        .iter()
        .map(|rollup| rollup_key(&rollup.group_id, rollup.tier))
        .collect::<Vec<_>>();
    let memory_receipt_groups = manifest
        .accepted_maximum_memory_receipts
        .iter()
        .map(|receipt| receipt.group_id.as_str())
        .collect::<Vec<_>>();
    let expected_regression_groups = scope
        .group_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let actual_regression_groups = manifest
        .regression_outcomes
        .iter()
        .map(|outcome| outcome.group_id.as_str())
        .collect::<Vec<_>>();
    if actual_keys != expected_keys
        || memory_receipt_groups != ACCEPTED_MAXIMUM_MEMORY_GROUPS
        || manifest
            .accepted_maximum_memory_receipts
            .iter()
            .any(|receipt| {
                DirectQualificationArtifactPath::try_new(Path::new(&receipt.path)).is_err()
                    || !valid_sha256(&receipt.report_sha256)
            })
        || manifest
            .rollups
            .iter()
            .any(|rollup| rollup.overall_outcome != GateOutcome::Passed)
        || manifest.regression_outcomes.len() != scope.group_ids.len()
        || actual_regression_groups != expected_regression_groups
        || manifest.regression_outcomes.iter().any(|outcome| {
            (outcome.checked_measurements == 0 && outcome.unseeded_measurements == 0)
                || match outcome.outcome {
                    SelfRegressionOutcome::Passed => outcome.unseeded_measurements != 0,
                    SelfRegressionOutcome::Unseeded => outcome.unseeded_measurements == 0,
                }
        })
    {
        return Err(CompletionError::Boundary);
    }
    Ok(())
}

fn validate_manifest_boundary(
    manifest: &CompletionManifest,
    input: &Path,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    scope: &CompletionScope,
) -> Result<(), CompletionError> {
    validate_manifest(manifest, scope)?;
    if Path::new(&manifest.output) != input {
        return Err(CompletionError::OutputBinding);
    }
    if manifest.performance_inventory_sha256 != expected_performance_inventory_sha256
        || manifest.correctness_inventory_sha256 != expected_correctness_inventory_sha256
    {
        return Err(CompletionError::InventoryIdentity);
    }
    Ok(())
}

fn completion_preflight(manifest: &CompletionManifest, report_json: &[u8]) -> CompletionPreflight {
    CompletionPreflight {
        schema_version: PREFLIGHT_SCHEMA_VERSION,
        report_sha256: sha256_hex(report_json),
        output: manifest.output.clone(),
        scope_id: manifest.scope_id.clone(),
        performance_inventory_sha256: manifest.performance_inventory_sha256.clone(),
        correctness_inventory_sha256: manifest.correctness_inventory_sha256.clone(),
        stab_commit: manifest.repository.commit_after.clone(),
        parity_policy_sha256: manifest.parity_policy_sha256.clone(),
        regression_policy_sha256: manifest.regression_policy_sha256.clone(),
        regression_baselines_sha256: manifest.regression_baselines_sha256.clone(),
        correctness_preflights: manifest.correctness_preflights.clone(),
        rollups: manifest
            .rollups
            .iter()
            .map(|rollup| rollup.artifact.clone())
            .collect(),
        source_report_count: manifest.source_reports.len(),
        memory_record_count: manifest.memory.len(),
        accepted_maximum_memory_receipts: manifest.accepted_maximum_memory_receipts.clone(),
        parity_outcome: manifest.parity_outcome,
        regression_outcomes: manifest.regression_outcomes.clone(),
    }
}

fn render_markdown(manifest: &CompletionManifest, report_sha256: &str) -> String {
    let regression = manifest
        .regression_outcomes
        .iter()
        .map(|outcome| {
            format!(
                "- `{}`: `{:?}` (`{}` checked, `{}` unseeded)",
                outcome.group_id,
                outcome.outcome,
                outcome.checked_measurements,
                outcome.unseeded_measurements
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "# Performance Qualification Completion\n\n- Scope: `{}`\n- Stab commit: `{}`\n- Stim commit: `{}`\n- Architecture: `{}`\n- CPU: `{}`\n- Soft `RLIMIT_NOFILE`: `{}`\n- Stim parity: `{:?}`\n- Environment: `valid`\n- Memory and scaling: `recorded`\n- Accepted-maximum memory receipts: `{}`\n- Rollups: `{}`\n- Source reports: `{}`\n- Completion report SHA-256: `{}`\n\n## Stab Self-Regression\n\n{}\n",
        manifest.scope_id,
        manifest.repository.commit_after,
        manifest.stim_commit,
        manifest.environment.architecture,
        manifest.environment.cpu_identity,
        manifest.environment.soft_nofile_limit,
        manifest.parity_outcome,
        manifest.accepted_maximum_memory_receipts.len(),
        manifest.rollups.len(),
        manifest.source_reports.len(),
        report_sha256,
        regression,
    )
}

fn read_completion_artifact(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    path: &DirectQualificationArtifactPath,
    name: &'static str,
    maximum_bytes: usize,
) -> Result<Vec<u8>, CompletionError> {
    Ok(super::artifact::read_artifact_bounded_with_repository(
        root,
        repository,
        path,
        name,
        maximum_bytes,
    )?)
}

fn schema_version(bytes: &[u8]) -> Result<u32, CompletionError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    let version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(CompletionError::Boundary)?;
    Ok(version)
}

fn parse_canonical<T>(bytes: &[u8]) -> Result<T, CompletionError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(CompletionError::Boundary);
    }
    let value: T = serde_json::from_slice(bytes)?;
    if canonical_json(&value)? != bytes {
        return Err(CompletionError::NonCanonical);
    }
    Ok(value)
}

fn canonical_json(value: &impl Serialize) -> Result<Vec<u8>, CompletionError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn require_clean_repository(state: &super::git::RepositoryState) -> Result<(), CompletionError> {
    if state.local_modifications {
        Err(CompletionError::DirtyRepository)
    } else {
        Ok(())
    }
}

fn require_same_clean_repository(
    before: &super::git::RepositoryState,
    after: &super::git::RepositoryState,
) -> Result<(), CompletionError> {
    require_clean_repository(before)?;
    require_clean_repository(after)?;
    if before.commit != after.commit {
        return Err(CompletionError::RepositoryChanged);
    }
    Ok(())
}

fn completion_repository_state(
    root: &RepoRoot,
    repository: &RepositoryBinding,
) -> Result<super::git::RepositoryState, CompletionError> {
    repository.require_current(root)?;
    let descriptor_root = repository.descriptor_root(root)?;
    let state = super::git::repository_state(&descriptor_root)?;
    repository.require_current(root)?;
    Ok(state)
}

fn require_completion_repository_state(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    expected: &RepositoryEvidence,
) -> Result<(), CompletionError> {
    let current = completion_repository_state(root, repository)?;
    if current.commit != expected.commit_before
        || current.commit != expected.commit_after
        || current.local_modifications != expected.local_modifications_before
        || current.local_modifications != expected.local_modifications_after
    {
        return Err(CompletionError::RepositoryChanged);
    }
    Ok(())
}

fn path_text(path: &Path) -> Result<String, CompletionError> {
    path.to_str()
        .map(ToString::to_string)
        .ok_or_else(|| CompletionError::PathEncoding(path.to_path_buf()))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn current_unix_epoch_seconds() -> Result<u64, CompletionError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
