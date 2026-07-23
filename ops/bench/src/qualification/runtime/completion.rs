use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Args;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::artifact::{DirectQualificationArtifactPath, QualificationOutput, RepositoryBinding};
use super::correctness::CorrectnessPreflightEvidence;
use super::invocation::WorkerIdentityEvidence;
use super::protocol::TimingBoundary;
use super::rollup::{RollupReplayEvidence, RollupSourceEvidence};
use super::run::{QualificationTier, RepositoryEvidence, sha256_hex};
use super::self_regression::{SelfRegressionOutcome, SelfRegressionSummary};
use super::statistics::GateOutcome;
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::qualification::model::{SizeClass, TimingBatchPolicy};
use crate::root::RepoRoot;

mod legacy;
#[cfg(test)]
mod tests;

const COMPLETION_SCHEMA_VERSION: u32 = 2;
const PREFLIGHT_SCHEMA_VERSION: u32 = 2;
const LEGACY_COMPLETION_SCHEMA_VERSION: u32 = 1;
const DEM_SCOPE_ID: &str = "dem-r6";
const DEM_PARSE_GROUP: &str = "PERFQ-M10-DEM-PARSE-CONTRACT";
const DEM_PRINT_GROUP: &str = "PERFQ-M10-DEM-PRINT-CONTRACT";
const DEFAULT_OUTPUT: &str = "target/benchmarks/qualification/completion-latest";
const MAX_COMPLETION_REPORT_BYTES: usize = 16 << 20;
const MAX_COMPLETION_PREFLIGHT_BYTES: usize = 4 << 20;
const MAX_COMPLETION_MARKDOWN_BYTES: usize = 4 << 20;
const MAX_ROLLUPS: usize = 16;
const EXPECTED_DEM_ROLLUPS: usize = 4;
const EXPECTED_DEM_REPORTS: usize = 36;

#[derive(Clone, Debug, Args)]
pub(crate) struct CompletionArgs {
    /// Source-owned architecture/revision completion scope.
    #[arg(long, default_value = DEM_SCOPE_ID)]
    scope: String,

    /// Full or soak scale-family rollup; repeat once per required group and tier.
    #[arg(long, required = true)]
    rollup: Vec<PathBuf>,

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
    correctness_preflight: CorrectnessPreflightEvidence,
    rollups: Vec<CompletionRollup>,
    source_reports: Vec<CompletionSourceReport>,
    memory: Vec<CompletionMemory>,
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
    rollups: Vec<CompletionArtifact>,
    source_report_count: usize,
    memory_record_count: usize,
    parity_outcome: GateOutcome,
    regression_outcomes: Vec<CompletionRegression>,
}

struct ReconstructedCompletion {
    manifest: CompletionManifest,
    rollup_evidence: Vec<RollupReplayEvidence>,
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
    let reconstructed = reconstruct(
        root,
        source_root,
        repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        &args.scope,
        &output,
        &rollup_paths,
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

pub(super) fn run_report_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: CompletionReportArgs,
) -> Result<PathBuf, CompletionError> {
    let input = DirectQualificationArtifactPath::try_new(&args.input)?;
    let report_json = read_completion_artifact(
        root,
        repository,
        &input,
        "report.json",
        MAX_COMPLETION_REPORT_BYTES,
    )?;
    let schema_version = schema_version(&report_json)?;
    if schema_version == LEGACY_COMPLETION_SCHEMA_VERSION {
        let summary = legacy::parse(&report_json)?;
        if Path::new(&summary.output) != input.as_path() {
            return Err(CompletionError::OutputBinding);
        }
        return Ok(input.into_path_buf());
    }
    if schema_version != COMPLETION_SCHEMA_VERSION {
        return Err(CompletionError::SchemaVersion(schema_version));
    }

    let preflight_json = read_completion_artifact(
        root,
        repository,
        &input,
        "preflight.json",
        MAX_COMPLETION_PREFLIGHT_BYTES,
    )?;
    let markdown = read_completion_artifact(
        root,
        repository,
        &input,
        "report.md",
        MAX_COMPLETION_MARKDOWN_BYTES,
    )?;
    let manifest: CompletionManifest = parse_canonical(&report_json)?;
    validate_manifest_boundary(
        &manifest,
        input.as_path(),
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?;
    let rollup_paths = manifest
        .rollups
        .iter()
        .map(|rollup| DirectQualificationArtifactPath::try_new(Path::new(&rollup.artifact.path)))
        .collect::<Result<Vec<_>, _>>()?;
    let reconstructed = reconstruct(
        root,
        source_root,
        repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        &manifest.scope_id,
        &input,
        &rollup_paths,
        manifest.generated_unix_epoch_seconds,
    )?;
    let reconstructed_json = canonical_json(&reconstructed.manifest)?;
    let reconstructed_preflight = canonical_json(&completion_preflight(
        &reconstructed.manifest,
        &reconstructed_json,
    ))?;
    let reconstructed_markdown =
        render_markdown(&reconstructed.manifest, &sha256_hex(&reconstructed_json));
    if reconstructed_json != report_json
        || reconstructed_preflight != preflight_json
        || reconstructed_markdown.as_bytes() != markdown
    {
        return Err(CompletionError::Reconstruction);
    }
    require_completion_artifacts_unchanged(
        root,
        repository,
        &input,
        &report_json,
        &preflight_json,
        &markdown,
    )?;
    Ok(input.into_path_buf())
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
    generated_unix_epoch_seconds: u64,
) -> Result<ReconstructedCompletion, CompletionError> {
    require_scope(scope_id, rollup_paths.len())?;
    let repository_before = super::run::bound_repository_state(root, repository)?;
    require_clean_repository(&repository_before)?;
    let mut rollups = Vec::with_capacity(rollup_paths.len());
    for path in rollup_paths {
        rollups.push(super::rollup::replay_with_repository(
            root,
            source_root,
            repository,
            expected_performance_inventory_sha256,
            expected_correctness_inventory_sha256,
            path.clone(),
        )?);
    }
    order_and_validate_scope(&mut rollups)?;
    let shared = shared_identity(&rollups)?;
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
    )?;
    let regression_outcomes =
        evaluate_regression(source_root, expected_performance_inventory_sha256, &rollups)?;
    let repository_after = super::run::bound_repository_state(root, repository)?;
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
    let source_reports = completion_source_reports(&rollups)?;
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
        },
        workers: shared.workers.clone(),
        timing_boundary: shared.timing_boundary,
        correctness_preflight: shared.correctness_preflight.clone(),
        rollups: completion_rollups,
        source_reports,
        memory,
        parity_outcome: GateOutcome::Passed,
        regression_outcomes,
        environment_valid: true,
        memory_scaling_status: MemoryScalingStatus::Recorded,
    };
    validate_manifest(&manifest)?;
    Ok(ReconstructedCompletion {
        manifest,
        rollup_evidence: rollups,
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

fn require_scope(scope_id: &str, rollup_count: usize) -> Result<(), CompletionError> {
    if scope_id != DEM_SCOPE_ID {
        return Err(CompletionError::UnknownScope(scope_id.to_string()));
    }
    if rollup_count != EXPECTED_DEM_ROLLUPS {
        return Err(CompletionError::RollupCount(rollup_count));
    }
    Ok(())
}

fn order_and_validate_scope(
    rollups: &mut Vec<RollupReplayEvidence>,
) -> Result<(), CompletionError> {
    let mut by_key = BTreeMap::new();
    for rollup in rollups.drain(..) {
        let key = rollup_key(&rollup.group_id, rollup.tier);
        if !expected_rollup_keys().contains(&key) {
            return Err(CompletionError::UnknownRollup(key));
        }
        if by_key.insert(key.clone(), rollup).is_some() {
            return Err(CompletionError::DuplicateRollup(key));
        }
    }
    for key in expected_rollup_keys() {
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

fn expected_rollup_keys() -> Vec<String> {
    [
        (DEM_PARSE_GROUP, QualificationTier::Full),
        (DEM_PARSE_GROUP, QualificationTier::Soak),
        (DEM_PRINT_GROUP, QualificationTier::Full),
        (DEM_PRINT_GROUP, QualificationTier::Soak),
    ]
    .into_iter()
    .map(|(group, tier)| rollup_key(group, tier))
    .collect()
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
            || rollup.correctness_preflight != first.correctness_preflight
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
) -> Result<BTreeMap<String, usize>, CompletionError> {
    let mut paths = BTreeSet::new();
    let mut counts = BTreeMap::new();
    for rollup in rollups {
        let expected_measurements = rollup
            .scales
            .first()
            .map(|scale| scale.measurements.len())
            .ok_or(CompletionError::SourceReportCount(0))?;
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
    if paths.len() != EXPECTED_DEM_REPORTS {
        return Err(CompletionError::SourceReportCount(paths.len()));
    }
    Ok(counts)
}

fn evaluate_regression(
    source_root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
    rollups: &[RollupReplayEvidence],
) -> Result<Vec<CompletionRegression>, CompletionError> {
    [DEM_PARSE_GROUP, DEM_PRINT_GROUP]
        .into_iter()
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
    if reports.len() != EXPECTED_DEM_REPORTS {
        return Err(CompletionError::SourceReportCount(reports.len()));
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
    bind_evidence(&mut output, &reconstructed.rollup_evidence)?;

    let expected_commit = reconstructed.manifest.repository.commit_after.clone();
    let expected_parity = reconstructed.manifest.parity_policy_sha256.clone();
    let expected_regression_policy = reconstructed.manifest.regression_policy_sha256.clone();
    let expected_regression_baselines = reconstructed.manifest.regression_baselines_sha256.clone();
    let correctness_bindings = reconstructed
        .rollup_evidence
        .iter()
        .flat_map(|rollup| rollup.correctness_bindings.iter())
        .collect::<Vec<_>>();
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
        for binding in &correctness_bindings {
            binding.require_current().map_err(|_| {
                super::artifact::ArtifactError::ExternalSourceChanged(
                    "correctness qualification evidence",
                )
            })?;
        }
        Ok(())
    })?;
    Ok(())
}

fn bind_evidence(
    output: &mut QualificationOutput,
    rollups: &[RollupReplayEvidence],
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

fn validate_manifest(manifest: &CompletionManifest) -> Result<(), CompletionError> {
    if manifest.schema_version != COMPLETION_SCHEMA_VERSION
        || manifest.scope_id != DEM_SCOPE_ID
        || manifest.stim_tag != STIM_TAG
        || manifest.stim_commit != STIM_COMMIT
        || manifest.repository.commit_before != manifest.repository.commit_after
        || manifest.repository.local_modifications_before
        || manifest.repository.local_modifications_after
        || manifest.rollups.len() != EXPECTED_DEM_ROLLUPS
        || manifest.source_reports.len() != EXPECTED_DEM_REPORTS
        || manifest.memory.len() != EXPECTED_DEM_REPORTS
        || manifest.parity_outcome != GateOutcome::Passed
        || !manifest.environment_valid
        || manifest.timing_boundary != TimingBoundary::RawWorkV2
    {
        return Err(CompletionError::Boundary);
    }
    let expected_keys = expected_rollup_keys();
    let actual_keys = manifest
        .rollups
        .iter()
        .map(|rollup| rollup_key(&rollup.group_id, rollup.tier))
        .collect::<Vec<_>>();
    let expected_regression_groups = [DEM_PARSE_GROUP, DEM_PRINT_GROUP];
    let actual_regression_groups = manifest
        .regression_outcomes
        .iter()
        .map(|outcome| outcome.group_id.as_str())
        .collect::<Vec<_>>();
    if actual_keys != expected_keys
        || manifest
            .rollups
            .iter()
            .any(|rollup| rollup.overall_outcome != GateOutcome::Passed)
        || manifest.regression_outcomes.len() != 2
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
) -> Result<(), CompletionError> {
    validate_manifest(manifest)?;
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
        rollups: manifest
            .rollups
            .iter()
            .map(|rollup| rollup.artifact.clone())
            .collect(),
        source_report_count: manifest.source_reports.len(),
        memory_record_count: manifest.memory.len(),
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
        "# Performance Qualification Completion\n\n- Scope: `{}`\n- Stab commit: `{}`\n- Stim commit: `{}`\n- Architecture: `{}`\n- CPU: `{}`\n- Stim parity: `{:?}`\n- Environment: `valid`\n- Memory and scaling: `recorded`\n- Rollups: `{}`\n- Source reports: `{}`\n- Completion report SHA-256: `{}`\n\n## Stab Self-Regression\n\n{}\n",
        manifest.scope_id,
        manifest.repository.commit_after,
        manifest.stim_commit,
        manifest.environment.architecture,
        manifest.environment.cpu_identity,
        manifest.parity_outcome,
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

fn require_completion_artifacts_unchanged(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    path: &DirectQualificationArtifactPath,
    report: &[u8],
    preflight: &[u8],
    markdown: &[u8],
) -> Result<(), CompletionError> {
    for (name, expected, limit) in [
        ("report.json", report, MAX_COMPLETION_REPORT_BYTES),
        ("preflight.json", preflight, MAX_COMPLETION_PREFLIGHT_BYTES),
        ("report.md", markdown, MAX_COMPLETION_MARKDOWN_BYTES),
    ] {
        if read_completion_artifact(root, repository, path, name, limit)? != expected {
            return Err(CompletionError::SourceMutation);
        }
    }
    Ok(())
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

fn path_text(path: &Path) -> Result<String, CompletionError> {
    path.to_str()
        .map(ToString::to_string)
        .ok_or_else(|| CompletionError::PathEncoding(path.to_path_buf()))
}

fn current_unix_epoch_seconds() -> Result<u64, CompletionError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

#[derive(Debug, Error)]
pub(super) enum CompletionError {
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),
    #[error(transparent)]
    Rollup(#[from] super::rollup::RollupError),
    #[error(transparent)]
    Parity(#[from] super::parity::ParityError),
    #[error(transparent)]
    SelfRegression(#[from] super::self_regression::SelfRegressionError),
    #[error(transparent)]
    Git(#[from] super::git::GitError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Time(#[from] std::time::SystemTimeError),
    #[error("unknown completion scope {0:?}")]
    UnknownScope(String),
    #[error("completion scope has invalid rollup count {0}")]
    RollupCount(usize),
    #[error("completion output collides with source evidence at {0}")]
    OutputCollision(PathBuf),
    #[error("completion repeats source path {0}")]
    DuplicatePath(PathBuf),
    #[error("completion repeats rollup identity {0}")]
    DuplicateRollup(String),
    #[error("completion omits rollup identity {0}")]
    MissingRollup(String),
    #[error("completion contains rollup outside its source-owned scope: {0}")]
    UnknownRollup(String),
    #[error("completion mixes repository, host, worker, correctness, or timing identities")]
    MixedIdentity,
    #[error("completion source report count is {0}, expected 36")]
    SourceReportCount(usize),
    #[error("completion source report failed explicit Stim parity: {0}")]
    FailedParity(PathBuf),
    #[error("completion producer repository is dirty")]
    DirtyRepository,
    #[error("completion producer repository changed during reconstruction")]
    RepositoryChanged,
    #[error("completion schema {0} is not supported")]
    SchemaVersion(u32),
    #[error("completion artifact violates its schema or source-owned scope")]
    Boundary,
    #[error("completion artifact is not canonical JSON")]
    NonCanonical,
    #[error("completion output path does not match its manifest")]
    OutputBinding,
    #[error("completion inventories do not match current source contracts")]
    InventoryIdentity,
    #[error("completion replay does not reconstruct the checked artifacts")]
    Reconstruction,
    #[error("completion source evidence changed during replay")]
    SourceMutation,
    #[error("completion path is not UTF-8: {0}")]
    PathEncoding(PathBuf),
}
