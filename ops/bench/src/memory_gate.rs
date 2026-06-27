use std::path::Path;

use serde::Deserialize;

use crate::error::BenchError;
use crate::report::CompareRowResult;

#[derive(Clone, Debug, Deserialize)]
struct MemoryBaselineReport {
    schema_version: u32,
    rows: Vec<MemoryBaselineRow>,
}

#[derive(Clone, Debug, Deserialize)]
struct MemoryBaselineRow {
    id: String,
    stab_allocation_bytes_max: Option<u64>,
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
    rows: Vec<MemoryBaselineRow>,
}

impl MemoryBaseline {
    fn from_report(report: MemoryBaselineReport) -> Result<Self, String> {
        if report.schema_version != 1 {
            return Err(format!(
                "memory baseline schema_version={} expected 1",
                report.schema_version
            ));
        }
        Ok(Self { rows: report.rows })
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
        if current_bytes <= allowed_bytes {
            row.memory_gate_status = "pass".to_string();
        } else {
            let message = format!(
                "allocation bytes {current_bytes} exceeds memory gate limit {allowed_bytes} from baseline {baseline_bytes}"
            );
            row.memory_gate_status = "fail".to_string();
            row.memory_gate_error = Some(message.clone());
            findings.blockers.push(format!("{}: {message}", row.id));
        }
    }
    findings
}

fn allowed_memory_bytes(baseline_bytes: u64) -> u64 {
    baseline_bytes.saturating_add(baseline_bytes / 4)
}

#[cfg(test)]
mod tests {
    use super::{MemoryBaseline, MemoryBaselineReport, apply_memory_gate};
    use crate::manifest::{Milestone, Runner};
    use crate::report::CompareRowResult;

    #[test]
    fn memory_gate_marks_pass_fail_and_missing_allocation_rows() {
        let baseline = MemoryBaseline::from_report(MemoryBaselineReport {
            schema_version: 1,
            rows: vec![
                baseline_row("pass-row", Some(100)),
                baseline_row("fail-row", Some(100)),
                baseline_row("missing-allocation-row", Some(100)),
                baseline_row("missing-baseline-allocation-row", None),
            ],
        })
        .expect("baseline");
        let mut rows = vec![
            row("pass-row", Some(125)),
            row("fail-row", Some(126)),
            row("missing-allocation-row", None),
            row("missing-baseline-row", Some(50)),
            row("missing-baseline-allocation-row", Some(50)),
        ];

        let findings = apply_memory_gate(&mut rows, &baseline);

        assert_eq!(rows.first().expect("pass row").memory_gate_status, "pass");
        assert_eq!(rows.get(1).expect("fail row").memory_gate_status, "fail");
        assert_eq!(
            rows.get(2).expect("missing current row").memory_gate_status,
            "missing-current-allocation"
        );
        assert_eq!(
            rows.get(3)
                .expect("missing baseline row")
                .memory_gate_status,
            "missing-baseline"
        );
        assert_eq!(
            rows.get(4)
                .expect("missing baseline allocation row")
                .memory_gate_status,
            "missing-baseline-allocation"
        );
        assert_eq!(
            findings.blockers,
            vec![
                "fail-row: allocation bytes 126 exceeds memory gate limit 125 from baseline 100",
                "missing-allocation-row: current row has no allocation byte maximum",
                "missing-baseline-row: memory baseline is missing row",
                "missing-baseline-allocation-row: memory baseline row has no allocation byte maximum",
            ]
        );
    }

    #[test]
    fn memory_gate_rejects_unsupported_baseline_schema() {
        let error = MemoryBaseline::from_report(MemoryBaselineReport {
            schema_version: 2,
            rows: Vec::new(),
        })
        .expect_err("reject unsupported schema");

        assert!(
            error
                .to_string()
                .contains("memory baseline schema_version=2 expected 1")
        );
    }

    fn baseline_row(id: &str, bytes: Option<u64>) -> super::MemoryBaselineRow {
        super::MemoryBaselineRow {
            id: id.to_string(),
            stab_allocation_bytes_max: bytes,
        }
    }

    fn row(id: &str, bytes: Option<u64>) -> CompareRowResult {
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
            relative_ratio: None,
            stab_allocation_count_max: None,
            stab_allocation_bytes_max: bytes,
            pass_fail_status: "not-comparable".to_string(),
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
