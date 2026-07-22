use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::BenchError;
use crate::manifest::{Runner, is_safe_benchmark_id};
use crate::report::CompareRowResult;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegressionWaivers {
    schema_version: u32,
    rows: Vec<RegressionWaiverRow>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegressionWaiverRow {
    id: String,
    reason: String,
    follow_up: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RegressionWaiverFindings {
    pub(crate) blockers: Vec<String>,
}

pub(crate) fn read_regression_waivers(path: &Path) -> Result<RegressionWaivers, BenchError> {
    let content =
        std::fs::read_to_string(path).map_err(|source| BenchError::ReadRegressionWaivers {
            path: path.to_path_buf(),
            source,
        })?;
    let waivers = serde_json::from_str::<RegressionWaivers>(&content).map_err(|source| {
        BenchError::ParseRegressionWaivers {
            path: path.to_path_buf(),
            source,
        }
    })?;
    waivers.validate(path)?;
    Ok(waivers)
}

pub(crate) fn apply_regression_waivers(
    rows: &mut [CompareRowResult],
    waivers: &RegressionWaivers,
) -> RegressionWaiverFindings {
    let mut findings = RegressionWaiverFindings::default();
    let waiver_rows = waivers.by_id();
    let mut used_waivers = BTreeSet::new();

    for row in rows {
        let Some(waiver) = waiver_rows.get(row.id.as_str()) else {
            continue;
        };
        if regression_waiver_applies(row) {
            row.regression_threshold_status = "waived-not-thresholdable".to_string();
            row.regression_threshold_waiver_reason = Some(waiver.reason.clone());
            row.regression_threshold_waiver_follow_up = Some(waiver.follow_up.clone());
            used_waivers.insert(row.id.clone());
        } else {
            let message = format!(
                "regression waiver cannot apply because threshold status is {}, pass/fail status is {}, runner is {}, comparability is {}, row status is {}, and relative ratio is {}",
                row.regression_threshold_status,
                row.pass_fail_status,
                row.runner.as_str(),
                row.comparability.as_str(),
                row.status,
                row.relative_ratio
                    .map_or_else(|| "none".to_string(), |ratio| format!("{ratio:.3}x")),
            );
            row.regression_threshold_status = "fail".to_string();
            row.regression_threshold_error = Some(message.clone());
            findings.blockers.push(format!("{}: {message}", row.id));
        }
    }

    for id in waiver_rows.keys() {
        if !used_waivers.contains(*id) {
            findings.blockers.push(format!(
                "{id}: regression waiver did not match a selected measured contract-only no-ratio row without threshold coverage"
            ));
        }
    }

    findings
}

fn regression_waiver_applies(row: &CompareRowResult) -> bool {
    row.regression_threshold_status == "not-configured"
        && row.pass_fail_status == "not-comparable"
        && row.runner == Runner::ContractOnly
        && row.status == "measured"
        && row.relative_ratio.is_none()
}

impl RegressionWaivers {
    fn by_id(&self) -> BTreeMap<&str, &RegressionWaiverRow> {
        self.rows
            .iter()
            .map(|row| (row.id.as_str(), row))
            .collect::<BTreeMap<_, _>>()
    }

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
                violations.push(format!("duplicate regression waiver row {}", row.id));
            }
            if row.reason.trim().is_empty() {
                violations.push(format!("{} has empty reason", row.id));
            }
            if row.follow_up.trim().is_empty() {
                violations.push(format!("{} has empty follow_up", row.id));
            }
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(BenchError::RegressionWaiverValidation {
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

    use super::{RegressionWaivers, apply_regression_waivers, read_regression_waivers};
    use crate::comparability::ComparabilityClass;
    use crate::manifest::{Milestone, Runner};
    use crate::report::CompareRowResult;

    #[test]
    fn regression_waivers_mark_only_measured_contract_only_no_ratio_rows() {
        let waivers = serde_json::from_str::<RegressionWaivers>(
            r#"{
                "schema_version": 1,
                "rows": [
                    {
                        "id": "contract-row",
                        "reason": "Pinned Stim has no comparable timing surface.",
                        "follow_up": "Replace this waiver if a comparable Stim filter appears."
                    }
                ]
            }"#,
        )
        .expect("parse waivers");
        let mut rows = vec![
            row(
                "thresholded-row",
                Runner::StimPerf,
                "measured",
                "pass",
                Some(1.0),
                "pass",
            ),
            row(
                "contract-row",
                Runner::ContractOnly,
                "measured",
                "not-comparable",
                None,
                "not-configured",
            ),
        ];

        let findings = apply_regression_waivers(&mut rows, &waivers);

        assert!(findings.blockers.is_empty());
        assert_eq!(
            rows.first()
                .expect("thresholded row")
                .regression_threshold_status,
            "pass"
        );
        let waived = rows.get(1).expect("waived row");
        assert_eq!(
            waived.regression_threshold_status,
            "waived-not-thresholdable"
        );
        assert_eq!(
            waived.regression_threshold_waiver_reason.as_deref(),
            Some("Pinned Stim has no comparable timing surface.")
        );
        assert_eq!(
            waived.regression_threshold_waiver_follow_up.as_deref(),
            Some("Replace this waiver if a comparable Stim filter appears.")
        );
    }

    #[test]
    fn regression_waivers_reject_stale_and_misapplied_rows() {
        let waivers = serde_json::from_str::<RegressionWaivers>(
            r#"{
                "schema_version": 1,
                "rows": [
                    {
                        "id": "comparable-row",
                        "reason": "bad waiver",
                        "follow_up": "remove"
                    },
                    {
                        "id": "stale-row",
                        "reason": "stale",
                        "follow_up": "remove"
                    }
                ]
            }"#,
        )
        .expect("parse waivers");
        let mut rows = vec![row(
            "comparable-row",
            Runner::StimPerf,
            "measured",
            "pass",
            Some(1.0),
            "not-configured",
        )];

        let findings = apply_regression_waivers(&mut rows, &waivers);

        assert_eq!(
            rows.first().expect("row").regression_threshold_status,
            "fail"
        );
        assert_eq!(
            findings.blockers,
            vec![
                "comparable-row: regression waiver cannot apply because threshold status is not-configured, pass/fail status is pass, runner is stim-perf, comparability is direct-match, row status is measured, and relative ratio is 1.000x",
                "comparable-row: regression waiver did not match a selected measured contract-only no-ratio row without threshold coverage",
                "stale-row: regression waiver did not match a selected measured contract-only no-ratio row without threshold coverage",
            ]
        );
    }

    #[test]
    fn regression_waivers_validate_schema_ids_and_required_text() {
        let waivers = serde_json::from_str::<RegressionWaivers>(
            r#"{
                "schema_version": 2,
                "rows": [
                    {"id": "../bad", "reason": "", "follow_up": "x"},
                    {"id": "duplicate-row", "reason": "x", "follow_up": "x"},
                    {"id": "duplicate-row", "reason": "x", "follow_up": ""}
                ]
            }"#,
        )
        .expect("parse waivers");

        let error = waivers
            .validate(Path::new("waivers.json"))
            .expect_err("reject invalid waivers");

        let text = error.to_string();
        assert!(text.contains("schema_version=2 expected 1"));
        assert!(text.contains("../bad has unsafe id"));
        assert!(text.contains("../bad has empty reason"));
        assert!(text.contains("duplicate regression waiver row duplicate-row"));
        assert!(text.contains("duplicate-row has empty follow_up"));
    }

    #[test]
    fn m12_primary_regression_waivers_validate_source_file() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmarks/m12-primary-regression-waivers.json");
        let waivers = read_regression_waivers(&path).expect("read source-owned M12 waivers");
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

        assert_eq!(waivers.schema_version, 1);
        assert_eq!(waivers.rows.len(), 3);
        assert!(
            waivers
                .rows
                .iter()
                .all(|row| row.id != "m10-dem-print-contract"),
            "the qualified direct print contract retires the legacy no-ratio waiver"
        );
        for row in &waivers.rows {
            assert!(
                primary_ids.contains(row.id.as_str()),
                "{} must still be selected by the M12 primary matrix",
                row.id
            );
        }
    }

    fn row(
        id: &str,
        runner: Runner,
        status: &str,
        pass_fail_status: &str,
        ratio: Option<f64>,
        regression_threshold_status: &str,
    ) -> CompareRowResult {
        CompareRowResult {
            id: id.to_string(),
            milestone: Milestone::M12,
            threshold_class: "performance-gate".to_string(),
            runner,
            comparability: match runner {
                Runner::ContractOnly => ComparabilityClass::ContractOnly,
                Runner::StimCli | Runner::StimPerf => ComparabilityClass::DirectMatch,
            },
            upstream_source: "future/performance-primary-matrix".to_string(),
            phase: "performance-hardening".to_string(),
            measurement: "primary-matrix".to_string(),
            status: status.to_string(),
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
            stab_resident_delta_bytes_max: None,
            pass_fail_status: pass_fail_status.to_string(),
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
            regression_threshold_status: regression_threshold_status.to_string(),
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
