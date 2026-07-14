use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use clap::Args;
use serde::Deserialize;
use thiserror::Error;

use super::group::BaselineEligibility;
use super::run::{ClaimClass, QualificationReport};
use super::statistics::GateOutcome;
use crate::root::RepoRoot;

const BASELINE_SCHEMA_VERSION: u32 = 2;
const MAX_BASELINE_BYTES: usize = 4 << 20;

#[derive(Clone, Debug, Args)]
pub(crate) struct RegressionArgs {
    /// Published qualification directory to evaluate.
    #[arg(long, default_value = "target/benchmarks/qualification/latest")]
    input: PathBuf,

    /// Source-owned regression baseline.
    #[arg(long, default_value = "benchmarks/qualification-baseline.json")]
    baseline: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegressionBaseline {
    schema_version: u32,
    performance_inventory_sha256: String,
    groups: Vec<RegressionGroup>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegressionGroup {
    group_id: String,
    baseline_eligibility: BaselineEligibility,
    measurements: Vec<RegressionRule>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegressionRule {
    measurement_id: String,
    max_median_ratio: String,
    max_confidence_interval_upper: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegressionSummary {
    pub(crate) group_id: String,
    pub(crate) checked_measurements: usize,
    pub(crate) report_only: bool,
}

pub(super) fn run(
    root: &RepoRoot,
    args: RegressionArgs,
) -> Result<RegressionSummary, RegressionError> {
    let baseline_path = root.resolve_relative(&args.baseline);
    let baseline_bytes = crate::source_file::read_repo_regular_file_bounded(
        root,
        &baseline_path,
        MAX_BASELINE_BYTES,
    )
    .map_err(|error| RegressionError::BaselineRead(error.to_string()))?;
    let baseline: RegressionBaseline =
        serde_json::from_slice(&baseline_bytes).map_err(RegressionError::BaselineJson)?;
    let contracts = super::group::load_groups(root, &baseline.performance_inventory_sha256)?;
    validate_baseline(&baseline, &contracts)?;
    let report_bytes = super::artifact::read_artifact(root, &args.input, "report.json")?;
    let report: QualificationReport =
        serde_json::from_slice(&report_bytes).map_err(RegressionError::ReportJson)?;
    super::report::validate_report(root, &report)?;
    if baseline.performance_inventory_sha256 != report.performance_inventory_sha256 {
        return Err(RegressionError::InventoryMismatch {
            baseline: baseline.performance_inventory_sha256,
            report: report.performance_inventory_sha256,
        });
    }
    let selected = baseline
        .groups
        .iter()
        .find(|group| group.group_id == report.group_id)
        .ok_or_else(|| RegressionError::MissingGroup(report.group_id.clone()))?;
    match rule_disposition(
        report.claim_class,
        report.promotable,
        selected.baseline_eligibility,
        &report.group_id,
    )? {
        RuleDisposition::ReportOnly => {
            return Ok(RegressionSummary {
                group_id: report.group_id,
                checked_measurements: 0,
                report_only: true,
            });
        }
        RuleDisposition::Gated => {}
    }
    let authoritative = super::report::authoritative_timing_attempt(&report)?;
    let mut checked = 0;
    for rule in &selected.measurements {
        let summary = authoritative
            .statistics
            .iter()
            .find(|summary| summary.measurement_id.to_string() == rule.measurement_id)
            .ok_or_else(|| RegressionError::MissingMeasurement(rule.measurement_id.clone()))?;
        require_passed_outcome(&rule.measurement_id, summary.outcome)?;
        let maximum_median = parse_ratio("max_median_ratio", &rule.max_median_ratio)?;
        let maximum_upper = parse_ratio(
            "max_confidence_interval_upper",
            &rule.max_confidence_interval_upper,
        )?;
        if summary.median_ratio > maximum_median
            || summary.confidence_interval_upper > maximum_upper
        {
            return Err(RegressionError::ThresholdExceeded {
                measurement_id: rule.measurement_id.clone(),
                median: summary.median_ratio,
                maximum_median,
                upper: summary.confidence_interval_upper,
                maximum_upper,
            });
        }
        checked += 1;
    }
    Ok(RegressionSummary {
        group_id: report.group_id,
        checked_measurements: checked,
        report_only: false,
    })
}

fn require_passed_outcome(
    measurement_id: &str,
    outcome: GateOutcome,
) -> Result<(), RegressionError> {
    if outcome == GateOutcome::Passed {
        Ok(())
    } else {
        Err(RegressionError::UnacceptableOutcome {
            measurement_id: measurement_id.to_string(),
            outcome,
        })
    }
}

pub(super) fn check_baseline(
    root: &RepoRoot,
    expected_inventory_sha256: &str,
) -> Result<(), RegressionError> {
    let path = root.path.join("benchmarks/qualification-baseline.json");
    let bytes = crate::source_file::read_repo_regular_file_bounded(root, &path, MAX_BASELINE_BYTES)
        .map_err(|error| RegressionError::BaselineRead(error.to_string()))?;
    let baseline: RegressionBaseline =
        serde_json::from_slice(&bytes).map_err(RegressionError::BaselineJson)?;
    let contracts = super::group::load_groups(root, expected_inventory_sha256)?;
    validate_baseline(&baseline, &contracts)?;
    if baseline.performance_inventory_sha256 != expected_inventory_sha256 {
        return Err(RegressionError::InventoryMismatch {
            baseline: baseline.performance_inventory_sha256,
            report: expected_inventory_sha256.to_string(),
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuleDisposition {
    ReportOnly,
    Gated,
}

fn rule_disposition(
    claim_class: ClaimClass,
    promotable: bool,
    baseline_eligibility: BaselineEligibility,
    group_id: &str,
) -> Result<RuleDisposition, RegressionError> {
    match (claim_class, promotable, baseline_eligibility) {
        (ClaimClass::DiagnosticInfrastructure, false, BaselineEligibility::ReportOnly) => {
            Ok(RuleDisposition::ReportOnly)
        }
        (ClaimClass::PromotablePerformance, true, BaselineEligibility::ThresholdEligible) => {
            Ok(RuleDisposition::Gated)
        }
        _ => Err(RegressionError::DispositionMismatch(group_id.to_string())),
    }
}

fn validate_baseline(
    baseline: &RegressionBaseline,
    contracts: &[super::group::GroupContract],
) -> Result<(), RegressionError> {
    if baseline.schema_version != BASELINE_SCHEMA_VERSION {
        return Err(RegressionError::SchemaVersion {
            actual: baseline.schema_version,
            expected: BASELINE_SCHEMA_VERSION,
        });
    }
    if !valid_sha256(&baseline.performance_inventory_sha256) {
        return Err(RegressionError::InvalidInventoryDigest);
    }
    let contract_by_id = contracts
        .iter()
        .map(|contract| (contract.id.to_string(), contract))
        .collect::<BTreeMap<_, _>>();
    if baseline.groups.len() != contract_by_id.len() {
        return Err(RegressionError::BaselineContractMismatch);
    }
    let mut group_ids = BTreeSet::new();
    for group in &baseline.groups {
        let contract = contract_by_id
            .get(&group.group_id)
            .ok_or(RegressionError::BaselineContractMismatch)?;
        if !group_ids.insert(&group.group_id)
            || group.baseline_eligibility != contract.baseline_eligibility
        {
            return Err(RegressionError::BaselineContractMismatch);
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
                return Err(RegressionError::DuplicateOrInvalidRule);
            }
            parse_ratio("max_median_ratio", &rule.max_median_ratio)?;
            parse_ratio(
                "max_confidence_interval_upper",
                &rule.max_confidence_interval_upper,
            )?;
        }
        match group.baseline_eligibility {
            BaselineEligibility::ReportOnly if group.measurements.is_empty() => {}
            BaselineEligibility::ThresholdEligible
                if observed_measurements == expected_measurements => {}
            _ => return Err(RegressionError::BaselineContractMismatch),
        }
    }
    Ok(())
}

fn parse_ratio(field: &'static str, value: &str) -> Result<f64, RegressionError> {
    let ratio = value
        .parse::<f64>()
        .map_err(|_| RegressionError::InvalidRatio {
            field,
            value: value.to_string(),
        })?;
    if ratio.is_finite() && ratio > 0.0 {
        Ok(ratio)
    } else {
        Err(RegressionError::InvalidRatio {
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
pub(super) enum RegressionError {
    #[error("failed to read the source-owned qualification baseline: {0}")]
    BaselineRead(String),
    #[error("qualification baseline JSON is invalid: {0}")]
    BaselineJson(serde_json::Error),
    #[error("qualification report JSON is invalid: {0}")]
    ReportJson(serde_json::Error),
    #[error("qualification baseline schema is {actual}, expected {expected}")]
    SchemaVersion { actual: u32, expected: u32 },
    #[error("qualification baseline inventory digest is invalid")]
    InvalidInventoryDigest,
    #[error("qualification baseline repeats or invalidates a group measurement rule")]
    DuplicateOrInvalidRule,
    #[error("qualification baseline does not exactly match the runtime group contract")]
    BaselineContractMismatch,
    #[error("qualification baseline field {field} has invalid ratio {value:?}")]
    InvalidRatio { field: &'static str, value: String },
    #[error("qualification baseline inventory {baseline} differs from report inventory {report}")]
    InventoryMismatch { baseline: String, report: String },
    #[error("qualification baseline omits runtime group {0}")]
    MissingGroup(String),
    #[error("qualification group {0} has an incompatible claim and baseline disposition")]
    DispositionMismatch(String),
    #[error("qualification report omits threshold measurement {0}")]
    MissingMeasurement(String),
    #[error("qualification measurement {measurement_id} has non-passing outcome {outcome:?}")]
    UnacceptableOutcome {
        measurement_id: String,
        outcome: GateOutcome,
    },
    #[error(
        "qualification measurement {measurement_id} exceeded regression limits: median {median:.6} > {maximum_median:.6} or upper {upper:.6} > {maximum_upper:.6}"
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
    use super::super::protocol::ProtocolId;
    use super::*;

    fn diagnostic_contract() -> super::super::group::GroupContract {
        super::super::group::GroupContract {
            id: ProtocolId::try_new("group").expect("group id"),
            claim_class: ClaimClass::DiagnosticInfrastructure,
            baseline_eligibility: BaselineEligibility::ReportOnly,
            workload_id: ProtocolId::try_new("workload").expect("workload id"),
            measurement_ids: vec![ProtocolId::try_new("main").expect("measurement id")],
            correctness_case_ids: Vec::new(),
        }
    }

    #[test]
    fn baseline_rejects_duplicate_rules_and_invalid_ratios() {
        let rule = RegressionRule {
            measurement_id: "main".to_string(),
            max_median_ratio: "1.25".to_string(),
            max_confidence_interval_upper: "1.25".to_string(),
        };
        let baseline = RegressionBaseline {
            schema_version: BASELINE_SCHEMA_VERSION,
            performance_inventory_sha256: "a".repeat(64),
            groups: vec![RegressionGroup {
                group_id: "group".to_string(),
                baseline_eligibility: BaselineEligibility::ReportOnly,
                measurements: vec![rule.clone(), rule],
            }],
        };
        assert!(validate_baseline(&baseline, &[diagnostic_contract()]).is_err());
        assert!(parse_ratio("ratio", "NaN").is_err());
        assert!(parse_ratio("ratio", "0").is_err());
    }

    #[test]
    fn diagnostic_evidence_can_be_report_only_but_never_thresholded() {
        assert_eq!(
            rule_disposition(
                ClaimClass::DiagnosticInfrastructure,
                false,
                BaselineEligibility::ReportOnly,
                "group"
            )
            .expect("diagnostic report-only disposition"),
            RuleDisposition::ReportOnly
        );
        assert!(
            rule_disposition(
                ClaimClass::DiagnosticInfrastructure,
                false,
                BaselineEligibility::ThresholdEligible,
                "group"
            )
            .is_err()
        );
        assert!(
            rule_disposition(
                ClaimClass::PromotablePerformance,
                false,
                BaselineEligibility::ThresholdEligible,
                "group"
            )
            .is_err()
        );
    }

    #[test]
    fn baseline_requires_one_exact_report_only_entry_for_diagnostics() {
        let contract = diagnostic_contract();
        let valid = RegressionBaseline {
            schema_version: BASELINE_SCHEMA_VERSION,
            performance_inventory_sha256: "a".repeat(64),
            groups: vec![RegressionGroup {
                group_id: "group".to_string(),
                baseline_eligibility: BaselineEligibility::ReportOnly,
                measurements: Vec::new(),
            }],
        };
        validate_baseline(&valid, std::slice::from_ref(&contract))
            .expect("complete report-only baseline");

        let mut missing = valid.clone();
        missing.groups.clear();
        assert!(matches!(
            validate_baseline(&missing, std::slice::from_ref(&contract)),
            Err(RegressionError::BaselineContractMismatch)
        ));

        let mut thresholded = valid;
        thresholded
            .groups
            .first_mut()
            .expect("group")
            .baseline_eligibility = BaselineEligibility::ThresholdEligible;
        assert!(matches!(
            validate_baseline(&thresholded, &[contract]),
            Err(RegressionError::BaselineContractMismatch)
        ));
    }

    #[test]
    fn gated_evidence_rejects_failed_and_noisy_authoritative_outcomes() {
        require_passed_outcome("main", GateOutcome::Passed).expect("passed outcome");
        assert!(matches!(
            require_passed_outcome("main", GateOutcome::Failed),
            Err(RegressionError::UnacceptableOutcome { .. })
        ));
        assert!(matches!(
            require_passed_outcome("main", GateOutcome::Noisy),
            Err(RegressionError::UnacceptableOutcome { .. })
        ));
    }
}
