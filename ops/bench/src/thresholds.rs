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
    #[serde(default)]
    max_relative_ratio: Option<f64>,
    #[serde(default)]
    measurement_thresholds: Vec<BenchmarkMeasurementThreshold>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkMeasurementThreshold {
    stim_name: String,
    stab_name: String,
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
            findings.blockers.push(format!(
                "{}: threshold row is not selected by the compare run",
                threshold.id
            ));
            continue;
        };
        let mut row_errors = Vec::new();
        row.regression_threshold_status = "configured".to_string();
        row.regression_threshold_max_ratio = threshold.max_relative_ratio;
        if let Some(max_relative_ratio) = threshold.max_relative_ratio {
            match row.relative_ratio {
                Some(ratio) if ratio <= max_relative_ratio => {}
                Some(ratio) => {
                    row_errors.push(format!(
                        "ratio {ratio:.3}x exceeds threshold {max_relative_ratio:.3}x"
                    ));
                }
                None => {
                    row_errors
                        .push("threshold cannot be checked without a comparable ratio".to_string());
                }
            }
        }
        for measurement_threshold in &threshold.measurement_thresholds {
            let Some(measurement_ratio) = row.measurement_ratios.iter().find(|ratio| {
                ratio.stim_name == measurement_threshold.stim_name
                    && ratio.stab_name == measurement_threshold.stab_name
            }) else {
                row_errors.push(format!(
                    "measurement threshold {} -> {} has no paired measurement ratio",
                    measurement_threshold.stim_name, measurement_threshold.stab_name
                ));
                continue;
            };
            if measurement_ratio.relative_ratio > measurement_threshold.max_relative_ratio {
                row_errors.push(format!(
                    "measurement {} -> {} ratio {:.3}x exceeds threshold {:.3}x",
                    measurement_threshold.stim_name,
                    measurement_threshold.stab_name,
                    measurement_ratio.relative_ratio,
                    measurement_threshold.max_relative_ratio
                ));
            }
        }
        if row_errors.is_empty() {
            row.regression_threshold_status = "pass".to_string();
        } else if row.relative_ratio.is_none() && threshold.max_relative_ratio.is_some() {
            row.regression_threshold_status = "not-comparable".to_string();
            let message = row_errors.join("; ");
            row.regression_threshold_error = Some(message.clone());
            findings.blockers.push(format!("{}: {message}", row.id));
        } else {
            row.regression_threshold_status = "fail".to_string();
            let message = row_errors.join("; ");
            row.regression_threshold_error = Some(message.clone());
            findings.blockers.push(format!("{}: {message}", row.id));
        }
    }
    findings
}

impl BenchmarkThresholds {
    fn validate(&self, path: &Path) -> Result<(), BenchError> {
        let mut violations = Vec::new();
        if !matches!(self.schema_version, 1 | 2) {
            violations.push(format!(
                "schema_version={} expected 1 or 2",
                self.schema_version
            ));
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
            if self.schema_version == 1 && !row.measurement_thresholds.is_empty() {
                violations.push(format!(
                    "{} uses measurement_thresholds but schema_version 1 only supports row-level thresholds",
                    row.id
                ));
            }
            if row.max_relative_ratio.is_none() && row.measurement_thresholds.is_empty() {
                violations.push(format!(
                    "{} must define max_relative_ratio or measurement_thresholds",
                    row.id
                ));
            }
            if let Some(max_relative_ratio) = row.max_relative_ratio
                && (!max_relative_ratio.is_finite() || max_relative_ratio <= 0.0)
            {
                violations.push(format!(
                    "{} has invalid max_relative_ratio {}",
                    row.id, max_relative_ratio
                ));
            }
            let mut measurement_ids = BTreeSet::new();
            for measurement in &row.measurement_thresholds {
                if measurement.stim_name.is_empty() {
                    violations.push(format!(
                        "{} has measurement threshold with empty stim_name",
                        row.id
                    ));
                }
                if measurement.stab_name.is_empty() {
                    violations.push(format!(
                        "{} has measurement threshold with empty stab_name",
                        row.id
                    ));
                }
                let measurement_id = (&measurement.stim_name, &measurement.stab_name);
                if !measurement_ids.insert(measurement_id) {
                    violations.push(format!(
                        "{} has duplicate measurement threshold {} -> {}",
                        row.id, measurement.stim_name, measurement.stab_name
                    ));
                }
                if !measurement.max_relative_ratio.is_finite()
                    || measurement.max_relative_ratio <= 0.0
                {
                    violations.push(format!(
                        "{} measurement {} -> {} has invalid max_relative_ratio {}",
                        row.id,
                        measurement.stim_name,
                        measurement.stab_name,
                        measurement.max_relative_ratio
                    ));
                }
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
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::{BenchmarkThresholds, apply_regression_thresholds, read_thresholds};
    use crate::comparability::ComparabilityClass;
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
    fn regression_thresholds_reject_unselected_threshold_rows() {
        let thresholds = serde_json::from_str::<BenchmarkThresholds>(
            r#"{
                "schema_version": 1,
                "rows": [
                    {"id": "pass-row", "max_relative_ratio": 1.25},
                    {"id": "stale-row", "max_relative_ratio": 1.25}
                ]
            }"#,
        )
        .expect("parse thresholds");
        let mut rows = vec![row("pass-row", Some(1.1))];

        let findings = apply_regression_thresholds(&mut rows, &thresholds);

        assert_eq!(
            rows.first().expect("pass row").regression_threshold_status,
            "pass"
        );
        assert_eq!(
            findings.blockers,
            vec!["stale-row: threshold row is not selected by the compare run"]
        );
    }

    #[test]
    fn regression_thresholds_check_schema2_measurement_thresholds() {
        let thresholds = serde_json::from_str::<BenchmarkThresholds>(
            r#"{
                "schema_version": 2,
                "rows": [
                    {
                        "id": "pass-row",
                        "measurement_thresholds": [
                            {
                                "stim_name": "stim_fast",
                                "stab_name": "stab_fast",
                                "max_relative_ratio": 1.25
                            }
                        ]
                    },
                    {
                        "id": "fail-row",
                        "measurement_thresholds": [
                            {
                                "stim_name": "stim_slow",
                                "stab_name": "stab_slow",
                                "max_relative_ratio": 1.25
                            }
                        ]
                    },
                    {
                        "id": "missing-pair-row",
                        "measurement_thresholds": [
                            {
                                "stim_name": "stim_missing",
                                "stab_name": "stab_missing",
                                "max_relative_ratio": 1.25
                            }
                        ]
                    }
                ]
            }"#,
        )
        .expect("parse thresholds");
        let mut rows = vec![
            row_with_measurement_ratio("pass-row", "stim_fast", "stab_fast", 1.1),
            row_with_measurement_ratio("fail-row", "stim_slow", "stab_slow", 1.4),
            row("missing-pair-row", Some(1.0)),
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
                .expect("missing pair row")
                .regression_threshold_status,
            "fail"
        );
        assert_eq!(
            findings.blockers,
            vec![
                "fail-row: measurement stim_slow -> stab_slow ratio 1.400x exceeds threshold 1.250x",
                "missing-pair-row: measurement threshold stim_missing -> stab_missing has no paired measurement ratio",
            ]
        );
    }

    #[test]
    fn regression_thresholds_validate_schema_ids_and_ratios() {
        let thresholds = serde_json::from_str::<BenchmarkThresholds>(
            r#"{
                "schema_version": 3,
                "rows": [
                    {"id": "../bad", "max_relative_ratio": 1.25},
                    {"id": "bad-ratio", "max_relative_ratio": 0.0},
                    {
                        "id": "bad-measurement",
                        "measurement_thresholds": [
                            {"stim_name": "", "stab_name": "", "max_relative_ratio": 0.0}
                        ]
                    }
                ]
            }"#,
        )
        .expect("parse thresholds");

        let error = thresholds
            .validate(std::path::Path::new("thresholds.json"))
            .expect_err("reject invalid thresholds");

        let text = error.to_string();
        assert!(text.contains("schema_version=3 expected 1 or 2"));
        assert!(text.contains("../bad has unsafe id"));
        assert!(text.contains("bad-ratio has invalid max_relative_ratio 0"));
        assert!(text.contains("bad-measurement has measurement threshold with empty stim_name"));
        assert!(text.contains("bad-measurement has measurement threshold with empty stab_name"));
        assert!(text.contains("bad-measurement measurement  ->  has invalid max_relative_ratio 0"));
    }

    #[test]
    fn m12_primary_thresholds_validate_source_file() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmarks/m12-primary-thresholds.json");
        let thresholds = read_thresholds(&path).expect("read source-owned M12 thresholds");
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("repo root");
        let root = crate::root::RepoRoot::resolve(root).expect("resolve repo root");
        let manifest = crate::manifest::BenchmarkManifest::read(&root).expect("read manifest");
        let primary_ids = manifest
            .compare_rows(None, true)
            .expect("primary rows")
            .into_iter()
            .map(|row| row.id.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(thresholds.schema_version, 2);
        assert_eq!(thresholds.rows.len(), 64);
        assert!(thresholds.rows.iter().all(|row| {
            row.max_relative_ratio == Some(1.25)
                || row
                    .measurement_thresholds
                    .iter()
                    .all(|measurement| measurement.max_relative_ratio == 1.25)
        }));
        for row in &thresholds.rows {
            assert!(
                primary_ids.contains(row.id.as_str()),
                "{} must still be selected by the M12 primary matrix",
                row.id
            );
        }
    }

    fn row(id: &str, ratio: Option<f64>) -> CompareRowResult {
        CompareRowResult {
            id: id.to_string(),
            milestone: Milestone::M12,
            threshold_class: "performance-gate".to_string(),
            runner: Runner::StimPerf,
            comparability: ComparabilityClass::DirectMatch,
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
            measurement_ratios: Vec::new(),
            stab_allocation_count_max: None,
            stab_allocation_bytes_max: None,
            stab_resident_bytes_max: None,
            pass_fail_status: "not-comparable".to_string(),
            beta_gate_status: "not-checked".to_string(),
            beta_gate_waiver_reason: None,
            beta_gate_waiver_follow_up: None,
            beta_gate_error: None,
            memory_gate_status: "not-required".to_string(),
            memory_gate_baseline_bytes_max: None,
            memory_gate_allowed_bytes_max: None,
            memory_gate_baseline_resident_bytes_max: None,
            memory_gate_allowed_resident_bytes_max: None,
            memory_gate_error: None,
            regression_threshold_status: "not-configured".to_string(),
            regression_threshold_max_ratio: None,
            regression_threshold_error: None,
            profiler_note_status: "not-required".to_string(),
            profiler_note_path: None,
            profiler_note_error: None,
        }
    }

    fn row_with_measurement_ratio(
        id: &str,
        stim_name: &str,
        stab_name: &str,
        relative_ratio: f64,
    ) -> CompareRowResult {
        let mut row = row(id, None);
        row.measurement_ratios
            .push(crate::report::MeasurementRatio {
                stim_name: stim_name.to_string(),
                stab_name: stab_name.to_string(),
                stim_seconds: 1.0,
                stab_seconds: relative_ratio,
                relative_ratio,
            });
        row
    }
}
