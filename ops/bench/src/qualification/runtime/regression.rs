use std::path::PathBuf;

use clap::Args;
use serde::Deserialize;
use thiserror::Error;

use super::run::{ClaimClass, QualificationReport};
use crate::root::RepoRoot;

const BASELINE_SCHEMA_VERSION: u32 = 1;
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
    groups: Vec<RegressionRule>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegressionRule {
    group_id: String,
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
    validate_baseline(&baseline)?;
    let report_bytes = super::artifact::read_artifact(root, &args.input, "report.json")?;
    let report: QualificationReport =
        serde_json::from_slice(&report_bytes).map_err(RegressionError::ReportJson)?;
    super::report::validate_report(&report)?;
    if baseline.performance_inventory_sha256 != report.performance_inventory_sha256 {
        return Err(RegressionError::InventoryMismatch {
            baseline: baseline.performance_inventory_sha256,
            report: report.performance_inventory_sha256,
        });
    }
    let selected = baseline
        .groups
        .iter()
        .filter(|rule| rule.group_id == report.group_id)
        .collect::<Vec<_>>();
    match rule_disposition(
        report.claim_class,
        report.promotable,
        selected.is_empty(),
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
    let mut checked = 0;
    for rule in selected {
        let summary = report
            .statistics
            .iter()
            .find(|summary| summary.measurement_id.to_string() == rule.measurement_id)
            .ok_or_else(|| RegressionError::MissingMeasurement(rule.measurement_id.clone()))?;
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

pub(super) fn check_baseline(
    root: &RepoRoot,
    expected_inventory_sha256: &str,
) -> Result<(), RegressionError> {
    let path = root.path.join("benchmarks/qualification-baseline.json");
    let bytes = crate::source_file::read_repo_regular_file_bounded(root, &path, MAX_BASELINE_BYTES)
        .map_err(|error| RegressionError::BaselineRead(error.to_string()))?;
    let baseline: RegressionBaseline =
        serde_json::from_slice(&bytes).map_err(RegressionError::BaselineJson)?;
    validate_baseline(&baseline)?;
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
    rules_empty: bool,
    group_id: &str,
) -> Result<RuleDisposition, RegressionError> {
    match (claim_class, promotable, rules_empty) {
        (ClaimClass::DiagnosticInfrastructure, false, true) => Ok(RuleDisposition::ReportOnly),
        (ClaimClass::DiagnosticInfrastructure, _, false) => Err(
            RegressionError::DiagnosticCannotBeGated(group_id.to_string()),
        ),
        (ClaimClass::PromotablePerformance, true, false) => Ok(RuleDisposition::Gated),
        (ClaimClass::PromotablePerformance, _, true) => {
            Err(RegressionError::MissingRule(group_id.to_string()))
        }
        (ClaimClass::PromotablePerformance, false, false) => Err(
            RegressionError::DiagnosticCannotBeGated(group_id.to_string()),
        ),
        (ClaimClass::DiagnosticInfrastructure, true, true) => Err(
            RegressionError::DiagnosticCannotBeGated(group_id.to_string()),
        ),
    }
}

fn validate_baseline(baseline: &RegressionBaseline) -> Result<(), RegressionError> {
    if baseline.schema_version != BASELINE_SCHEMA_VERSION {
        return Err(RegressionError::SchemaVersion {
            actual: baseline.schema_version,
            expected: BASELINE_SCHEMA_VERSION,
        });
    }
    if !valid_sha256(&baseline.performance_inventory_sha256) {
        return Err(RegressionError::InvalidInventoryDigest);
    }
    let mut keys = std::collections::BTreeSet::new();
    for rule in &baseline.groups {
        if rule.group_id.is_empty()
            || rule.measurement_id.is_empty()
            || !keys.insert((&rule.group_id, &rule.measurement_id))
        {
            return Err(RegressionError::DuplicateOrInvalidRule);
        }
        parse_ratio("max_median_ratio", &rule.max_median_ratio)?;
        parse_ratio(
            "max_confidence_interval_upper",
            &rule.max_confidence_interval_upper,
        )?;
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
    #[error("qualification baseline field {field} has invalid ratio {value:?}")]
    InvalidRatio { field: &'static str, value: String },
    #[error("qualification baseline inventory {baseline} differs from report inventory {report}")]
    InventoryMismatch { baseline: String, report: String },
    #[error("promotable qualification group {0} has no regression rule")]
    MissingRule(String),
    #[error("diagnostic qualification group {0} cannot be consumed as timing-gate evidence")]
    DiagnosticCannotBeGated(String),
    #[error("qualification report omits threshold measurement {0}")]
    MissingMeasurement(String),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_rejects_duplicate_rules_and_invalid_ratios() {
        let rule = RegressionRule {
            group_id: "group".to_string(),
            measurement_id: "main".to_string(),
            max_median_ratio: "1.25".to_string(),
            max_confidence_interval_upper: "1.25".to_string(),
        };
        let baseline = RegressionBaseline {
            schema_version: BASELINE_SCHEMA_VERSION,
            performance_inventory_sha256: "a".repeat(64),
            groups: vec![rule.clone(), rule],
        };
        assert!(validate_baseline(&baseline).is_err());
        assert!(parse_ratio("ratio", "NaN").is_err());
        assert!(parse_ratio("ratio", "0").is_err());
    }

    #[test]
    fn diagnostic_evidence_can_be_report_only_but_never_thresholded() {
        assert_eq!(
            rule_disposition(ClaimClass::DiagnosticInfrastructure, false, true, "group")
                .expect("diagnostic report-only disposition"),
            RuleDisposition::ReportOnly
        );
        assert!(
            rule_disposition(ClaimClass::DiagnosticInfrastructure, false, false, "group").is_err()
        );
        assert!(
            rule_disposition(ClaimClass::PromotablePerformance, false, false, "group").is_err()
        );
    }
}
