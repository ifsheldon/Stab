use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::BenchError;
use crate::manifest::is_safe_benchmark_id;
use crate::report::CompareRowResult;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BenchmarkThresholds {
    schema_version: u32,
    rows: Vec<BenchmarkThresholdRow>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkThresholdRow {
    id: String,
    max_relative_ratio: f64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RegressionThresholdFindings {
    pub(crate) blockers: Vec<String>,
}

pub(crate) fn read_thresholds(path: &Path) -> Result<BenchmarkThresholds, BenchError> {
    let content = std::fs::read_to_string(path).map_err(|source| BenchError::ReadThresholds {
        path: path.to_path_buf(),
        source,
    })?;
    let thresholds = serde_json::from_str::<BenchmarkThresholds>(&content).map_err(|source| {
        BenchError::ParseThresholds {
            path: path.to_path_buf(),
            source,
        }
    })?;
    thresholds.validate(path)?;
    Ok(thresholds)
}

pub(crate) fn apply_regression_thresholds(
    rows: &mut [CompareRowResult],
    thresholds: &BenchmarkThresholds,
) -> RegressionThresholdFindings {
    let mut findings = RegressionThresholdFindings::default();
    for threshold in &thresholds.rows {
        let Some(row) = rows.iter_mut().find(|row| row.id == threshold.id) else {
            continue;
        };
        row.regression_threshold_status = "configured".to_string();
        row.regression_threshold_max_ratio = Some(threshold.max_relative_ratio);
        match row.relative_ratio {
            Some(ratio) if ratio <= threshold.max_relative_ratio => {
                row.regression_threshold_status = "pass".to_string();
            }
            Some(ratio) => {
                let message = format!(
                    "ratio {ratio:.3}x exceeds threshold {:.3}x",
                    threshold.max_relative_ratio
                );
                row.regression_threshold_status = "fail".to_string();
                row.regression_threshold_error = Some(message.clone());
                findings.blockers.push(format!("{}: {message}", row.id));
            }
            None => {
                let message = "threshold cannot be checked without a comparable ratio".to_string();
                row.regression_threshold_status = "not-comparable".to_string();
                row.regression_threshold_error = Some(message.clone());
                findings.blockers.push(format!("{}: {message}", row.id));
            }
        }
    }
    findings
}

impl BenchmarkThresholds {
    fn validate(&self, path: &Path) -> Result<(), BenchError> {
        let mut violations = Vec::new();
        if self.schema_version != 1 {
            violations.push(format!("schema_version={} expected 1", self.schema_version));
        }
        let mut ids = BTreeSet::new();
        for row in &self.rows {
            if row.id.is_empty() {
                violations.push("row with empty id".to_string());
            } else if !is_safe_benchmark_id(&row.id) {
                violations.push(format!("{} has unsafe id", row.id));
            } else if !ids.insert(row.id.clone()) {
                violations.push(format!("duplicate threshold row {}", row.id));
            }
            if !row.max_relative_ratio.is_finite() || row.max_relative_ratio <= 0.0 {
                violations.push(format!(
                    "{} has invalid max_relative_ratio {}",
                    row.id, row.max_relative_ratio
                ));
            }
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(BenchError::ThresholdValidation {
                path: PathBuf::from(path),
                details: violations.join("\n").into_boxed_str(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{BenchmarkThresholds, apply_regression_thresholds, read_thresholds};
    use crate::manifest::{Milestone, Runner};
    use crate::report::CompareRowResult;

    #[test]
    fn regression_thresholds_mark_pass_fail_and_uncomparable_rows() {
        let thresholds = serde_json::from_str::<BenchmarkThresholds>(
            r#"{
                "schema_version": 1,
                "rows": [
                    {"id": "pass-row", "max_relative_ratio": 1.25},
                    {"id": "fail-row", "max_relative_ratio": 1.25},
                    {"id": "missing-row", "max_relative_ratio": 1.25}
                ]
            }"#,
        )
        .expect("parse thresholds");
        let mut rows = vec![
            row("pass-row", Some(1.1)),
            row("fail-row", Some(1.4)),
            row("missing-row", None),
        ];

        let findings = apply_regression_thresholds(&mut rows, &thresholds);

        assert_eq!(
            rows.first().expect("pass row").regression_threshold_status,
            "pass"
        );
        assert_eq!(
            rows.get(1).expect("fail row").regression_threshold_status,
            "fail"
        );
        assert_eq!(
            rows.get(2)
                .expect("missing row")
                .regression_threshold_status,
            "not-comparable"
        );
        assert_eq!(
            findings.blockers,
            vec![
                "fail-row: ratio 1.400x exceeds threshold 1.250x",
                "missing-row: threshold cannot be checked without a comparable ratio",
            ]
        );
    }

    #[test]
    fn regression_thresholds_validate_schema_ids_and_ratios() {
        let thresholds = serde_json::from_str::<BenchmarkThresholds>(
            r#"{
                "schema_version": 2,
                "rows": [
                    {"id": "../bad", "max_relative_ratio": 1.25},
                    {"id": "bad-ratio", "max_relative_ratio": 0.0}
                ]
            }"#,
        )
        .expect("parse thresholds");

        let error = thresholds
            .validate(std::path::Path::new("thresholds.json"))
            .expect_err("reject invalid thresholds");

        let text = error.to_string();
        assert!(text.contains("schema_version=2 expected 1"));
        assert!(text.contains("../bad has unsafe id"));
        assert!(text.contains("bad-ratio has invalid max_relative_ratio 0"));
    }

    #[test]
    fn m12_primary_thresholds_validate_source_file() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmarks/m12-primary-thresholds.json");
        let thresholds = read_thresholds(&path).expect("read source-owned M12 thresholds");

        assert_eq!(thresholds.schema_version, 1);
        assert_eq!(thresholds.rows.len(), 63);
        assert!(
            thresholds
                .rows
                .iter()
                .all(|row| row.max_relative_ratio == 1.25)
        );
    }

    fn row(id: &str, ratio: Option<f64>) -> CompareRowResult {
        CompareRowResult {
            id: id.to_string(),
            milestone: Milestone::M12,
            threshold_class: "performance-gate".to_string(),
            runner: Runner::StimPerf,
            upstream_source: "future/performance-primary-matrix".to_string(),
            phase: "performance-hardening".to_string(),
            measurement: "primary-matrix".to_string(),
            status: "measured".to_string(),
            baseline_summary: String::new(),
            stab_summary: String::new(),
            note: None,
            stim_measurements: Vec::new(),
            stab_measurements: Vec::new(),
            stim_median_seconds: None,
            stab_median_seconds: None,
            relative_ratio: ratio,
            stab_allocation_count_max: None,
            stab_allocation_bytes_max: None,
            pass_fail_status: "not-comparable".to_string(),
            beta_gate_status: "not-checked".to_string(),
            beta_gate_waiver_reason: None,
            beta_gate_waiver_follow_up: None,
            beta_gate_error: None,
            memory_gate_status: "not-required".to_string(),
            memory_gate_baseline_bytes_max: None,
            memory_gate_allowed_bytes_max: None,
            memory_gate_error: None,
            regression_threshold_status: "not-configured".to_string(),
            regression_threshold_max_ratio: None,
            regression_threshold_error: None,
            profiler_note_status: "not-required".to_string(),
            profiler_note_path: None,
            profiler_note_error: None,
        }
    }
}
