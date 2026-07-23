use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use clap::Args;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::artifact::{DirectQualificationArtifactPath, QualificationOutput, RepositoryBinding};
use super::protocol::{RAW_WORK_TIMING_BOUNDARY, TimingBoundary};
use super::rollup::{RollupRegressionScale, RollupReplayEvidence};
use super::run::QualificationTier;
use crate::qualification::model::SizeClass;
use crate::root::RepoRoot;

const REGRESSION_POLICY_SCHEMA_VERSION: u32 = 1;
const REGRESSION_BASELINE_SCHEMA_VERSION: u32 = 1;
const MAX_POLICY_BYTES: usize = 1 << 20;
const MAX_BASELINE_BYTES: usize = 8 << 20;
const MAX_ENTRIES: usize = 4_096;
const DEFAULT_TOLERANCE: f64 = 1.15;
const MAX_EXCEPTION_TOLERANCE: f64 = 1.25;
pub(super) const DEFAULT_REGRESSION_POLICY: &str =
    "benchmarks/qualification-regression-policy.json";
pub(super) const DEFAULT_REGRESSION_BASELINES: &str =
    "benchmarks/qualification-regression-baselines.json";

#[derive(Clone, Debug, Args)]
pub(crate) struct SelfRegressionArgs {
    /// Accepted full-tier scale-family rollup.
    #[arg(long)]
    full: PathBuf,

    /// Accepted soak-tier scale-family rollup.
    #[arg(long)]
    soak: PathBuf,

    /// Source-owned Stab self-regression policy.
    #[arg(long, default_value = DEFAULT_REGRESSION_POLICY)]
    policy: PathBuf,

    /// Architecture-specific accepted Stab regression baselines.
    #[arg(long, default_value = DEFAULT_REGRESSION_BASELINES)]
    baselines: PathBuf,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct BaselineCandidateArgs {
    /// Accepted full-tier scale-family rollup.
    #[arg(long)]
    full: PathBuf,

    /// Accepted soak-tier scale-family rollup.
    #[arg(long)]
    soak: PathBuf,

    /// New immutable directory receiving candidate.json.
    #[arg(long)]
    out: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SelfRegressionOutcome {
    Passed,
    Unseeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SelfRegressionSummary {
    pub(crate) group_id: String,
    pub(crate) checked_measurements: usize,
    pub(crate) unseeded_measurements: usize,
    pub(crate) outcome: SelfRegressionOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RegressionSourceIdentities {
    pub(super) policy_sha256: String,
    pub(super) baselines_sha256: String,
    pub(super) default_max_relative_ratio: String,
    pub(super) seeded_identity_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegressionPolicy {
    schema_version: u32,
    default_max_relative_ratio: String,
    exceptions: Vec<RegressionException>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegressionException {
    group_id: String,
    family_id: String,
    scale_id: String,
    measurement_id: String,
    max_relative_ratio: String,
    justification: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegressionBaselineFile {
    schema_version: u32,
    performance_inventory_sha256: String,
    entries: Vec<RegressionBaselineEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegressionBaselineEntry {
    key: RegressionKey,
    accepted_median_ratio: String,
    accepted_confidence_interval_upper: String,
    full_rollup_sha256: String,
    soak_rollup_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RegressionKey {
    group_id: String,
    family_id: String,
    scale_id: String,
    size_class: SizeClass,
    measurement_id: String,
    host_profile_id: String,
    cpu_identity: String,
    architecture: String,
    target_triple: String,
    toolchain_sha256: String,
    stim_commit: String,
    stim_build_fingerprint: String,
    timing_boundary: TimingBoundary,
    workload_contract_sha256: String,
}

#[derive(Clone, Debug)]
struct CurrentMeasurement {
    key: RegressionKey,
    median_ratio: f64,
    confidence_interval_upper: f64,
}

pub(super) fn run_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: SelfRegressionArgs,
) -> Result<SelfRegressionSummary, SelfRegressionError> {
    let (full, soak) = replay_pair(
        root,
        source_root,
        repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        &args.full,
        &args.soak,
    )?;
    let summary = evaluate_evidence_with_sources(
        source_root,
        expected_performance_inventory_sha256,
        &full,
        &soak,
        &args.policy,
        &args.baselines,
    )?;
    if summary.outcome == SelfRegressionOutcome::Unseeded {
        return Err(SelfRegressionError::Unseeded {
            group_id: summary.group_id,
            count: summary.unseeded_measurements,
        });
    }
    Ok(summary)
}

pub(super) fn candidate_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: BaselineCandidateArgs,
) -> Result<PathBuf, SelfRegressionError> {
    let output_path = DirectQualificationArtifactPath::try_new(&args.out)?;
    QualificationOutput::require_absent_with_repository(root, repository, &output_path)?;
    let (full, soak) = replay_pair(
        root,
        source_root,
        repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        &args.full,
        &args.soak,
    )?;
    let entries = candidate_entries(
        current_measurements(&full, &soak)?,
        &full.report_sha256,
        &soak.report_sha256,
    );
    let candidate = RegressionBaselineFile {
        schema_version: REGRESSION_BASELINE_SCHEMA_VERSION,
        performance_inventory_sha256: expected_performance_inventory_sha256.to_string(),
        entries,
    };
    validate_baselines(&candidate, expected_performance_inventory_sha256)?;
    let bytes = render_json(&candidate)?;
    let mut output =
        QualificationOutput::begin_new_with_repository(root, repository, &output_path)?;
    output.write("candidate.json", &bytes)?;
    for rollup in [&full, &soak] {
        let path = DirectQualificationArtifactPath::try_new(&rollup.output)?;
        output.require_sibling_artifact_digest(
            &path,
            "report.json",
            &rollup.report_sha256,
            super::rollup::MAX_ROLLUP_REPORT_BYTES,
        )?;
        output.require_sibling_artifact_digest(
            &path,
            "preflight.json",
            &rollup.preflight_sha256,
            super::rollup::MAX_ROLLUP_PREFLIGHT_BYTES,
        )?;
        output.require_sibling_artifact_digest(
            &path,
            "report.md",
            &rollup.markdown_sha256,
            super::rollup::MAX_ROLLUP_MARKDOWN_BYTES,
        )?;
    }
    let correctness_bindings = [&full, &soak]
        .into_iter()
        .flat_map(|rollup| rollup.correctness_bindings.iter())
        .collect::<Vec<_>>();
    output.commit_new_with_source_validation(|binding| {
        binding.require_current(root)?;
        for correctness in &correctness_bindings {
            correctness.require_current().map_err(|_| {
                super::artifact::ArtifactError::ExternalSourceChanged(
                    "correctness qualification evidence",
                )
            })?;
        }
        Ok(())
    })?;
    Ok(output_path.into_path_buf())
}

fn candidate_entries(
    current: Vec<CurrentMeasurement>,
    full_rollup_sha256: &str,
    soak_rollup_sha256: &str,
) -> Vec<RegressionBaselineEntry> {
    current
        .into_iter()
        .map(|measurement| RegressionBaselineEntry {
            key: measurement.key,
            accepted_median_ratio: ratio_text(measurement.median_ratio),
            accepted_confidence_interval_upper: ratio_text(measurement.confidence_interval_upper),
            full_rollup_sha256: full_rollup_sha256.to_string(),
            soak_rollup_sha256: soak_rollup_sha256.to_string(),
        })
        .collect()
}

pub(super) fn check_sources(
    root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
) -> Result<(), SelfRegressionError> {
    load_checked_sources(
        root,
        expected_performance_inventory_sha256,
        Path::new(DEFAULT_REGRESSION_POLICY),
        Path::new(DEFAULT_REGRESSION_BASELINES),
    )
    .map(|_| ())
}

pub(super) fn source_identities(
    root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
) -> Result<RegressionSourceIdentities, SelfRegressionError> {
    let (policy, baselines) = load_checked_sources(
        root,
        expected_performance_inventory_sha256,
        Path::new(DEFAULT_REGRESSION_POLICY),
        Path::new(DEFAULT_REGRESSION_BASELINES),
    )?;
    Ok(RegressionSourceIdentities {
        policy_sha256: source_sha256(root, Path::new(DEFAULT_REGRESSION_POLICY), MAX_POLICY_BYTES)?,
        baselines_sha256: source_sha256(
            root,
            Path::new(DEFAULT_REGRESSION_BASELINES),
            MAX_BASELINE_BYTES,
        )?,
        default_max_relative_ratio: policy.default_max_relative_ratio,
        seeded_identity_count: baselines.entries.len(),
    })
}

pub(super) fn evaluate_evidence(
    source_root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
    full: &RollupReplayEvidence,
    soak: &RollupReplayEvidence,
) -> Result<SelfRegressionSummary, SelfRegressionError> {
    evaluate_evidence_with_sources(
        source_root,
        expected_performance_inventory_sha256,
        full,
        soak,
        Path::new(DEFAULT_REGRESSION_POLICY),
        Path::new(DEFAULT_REGRESSION_BASELINES),
    )
}

fn evaluate_evidence_with_sources(
    source_root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
    full: &RollupReplayEvidence,
    soak: &RollupReplayEvidence,
    policy_path: &Path,
    baselines_path: &Path,
) -> Result<SelfRegressionSummary, SelfRegressionError> {
    let current = current_measurements(full, soak)?;
    let (policy, baselines) = load_checked_sources(
        source_root,
        expected_performance_inventory_sha256,
        policy_path,
        baselines_path,
    )?;
    evaluate_current(full.group_id.clone(), &current, &policy, &baselines)
}

fn load_checked_sources(
    root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
    policy_path: &Path,
    baselines_path: &Path,
) -> Result<(RegressionPolicy, RegressionBaselineFile), SelfRegressionError> {
    let policy = load_policy(root, policy_path)?;
    validate_policy(&policy)?;
    let baselines = load_baselines(root, baselines_path)?;
    validate_baselines(&baselines, expected_performance_inventory_sha256)?;
    let contracts = super::group::load_groups(root, expected_performance_inventory_sha256)?;
    validate_source_targets(&policy, &baselines, &contracts)?;
    Ok((policy, baselines))
}

fn evaluate_current(
    group_id: String,
    current: &[CurrentMeasurement],
    policy: &RegressionPolicy,
    baselines: &RegressionBaselineFile,
) -> Result<SelfRegressionSummary, SelfRegressionError> {
    let mut checked = 0;
    let mut unseeded = 0;
    for measurement in current {
        let Some(baseline) = baselines
            .entries
            .iter()
            .find(|entry| entry.key == measurement.key)
        else {
            unseeded += 1;
            continue;
        };
        let accepted_median =
            parse_positive_ratio("accepted_median_ratio", &baseline.accepted_median_ratio)?;
        let accepted_upper = parse_positive_ratio(
            "accepted_confidence_interval_upper",
            &baseline.accepted_confidence_interval_upper,
        )?;
        let tolerance = tolerance_for(policy, &measurement.key)?;
        let median_deterioration = measurement.median_ratio / accepted_median;
        let upper_deterioration = measurement.confidence_interval_upper / accepted_upper;
        if median_deterioration > tolerance || upper_deterioration > tolerance {
            return Err(SelfRegressionError::ThresholdExceeded {
                group_id: measurement.key.group_id.clone(),
                family_id: measurement.key.family_id.clone(),
                scale_id: measurement.key.scale_id.clone(),
                measurement_id: measurement.key.measurement_id.clone(),
                median_deterioration,
                upper_deterioration,
                tolerance,
            });
        }
        checked += 1;
    }
    Ok(SelfRegressionSummary {
        group_id,
        checked_measurements: checked,
        unseeded_measurements: unseeded,
        outcome: if unseeded == 0 {
            SelfRegressionOutcome::Passed
        } else {
            SelfRegressionOutcome::Unseeded
        },
    })
}

fn source_sha256(
    root: &RepoRoot,
    path: &Path,
    maximum_bytes: usize,
) -> Result<String, SelfRegressionError> {
    let path = root.resolve_relative(path);
    let bytes = crate::source_file::read_repo_regular_file_bounded(root, &path, maximum_bytes)
        .map_err(|error| SelfRegressionError::Read(error.to_string()))?;
    Ok(super::run::sha256_hex(&bytes))
}

fn replay_pair(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    full: &Path,
    soak: &Path,
) -> Result<(RollupReplayEvidence, RollupReplayEvidence), SelfRegressionError> {
    let full = super::rollup::replay_with_repository(
        root,
        source_root,
        repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        DirectQualificationArtifactPath::try_new(full)?,
    )?;
    let soak = super::rollup::replay_with_repository(
        root,
        source_root,
        repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        DirectQualificationArtifactPath::try_new(soak)?,
    )?;
    Ok((full, soak))
}

fn current_measurements(
    full: &RollupReplayEvidence,
    soak: &RollupReplayEvidence,
) -> Result<Vec<CurrentMeasurement>, SelfRegressionError> {
    require_matching_rollups(full, soak)?;
    let mut result = Vec::new();
    for full_scale in &full.scales {
        let soak_scale = soak
            .scales
            .iter()
            .find(|scale| scale.scale_id == full_scale.scale_id)
            .ok_or(SelfRegressionError::RollupIdentity)?;
        require_matching_scale(full_scale, soak_scale)?;
        for full_measurement in &full_scale.measurements {
            let soak_measurement = soak_scale
                .measurements
                .iter()
                .find(|measurement| measurement.measurement_id == full_measurement.measurement_id)
                .ok_or(SelfRegressionError::RollupIdentity)?;
            if full_measurement.outcome != super::statistics::GateOutcome::Passed
                || soak_measurement.outcome != super::statistics::GateOutcome::Passed
            {
                return Err(SelfRegressionError::UnacceptedRollup);
            }
            result.push(CurrentMeasurement {
                key: RegressionKey {
                    group_id: full.group_id.clone(),
                    family_id: full_scale.family_id.clone(),
                    scale_id: full_scale.scale_id.clone(),
                    size_class: full_scale.size_class,
                    measurement_id: full_measurement.measurement_id.clone(),
                    host_profile_id: full.host_profile_id.clone(),
                    cpu_identity: full.cpu_identity.clone(),
                    architecture: full.architecture.clone(),
                    target_triple: full.target_triple.clone(),
                    toolchain_sha256: full.toolchain_sha256.clone(),
                    stim_commit: full.stim_commit.clone(),
                    stim_build_fingerprint: full.workers.stim_build_fingerprint.clone(),
                    timing_boundary: full.timing_boundary,
                    workload_contract_sha256: workload_contract_digest(full, full_scale)?,
                },
                median_ratio: full_measurement
                    .median_ratio
                    .max(soak_measurement.median_ratio),
                confidence_interval_upper: full_measurement
                    .confidence_interval_upper
                    .max(soak_measurement.confidence_interval_upper),
            });
        }
    }
    Ok(result)
}

fn require_matching_rollups(
    full: &RollupReplayEvidence,
    soak: &RollupReplayEvidence,
) -> Result<(), SelfRegressionError> {
    if full.tier != QualificationTier::Full
        || soak.tier != QualificationTier::Soak
        || full.group_id != soak.group_id
        || full.group_contract_sha256 != soak.group_contract_sha256
        || full.performance_inventory_sha256 != soak.performance_inventory_sha256
        || full.stab_commit != soak.stab_commit
        || full.stim_commit != soak.stim_commit
        || full.host_policy_sha256 != soak.host_policy_sha256
        || full.host_profile_id != soak.host_profile_id
        || full.architecture != soak.architecture
        || full.cpu_identity != soak.cpu_identity
        || full.target_triple != soak.target_triple
        || full.toolchain_sha256 != soak.toolchain_sha256
        || full.workers != soak.workers
        || full.timing_boundary != soak.timing_boundary
        || full.workload_id != soak.workload_id
        || full.timing_batch_policy != soak.timing_batch_policy
        || full.comparator_sources != soak.comparator_sources
        || full.overall_outcome != super::statistics::GateOutcome::Passed
        || soak.overall_outcome != super::statistics::GateOutcome::Passed
        || full.scales.len() != soak.scales.len()
    {
        return Err(SelfRegressionError::RollupIdentity);
    }
    Ok(())
}

fn require_matching_scale(
    full: &RollupRegressionScale,
    soak: &RollupRegressionScale,
) -> Result<(), SelfRegressionError> {
    if full.scale_id != soak.scale_id
        || full.family_id != soak.family_id
        || full.size_class != soak.size_class
        || full.work_items != soak.work_items
        || full.input_digest != soak.input_digest
        || full.measurements.len() != soak.measurements.len()
    {
        return Err(SelfRegressionError::RollupIdentity);
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WorkloadDigestMaterial<'a> {
    group_id: &'a str,
    workload_id: &'a str,
    family_id: &'a str,
    scale_id: &'a str,
    size_class: SizeClass,
    work_items: u64,
    input_digest: &'a str,
    timing_batch_policy: crate::qualification::model::TimingBatchPolicy,
    timing_boundary: TimingBoundary,
    comparator_sources: &'a [(String, String)],
}

fn workload_contract_digest(
    rollup: &RollupReplayEvidence,
    scale: &RollupRegressionScale,
) -> Result<String, SelfRegressionError> {
    let bytes = serde_json::to_vec(&WorkloadDigestMaterial {
        group_id: &rollup.group_id,
        workload_id: &rollup.workload_id,
        family_id: &scale.family_id,
        scale_id: &scale.scale_id,
        size_class: scale.size_class,
        work_items: scale.work_items,
        input_digest: &scale.input_digest,
        timing_batch_policy: rollup.timing_batch_policy,
        timing_boundary: rollup.timing_boundary,
        comparator_sources: &rollup.comparator_sources,
    })?;
    Ok(super::run::sha256_hex(&bytes))
}

fn load_policy(root: &RepoRoot, path: &Path) -> Result<RegressionPolicy, SelfRegressionError> {
    let path = root.resolve_relative(path);
    let bytes = crate::source_file::read_repo_regular_file_bounded(root, &path, MAX_POLICY_BYTES)
        .map_err(|error| SelfRegressionError::Read(error.to_string()))?;
    serde_json::from_slice(&bytes).map_err(SelfRegressionError::Json)
}

fn load_baselines(
    root: &RepoRoot,
    path: &Path,
) -> Result<RegressionBaselineFile, SelfRegressionError> {
    let path = root.resolve_relative(path);
    let bytes = crate::source_file::read_repo_regular_file_bounded(root, &path, MAX_BASELINE_BYTES)
        .map_err(|error| SelfRegressionError::Read(error.to_string()))?;
    serde_json::from_slice(&bytes).map_err(SelfRegressionError::Json)
}

fn validate_policy(policy: &RegressionPolicy) -> Result<(), SelfRegressionError> {
    if policy.schema_version != REGRESSION_POLICY_SCHEMA_VERSION {
        return Err(SelfRegressionError::PolicySchema);
    }
    let default = parse_positive_ratio(
        "default_max_relative_ratio",
        &policy.default_max_relative_ratio,
    )?;
    if default != DEFAULT_TOLERANCE {
        return Err(SelfRegressionError::InvalidTolerance(default));
    }
    let mut unique = BTreeSet::new();
    for exception in &policy.exceptions {
        let key = (
            exception.group_id.as_str(),
            exception.family_id.as_str(),
            exception.scale_id.as_str(),
            exception.measurement_id.as_str(),
        );
        let tolerance = parse_positive_ratio("max_relative_ratio", &exception.max_relative_ratio)?;
        if !unique.insert(key)
            || tolerance <= default
            || tolerance > MAX_EXCEPTION_TOLERANCE
            || exception.justification.trim().len() < 16
        {
            return Err(SelfRegressionError::InvalidException);
        }
    }
    Ok(())
}

fn validate_baselines(
    baselines: &RegressionBaselineFile,
    expected_performance_inventory_sha256: &str,
) -> Result<(), SelfRegressionError> {
    if baselines.schema_version != REGRESSION_BASELINE_SCHEMA_VERSION
        || baselines.performance_inventory_sha256 != expected_performance_inventory_sha256
        || baselines.entries.len() > MAX_ENTRIES
    {
        return Err(SelfRegressionError::BaselineIdentity);
    }
    let mut unique = BTreeSet::new();
    for entry in &baselines.entries {
        let encoded = serde_json::to_vec(&entry.key)?;
        if !unique.insert(encoded)
            || !valid_sha256(&entry.key.toolchain_sha256)
            || !valid_sha256(&entry.key.stim_build_fingerprint)
            || !valid_sha256(&entry.key.workload_contract_sha256)
            || !valid_sha256(&entry.full_rollup_sha256)
            || !valid_sha256(&entry.soak_rollup_sha256)
        {
            return Err(SelfRegressionError::BaselineIdentity);
        }
        let median = parse_positive_ratio("accepted_median_ratio", &entry.accepted_median_ratio)?;
        let upper = parse_positive_ratio(
            "accepted_confidence_interval_upper",
            &entry.accepted_confidence_interval_upper,
        )?;
        if upper < median {
            return Err(SelfRegressionError::BaselineIdentity);
        }
    }
    Ok(())
}

fn validate_source_targets(
    policy: &RegressionPolicy,
    baselines: &RegressionBaselineFile,
    contracts: &[super::group::GroupContract],
) -> Result<(), SelfRegressionError> {
    for exception in &policy.exceptions {
        let Some(contract) = release_contract(contracts, &exception.group_id) else {
            return Err(SelfRegressionError::InvalidException);
        };
        let scale_matches = contract.scales.iter().any(|scale| {
            scale.id.to_string() == exception.scale_id
                && scale.family_id.to_string() == exception.family_id
        });
        let measurement_matches = contract
            .measurement_ids
            .iter()
            .any(|measurement| measurement.to_string() == exception.measurement_id);
        if !scale_matches || !measurement_matches {
            return Err(SelfRegressionError::InvalidException);
        }
    }
    for entry in &baselines.entries {
        let key = &entry.key;
        let Some(contract) = release_contract(contracts, &key.group_id) else {
            return Err(SelfRegressionError::BaselineIdentity);
        };
        let scale_matches = contract.scales.iter().any(|scale| {
            scale.id.to_string() == key.scale_id
                && scale.family_id.to_string() == key.family_id
                && scale.size_class == key.size_class
        });
        let measurement_matches = contract
            .measurement_ids
            .iter()
            .any(|measurement| measurement.to_string() == key.measurement_id);
        if !scale_matches
            || !measurement_matches
            || key.host_profile_id.is_empty()
            || key.cpu_identity.is_empty()
            || key.architecture.is_empty()
            || key.target_triple.is_empty()
            || key.stim_commit != crate::config::STIM_COMMIT
            || key.timing_boundary != RAW_WORK_TIMING_BOUNDARY
        {
            return Err(SelfRegressionError::BaselineIdentity);
        }
    }
    Ok(())
}

fn release_contract<'a>(
    contracts: &'a [super::group::GroupContract],
    group_id: &str,
) -> Option<&'a super::group::GroupContract> {
    contracts.iter().find(|contract| {
        contract.id.to_string() == group_id
            && contract.claim_class == super::run::ClaimClass::PromotablePerformance
            && contract.parity_eligibility == super::group::ParityEligibility::ThresholdEligible
    })
}

fn tolerance_for(
    policy: &RegressionPolicy,
    key: &RegressionKey,
) -> Result<f64, SelfRegressionError> {
    let selected = policy.exceptions.iter().find(|exception| {
        exception.group_id == key.group_id
            && exception.family_id == key.family_id
            && exception.scale_id == key.scale_id
            && exception.measurement_id == key.measurement_id
    });
    parse_positive_ratio(
        "regression tolerance",
        selected.map_or(policy.default_max_relative_ratio.as_str(), |exception| {
            exception.max_relative_ratio.as_str()
        }),
    )
}

fn parse_positive_ratio(name: &'static str, value: &str) -> Result<f64, SelfRegressionError> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| SelfRegressionError::InvalidRatio {
            name,
            value: value.to_string(),
        })?;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(SelfRegressionError::InvalidRatio {
            name,
            value: value.to_string(),
        });
    }
    Ok(parsed)
}

fn ratio_text(value: f64) -> String {
    format!("{value:.17}")
}

fn render_json<T: Serialize>(value: &T) -> Result<Vec<u8>, SelfRegressionError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Error)]
pub(super) enum SelfRegressionError {
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),
    #[error(transparent)]
    Rollup(#[from] super::rollup::RollupError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Group(#[from] super::group::GroupError),
    #[error("failed to read self-regression source contract: {0}")]
    Read(String),
    #[error("self-regression policy schema is unsupported")]
    PolicySchema,
    #[error("self-regression default tolerance {0} must be exactly 1.15")]
    InvalidTolerance(f64),
    #[error("self-regression exception is duplicated, unjustified, or outside 1.15..=1.25")]
    InvalidException,
    #[error("self-regression baseline identity is stale or malformed")]
    BaselineIdentity,
    #[error("self-regression ratio {name}={value:?} is not positive and finite")]
    InvalidRatio { name: &'static str, value: String },
    #[error("full and soak rollups do not have one matching accepted identity")]
    RollupIdentity,
    #[error("full or soak rollup did not pass Stim parity")]
    UnacceptedRollup,
    #[error(
        "Stab self-regression is unseeded for group {group_id}: {count} measurement identities lack an accepted architecture baseline"
    )]
    Unseeded { group_id: String, count: usize },
    #[error(
        "Stab self-regression exceeded tolerance for {group_id}/{family_id}/{scale_id}/{measurement_id}: median deterioration {median_deterioration:.6}x, upper-bound deterioration {upper_deterioration:.6}x, maximum {tolerance:.6}x"
    )]
    ThresholdExceeded {
        group_id: String,
        family_id: String,
        scale_id: String,
        measurement_id: String,
        median_deterioration: f64,
        upper_deterioration: f64,
        tolerance: f64,
    },
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;

    fn key() -> RegressionKey {
        RegressionKey {
            group_id: "group".to_string(),
            family_id: "family".to_string(),
            scale_id: "family-small".to_string(),
            size_class: SizeClass::Small,
            measurement_id: "main".to_string(),
            host_profile_id: "host".to_string(),
            cpu_identity: "cpu".to_string(),
            architecture: "aarch64".to_string(),
            target_triple: "aarch64-unknown-linux-gnu".to_string(),
            toolchain_sha256: "a".repeat(64),
            stim_commit: "b".repeat(40),
            stim_build_fingerprint: "c".repeat(64),
            timing_boundary: TimingBoundary::RawWorkV2,
            workload_contract_sha256: "d".repeat(64),
        }
    }

    fn policy(default: &str) -> RegressionPolicy {
        RegressionPolicy {
            schema_version: REGRESSION_POLICY_SCHEMA_VERSION,
            default_max_relative_ratio: default.to_string(),
            exceptions: Vec::new(),
        }
    }

    fn baseline(entry: RegressionBaselineEntry) -> RegressionBaselineFile {
        RegressionBaselineFile {
            schema_version: REGRESSION_BASELINE_SCHEMA_VERSION,
            performance_inventory_sha256: "e".repeat(64),
            entries: vec![entry],
        }
    }

    fn entry() -> RegressionBaselineEntry {
        RegressionBaselineEntry {
            key: key(),
            accepted_median_ratio: "1.0".to_string(),
            accepted_confidence_interval_upper: "1.0".to_string(),
            full_rollup_sha256: "f".repeat(64),
            soak_rollup_sha256: "0".repeat(64),
        }
    }

    fn contract() -> super::super::group::GroupContract {
        super::super::group::GroupContract {
            id: super::super::protocol::ProtocolId::try_new("group").expect("group id"),
            claim_class: super::super::run::ClaimClass::PromotablePerformance,
            parity_eligibility: super::super::group::ParityEligibility::ThresholdEligible,
            timing_batch_policy: crate::qualification::model::TimingBatchPolicy::CommonIterations,
            workload_id: super::super::protocol::ProtocolId::try_new("workload")
                .expect("workload id"),
            measurement_ids: vec![
                super::super::protocol::ProtocolId::try_new("main").expect("measurement id"),
            ],
            scales: vec![super::super::group::ScaleContract {
                id: super::super::protocol::ProtocolId::try_new("family-small").expect("scale id"),
                family_id: super::super::protocol::ProtocolId::try_new("family")
                    .expect("family id"),
                size_class: SizeClass::Small,
                work_items: NonZeroU64::new(1).expect("nonzero work"),
                input_bytes: 0,
                input_digest: super::super::protocol::InputDigest::try_new("1".repeat(64))
                    .expect("input digest"),
            }],
            correctness_case_ids: Vec::new(),
            owner: super::super::protocol::ProtocolId::try_new("owner").expect("owner id"),
            profiler_note: None,
            comparator_sources: Vec::new(),
        }
    }

    fn passes(current_median: f64, current_upper: f64, accepted: &RegressionBaselineEntry) -> bool {
        let tolerance = tolerance_for(&policy("1.15"), &accepted.key).expect("tolerance");
        current_median
            / parse_positive_ratio("median", &accepted.accepted_median_ratio).expect("median")
            <= tolerance
            && current_upper
                / parse_positive_ratio("upper", &accepted.accepted_confidence_interval_upper)
                    .expect("upper")
                <= tolerance
    }

    #[test]
    fn default_regression_boundary_is_exact_and_independent_per_statistic() {
        let accepted = entry();
        assert!(passes(1.149, 1.149, &accepted));
        assert!(!passes(1.151, 1.149, &accepted));
        assert!(!passes(1.149, 1.151, &accepted));
        assert!(passes(0.5, 0.75, &accepted));
    }

    #[test]
    fn policy_rejects_invalid_exceptions() {
        let mut invalid = policy("1.15");
        invalid.exceptions.push(RegressionException {
            group_id: "group".to_string(),
            family_id: "family".to_string(),
            scale_id: "small".to_string(),
            measurement_id: "main".to_string(),
            max_relative_ratio: "1.251".to_string(),
            justification: "A source-owned reason longer than sixteen bytes.".to_string(),
        });
        assert!(matches!(
            validate_policy(&invalid),
            Err(SelfRegressionError::InvalidException)
        ));
        let exception = invalid.exceptions.first_mut().expect("first exception");
        exception.max_relative_ratio = "1.20".to_string();
        exception.justification = "short".to_string();
        assert!(matches!(
            validate_policy(&invalid),
            Err(SelfRegressionError::InvalidException)
        ));
    }

    #[test]
    fn policy_requires_the_exact_default_regression_tolerance() {
        assert!(validate_policy(&policy("1.15")).is_ok());
        assert!(matches!(
            validate_policy(&policy("1.149")),
            Err(SelfRegressionError::InvalidTolerance(value)) if value == 1.149
        ));
    }

    #[test]
    fn workload_digest_material_excludes_source_and_profiler_identity() {
        let material = WorkloadDigestMaterial {
            group_id: "group",
            workload_id: "workload",
            family_id: "family",
            scale_id: "family-small",
            size_class: SizeClass::Small,
            work_items: 64,
            input_digest: "a",
            timing_batch_policy: crate::qualification::model::TimingBatchPolicy::CommonIterations,
            timing_boundary: TimingBoundary::RawWorkV2,
            comparator_sources: &[("comparator.cc".to_string(), "b".repeat(64))],
        };
        let value = serde_json::to_value(material).expect("workload digest material");
        assert!(value.get("group_contract_sha256").is_none());
        assert!(value.get("profiler_note").is_none());
        assert!(value.get("stab_commit").is_none());
    }

    #[test]
    fn baselines_reject_duplicates_and_stale_identities() {
        let mut duplicated = baseline(entry());
        let first_baseline = duplicated.entries.first().expect("first baseline").clone();
        duplicated.entries.push(first_baseline);
        assert!(matches!(
            validate_baselines(&duplicated, &"e".repeat(64)),
            Err(SelfRegressionError::BaselineIdentity)
        ));
        let mut stale = baseline(entry());
        stale
            .entries
            .first_mut()
            .expect("first baseline")
            .key
            .workload_contract_sha256 = "stale".to_string();
        assert!(matches!(
            validate_baselines(&stale, &"e".repeat(64)),
            Err(SelfRegressionError::BaselineIdentity)
        ));
        assert!(matches!(
            validate_baselines(&baseline(entry()), &"1".repeat(64)),
            Err(SelfRegressionError::BaselineIdentity)
        ));
        let mut inverted = baseline(entry());
        let entry = inverted.entries.first_mut().expect("first baseline");
        entry.accepted_median_ratio = "1.1".to_string();
        entry.accepted_confidence_interval_upper = "1.0".to_string();
        assert!(matches!(
            validate_baselines(&inverted, &"e".repeat(64)),
            Err(SelfRegressionError::BaselineIdentity)
        ));
    }

    #[test]
    fn source_targets_must_match_an_active_release_contract() {
        let mut accepted = entry();
        accepted.key.stim_commit = crate::config::STIM_COMMIT.to_string();
        assert!(
            validate_source_targets(&policy("1.15"), &baseline(accepted.clone()), &[contract()])
                .is_ok()
        );

        let mut stale = accepted.clone();
        stale.key.family_id = "retired-family".to_string();
        assert!(matches!(
            validate_source_targets(&policy("1.15"), &baseline(stale), &[contract()]),
            Err(SelfRegressionError::BaselineIdentity)
        ));

        let mut diagnostic = contract();
        diagnostic.claim_class = super::super::run::ClaimClass::DiagnosticInfrastructure;
        diagnostic.parity_eligibility = super::super::group::ParityEligibility::ReportOnly;
        assert!(matches!(
            validate_source_targets(&policy("1.15"), &baseline(accepted), &[diagnostic]),
            Err(SelfRegressionError::BaselineIdentity)
        ));
    }

    #[test]
    fn regression_exceptions_cannot_target_retired_measurements() {
        let mut with_exception = policy("1.15");
        with_exception.exceptions.push(RegressionException {
            group_id: "group".to_string(),
            family_id: "family".to_string(),
            scale_id: "family-small".to_string(),
            measurement_id: "retired-measurement".to_string(),
            max_relative_ratio: "1.20".to_string(),
            justification: "A committed source-owned exception for a noisy workload.".to_string(),
        });
        assert!(matches!(
            validate_source_targets(
                &with_exception,
                &RegressionBaselineFile {
                    schema_version: REGRESSION_BASELINE_SCHEMA_VERSION,
                    performance_inventory_sha256: "e".repeat(64),
                    entries: Vec::new(),
                },
                &[contract()]
            ),
            Err(SelfRegressionError::InvalidException)
        ));
    }

    #[test]
    fn candidate_values_choose_the_worse_full_or_soak_statistic() {
        let current = vec![CurrentMeasurement {
            key: key(),
            median_ratio: 1.0,
            confidence_interval_upper: 1.1,
        }];
        let first = candidate_entries(current.clone(), &"f".repeat(64), &"0".repeat(64));
        let second = candidate_entries(current, &"f".repeat(64), &"0".repeat(64));
        assert_eq!(
            serde_json::to_vec(&first).expect("first candidate"),
            serde_json::to_vec(&second).expect("second candidate")
        );
        let entry = first.first().expect("first candidate entry");
        assert_eq!(entry.accepted_median_ratio, "1.00000000000000000");
        assert_eq!(
            entry.accepted_confidence_interval_upper,
            "1.10000000000000009"
        );
    }

    #[test]
    fn missing_or_identity_mismatched_baselines_are_unseeded() {
        let current = vec![CurrentMeasurement {
            key: key(),
            median_ratio: 1.0,
            confidence_interval_upper: 1.0,
        }];
        let empty = RegressionBaselineFile {
            schema_version: REGRESSION_BASELINE_SCHEMA_VERSION,
            performance_inventory_sha256: "e".repeat(64),
            entries: Vec::new(),
        };
        let summary = evaluate_current("group".to_string(), &current, &policy("1.15"), &empty)
            .expect("unseeded evaluation");
        assert_eq!(summary.outcome, SelfRegressionOutcome::Unseeded);
        assert_eq!(summary.unseeded_measurements, 1);

        let mut mismatched = entry();
        mismatched.key.cpu_identity = "other-cpu".to_string();
        let summary = evaluate_current(
            "group".to_string(),
            &current,
            &policy("1.15"),
            &baseline(mismatched),
        )
        .expect("identity mismatch is unseeded");
        assert_eq!(summary.outcome, SelfRegressionOutcome::Unseeded);
    }
}
