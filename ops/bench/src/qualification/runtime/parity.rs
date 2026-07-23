use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use clap::Args;
use serde::Deserialize;
use thiserror::Error;

use super::group::ParityEligibility;
use super::run::ClaimClass;
use super::statistics::GateOutcome;
use super::{artifact::DirectQualificationArtifactPath, artifact::RepositoryBinding};
use crate::root::RepoRoot;

const PARITY_POLICY_SCHEMA_VERSION: u32 = 2;
const MAX_PARITY_POLICY_BYTES: usize = 4 << 20;
pub(super) const MAX_PARITY_RATIO: f64 = 1.25;
pub(super) const DEFAULT_PARITY_POLICY: &str = "benchmarks/qualification-parity-policy.json";

#[derive(Clone, Debug, Args)]
pub(crate) struct ParityArgs {
    /// Published qualification directory to evaluate.
    #[arg(long, default_value = "target/benchmarks/qualification/latest")]
    input: PathBuf,

    /// Source-owned Stim parity policy.
    #[arg(long, visible_alias = "baseline", default_value = DEFAULT_PARITY_POLICY)]
    policy: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParityPolicy {
    schema_version: u32,
    performance_inventory_sha256: String,
    groups: Vec<ParityGroup>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParityGroup {
    group_id: String,
    parity_eligibility: ParityEligibility,
    measurements: Vec<ParityRule>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParityRule {
    measurement_id: String,
    max_median_ratio: String,
    max_confidence_interval_upper: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParitySummary {
    pub(crate) group_id: String,
    pub(crate) checked_measurements: usize,
    pub(crate) report_only: bool,
}

pub(super) fn run_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    input: &DirectQualificationArtifactPath,
) -> Result<ParitySummary, ParityError> {
    run_with_repository_and_policy(
        root,
        source_root,
        repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        input,
        Path::new(DEFAULT_PARITY_POLICY),
    )
}

pub(super) fn run_args_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: ParityArgs,
) -> Result<ParitySummary, ParityError> {
    let input = DirectQualificationArtifactPath::try_new(&args.input)?;
    run_with_repository_and_policy(
        root,
        source_root,
        repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        &input,
        &args.policy,
    )
}

pub(super) fn run_with_repository_and_policy(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    input: &DirectQualificationArtifactPath,
    policy: &Path,
) -> Result<ParitySummary, ParityError> {
    let policy = load_checked_policy(source_root, expected_performance_inventory_sha256, policy)?;
    let evidence = super::report::load_validated_published_evidence(
        root,
        source_root,
        repository,
        input,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?;
    evaluate_report_with_policy(&evidence.report, &policy)
}

pub(super) fn evaluate_report(
    source_root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
    report: &super::run::QualificationReport,
) -> Result<ParitySummary, ParityError> {
    let policy = load_checked_policy(
        source_root,
        expected_performance_inventory_sha256,
        Path::new(DEFAULT_PARITY_POLICY),
    )?;
    evaluate_report_with_policy(report, &policy)
}

fn evaluate_report_with_policy(
    report: &super::run::QualificationReport,
    policy: &ParityPolicy,
) -> Result<ParitySummary, ParityError> {
    if policy.performance_inventory_sha256 != report.performance_inventory_sha256 {
        return Err(ParityError::InventoryMismatch {
            policy: policy.performance_inventory_sha256.clone(),
            report: report.performance_inventory_sha256.clone(),
        });
    }
    let selected = policy
        .groups
        .iter()
        .find(|group| group.group_id == report.group_id)
        .ok_or_else(|| ParityError::MissingGroup(report.group_id.clone()))?;
    match rule_disposition(
        report.claim_class,
        report.promotable,
        selected.parity_eligibility,
        &report.group_id,
    )? {
        RuleDisposition::ReportOnly => {
            return Ok(ParitySummary {
                group_id: report.group_id.clone(),
                checked_measurements: 0,
                report_only: true,
            });
        }
        RuleDisposition::Gated => {}
    }
    let authoritative = super::report::authoritative_timing_attempt(report)?;
    let mut checked = 0;
    for rule in &selected.measurements {
        let summary = authoritative
            .statistics
            .iter()
            .find(|summary| summary.measurement_id.to_string() == rule.measurement_id)
            .ok_or_else(|| ParityError::MissingMeasurement(rule.measurement_id.clone()))?;
        require_passed_outcome(&rule.measurement_id, summary.outcome)?;
        let maximum_median = parse_ratio("max_median_ratio", &rule.max_median_ratio)?;
        let maximum_upper = parse_ratio(
            "max_confidence_interval_upper",
            &rule.max_confidence_interval_upper,
        )?;
        if summary.median_ratio > maximum_median
            || summary.confidence_interval_upper > maximum_upper
        {
            return Err(ParityError::ThresholdExceeded {
                measurement_id: rule.measurement_id.clone(),
                median: summary.median_ratio,
                maximum_median,
                upper: summary.confidence_interval_upper,
                maximum_upper,
            });
        }
        checked += 1;
    }
    Ok(ParitySummary {
        group_id: report.group_id.clone(),
        checked_measurements: checked,
        report_only: false,
    })
}

fn load_checked_policy(
    source_root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
    policy_path: &Path,
) -> Result<ParityPolicy, ParityError> {
    let policy_path = source_root.resolve_relative(policy_path);
    let policy_bytes = crate::source_file::read_repo_regular_file_bounded(
        source_root,
        &policy_path,
        MAX_PARITY_POLICY_BYTES,
    )
    .map_err(|error| ParityError::PolicyRead(error.to_string()))?;
    let policy: ParityPolicy =
        serde_json::from_slice(&policy_bytes).map_err(ParityError::PolicyJson)?;
    let contracts = super::group::load_groups(source_root, expected_performance_inventory_sha256)?;
    validate_policy(&policy, &contracts)?;
    if policy.performance_inventory_sha256 != expected_performance_inventory_sha256 {
        return Err(ParityError::InventoryMismatch {
            policy: policy.performance_inventory_sha256,
            report: expected_performance_inventory_sha256.to_string(),
        });
    }
    Ok(policy)
}

fn require_passed_outcome(measurement_id: &str, outcome: GateOutcome) -> Result<(), ParityError> {
    if outcome == GateOutcome::Passed {
        Ok(())
    } else {
        Err(ParityError::UnacceptableOutcome {
            measurement_id: measurement_id.to_string(),
            outcome,
        })
    }
}

pub(super) fn check_policy(
    root: &RepoRoot,
    expected_inventory_sha256: &str,
) -> Result<(), ParityError> {
    let path = root.path.join(DEFAULT_PARITY_POLICY);
    let bytes =
        crate::source_file::read_repo_regular_file_bounded(root, &path, MAX_PARITY_POLICY_BYTES)
            .map_err(|error| ParityError::PolicyRead(error.to_string()))?;
    let policy: ParityPolicy = serde_json::from_slice(&bytes).map_err(ParityError::PolicyJson)?;
    let contracts = super::group::load_groups(root, expected_inventory_sha256)?;
    validate_policy(&policy, &contracts)?;
    if policy.performance_inventory_sha256 != expected_inventory_sha256 {
        return Err(ParityError::InventoryMismatch {
            policy: policy.performance_inventory_sha256,
            report: expected_inventory_sha256.to_string(),
        });
    }
    Ok(())
}

pub(super) fn policy_sha256(
    root: &RepoRoot,
    expected_inventory_sha256: &str,
) -> Result<String, ParityError> {
    check_policy(root, expected_inventory_sha256)?;
    let path = root.path.join(DEFAULT_PARITY_POLICY);
    let bytes =
        crate::source_file::read_repo_regular_file_bounded(root, &path, MAX_PARITY_POLICY_BYTES)
            .map_err(|error| ParityError::PolicyRead(error.to_string()))?;
    Ok(super::run::sha256_hex(&bytes))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuleDisposition {
    ReportOnly,
    Gated,
}

fn rule_disposition(
    claim_class: ClaimClass,
    promotable: bool,
    parity_eligibility: ParityEligibility,
    group_id: &str,
) -> Result<RuleDisposition, ParityError> {
    match (claim_class, promotable, parity_eligibility) {
        (ClaimClass::DiagnosticInfrastructure, false, ParityEligibility::ReportOnly) => {
            Ok(RuleDisposition::ReportOnly)
        }
        (ClaimClass::PromotablePerformance, true, ParityEligibility::ThresholdEligible) => {
            Ok(RuleDisposition::Gated)
        }
        _ => Err(ParityError::DispositionMismatch(group_id.to_string())),
    }
}

fn validate_policy(
    policy: &ParityPolicy,
    contracts: &[super::group::GroupContract],
) -> Result<(), ParityError> {
    if policy.schema_version != PARITY_POLICY_SCHEMA_VERSION {
        return Err(ParityError::SchemaVersion {
            actual: policy.schema_version,
            expected: PARITY_POLICY_SCHEMA_VERSION,
        });
    }
    if !valid_sha256(&policy.performance_inventory_sha256) {
        return Err(ParityError::InvalidInventoryDigest);
    }
    let contract_by_id = contracts
        .iter()
        .map(|contract| (contract.id.to_string(), contract))
        .collect::<BTreeMap<_, _>>();
    if policy.groups.len() != contract_by_id.len() {
        return Err(ParityError::PolicyContractMismatch);
    }
    let mut group_ids = BTreeSet::new();
    for group in &policy.groups {
        let contract = contract_by_id
            .get(&group.group_id)
            .ok_or(ParityError::PolicyContractMismatch)?;
        if !group_ids.insert(&group.group_id)
            || group.parity_eligibility != contract.parity_eligibility
        {
            return Err(ParityError::PolicyContractMismatch);
        }
        let expected_measurements = contract
            .measurement_ids
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        let mut observed_measurements = BTreeSet::new();
        for rule in &group.measurements {
            if rule.measurement_id.is_empty()
                || !observed_measurements.insert(rule.measurement_id.clone())
            {
                return Err(ParityError::DuplicateOrInvalidRule);
            }
            parse_ratio("max_median_ratio", &rule.max_median_ratio)?;
            parse_ratio(
                "max_confidence_interval_upper",
                &rule.max_confidence_interval_upper,
            )?;
        }
        match group.parity_eligibility {
            ParityEligibility::ReportOnly if group.measurements.is_empty() => {}
            ParityEligibility::ThresholdEligible
                if observed_measurements == expected_measurements => {}
            _ => return Err(ParityError::PolicyContractMismatch),
        }
    }
    Ok(())
}

fn parse_ratio(field: &'static str, value: &str) -> Result<f64, ParityError> {
    let ratio = value
        .parse::<f64>()
        .map_err(|_| ParityError::InvalidRatio {
            field,
            value: value.to_string(),
        })?;
    if ratio.is_finite() && ratio > 0.0 && ratio <= MAX_PARITY_RATIO {
        Ok(ratio)
    } else {
        Err(ParityError::InvalidRatio {
            field,
            value: value.to_string(),
        })
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Error)]
pub(super) enum ParityError {
    #[error("failed to read the source-owned qualification parity policy: {0}")]
    PolicyRead(String),
    #[error("qualification parity policy JSON is invalid: {0}")]
    PolicyJson(serde_json::Error),
    #[error("qualification parity policy schema is {actual}, expected {expected}")]
    SchemaVersion { actual: u32, expected: u32 },
    #[error("qualification parity policy inventory digest is invalid")]
    InvalidInventoryDigest,
    #[error("qualification parity policy repeats or invalidates a group measurement rule")]
    DuplicateOrInvalidRule,
    #[error("qualification parity policy does not exactly match the runtime group contract")]
    PolicyContractMismatch,
    #[error("qualification parity policy field {field} has invalid ratio {value:?}")]
    InvalidRatio { field: &'static str, value: String },
    #[error(
        "qualification parity policy inventory {policy} differs from report inventory {report}"
    )]
    InventoryMismatch { policy: String, report: String },
    #[error("qualification parity policy omits runtime group {0}")]
    MissingGroup(String),
    #[error("qualification group {0} has an incompatible claim and parity disposition")]
    DispositionMismatch(String),
    #[error("qualification report omits threshold measurement {0}")]
    MissingMeasurement(String),
    #[error("qualification measurement {measurement_id} has non-passing outcome {outcome:?}")]
    UnacceptableOutcome {
        measurement_id: String,
        outcome: GateOutcome,
    },
    #[error(
        "qualification measurement {measurement_id} exceeded Stim parity limits: median {median:.6} > {maximum_median:.6} or upper {upper:.6} > {maximum_upper:.6}"
    )]
    ThresholdExceeded {
        measurement_id: String,
        median: f64,
        maximum_median: f64,
        upper: f64,
        maximum_upper: f64,
    },
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),
    #[error(transparent)]
    Report(#[from] super::report::ReportError),
    #[error(transparent)]
    Group(#[from] super::group::GroupError),
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::super::protocol::ProtocolId;
    use super::*;

    fn diagnostic_contract() -> super::super::group::GroupContract {
        super::super::group::GroupContract {
            id: ProtocolId::try_new("group").expect("group id"),
            claim_class: ClaimClass::DiagnosticInfrastructure,
            parity_eligibility: ParityEligibility::ReportOnly,
            timing_batch_policy: crate::qualification::model::TimingBatchPolicy::CommonIterations,
            workload_id: ProtocolId::try_new("workload").expect("workload id"),
            measurement_ids: vec![ProtocolId::try_new("main").expect("measurement id")],
            scales: vec![super::super::group::ScaleContract {
                id: ProtocolId::try_new("default").expect("scale id"),
                family_id: ProtocolId::try_new("default").expect("family id"),
                size_class: crate::qualification::model::SizeClass::Small,
                work_items: NonZeroU64::new(1).expect("positive work"),
                input_bytes: 0,
                input_digest: super::super::protocol::InputDigest::try_new(
                    "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1",
                )
                .expect("empty input digest"),
            }],
            correctness_case_ids: Vec::new(),
            owner: ProtocolId::try_new("ops/bench").expect("owner"),
            profiler_note: None,
            comparator_sources: Vec::new(),
        }
    }

    #[test]
    fn parity_policy_rejects_duplicate_rules_and_invalid_ratios() {
        let rule = ParityRule {
            measurement_id: "main".to_string(),
            max_median_ratio: "1.25".to_string(),
            max_confidence_interval_upper: "1.25".to_string(),
        };
        let policy = ParityPolicy {
            schema_version: PARITY_POLICY_SCHEMA_VERSION,
            performance_inventory_sha256: "a".repeat(64),
            groups: vec![ParityGroup {
                group_id: "group".to_string(),
                parity_eligibility: ParityEligibility::ReportOnly,
                measurements: vec![rule.clone(), rule],
            }],
        };
        assert!(validate_policy(&policy, &[diagnostic_contract()]).is_err());
        assert!(parse_ratio("ratio", "NaN").is_err());
        assert!(parse_ratio("ratio", "0").is_err());
        assert!(parse_ratio("ratio", "1.2500001").is_err());
    }

    #[test]
    fn diagnostic_evidence_can_be_report_only_but_never_thresholded() {
        assert_eq!(
            rule_disposition(
                ClaimClass::DiagnosticInfrastructure,
                false,
                ParityEligibility::ReportOnly,
                "group"
            )
            .expect("diagnostic report-only disposition"),
            RuleDisposition::ReportOnly
        );
        assert!(
            rule_disposition(
                ClaimClass::DiagnosticInfrastructure,
                false,
                ParityEligibility::ThresholdEligible,
                "group"
            )
            .is_err()
        );
        assert!(
            rule_disposition(
                ClaimClass::PromotablePerformance,
                false,
                ParityEligibility::ThresholdEligible,
                "group"
            )
            .is_err()
        );
    }

    #[test]
    fn parity_policy_requires_one_exact_report_only_entry_for_diagnostics() {
        let contract = diagnostic_contract();
        let valid = ParityPolicy {
            schema_version: PARITY_POLICY_SCHEMA_VERSION,
            performance_inventory_sha256: "a".repeat(64),
            groups: vec![ParityGroup {
                group_id: "group".to_string(),
                parity_eligibility: ParityEligibility::ReportOnly,
                measurements: Vec::new(),
            }],
        };
        validate_policy(&valid, std::slice::from_ref(&contract))
            .expect("complete report-only parity policy");

        let mut missing = valid.clone();
        missing.groups.clear();
        assert!(matches!(
            validate_policy(&missing, std::slice::from_ref(&contract)),
            Err(ParityError::PolicyContractMismatch)
        ));

        let mut thresholded = valid;
        thresholded
            .groups
            .first_mut()
            .expect("group")
            .parity_eligibility = ParityEligibility::ThresholdEligible;
        assert!(matches!(
            validate_policy(&thresholded, &[contract]),
            Err(ParityError::PolicyContractMismatch)
        ));
    }

    #[test]
    fn gated_evidence_rejects_failed_and_noisy_authoritative_outcomes() {
        require_passed_outcome("main", GateOutcome::Passed).expect("passed outcome");
        assert!(matches!(
            require_passed_outcome("main", GateOutcome::Failed),
            Err(ParityError::UnacceptableOutcome { .. })
        ));
        assert!(matches!(
            require_passed_outcome("main", GateOutcome::Noisy),
            Err(ParityError::UnacceptableOutcome { .. })
        ));
    }
}
