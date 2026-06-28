use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use crate::manifest::is_safe_benchmark_id;

#[derive(Clone, Debug, Deserialize)]
struct OptimizationLog {
    schema_version: u32,
    rows: Vec<OptimizationLogRow>,
}

#[derive(Clone, Debug, Deserialize)]
struct OptimizationLogRow {
    id: String,
    before_report: String,
    after_report: String,
    dominant_cost: String,
    optimization_summary: String,
    semantic_checks: Vec<String>,
    follow_up: String,
}

impl OptimizationLog {
    fn validate(&self) -> Result<(), String> {
        let mut violations = Vec::new();
        if self.schema_version != 1 {
            violations.push(format!(
                "optimization log schema_version={} expected 1",
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
            schema_version: 2,
            rows: vec![super::OptimizationLogRow {
                id: "../bad".to_string(),
                before_report: "/tmp/report/compare.json".to_string(),
                after_report: "target/benchmarks/missing/report.md".to_string(),
                dominant_cost: String::new(),
                optimization_summary: String::new(),
                semantic_checks: Vec::new(),
                follow_up: String::new(),
            }],
        };

        let error = log.validate().expect_err("reject invalid optimization log");

        assert!(error.contains("schema_version=2 expected 1"));
        assert!(error.contains("../bad has unsafe id"));
        assert!(error.contains("before_report must be repository-relative"));
        assert!(error.contains("after_report must reference target/benchmarks"));
        assert!(error.contains("has empty dominant_cost"));
        assert!(error.contains("has no semantic_checks"));
    }
}
