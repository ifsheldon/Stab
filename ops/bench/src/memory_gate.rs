use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use crate::error::BenchError;
use crate::manifest::is_safe_benchmark_id;
use crate::report::CompareRowResult;

const RESIDENT_DELTA_ABSOLUTE_SLACK_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, Deserialize)]
struct MemoryBaselineReport {
    schema_version: u32,
    rows: Vec<MemoryBaselineRow>,
}

#[derive(Clone, Debug, Deserialize)]
struct MemoryBaselineRow {
    id: String,
    stab_allocation_bytes_max: Option<u64>,
    stab_resident_bytes_max: Option<u64>,
    #[serde(default)]
    stab_resident_delta_bytes_max: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MemoryGateFindings {
    pub(crate) blockers: Vec<String>,
}

pub(crate) fn read_memory_baseline(path: &Path) -> Result<MemoryBaseline, BenchError> {
    let content =
        std::fs::read_to_string(path).map_err(|source| BenchError::ReadCompareReport {
            path: path.to_path_buf(),
            source,
        })?;
    let report = serde_json::from_str::<MemoryBaselineReport>(&content).map_err(|source| {
        BenchError::ParseCompareReport {
            path: path.to_path_buf(),
            source,
        }
    })?;
    MemoryBaseline::from_report(report).map_err(|details| BenchError::MemoryBaselineValidation {
        path: path.to_path_buf(),
        details: details.into_boxed_str(),
    })
}

#[derive(Clone, Debug)]
pub(crate) struct MemoryBaseline {
    schema_version: u32,
    rows: Vec<MemoryBaselineRow>,
}

impl MemoryBaseline {
    fn from_report(report: MemoryBaselineReport) -> Result<Self, String> {
        let mut violations = Vec::new();
        if !matches!(report.schema_version, 1 | 2) {
            violations.push(format!(
                "memory baseline schema_version={} expected 1 or 2",
                report.schema_version
            ));
        }
        let mut ids = BTreeSet::new();
        for row in &report.rows {
            if row.id.is_empty() {
                violations.push("row with empty id".to_string());
            } else if !is_safe_benchmark_id(&row.id) {
                violations.push(format!("{} has unsafe id", row.id));
            } else if !ids.insert(row.id.clone()) {
                violations.push(format!("duplicate memory baseline row {}", row.id));
            }
        }
        if !violations.is_empty() {
            return Err(violations.join("\n"));
        }
        Ok(Self {
            schema_version: report.schema_version,
            rows: report.rows,
        })
    }

    fn row(&self, id: &str) -> Option<&MemoryBaselineRow> {
        self.rows.iter().find(|row| row.id == id)
    }
}

pub(crate) fn apply_memory_gate(
    rows: &mut [CompareRowResult],
    baseline: &MemoryBaseline,
) -> MemoryGateFindings {
    let mut findings = MemoryGateFindings::default();
    for row in rows {
        let Some(baseline_row) = baseline.row(&row.id) else {
            let message = "memory baseline is missing row".to_string();
            row.memory_gate_status = "missing-baseline".to_string();
            row.memory_gate_error = Some(message.clone());
            findings.blockers.push(format!("{}: {message}", row.id));
            continue;
        };
        let Some(baseline_bytes) = baseline_row.stab_allocation_bytes_max else {
            let message = "memory baseline row has no allocation byte maximum".to_string();
            row.memory_gate_status = "missing-baseline-allocation".to_string();
            row.memory_gate_error = Some(message.clone());
            findings.blockers.push(format!("{}: {message}", row.id));
            continue;
        };
        row.memory_gate_baseline_bytes_max = Some(baseline_bytes);
        let allowed_bytes = allowed_memory_bytes(baseline_bytes);
        row.memory_gate_allowed_bytes_max = Some(allowed_bytes);
        let Some(current_bytes) = row.stab_allocation_bytes_max else {
            let message = "current row has no allocation byte maximum".to_string();
            row.memory_gate_status = "missing-current-allocation".to_string();
            row.memory_gate_error = Some(message.clone());
            findings.blockers.push(format!("{}: {message}", row.id));
            continue;
        };
        let mut row_blockers = Vec::new();
        if current_bytes > allowed_bytes {
            row_blockers.push(format!(
                "allocation bytes {current_bytes} exceeds memory gate limit {allowed_bytes} from baseline {baseline_bytes}"
            ));
        }
        match baseline.schema_version {
            1 => apply_absolute_resident_gate(&mut row_blockers, row, baseline_row, &mut findings),
            2 => apply_resident_delta_gate(&mut row_blockers, row, baseline_row, &mut findings),
            _ => unreachable!("memory baseline validation rejects unsupported schema"),
        }
        if row.memory_gate_error.is_some() {
            continue;
        }
        if row_blockers.is_empty() {
            row.memory_gate_status = "pass".to_string();
        } else {
            let message = row_blockers.join("; ");
            row.memory_gate_status = "fail".to_string();
            row.memory_gate_error = Some(message.clone());
            findings.blockers.push(format!("{}: {message}", row.id));
        }
    }
    findings
}

fn apply_absolute_resident_gate(
    row_blockers: &mut Vec<String>,
    row: &mut CompareRowResult,
    baseline_row: &MemoryBaselineRow,
    findings: &mut MemoryGateFindings,
) {
    let Some(baseline_resident_bytes) = baseline_row.stab_resident_bytes_max else {
        let message = "memory baseline row has no resident byte maximum".to_string();
        row.memory_gate_status = "missing-baseline-resident".to_string();
        row.memory_gate_error = Some(message.clone());
        findings.blockers.push(format!("{}: {message}", row.id));
        return;
    };
    row.memory_gate_baseline_resident_bytes_max = Some(baseline_resident_bytes);
    let allowed_resident_bytes = allowed_memory_bytes(baseline_resident_bytes);
    row.memory_gate_allowed_resident_bytes_max = Some(allowed_resident_bytes);
    let Some(current_resident_bytes) = row.stab_resident_bytes_max else {
        let message = "current row has no resident byte maximum".to_string();
        row.memory_gate_status = "missing-current-resident".to_string();
        row.memory_gate_error = Some(message.clone());
        findings.blockers.push(format!("{}: {message}", row.id));
        return;
    };
    if current_resident_bytes > allowed_resident_bytes {
        row_blockers.push(format!(
            "resident bytes {current_resident_bytes} exceeds memory gate limit {allowed_resident_bytes} from baseline {baseline_resident_bytes}"
        ));
    }
}

fn apply_resident_delta_gate(
    row_blockers: &mut Vec<String>,
    row: &mut CompareRowResult,
    baseline_row: &MemoryBaselineRow,
    findings: &mut MemoryGateFindings,
) {
    row.memory_gate_baseline_resident_bytes_max = baseline_row.stab_resident_bytes_max;
    let Some(baseline_delta_bytes) = baseline_row.stab_resident_delta_bytes_max else {
        let message = "memory baseline row has no resident delta byte maximum".to_string();
        row.memory_gate_status = "missing-baseline-resident-delta".to_string();
        row.memory_gate_error = Some(message.clone());
        findings.blockers.push(format!("{}: {message}", row.id));
        return;
    };
    row.memory_gate_baseline_resident_delta_bytes_max = Some(baseline_delta_bytes);
    let allowed_delta_bytes = allowed_resident_delta_bytes(baseline_delta_bytes);
    row.memory_gate_allowed_resident_delta_bytes_max = Some(allowed_delta_bytes);
    let Some(current_delta_bytes) = row.stab_resident_delta_bytes_max else {
        let message = "current row has no resident delta byte maximum".to_string();
        row.memory_gate_status = "missing-current-resident-delta".to_string();
        row.memory_gate_error = Some(message.clone());
        findings.blockers.push(format!("{}: {message}", row.id));
        return;
    };
    if current_delta_bytes > allowed_delta_bytes {
        row_blockers.push(format!(
            "resident delta bytes {current_delta_bytes} exceeds memory gate limit {allowed_delta_bytes} from baseline {baseline_delta_bytes}"
        ));
    }
}

fn allowed_memory_bytes(baseline_bytes: u64) -> u64 {
    baseline_bytes.saturating_add(baseline_bytes / 4)
}

fn allowed_resident_delta_bytes(baseline_bytes: u64) -> u64 {
    baseline_bytes.saturating_add((baseline_bytes / 4).max(RESIDENT_DELTA_ABSOLUTE_SLACK_BYTES))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{MemoryBaseline, MemoryBaselineReport, apply_memory_gate};
    use crate::comparability::ComparabilityClass;
    use crate::manifest::{Milestone, Runner};
    use crate::report::CompareRowResult;

    #[test]
    fn memory_gate_marks_pass_fail_and_missing_memory_rows() {
        let baseline = MemoryBaseline::from_report(MemoryBaselineReport {
            schema_version: 1,
            rows: vec![
                baseline_row("pass-row", Some(100), Some(1000)),
                baseline_row("allocation-fail-row", Some(100), Some(1000)),
                baseline_row("resident-fail-row", Some(100), Some(1000)),
                baseline_row("missing-allocation-row", Some(100), Some(1000)),
                baseline_row("missing-resident-row", Some(100), Some(1000)),
                baseline_row("missing-baseline-allocation-row", None, Some(1000)),
                baseline_row("missing-baseline-resident-row", Some(100), None),
            ],
        })
        .expect("baseline");
        let mut rows = vec![
            row("pass-row", Some(125), Some(1250)),
            row("allocation-fail-row", Some(126), Some(1250)),
            row("resident-fail-row", Some(125), Some(1251)),
            row("missing-allocation-row", None, Some(1000)),
            row("missing-resident-row", Some(100), None),
            row("missing-baseline-row", Some(50), Some(500)),
            row("missing-baseline-allocation-row", Some(50), Some(500)),
            row("missing-baseline-resident-row", Some(50), Some(500)),
        ];

        let findings = apply_memory_gate(&mut rows, &baseline);

        assert_eq!(rows.first().expect("pass row").memory_gate_status, "pass");
        assert_eq!(rows.get(1).expect("fail row").memory_gate_status, "fail");
        assert_eq!(
            rows.get(2).expect("rss fail row").memory_gate_status,
            "fail"
        );
        assert_eq!(
            rows.get(3)
                .expect("missing allocation row")
                .memory_gate_status,
            "missing-current-allocation"
        );
        assert_eq!(
            rows.get(3)
                .expect("missing allocation row")
                .memory_gate_error
                .as_deref(),
            Some("current row has no allocation byte maximum")
        );
        assert_eq!(
            rows.get(4)
                .expect("missing resident row")
                .memory_gate_status,
            "missing-current-resident"
        );
        assert_eq!(
            rows.get(5)
                .expect("missing baseline row")
                .memory_gate_status,
            "missing-baseline"
        );
        assert_eq!(
            rows.get(6)
                .expect("missing baseline allocation row")
                .memory_gate_status,
            "missing-baseline-allocation"
        );
        assert_eq!(
            rows.get(7)
                .expect("missing baseline resident row")
                .memory_gate_status,
            "missing-baseline-resident"
        );
        assert_eq!(
            findings.blockers,
            vec![
                "allocation-fail-row: allocation bytes 126 exceeds memory gate limit 125 from baseline 100",
                "resident-fail-row: resident bytes 1251 exceeds memory gate limit 1250 from baseline 1000",
                "missing-allocation-row: current row has no allocation byte maximum",
                "missing-resident-row: current row has no resident byte maximum",
                "missing-baseline-row: memory baseline is missing row",
                "missing-baseline-allocation-row: memory baseline row has no allocation byte maximum",
                "missing-baseline-resident-row: memory baseline row has no resident byte maximum",
            ]
        );
    }

    #[test]
    fn memory_gate_v2_uses_resident_delta_instead_of_absolute_resident_bytes() {
        let baseline = MemoryBaseline::from_report(MemoryBaselineReport {
            schema_version: 2,
            rows: vec![
                baseline_row_with_delta("absolute-drift-row", Some(100), Some(1000), Some(10)),
                baseline_row_with_delta("delta-fail-row", Some(100), Some(1000), Some(10)),
                baseline_row_with_delta("missing-delta-row", Some(100), Some(1000), Some(10)),
                baseline_row_with_delta("missing-baseline-delta-row", Some(100), Some(1000), None),
            ],
        })
        .expect("baseline");
        let mut rows = vec![
            row_with_delta("absolute-drift-row", Some(125), Some(10_000), Some(12)),
            row_with_delta("delta-fail-row", Some(125), Some(1000), Some(70_000)),
            row_with_delta("missing-delta-row", Some(125), Some(1000), None),
            row_with_delta("missing-baseline-delta-row", Some(125), Some(1000), Some(1)),
        ];

        let findings = apply_memory_gate(&mut rows, &baseline);

        assert_eq!(rows.first().expect("drift row").memory_gate_status, "pass");
        assert_eq!(
            rows.first()
                .expect("drift row")
                .memory_gate_baseline_resident_delta_bytes_max,
            Some(10)
        );
        assert_eq!(
            rows.first()
                .expect("drift row")
                .memory_gate_allowed_resident_delta_bytes_max,
            Some(65_546)
        );
        assert_eq!(rows.get(1).expect("delta fail").memory_gate_status, "fail");
        assert_eq!(
            rows.get(2).expect("missing current").memory_gate_status,
            "missing-current-resident-delta"
        );
        assert_eq!(
            rows.get(3).expect("missing baseline").memory_gate_status,
            "missing-baseline-resident-delta"
        );
        assert_eq!(
            findings.blockers,
            vec![
                "delta-fail-row: resident delta bytes 70000 exceeds memory gate limit 65546 from baseline 10",
                "missing-delta-row: current row has no resident delta byte maximum",
                "missing-baseline-delta-row: memory baseline row has no resident delta byte maximum",
            ]
        );
    }

    #[test]
    fn memory_gate_rejects_unsupported_baseline_schema() {
        let error = MemoryBaseline::from_report(MemoryBaselineReport {
            schema_version: 3,
            rows: Vec::new(),
        })
        .expect_err("reject unsupported schema");

        assert!(
            error
                .to_string()
                .contains("memory baseline schema_version=3 expected 1 or 2")
        );
    }

    #[test]
    fn memory_baseline_validates_ids() {
        let error = MemoryBaseline::from_report(MemoryBaselineReport {
            schema_version: 1,
            rows: vec![
                baseline_row("../bad", Some(100), Some(1000)),
                baseline_row("duplicate-row", Some(100), Some(1000)),
                baseline_row("duplicate-row", Some(200), Some(2000)),
            ],
        })
        .expect_err("reject invalid ids");

        let text = error.to_string();
        assert!(text.contains("../bad has unsafe id"));
        assert!(text.contains("duplicate memory baseline row duplicate-row"));
    }

    #[test]
    fn memory_baseline_accepts_full_compare_report_shape() {
        let report = serde_json::from_str::<MemoryBaselineReport>(
            r#"{
                "schema_version": 1,
                "command": {"primary": true},
                "rows": [
                    {
                        "id": "m8-sample-throughput-1024",
                        "status": "measured",
                        "stab_allocation_bytes_max": 2048,
                        "stab_resident_bytes_max": 4096,
                        "memory_gate_status": "not-required"
                    }
                ]
            }"#,
        )
        .expect("parse full compare report subset");
        let baseline = MemoryBaseline::from_report(report).expect("accept full compare report");

        assert_eq!(baseline.rows.len(), 1);
        assert_eq!(
            baseline
                .row("m8-sample-throughput-1024")
                .expect("row")
                .stab_allocation_bytes_max,
            Some(2048)
        );
        assert_eq!(
            baseline
                .row("m8-sample-throughput-1024")
                .expect("row")
                .stab_resident_bytes_max,
            Some(4096)
        );
    }

    #[test]
    fn m12_primary_memory_baseline_validates_source_file() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmarks/m12-primary-memory-baseline.json");
        let baseline = super::read_memory_baseline(&path).expect("read M12 memory baseline");

        assert_eq!(baseline.rows.len(), 84);
        assert_eq!(baseline.schema_version, 2);
        assert!(
            baseline
                .rows
                .iter()
                .any(|row| row.id == "m5-simd-bit-table"),
            "transpose timing migration must preserve the legacy memory baseline until PQ6"
        );
        assert!(
            baseline.rows.iter().any(|row| row.id == "m6-pauli-string"),
            "Pauli timing migration must preserve the legacy memory baseline until PQ6"
        );
        assert!(
            baseline.rows.iter().any(|row| row.id == "m6-pauli-iter"),
            "Pauli iterator timing migration must preserve the legacy memory baseline until PQ6"
        );
        assert!(
            baseline
                .rows
                .iter()
                .any(|row| row.id == "m6-clifford-string"),
            "Clifford timing migration must preserve the legacy memory baseline until PQ6"
        );
        for id in ["m10-dem-parse-contract", "m10-dem-print-contract"] {
            assert!(
                baseline.rows.iter().any(|row| row.id == id),
                "DEM timing migration must preserve the legacy memory baseline {id} until PQ6"
            );
        }
        assert!(
            baseline
                .rows
                .iter()
                .all(|row| row.stab_allocation_bytes_max.is_some()
                    && row.stab_resident_bytes_max.is_some()
                    && row.stab_resident_delta_bytes_max.is_some())
        );
    }

    fn baseline_row(
        id: &str,
        allocation_bytes: Option<u64>,
        resident_bytes: Option<u64>,
    ) -> super::MemoryBaselineRow {
        baseline_row_with_delta(id, allocation_bytes, resident_bytes, None)
    }

    fn baseline_row_with_delta(
        id: &str,
        allocation_bytes: Option<u64>,
        resident_bytes: Option<u64>,
        resident_delta_bytes: Option<u64>,
    ) -> super::MemoryBaselineRow {
        super::MemoryBaselineRow {
            id: id.to_string(),
            stab_allocation_bytes_max: allocation_bytes,
            stab_resident_bytes_max: resident_bytes,
            stab_resident_delta_bytes_max: resident_delta_bytes,
        }
    }

    fn row(
        id: &str,
        allocation_bytes: Option<u64>,
        resident_bytes: Option<u64>,
    ) -> CompareRowResult {
        row_with_delta(id, allocation_bytes, resident_bytes, None)
    }

    fn row_with_delta(
        id: &str,
        allocation_bytes: Option<u64>,
        resident_bytes: Option<u64>,
        resident_delta_bytes: Option<u64>,
    ) -> CompareRowResult {
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
            relative_ratio: None,
            measurement_ratios: Vec::new(),
            stab_allocation_count_max: None,
            stab_allocation_bytes_max: allocation_bytes,
            stab_resident_bytes_max: resident_bytes,
            stab_resident_delta_bytes_max: resident_delta_bytes,
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
            memory_gate_baseline_resident_delta_bytes_max: None,
            memory_gate_allowed_resident_delta_bytes_max: None,
            memory_gate_error: None,
            regression_threshold_status: "not-configured".to_string(),
            regression_threshold_max_ratio: None,
            regression_threshold_waiver_reason: None,
            regression_threshold_waiver_follow_up: None,
            regression_threshold_error: None,
            profiler_note_status: "not-required".to_string(),
            profiler_note_path: None,
            profiler_note_error: None,
        }
    }
}
