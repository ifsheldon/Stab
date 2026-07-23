use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::artifact::{DirectQualificationArtifactPath, QualificationOutput, RepositoryBinding};
use super::correctness::{CorrectnessPreflightEvidence, CorrectnessPreflightStatus};
use super::group::{GroupContract, ParityEligibility, ProfilerNoteContract, ScaleContract};
use super::invocation::WorkerIdentityEvidence;
use super::protocol::{RAW_WORK_TIMING_BOUNDARY, TimingBoundary};
use super::run::{
    ClaimClass, QualificationReport, QualificationTier, RepositoryEvidence, sha256_hex,
};
use super::statistics::GateOutcome;
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::qualification::model::{SizeClass, TimingBatchPolicy};
use crate::root::RepoRoot;

mod render;
mod repository;

use render::{preflight, render_json, render_markdown};
use repository::require_current_producer;
#[cfg(test)]
use repository::require_current_producer_state;

const ROLLUP_SCHEMA_VERSION: u32 = 5;
const ROLLUP_PREFLIGHT_SCHEMA_VERSION: u32 = 2;
const DEFAULT_OUTPUT: &str = "target/benchmarks/qualification/rollup-latest";
pub(super) const MAX_ROLLUP_REPORT_BYTES: usize = 4 << 20;
pub(super) const MAX_ROLLUP_PREFLIGHT_BYTES: usize = 1 << 20;
pub(super) const MAX_ROLLUP_MARKDOWN_BYTES: usize = 4 << 20;
const MAX_SCALE_REPORTS: usize = 64;

#[derive(Clone, Debug, Args)]
pub(crate) struct RollupArgs {
    /// Source-owned runtime group whose complete scale family is required.
    #[arg(long)]
    group: String,

    /// Promotable tier shared by every source report.
    #[arg(long, value_enum)]
    tier: RollupTier,

    /// Published scale report directory; repeat exactly once per source-owned scale.
    #[arg(long, required = true)]
    input: Vec<PathBuf>,

    /// Atomic rollup directory beside the source reports.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    out: PathBuf,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct RollupReportArgs {
    /// Published scale-family rollup directory to replay and refresh.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    input: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RollupTier {
    Full,
    Soak,
}

impl From<RollupTier> for QualificationTier {
    fn from(value: RollupTier) -> Self {
        match value {
            RollupTier::Full => Self::Full,
            RollupTier::Soak => Self::Soak,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SharedIdentity {
    group_id: String,
    group_contract_sha256: String,
    claim_class: ClaimClass,
    parity_eligibility: ParityEligibility,
    owner: String,
    profiler_note: Option<ProfilerNoteContract>,
    tier: QualificationTier,
    performance_inventory_sha256: String,
    correctness_inventory_sha256: String,
    stim_tag: String,
    stim_commit: String,
    stab_commit: String,
    local_modifications: bool,
    host_verified: bool,
    host_policy_sha256: String,
    host_profile_id: String,
    operating_system: String,
    architecture: String,
    cpu_identity: String,
    rust_toolchain: String,
    target_triple: String,
    toolchain_sha256: String,
    workers: WorkerIdentityEvidence,
    correctness_preflight: CorrectnessPreflightEvidence,
}

#[derive(Clone, Debug)]
struct Candidate {
    shared: SharedIdentity,
    source: SourceReportBinding,
    generated_unix_epoch_seconds: u64,
    scale_id: String,
    work_items: u64,
    promotable: bool,
    measurements: Vec<RollupMeasurement>,
    memory: RollupMemory,
}

struct LoadedCandidate {
    path: DirectQualificationArtifactPath,
    report_sha256: String,
    preflight_sha256: String,
    markdown_sha256: String,
    correctness_binding: Arc<super::correctness::CorrectnessArtifactBinding>,
    candidate: Candidate,
}

struct AssemblyContext<'a> {
    contract: &'a GroupContract,
    group_contract_sha256: &'a str,
    expected_performance_inventory_sha256: &'a str,
    expected_correctness_inventory_sha256: &'a str,
    tier: QualificationTier,
    output_path: &'a DirectQualificationArtifactPath,
    producer_repository: RepositoryEvidence,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceReportBinding {
    path: String,
    report_sha256: String,
    preflight_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RollupReport {
    schema_version: u32,
    output: String,
    group_id: String,
    group_contract_sha256: String,
    claim_class: ClaimClass,
    parity_eligibility: ParityEligibility,
    owner: String,
    profiler_note: Option<ProfilerNoteContract>,
    tier: QualificationTier,
    generated_unix_epoch_seconds: u64,
    performance_inventory_sha256: String,
    correctness_inventory_sha256: String,
    stim_tag: String,
    stim_commit: String,
    stab_commit: String,
    producer_repository: RepositoryEvidence,
    host_policy_sha256: String,
    host_profile_id: String,
    operating_system: String,
    architecture: String,
    cpu_identity: String,
    rust_toolchain: String,
    target_triple: String,
    toolchain_sha256: String,
    workers: WorkerIdentityEvidence,
    correctness_preflight: CorrectnessPreflightEvidence,
    required_scale_count: usize,
    passed_measurements: usize,
    failed_measurements: usize,
    noisy_measurements: usize,
    overall_outcome: GateOutcome,
    scales: Vec<RollupScale>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RollupScale {
    scale_id: String,
    family_id: String,
    size_class: SizeClass,
    work_items: u64,
    input_bytes: u64,
    input_digest: String,
    source: SourceReportBinding,
    generated_unix_epoch_seconds: u64,
    measurements: Vec<RollupMeasurement>,
    memory: RollupMemory,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RollupMeasurement {
    measurement_id: String,
    pair_count: usize,
    median_ratio: f64,
    confidence_interval_lower: f64,
    confidence_interval_upper: f64,
    ratio_relative_mad: f64,
    threshold: f64,
    outcome: GateOutcome,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RollupMemory {
    pub(super) stim_setup_rss_bytes: u64,
    pub(super) stim_peak_rss_bytes: u64,
    pub(super) stim_parent_observed_peak_rss_bytes: Option<u64>,
    pub(super) stab_setup_rss_bytes: u64,
    pub(super) stab_peak_rss_bytes: u64,
    pub(super) stab_parent_observed_peak_rss_bytes: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RollupPreflight {
    schema_version: u32,
    report_sha256: String,
    output: String,
    group_id: String,
    tier: QualificationTier,
    performance_inventory_sha256: String,
    correctness_inventory_sha256: String,
    stab_commit: String,
    producer_repository: RepositoryEvidence,
    architecture: String,
    target_triple: String,
    required_scales: Vec<String>,
    source_reports: Vec<SourceReportBinding>,
    overall_outcome: GateOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RollupSourceEvidence {
    pub(super) scale_id: String,
    pub(super) path: PathBuf,
    pub(super) report_sha256: String,
    pub(super) preflight_sha256: String,
    pub(super) markdown_sha256: String,
}

#[derive(Clone, Debug)]
pub(super) struct RollupReplayEvidence {
    pub(super) output: PathBuf,
    pub(super) report_sha256: String,
    pub(super) preflight_sha256: String,
    pub(super) markdown_sha256: String,
    pub(super) group_id: String,
    pub(super) group_contract_sha256: String,
    pub(super) tier: QualificationTier,
    pub(super) performance_inventory_sha256: String,
    pub(super) stab_commit: String,
    pub(super) stim_commit: String,
    pub(super) host_policy_sha256: String,
    pub(super) host_profile_id: String,
    pub(super) operating_system: String,
    pub(super) architecture: String,
    pub(super) cpu_identity: String,
    pub(super) rust_toolchain: String,
    pub(super) target_triple: String,
    pub(super) toolchain_sha256: String,
    pub(super) timing_boundary: TimingBoundary,
    pub(super) workload_id: String,
    pub(super) timing_batch_policy: TimingBatchPolicy,
    pub(super) comparator_sources: Vec<(String, String)>,
    pub(super) workers: WorkerIdentityEvidence,
    pub(super) correctness_preflight: CorrectnessPreflightEvidence,
    pub(super) correctness_bindings: Vec<Arc<super::correctness::CorrectnessArtifactBinding>>,
    pub(super) overall_outcome: GateOutcome,
    pub(super) sources: Vec<RollupSourceEvidence>,
    pub(super) scales: Vec<RollupRegressionScale>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RollupRegressionScale {
    pub(super) scale_id: String,
    pub(super) family_id: String,
    pub(super) size_class: SizeClass,
    pub(super) work_items: u64,
    pub(super) input_digest: String,
    pub(super) measurements: Vec<RollupRegressionMeasurement>,
    pub(super) memory: RollupMemory,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RollupRegressionMeasurement {
    pub(super) measurement_id: String,
    pub(super) median_ratio: f64,
    pub(super) confidence_interval_upper: f64,
    pub(super) outcome: GateOutcome,
}

pub(super) fn run_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    live_repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: RollupArgs,
) -> Result<PathBuf, RollupError> {
    let output_path = DirectQualificationArtifactPath::try_new(&args.out)?;
    let input_paths = collect_input_paths(args.input.iter().map(PathBuf::as_path), &output_path)?;
    QualificationOutput::require_absent_with_repository(root, live_repository, &output_path)?;
    let repository_before = super::run::bound_repository_state(root, live_repository)?;
    require_clean_repository(&repository_before)?;
    let tier = QualificationTier::from(args.tier);
    let resolved = super::group::load_group(
        source_root,
        expected_performance_inventory_sha256,
        &args.group,
    )?;
    live_repository.require_current(root)?;
    if resolved.contract.scales.len() > MAX_SCALE_REPORTS
        || args.input.len() != resolved.contract.scales.len()
    {
        return Err(RollupError::InputCount {
            actual: args.input.len(),
            expected: resolved.contract.scales.len(),
        });
    }
    let loaded = load_candidates(
        root,
        source_root,
        live_repository,
        &input_paths,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?;
    let expected_stab_commit = expected_stab_commit(&loaded)?;
    live_repository.require_current(root)?;
    let repository_after = super::run::bound_repository_state(root, live_repository)?;
    let producer_repository =
        bind_producer_repository(repository_before, repository_after, &expected_stab_commit)?;

    let report = assemble(
        AssemblyContext {
            contract: &resolved.contract,
            group_contract_sha256: &resolved.source_sha256,
            expected_performance_inventory_sha256,
            expected_correctness_inventory_sha256,
            tier,
            output_path: &output_path,
            producer_repository,
        },
        loaded
            .iter()
            .map(|evidence| evidence.candidate.clone())
            .collect(),
    )?;
    let report_json = render_json(&report)?;
    let preflight = preflight(&report, &report_json);
    let preflight_json = render_json(&preflight)?;
    let markdown = render_markdown(&report, &sha256_hex(&report_json));

    let mut output =
        QualificationOutput::begin_new_with_repository(root, live_repository, &output_path)?;
    output.write("report.json", &report_json)?;
    output.write("preflight.json", &preflight_json)?;
    output.write("report.md", markdown.as_bytes())?;
    require_current_sources(&mut output, &loaded)?;
    require_current_correctness(&loaded)?;
    require_current_producer(root, live_repository, &report.producer_repository)?;
    output.commit_new_with_source_validation(|repository| {
        super::run::require_current_repository(root, &report.producer_repository, repository)?;
        require_current_correctness(&loaded)
    })?;
    Ok(output_path.into_path_buf())
}

pub(super) fn run_report_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    live_repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: RollupReportArgs,
) -> Result<PathBuf, RollupError> {
    let output_path = DirectQualificationArtifactPath::try_new(&args.input)?;
    Ok(replay_with_repository(
        root,
        source_root,
        live_repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        output_path,
    )?
    .output)
}

pub(super) fn replay_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    live_repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    output_path: DirectQualificationArtifactPath,
) -> Result<RollupReplayEvidence, RollupError> {
    let repository_before = super::run::bound_repository_state(root, live_repository)?;
    require_clean_repository(&repository_before)?;
    let existing_report_json = super::artifact::read_artifact_bounded_with_repository(
        root,
        live_repository,
        &output_path,
        "report.json",
        MAX_ROLLUP_REPORT_BYTES,
    )?;
    let existing_preflight_json = super::artifact::read_artifact_bounded_with_repository(
        root,
        live_repository,
        &output_path,
        "preflight.json",
        MAX_ROLLUP_PREFLIGHT_BYTES,
    )?;
    let existing_markdown = super::artifact::read_artifact_bounded_with_repository(
        root,
        live_repository,
        &output_path,
        "report.md",
        MAX_ROLLUP_MARKDOWN_BYTES,
    )?;
    let existing_report = parse_existing_rollup(
        &existing_report_json,
        &existing_preflight_json,
        &output_path,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?;
    let resolved = super::group::load_group(
        source_root,
        expected_performance_inventory_sha256,
        &existing_report.group_id,
    )?;
    live_repository.require_current(root)?;
    if existing_report.scales.len() != resolved.contract.scales.len()
        || existing_report.scales.len() > MAX_SCALE_REPORTS
    {
        return Err(RollupError::InputCount {
            actual: existing_report.scales.len(),
            expected: resolved.contract.scales.len(),
        });
    }
    let input_paths = collect_input_paths(
        existing_report
            .scales
            .iter()
            .map(|scale| Path::new(&scale.source.path)),
        &output_path,
    )?;
    let loaded = load_candidates(
        root,
        source_root,
        live_repository,
        &input_paths,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?;
    let expected_stab_commit = expected_stab_commit(&loaded)?;
    live_repository.require_current(root)?;
    let repository_after = super::run::bound_repository_state(root, live_repository)?;
    let producer_repository =
        bind_producer_repository(repository_before, repository_after, &expected_stab_commit)?;
    let reconstructed = assemble(
        AssemblyContext {
            contract: &resolved.contract,
            group_contract_sha256: &resolved.source_sha256,
            expected_performance_inventory_sha256,
            expected_correctness_inventory_sha256,
            tier: existing_report.tier,
            output_path: &output_path,
            producer_repository,
        },
        loaded
            .iter()
            .map(|evidence| evidence.candidate.clone())
            .collect(),
    )?;
    let report_json = require_reconstruction(&existing_report_json, &reconstructed)?;
    let preflight_json = render_json(&preflight(&reconstructed, &report_json))?;
    if preflight_json != existing_preflight_json {
        return Err(RollupError::PreflightBinding);
    }
    let markdown = render_markdown(&reconstructed, &sha256_hex(&report_json));

    let mut output =
        QualificationOutput::begin_with_repository(root, live_repository, &output_path)?;
    output.require_current_artifact("report.json", &existing_report_json)?;
    output.require_current_artifact("preflight.json", &existing_preflight_json)?;
    output.require_current_artifact("report.md", &existing_markdown)?;
    output.write("report.json", &report_json)?;
    output.write("preflight.json", &preflight_json)?;
    output.write("report.md", markdown.as_bytes())?;
    require_current_sources(&mut output, &loaded)?;
    require_current_correctness(&loaded)?;
    require_current_producer(root, live_repository, &reconstructed.producer_repository)?;
    output.commit_with_source_validation(|repository| {
        super::run::require_current_repository(
            root,
            &reconstructed.producer_repository,
            repository,
        )?;
        require_current_correctness(&loaded)
    })?;
    let regression_scales = reconstructed
        .scales
        .iter()
        .map(|scale| RollupRegressionScale {
            scale_id: scale.scale_id.clone(),
            family_id: scale.family_id.clone(),
            size_class: scale.size_class,
            work_items: scale.work_items,
            input_digest: scale.input_digest.clone(),
            measurements: scale
                .measurements
                .iter()
                .map(|measurement| RollupRegressionMeasurement {
                    measurement_id: measurement.measurement_id.clone(),
                    median_ratio: measurement.median_ratio,
                    confidence_interval_upper: measurement.confidence_interval_upper,
                    outcome: measurement.outcome,
                })
                .collect(),
            memory: scale.memory.clone(),
        })
        .collect();
    let comparator_sources = resolved
        .contract
        .comparator_sources
        .iter()
        .map(|source| {
            (
                source.path.as_str().to_string(),
                source.sha256.as_str().to_string(),
            )
        })
        .collect();
    Ok(RollupReplayEvidence {
        output: output_path.into_path_buf(),
        report_sha256: sha256_hex(&report_json),
        preflight_sha256: sha256_hex(&preflight_json),
        markdown_sha256: sha256_hex(markdown.as_bytes()),
        group_id: reconstructed.group_id,
        group_contract_sha256: reconstructed.group_contract_sha256,
        tier: reconstructed.tier,
        performance_inventory_sha256: reconstructed.performance_inventory_sha256,
        stab_commit: reconstructed.stab_commit,
        stim_commit: reconstructed.stim_commit,
        host_policy_sha256: reconstructed.host_policy_sha256,
        host_profile_id: reconstructed.host_profile_id,
        operating_system: reconstructed.operating_system,
        architecture: reconstructed.architecture,
        cpu_identity: reconstructed.cpu_identity,
        rust_toolchain: reconstructed.rust_toolchain,
        target_triple: reconstructed.target_triple,
        toolchain_sha256: reconstructed.toolchain_sha256,
        timing_boundary: RAW_WORK_TIMING_BOUNDARY,
        workload_id: resolved.contract.workload_id.to_string(),
        timing_batch_policy: resolved.contract.timing_batch_policy,
        comparator_sources,
        workers: reconstructed.workers,
        correctness_preflight: reconstructed.correctness_preflight,
        correctness_bindings: loaded
            .iter()
            .map(|candidate| Arc::clone(&candidate.correctness_binding))
            .collect(),
        overall_outcome: reconstructed.overall_outcome,
        scales: regression_scales,
        sources: reconstructed
            .scales
            .into_iter()
            .map(|scale| {
                let markdown_sha256 = loaded
                    .iter()
                    .find(|candidate| candidate.candidate.scale_id == scale.scale_id)
                    .map(|candidate| candidate.markdown_sha256.clone())
                    .ok_or_else(|| RollupError::ScaleContract(scale.scale_id.clone()))?;
                Ok(RollupSourceEvidence {
                    scale_id: scale.scale_id,
                    path: PathBuf::from(scale.source.path),
                    report_sha256: scale.source.report_sha256,
                    preflight_sha256: scale.source.preflight_sha256,
                    markdown_sha256,
                })
            })
            .collect::<Result<Vec<_>, RollupError>>()?,
    })
}

fn collect_input_paths<'a>(
    paths: impl IntoIterator<Item = &'a Path>,
    output_path: &DirectQualificationArtifactPath,
) -> Result<Vec<DirectQualificationArtifactPath>, RollupError> {
    let mut unique = BTreeSet::new();
    let mut inputs = Vec::new();
    for input in paths {
        let path = DirectQualificationArtifactPath::try_new(input)?;
        if path == *output_path {
            return Err(RollupError::OutputCollision(path.into_path_buf()));
        }
        if !unique.insert(path.clone()) {
            return Err(RollupError::DuplicateInput(path.into_path_buf()));
        }
        inputs.push(path);
    }
    Ok(inputs)
}

fn load_candidates(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    input_paths: &[DirectQualificationArtifactPath],
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
) -> Result<Vec<LoadedCandidate>, RollupError> {
    let mut loaded = Vec::with_capacity(input_paths.len());
    for path in input_paths {
        let evidence = super::report::load_validated_published_evidence(
            root,
            source_root,
            repository,
            path,
            expected_performance_inventory_sha256,
            expected_correctness_inventory_sha256,
        )?;
        let candidate = Candidate::from_report(
            path.as_path(),
            evidence.report,
            evidence.report_sha256.clone(),
            evidence.preflight_sha256.clone(),
        )?;
        loaded.push(LoadedCandidate {
            path: path.clone(),
            report_sha256: evidence.report_sha256,
            preflight_sha256: evidence.preflight_sha256,
            markdown_sha256: evidence.markdown_sha256,
            correctness_binding: evidence.correctness_binding,
            candidate,
        });
    }
    Ok(loaded)
}

fn expected_stab_commit(loaded: &[LoadedCandidate]) -> Result<String, RollupError> {
    loaded
        .first()
        .map(|evidence| evidence.candidate.shared.stab_commit.clone())
        .ok_or(RollupError::InputCount {
            actual: 0,
            expected: 1,
        })
}

fn require_clean_repository(state: &super::git::RepositoryState) -> Result<(), RollupError> {
    if state.local_modifications {
        Err(RollupError::DirtyProducer)
    } else {
        Ok(())
    }
}

fn bind_producer_repository(
    before: super::git::RepositoryState,
    after: super::git::RepositoryState,
    expected_commit: &str,
) -> Result<RepositoryEvidence, RollupError> {
    require_clean_repository(&before)?;
    require_clean_repository(&after)?;
    if before.commit != after.commit {
        return Err(RollupError::RepositoryChanged {
            before: before.commit,
            after: after.commit,
        });
    }
    if before.commit != expected_commit {
        return Err(RollupError::ProducerCommit {
            actual: before.commit,
            expected: expected_commit.to_string(),
        });
    }
    Ok(RepositoryEvidence {
        commit_before: before.commit,
        commit_after: after.commit,
        local_modifications_before: before.local_modifications,
        local_modifications_after: after.local_modifications,
    })
}

fn parse_existing_rollup(
    report_json: &[u8],
    preflight_json: &[u8],
    output_path: &DirectQualificationArtifactPath,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
) -> Result<RollupReport, RollupError> {
    if report_json.is_empty() || !report_json.ends_with(b"\n") {
        return Err(RollupError::ReportBoundary);
    }
    let report: RollupReport = serde_json::from_slice(report_json)?;
    if render_json(&report)? != report_json {
        return Err(RollupError::NonCanonicalReport);
    }
    if report.schema_version != ROLLUP_SCHEMA_VERSION {
        return Err(RollupError::SchemaVersion {
            actual: report.schema_version,
            expected: ROLLUP_SCHEMA_VERSION,
        });
    }
    if Path::new(&report.output) != output_path.as_path() {
        return Err(RollupError::OutputBinding);
    }
    if report.performance_inventory_sha256 != expected_performance_inventory_sha256
        || report.correctness_inventory_sha256 != expected_correctness_inventory_sha256
        || report.stim_tag != STIM_TAG
        || report.stim_commit != STIM_COMMIT
        || report.scales.is_empty()
        || report.scales.len() > MAX_SCALE_REPORTS
    {
        return Err(RollupError::Identity);
    }
    let expected_preflight = render_json(&preflight(&report, report_json))?;
    if expected_preflight != preflight_json {
        return Err(RollupError::PreflightBinding);
    }
    Ok(report)
}

fn require_reconstruction(
    existing_report_json: &[u8],
    reconstructed: &RollupReport,
) -> Result<Vec<u8>, RollupError> {
    let reconstructed_json = render_json(reconstructed)?;
    if reconstructed_json != existing_report_json {
        return Err(RollupError::Reconstruction);
    }
    Ok(reconstructed_json)
}

fn require_current_sources(
    output: &mut QualificationOutput,
    loaded: &[LoadedCandidate],
) -> Result<(), RollupError> {
    for evidence in loaded {
        output.require_sibling_artifact_digest(
            &evidence.path,
            "report.json",
            &evidence.report_sha256,
            super::report::MAX_PUBLISHED_REPORT_BYTES,
        )?;
        output.require_sibling_artifact_digest(
            &evidence.path,
            "preflight.json",
            &evidence.preflight_sha256,
            super::report::MAX_PUBLISHED_PREFLIGHT_BYTES,
        )?;
        output.require_sibling_artifact_digest(
            &evidence.path,
            "report.md",
            &evidence.markdown_sha256,
            super::report::MAX_PUBLISHED_MARKDOWN_BYTES,
        )?;
    }
    Ok(())
}

fn require_current_correctness(
    loaded: &[LoadedCandidate],
) -> Result<(), super::artifact::ArtifactError> {
    for evidence in loaded {
        evidence
            .correctness_binding
            .require_current()
            .map_err(|_| {
                super::artifact::ArtifactError::ExternalSourceChanged(
                    "correctness qualification evidence",
                )
            })?;
    }
    Ok(())
}

impl Candidate {
    fn from_report(
        path: &Path,
        report: QualificationReport,
        report_sha256: String,
        preflight_sha256: String,
    ) -> Result<Self, RollupError> {
        let measurements = super::report::authoritative_timing_attempt(&report)?
            .statistics
            .iter()
            .map(|summary| RollupMeasurement {
                measurement_id: summary.measurement_id.to_string(),
                pair_count: summary.pair_count,
                median_ratio: summary.median_ratio,
                confidence_interval_lower: summary.confidence_interval_lower,
                confidence_interval_upper: summary.confidence_interval_upper,
                ratio_relative_mad: summary.ratio_relative_mad,
                threshold: summary.threshold,
                outcome: summary.outcome,
            })
            .collect();
        let toolchain_json = serde_json::to_vec(&report.toolchain)?;
        let path = path
            .to_str()
            .ok_or_else(|| RollupError::InvalidPath(path.to_path_buf()))?
            .to_string();
        Ok(Self {
            shared: SharedIdentity {
                group_id: report.group_id,
                group_contract_sha256: report.group_contract_sha256,
                claim_class: report.claim_class,
                parity_eligibility: report.parity_eligibility,
                owner: report.owner,
                profiler_note: report.profiler_note,
                tier: report.tier,
                performance_inventory_sha256: report.performance_inventory_sha256,
                correctness_inventory_sha256: report.correctness_inventory_sha256,
                stim_tag: report.stim_tag,
                stim_commit: report.stim_commit,
                stab_commit: report.repository.commit_after,
                local_modifications: report.repository.local_modifications_before
                    || report.repository.local_modifications_after,
                host_verified: report.host.verified,
                host_policy_sha256: report.host.policy_sha256,
                host_profile_id: report.host.profile_id,
                operating_system: report.host.operating_system,
                architecture: report.host.architecture,
                cpu_identity: report.host.cpu_identity,
                rust_toolchain: report.toolchain.rust_toolchain,
                target_triple: report.toolchain.target_triple,
                toolchain_sha256: sha256_hex(&toolchain_json),
                workers: report.workers,
                correctness_preflight: report.correctness_preflight,
            },
            source: SourceReportBinding {
                path,
                report_sha256,
                preflight_sha256,
            },
            generated_unix_epoch_seconds: report.generated_unix_epoch_seconds,
            scale_id: report.scale_id,
            work_items: report.command.work_items,
            promotable: report.promotable,
            measurements,
            memory: RollupMemory {
                stim_setup_rss_bytes: report.memory.stim_setup_rss_bytes,
                stim_peak_rss_bytes: report.memory.stim_peak_rss_bytes,
                stim_parent_observed_peak_rss_bytes: report
                    .memory
                    .stim_parent_observed_peak_rss_bytes,
                stab_setup_rss_bytes: report.memory.stab_setup_rss_bytes,
                stab_peak_rss_bytes: report.memory.stab_peak_rss_bytes,
                stab_parent_observed_peak_rss_bytes: report
                    .memory
                    .stab_parent_observed_peak_rss_bytes,
            },
        })
    }
}

fn assemble(
    context: AssemblyContext<'_>,
    candidates: Vec<Candidate>,
) -> Result<RollupReport, RollupError> {
    let AssemblyContext {
        contract,
        group_contract_sha256,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        tier,
        output_path,
        producer_repository,
    } = context;
    require_promotable_tier(tier)?;
    if contract.claim_class != ClaimClass::PromotablePerformance
        || contract.parity_eligibility != ParityEligibility::ThresholdEligible
    {
        return Err(RollupError::GroupDisposition(contract.id.to_string()));
    }
    if candidates.len() != contract.scales.len() || candidates.len() > MAX_SCALE_REPORTS {
        return Err(RollupError::InputCount {
            actual: candidates.len(),
            expected: contract.scales.len(),
        });
    }
    let first = candidates.first().ok_or(RollupError::InputCount {
        actual: 0,
        expected: contract.scales.len(),
    })?;
    validate_shared(
        &first.shared,
        contract,
        group_contract_sha256,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        tier,
    )?;
    if !first.promotable {
        return Err(RollupError::NonPromotable(first.scale_id.clone()));
    }
    let shared = first.shared.clone();

    let mut by_scale = BTreeMap::new();
    for candidate in candidates {
        if candidate.shared != shared {
            return Err(RollupError::MixedIdentity(candidate.scale_id));
        }
        if !candidate.promotable {
            return Err(RollupError::NonPromotable(candidate.scale_id));
        }
        if by_scale
            .insert(candidate.scale_id.clone(), candidate)
            .is_some()
        {
            return Err(RollupError::DuplicateScale);
        }
    }

    let expected_measurements = contract
        .measurement_ids
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    let mut scales = Vec::with_capacity(contract.scales.len());
    for scale in &contract.scales {
        let scale_id = scale.id.to_string();
        let candidate = by_scale
            .remove(&scale_id)
            .ok_or_else(|| RollupError::MissingScale(scale_id.clone()))?;
        validate_scale(&candidate, scale, &expected_measurements)?;
        scales.push(RollupScale {
            scale_id,
            family_id: scale.family_id.to_string(),
            size_class: scale.size_class,
            work_items: scale.work_items.get(),
            input_bytes: scale.input_bytes,
            input_digest: scale.input_digest.as_str().to_string(),
            source: candidate.source,
            generated_unix_epoch_seconds: candidate.generated_unix_epoch_seconds,
            measurements: candidate.measurements,
            memory: candidate.memory,
        });
    }
    if !by_scale.is_empty() {
        return Err(RollupError::UnknownScale(
            by_scale.into_keys().next().unwrap_or_default(),
        ));
    }

    let mut passed = 0;
    let mut failed = 0;
    let mut noisy = 0;
    let generated_unix_epoch_seconds = scales
        .iter()
        .map(|scale| scale.generated_unix_epoch_seconds)
        .max()
        .unwrap_or(0);
    for measurement in scales.iter().flat_map(|scale| &scale.measurements) {
        match measurement.outcome {
            GateOutcome::Passed => passed += 1,
            GateOutcome::Failed => failed += 1,
            GateOutcome::Noisy => noisy += 1,
        }
    }
    let overall_outcome = if failed != 0 {
        GateOutcome::Failed
    } else if noisy != 0 {
        GateOutcome::Noisy
    } else {
        GateOutcome::Passed
    };
    let output = output_path
        .as_path()
        .to_str()
        .ok_or_else(|| RollupError::InvalidPath(output_path.as_path().to_path_buf()))?
        .to_string();
    Ok(RollupReport {
        schema_version: ROLLUP_SCHEMA_VERSION,
        output,
        group_id: shared.group_id,
        group_contract_sha256: shared.group_contract_sha256,
        claim_class: shared.claim_class,
        parity_eligibility: shared.parity_eligibility,
        owner: shared.owner,
        profiler_note: shared.profiler_note,
        tier: shared.tier,
        generated_unix_epoch_seconds,
        performance_inventory_sha256: shared.performance_inventory_sha256,
        correctness_inventory_sha256: shared.correctness_inventory_sha256,
        stim_tag: shared.stim_tag,
        stim_commit: shared.stim_commit,
        stab_commit: shared.stab_commit,
        producer_repository,
        host_policy_sha256: shared.host_policy_sha256,
        host_profile_id: shared.host_profile_id,
        operating_system: shared.operating_system,
        architecture: shared.architecture,
        cpu_identity: shared.cpu_identity,
        rust_toolchain: shared.rust_toolchain,
        target_triple: shared.target_triple,
        toolchain_sha256: shared.toolchain_sha256,
        workers: shared.workers,
        correctness_preflight: shared.correctness_preflight,
        required_scale_count: contract.scales.len(),
        passed_measurements: passed,
        failed_measurements: failed,
        noisy_measurements: noisy,
        overall_outcome,
        scales,
    })
}

fn validate_shared(
    identity: &SharedIdentity,
    contract: &GroupContract,
    group_contract_sha256: &str,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    tier: QualificationTier,
) -> Result<(), RollupError> {
    if identity.group_id != contract.id.to_string()
        || identity.group_contract_sha256 != group_contract_sha256
        || identity.claim_class != contract.claim_class
        || identity.parity_eligibility != contract.parity_eligibility
        || identity.owner != contract.owner.to_string()
        || identity.profiler_note != contract.profiler_note
        || identity.tier != tier
        || identity.performance_inventory_sha256 != expected_performance_inventory_sha256
        || identity.correctness_inventory_sha256 != expected_correctness_inventory_sha256
        || identity.stim_tag != STIM_TAG
        || identity.stim_commit != STIM_COMMIT
        || identity.stab_commit.len() != 40
        || identity.local_modifications
        || !identity.host_verified
        || identity.host_policy_sha256.len() != 64
        || identity.host_profile_id.is_empty()
        || identity.operating_system.is_empty()
        || identity.architecture.is_empty()
        || identity.cpu_identity.is_empty()
        || identity.rust_toolchain.is_empty()
        || identity.target_triple.is_empty()
        || identity.toolchain_sha256.len() != 64
        || !valid_worker_identity(&identity.workers)
        || identity.correctness_preflight.status != CorrectnessPreflightStatus::Passed
        || identity.correctness_preflight.case_ids != contract.correctness_case_ids
    {
        return Err(RollupError::Identity);
    }
    Ok(())
}

fn valid_worker_identity(identity: &WorkerIdentityEvidence) -> bool {
    [
        &identity.stim_source_sha256,
        &identity.stim_build_fingerprint,
        &identity.stim_binary_sha256,
        &identity.stab_source_sha256,
        &identity.stab_build_fingerprint,
        &identity.stab_binary_sha256,
        &identity.contract_preflight_sha256,
    ]
    .into_iter()
    .all(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn validate_scale(
    candidate: &Candidate,
    scale: &ScaleContract,
    expected_measurements: &BTreeSet<String>,
) -> Result<(), RollupError> {
    let actual_measurements = candidate
        .measurements
        .iter()
        .map(|measurement| measurement.measurement_id.clone())
        .collect::<BTreeSet<_>>();
    if candidate.work_items != scale.work_items.get()
        || candidate.measurements.len() != expected_measurements.len()
        || actual_measurements != *expected_measurements
    {
        return Err(RollupError::ScaleContract(candidate.scale_id.clone()));
    }
    Ok(())
}

fn require_promotable_tier(tier: QualificationTier) -> Result<(), RollupError> {
    match tier {
        QualificationTier::Full | QualificationTier::Soak => Ok(()),
        QualificationTier::Pr => Err(RollupError::NonPromotableTier),
    }
}

#[derive(Debug, Error)]
pub(super) enum RollupError {
    #[error("qualification scale-family rollups require full or soak evidence")]
    NonPromotableTier,
    #[error("qualification rollup has {actual} input reports, expected {expected}")]
    InputCount { actual: usize, expected: usize },
    #[error("qualification rollup path must be a direct qualification artifact: {0}")]
    InvalidPath(PathBuf),
    #[error("qualification rollup repeats source report {0}")]
    DuplicateInput(PathBuf),
    #[error("qualification rollup output collides with source report {0}")]
    OutputCollision(PathBuf),
    #[error("qualification group {0} is not eligible for a product scale-family rollup")]
    GroupDisposition(String),
    #[error("qualification rollup source identity does not match the source-owned contract")]
    Identity,
    #[error(
        "qualification rollup mixes commit, inventory, correctness, worker, host, or tier identity at scale {0}"
    )]
    MixedIdentity(String),
    #[error("qualification rollup source scale {0} is not promotable")]
    NonPromotable(String),
    #[error("qualification rollup repeats a scale")]
    DuplicateScale,
    #[error("qualification rollup is missing scale {0}")]
    MissingScale(String),
    #[error("qualification rollup contains unknown scale {0}")]
    UnknownScale(String),
    #[error("qualification rollup scale {0} has stale work or measurement identity")]
    ScaleContract(String),
    #[error("qualification rollup producer checkout contains local modifications")]
    DirtyProducer,
    #[error("qualification rollup producer revision changed from {before} to {after}")]
    RepositoryChanged { before: String, after: String },
    #[error(
        "qualification rollup producer revision is {actual}, expected source revision {expected}"
    )]
    ProducerCommit { actual: String, expected: String },
    #[error("qualification rollup report must be nonempty canonical JSON ending in a newline")]
    ReportBoundary,
    #[error("qualification rollup report is not canonical JSON")]
    NonCanonicalReport,
    #[error("qualification rollup schema is {actual}, expected {expected}")]
    SchemaVersion { actual: u32, expected: u32 },
    #[error("qualification rollup report is not bound to its published output directory")]
    OutputBinding,
    #[error("qualification rollup preflight does not match its report")]
    PreflightBinding,
    #[error("qualification rollup cannot be reconstructed from current source evidence")]
    Reconstruction,
    #[error(transparent)]
    Group(#[from] super::group::GroupError),
    #[error(transparent)]
    Report(#[from] super::report::ReportError),
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),
    #[error(transparent)]
    Git(#[from] super::git::GitError),
    #[error("failed to serialize qualification rollup evidence: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests;
