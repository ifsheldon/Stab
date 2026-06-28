use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use crate::manifest::is_safe_benchmark_id;
use crate::report::BETA_GATE_MAX_RELATIVE_RATIO;

#[derive(Clone, Debug, Deserialize)]
struct OptimizationLog {
    schema_version: u32,
    rows: Vec<OptimizationLogRow>,
}

#[derive(Clone, Debug, Deserialize)]
struct OptimizationLogRow {
    id: String,
    before_report: String,
    before_evidence: OptimizationLogEvidence,
    after_report: String,
    after_evidence: OptimizationLogEvidence,
    dominant_cost: String,
    optimization_summary: String,
    semantic_checks: Vec<String>,
    follow_up: String,
}

#[derive(Clone, Debug, Deserialize)]
struct OptimizationLogEvidence {
    gate_status: String,
    relative_ratio: f64,
    hot_path_status: String,
    evidence_note: String,
    source_profiler_note: Option<String>,
}

impl OptimizationLog {
    fn validate(&self) -> Result<(), String> {
        let mut violations = Vec::new();
        if self.schema_version != 2 {
            violations.push(format!(
                "optimization log schema_version={} expected 2",
                self.schema_version
            ));
        }
        let mut ids = BTreeSet::new();
        for row in &self.rows {
            row.validate(&mut ids, &mut violations);
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations.join("\n"))
        }
    }
}

impl OptimizationLogRow {
    fn validate(&self, ids: &mut BTreeSet<String>, violations: &mut Vec<String>) {
        if self.id.is_empty() {
            violations.push("row with empty id".to_string());
        } else if !is_safe_benchmark_id(&self.id) {
            violations.push(format!("{} has unsafe id", self.id));
        } else if !ids.insert(self.id.clone()) {
            violations.push(format!("duplicate optimization log row {}", self.id));
        }
        validate_report_path(&self.id, "before_report", &self.before_report, violations);
        validate_report_path(&self.id, "after_report", &self.after_report, violations);
        self.before_evidence
            .validate(&self.id, "before_evidence", violations);
        self.after_evidence
            .validate(&self.id, "after_evidence", violations);
        validate_nonempty_text(&self.id, "dominant_cost", &self.dominant_cost, violations);
        validate_nonempty_text(
            &self.id,
            "optimization_summary",
            &self.optimization_summary,
            violations,
        );
        validate_nonempty_text(&self.id, "follow_up", &self.follow_up, violations);
        if self.semantic_checks.is_empty() {
            violations.push(format!("{} has no semantic_checks", self.id));
        }
        for check in &self.semantic_checks {
            validate_nonempty_text(&self.id, "semantic_checks", check, violations);
        }
    }
}

impl OptimizationLogEvidence {
    fn validate(&self, row_id: &str, field: &str, violations: &mut Vec<String>) {
        validate_nonempty_text(
            row_id,
            &format!("{field}.evidence_note"),
            &self.evidence_note,
            violations,
        );
        validate_gate_status(
            row_id,
            field,
            &self.gate_status,
            self.relative_ratio,
            violations,
        );
        validate_hot_path_status(
            row_id,
            field,
            &self.hot_path_status,
            self.relative_ratio,
            self.source_profiler_note.as_deref(),
            violations,
        );
        if field == "after_evidence" && self.hot_path_status == "above-profiler-threshold" {
            violations.push(format!(
                "{row_id} after_evidence.hot_path_status must be below-profiler-threshold or covered-by-source-profiler-note"
            ));
        }
        if field == "after_evidence" && self.gate_status != "pass" {
            violations.push(format!("{row_id} after_evidence.gate_status must be pass"));
        }
    }
}

fn validate_report_path(row_id: &str, field: &str, value: &str, violations: &mut Vec<String>) {
    let path = Path::new(value);
    if value.is_empty() {
        violations.push(format!("{row_id} has empty {field}"));
        return;
    }
    if path.is_absolute() {
        violations.push(format!("{row_id} {field} must be repository-relative"));
    }
    let components = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>();
    if components
        .iter()
        .any(|component| component.as_ref() == "..")
    {
        violations.push(format!(
            "{row_id} {field} must not contain parent traversal"
        ));
    }
    if !value.starts_with("target/benchmarks/") || !value.ends_with("/compare.json") {
        violations.push(format!(
            "{row_id} {field} must reference target/benchmarks/<report>/compare.json"
        ));
    }
}

fn validate_source_profiler_note_path(
    row_id: &str,
    field: &str,
    value: &str,
    violations: &mut Vec<String>,
) {
    validate_report_path_is_relative(row_id, field, value, violations);
    if !value.starts_with("benchmarks/profiler-notes/m12/") || !value.ends_with(".md") {
        violations.push(format!(
            "{row_id} {field} must reference benchmarks/profiler-notes/m12/<id>.md"
        ));
    }
    let expected = format!("benchmarks/profiler-notes/m12/{row_id}.md");
    if value != expected {
        violations.push(format!("{row_id} {field} must be {expected}"));
    }
    let repo_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(value);
    if !repo_path.is_file() {
        violations.push(format!(
            "{row_id} {field} must reference an existing source-owned profiler note"
        ));
    }
}

fn validate_report_path_is_relative(
    row_id: &str,
    field: &str,
    value: &str,
    violations: &mut Vec<String>,
) {
    let path = Path::new(value);
    if path.is_absolute() {
        violations.push(format!("{row_id} {field} must be repository-relative"));
    }
    if path
        .components()
        .any(|component| component.as_os_str().to_string_lossy().as_ref() == "..")
    {
        violations.push(format!(
            "{row_id} {field} must not contain parent traversal"
        ));
    }
}

fn validate_gate_status(
    row_id: &str,
    field: &str,
    value: &str,
    relative_ratio: f64,
    violations: &mut Vec<String>,
) {
    if !relative_ratio.is_finite() || relative_ratio <= 0.0 {
        violations.push(format!(
            "{row_id} {field}.relative_ratio must be positive and finite"
        ));
    }
    match value {
        "pass" if relative_ratio > BETA_GATE_MAX_RELATIVE_RATIO => violations.push(format!(
            "{row_id} {field}.gate_status=pass conflicts with relative_ratio={relative_ratio}"
        )),
        "fail" if relative_ratio <= BETA_GATE_MAX_RELATIVE_RATIO => violations.push(format!(
            "{row_id} {field}.gate_status=fail conflicts with relative_ratio={relative_ratio}"
        )),
        "pass" | "fail" => {}
        _ => violations.push(format!("{row_id} {field}.gate_status must be pass or fail")),
    }
}

fn validate_hot_path_status(
    row_id: &str,
    field: &str,
    value: &str,
    relative_ratio: f64,
    source_profiler_note: Option<&str>,
    violations: &mut Vec<String>,
) {
    match value {
        "below-profiler-threshold" if relative_ratio > 1.5 => violations.push(format!(
            "{row_id} {field}.hot_path_status=below-profiler-threshold conflicts with relative_ratio={relative_ratio}"
        )),
        "above-profiler-threshold" if relative_ratio <= 1.5 => violations.push(format!(
            "{row_id} {field}.hot_path_status=above-profiler-threshold conflicts with relative_ratio={relative_ratio}"
        )),
        "covered-by-source-profiler-note" if relative_ratio <= 1.5 => violations.push(format!(
            "{row_id} {field}.hot_path_status=covered-by-source-profiler-note conflicts with relative_ratio={relative_ratio}"
        )),
        "covered-by-source-profiler-note" => {
            if let Some(path) = source_profiler_note {
                validate_source_profiler_note_path(
                    row_id,
                    &format!("{field}.source_profiler_note"),
                    path,
                    violations,
                );
            } else {
                violations.push(format!(
                    "{row_id} {field}.source_profiler_note is required when hot_path_status is covered-by-source-profiler-note"
                ));
            }
        }
        "below-profiler-threshold" | "above-profiler-threshold" => {
            if source_profiler_note.is_some() {
                violations.push(format!(
                    "{row_id} {field}.source_profiler_note is only allowed for covered-by-source-profiler-note"
                ));
            }
        }
        _ => violations.push(format!(
            "{row_id} {field}.hot_path_status must be above-profiler-threshold, below-profiler-threshold, or covered-by-source-profiler-note"
        )),
    }
}

fn validate_nonempty_text(row_id: &str, field: &str, value: &str, violations: &mut Vec<String>) {
    if value.trim().is_empty() {
        violations.push(format!("{row_id} has empty {field}"));
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::OptimizationLog;

    #[test]
    fn m12_optimization_log_validates_source_file() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmarks/profiler-notes/m12/optimization-log.json");
        let content = std::fs::read_to_string(path).expect("read optimization log");
        let log: OptimizationLog = serde_json::from_str(&content).expect("parse optimization log");

        log.validate().expect("valid optimization log");
        let ids = log
            .rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>();
        for required_id in [
            "m4-circuit-parse",
            "m4-gate-lookup",
            "m5-sparse-xor",
            "m6-clifford-string",
            "m6-pauli-string",
            "m6-pauli-iter",
            "m8-measure-reader",
            "m8-probability-util",
            "m8-sample-throughput-1000000",
            "m8-sample-primary-rotated-surface-contract",
            "m8-sample-primary-unrotated-surface-contract",
            "m10-error-decomp",
        ] {
            assert!(
                ids.contains(&required_id),
                "optimization log missing {required_id}"
            );
        }
    }

    #[test]
    fn optimization_log_rejects_unsafe_or_empty_rows() {
        let log = OptimizationLog {
            schema_version: 1,
            rows: vec![super::OptimizationLogRow {
                id: "../bad".to_string(),
                before_report: "/tmp/report/compare.json".to_string(),
                before_evidence: super::OptimizationLogEvidence {
                    gate_status: "maybe".to_string(),
                    relative_ratio: -1.0,
                    hot_path_status: "below-profiler-threshold".to_string(),
                    evidence_note: String::new(),
                    source_profiler_note: None,
                },
                after_report: "target/benchmarks/missing/report.md".to_string(),
                after_evidence: super::OptimizationLogEvidence {
                    gate_status: "pass".to_string(),
                    relative_ratio: 1.75,
                    hot_path_status: "below-profiler-threshold".to_string(),
                    evidence_note: "claims a threshold crossing with a too-large ratio".to_string(),
                    source_profiler_note: None,
                },
                dominant_cost: String::new(),
                optimization_summary: String::new(),
                semantic_checks: Vec::new(),
                follow_up: String::new(),
            }],
        };

        let error = log.validate().expect_err("reject invalid optimization log");

        assert!(error.contains("schema_version=1 expected 2"));
        assert!(error.contains("../bad has unsafe id"));
        assert!(error.contains("before_report must be repository-relative"));
        assert!(error.contains("after_report must reference target/benchmarks"));
        assert!(error.contains("before_evidence.relative_ratio must be positive and finite"));
        assert!(error.contains("before_evidence.gate_status must be pass or fail"));
        assert!(error.contains("has empty before_evidence.evidence_note"));
        assert!(error.contains("after_evidence.hot_path_status=below-profiler-threshold"));
        assert!(error.contains("has empty dominant_cost"));
        assert!(error.contains("has no semantic_checks"));
    }

    #[test]
    fn optimization_log_requires_source_note_for_after_rows_above_profiler_threshold() {
        let log = OptimizationLog {
            schema_version: 2,
            rows: vec![super::OptimizationLogRow {
                id: "m4-gate-lookup".to_string(),
                before_report: "target/benchmarks/before/compare.json".to_string(),
                before_evidence: super::OptimizationLogEvidence {
                    gate_status: "fail".to_string(),
                    relative_ratio: 41.28,
                    hot_path_status: "above-profiler-threshold".to_string(),
                    evidence_note: "before evidence".to_string(),
                    source_profiler_note: None,
                },
                after_report: "target/benchmarks/after/compare.json".to_string(),
                after_evidence: super::OptimizationLogEvidence {
                    gate_status: "pass".to_string(),
                    relative_ratio: 1.64,
                    hot_path_status: "covered-by-source-profiler-note".to_string(),
                    evidence_note: "after evidence".to_string(),
                    source_profiler_note: None,
                },
                dominant_cost: "cost".to_string(),
                optimization_summary: "summary".to_string(),
                semantic_checks: vec!["cargo test".to_string()],
                follow_up: "follow up".to_string(),
            }],
        };

        let error = log.validate().expect_err("reject missing profiler note");

        assert!(error.contains("after_evidence.source_profiler_note is required"));
    }

    #[test]
    fn optimization_log_rejects_after_rows_above_threshold_without_note_coverage() {
        let log = OptimizationLog {
            schema_version: 2,
            rows: vec![super::OptimizationLogRow {
                id: "m4-gate-lookup".to_string(),
                before_report: "target/benchmarks/before/compare.json".to_string(),
                before_evidence: super::OptimizationLogEvidence {
                    gate_status: "fail".to_string(),
                    relative_ratio: 41.28,
                    hot_path_status: "above-profiler-threshold".to_string(),
                    evidence_note: "before evidence".to_string(),
                    source_profiler_note: None,
                },
                after_report: "target/benchmarks/after/compare.json".to_string(),
                after_evidence: super::OptimizationLogEvidence {
                    gate_status: "pass".to_string(),
                    relative_ratio: 1.64,
                    hot_path_status: "above-profiler-threshold".to_string(),
                    evidence_note: "after evidence".to_string(),
                    source_profiler_note: None,
                },
                dominant_cost: "cost".to_string(),
                optimization_summary: "summary".to_string(),
                semantic_checks: vec!["cargo test".to_string()],
                follow_up: "follow up".to_string(),
            }],
        };

        let error = log
            .validate()
            .expect_err("reject uncovered after-threshold evidence");

        assert!(error.contains("after_evidence.hot_path_status must be below"));
    }

    #[test]
    fn optimization_log_rejects_after_rows_that_still_fail_beta_gate() {
        let log = OptimizationLog {
            schema_version: 2,
            rows: vec![super::OptimizationLogRow {
                id: "m4-gate-lookup".to_string(),
                before_report: "target/benchmarks/before/compare.json".to_string(),
                before_evidence: super::OptimizationLogEvidence {
                    gate_status: "fail".to_string(),
                    relative_ratio: 41.28,
                    hot_path_status: "above-profiler-threshold".to_string(),
                    evidence_note: "before evidence".to_string(),
                    source_profiler_note: None,
                },
                after_report: "target/benchmarks/after/compare.json".to_string(),
                after_evidence: super::OptimizationLogEvidence {
                    gate_status: "fail".to_string(),
                    relative_ratio: 2.5,
                    hot_path_status: "covered-by-source-profiler-note".to_string(),
                    evidence_note: "after evidence".to_string(),
                    source_profiler_note: Some(
                        "benchmarks/profiler-notes/m12/m4-gate-lookup.md".to_string(),
                    ),
                },
                dominant_cost: "cost".to_string(),
                optimization_summary: "summary".to_string(),
                semantic_checks: vec!["cargo test".to_string()],
                follow_up: "follow up".to_string(),
            }],
        };

        let error = log
            .validate()
            .expect_err("reject beta-failing after evidence");

        assert!(error.contains("after_evidence.gate_status must be pass"));
    }
}
