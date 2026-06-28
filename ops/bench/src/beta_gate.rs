use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::BenchError;
use crate::manifest::{Runner, is_safe_benchmark_id};
use crate::report::CompareRowResult;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BetaWaivers {
    schema_version: u32,
    rows: Vec<BetaWaiverRow>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BetaWaiverRow {
    id: String,
    reason: String,
    follow_up: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct BetaGateFindings {
    pub(crate) blockers: Vec<String>,
}

pub(crate) fn read_beta_waivers(path: &Path) -> Result<BetaWaivers, BenchError> {
    let content = std::fs::read_to_string(path).map_err(|source| BenchError::ReadBetaWaivers {
        path: path.to_path_buf(),
        source,
    })?;
    let waivers = serde_json::from_str::<BetaWaivers>(&content).map_err(|source| {
        BenchError::ParseBetaWaivers {
            path: path.to_path_buf(),
            source,
        }
    })?;
    waivers.validate(path)?;
    Ok(waivers)
}

pub(crate) fn apply_beta_gate(
    rows: &mut [CompareRowResult],
    waivers: Option<&BetaWaivers>,
) -> BetaGateFindings {
    let mut findings = BetaGateFindings::default();
    let waiver_rows = waivers.map(BetaWaivers::by_id).unwrap_or_default();
    let mut used_waivers = BTreeSet::new();

    for row in rows {
        match row.pass_fail_status.as_str() {
            "pass" => {
                row.beta_gate_status = "pass".to_string();
            }
            "fail" => {
                let message = format!(
                    "ratio {} exceeds 2.000x beta gate",
                    row.relative_ratio
                        .map_or_else(|| "unknown".to_string(), |ratio| format!("{ratio:.3}x"))
                );
                row.beta_gate_status = "fail".to_string();
                row.beta_gate_error = Some(message.clone());
                findings.blockers.push(format!("{}: {message}", row.id));
            }
            other => match waiver_rows.get(row.id.as_str()) {
                Some(waiver) if beta_waiver_applies(row) => {
                    row.beta_gate_status = "waived-not-comparable".to_string();
                    row.beta_gate_waiver_reason = Some(waiver.reason.clone());
                    row.beta_gate_waiver_follow_up = Some(waiver.follow_up.clone());
                    used_waivers.insert(row.id.clone());
                }
                Some(_) => {
                    let message = format!(
                        "beta waiver cannot apply because status is {other}, runner is {}, comparability is {}, and row status is {}",
                        row.runner.as_str(),
                        row.comparability.as_str(),
                        row.status
                    );
                    row.beta_gate_status = "fail".to_string();
                    row.beta_gate_error = Some(message.clone());
                    findings.blockers.push(format!("{}: {message}", row.id));
                }
                None => {
                    let message = format!("beta gate is not proven because status is {other}");
                    row.beta_gate_status = "not-proven".to_string();
                    row.beta_gate_error = Some(message.clone());
                    findings.blockers.push(format!("{}: {message}", row.id));
                }
            },
        }
    }

    for id in waiver_rows.keys() {
        if !used_waivers.contains(*id) {
            findings.blockers.push(format!(
                "{id}: beta waiver did not match a selected measured contract-only not-comparable row"
            ));
        }
    }

    findings
}

fn beta_waiver_applies(row: &CompareRowResult) -> bool {
    row.pass_fail_status == "not-comparable"
        && row.runner == Runner::ContractOnly
        && row.status == "measured"
}

impl BetaWaivers {
    fn by_id(&self) -> BTreeMap<&str, &BetaWaiverRow> {
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
                violations.push(format!("duplicate beta waiver row {}", row.id));
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
            Err(BenchError::BetaWaiverValidation {
                path: PathBuf::from(path),
                details: violations.join("\n").into_boxed_str(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{BetaWaivers, apply_beta_gate, read_beta_waivers};
    use crate::comparability::ComparabilityClass;
    use crate::manifest::{Milestone, Runner};
    use crate::report::CompareRowResult;

    #[test]
    fn beta_gate_allows_only_measured_contract_rows_with_explicit_waivers() {
        let waivers = serde_json::from_str::<BetaWaivers>(
            r#"{
                "schema_version": 1,
                "rows": [
                    {
                        "id": "contract-row",
                        "reason": "Pinned Stim has no public canonical printer CLI.",
                        "follow_up": "Replace this row if Stim exposes one."
                    }
                ]
            }"#,
        )
        .expect("parse waivers");
        let mut rows = vec![
            row(
                "passing-row",
                Runner::StimCli,
                "measured",
                "pass",
                Some(1.0),
            ),
            row(
                "contract-row",
                Runner::ContractOnly,
                "measured",
                "not-comparable",
                None,
            ),
        ];

        let findings = apply_beta_gate(&mut rows, Some(&waivers));

        assert!(findings.blockers.is_empty());
        assert_eq!(rows.first().expect("passing row").beta_gate_status, "pass");
        let waived = rows.get(1).expect("waived row");
        assert_eq!(waived.beta_gate_status, "waived-not-comparable");
        assert_eq!(
            waived.beta_gate_waiver_reason.as_deref(),
            Some("Pinned Stim has no public canonical printer CLI.")
        );
        assert_eq!(
            waived.beta_gate_waiver_follow_up.as_deref(),
            Some("Replace this row if Stim exposes one.")
        );
    }

    #[test]
    fn beta_gate_rejects_unwaived_unproven_rows_and_failing_ratios() {
        let mut rows = vec![
            row(
                "missing-row",
                Runner::StimCli,
                "measured",
                "not-comparable",
                None,
            ),
            row(
                "failing-row",
                Runner::StimPerf,
                "measured",
                "fail",
                Some(2.25),
            ),
        ];

        let findings = apply_beta_gate(&mut rows, None);

        assert_eq!(
            findings.blockers,
            vec![
                "missing-row: beta gate is not proven because status is not-comparable",
                "failing-row: ratio 2.250x exceeds 2.000x beta gate",
            ]
        );
        assert_eq!(
            rows.first().expect("missing row").beta_gate_status,
            "not-proven"
        );
        assert_eq!(rows.get(1).expect("failing row").beta_gate_status, "fail");
    }

    #[test]
    fn beta_gate_rejects_stale_or_misapplied_waivers() {
        let waivers = serde_json::from_str::<BetaWaivers>(
            r#"{
                "schema_version": 1,
                "rows": [
                    {
                        "id": "passing-row",
                        "reason": "stale",
                        "follow_up": "remove"
                    },
                    {
                        "id": "missing-row",
                        "reason": "not a contract row",
                        "follow_up": "fix baseline"
                    }
                ]
            }"#,
        )
        .expect("parse waivers");
        let mut rows = vec![
            row(
                "passing-row",
                Runner::StimCli,
                "measured",
                "pass",
                Some(1.0),
            ),
            row(
                "missing-row",
                Runner::StimCli,
                "measured",
                "not-comparable",
                None,
            ),
        ];

        let findings = apply_beta_gate(&mut rows, Some(&waivers));

        assert_eq!(
            findings.blockers,
            vec![
                "missing-row: beta waiver cannot apply because status is not-comparable, runner is stim-cli, comparability is direct-match, and row status is measured",
                "missing-row: beta waiver did not match a selected measured contract-only not-comparable row",
                "passing-row: beta waiver did not match a selected measured contract-only not-comparable row",
            ]
        );
    }

    #[test]
    fn beta_waivers_validate_schema_ids_and_required_text() {
        let waivers = serde_json::from_str::<BetaWaivers>(
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
            .validate(std::path::Path::new("waivers.json"))
            .expect_err("reject invalid waivers");

        let text = error.to_string();
        assert!(text.contains("schema_version=2 expected 1"));
        assert!(text.contains("../bad has unsafe id"));
        assert!(text.contains("../bad has empty reason"));
        assert!(text.contains("duplicate beta waiver row duplicate-row"));
        assert!(text.contains("duplicate-row has empty follow_up"));
    }

    #[test]
    fn m12_primary_beta_waivers_validate_source_file() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmarks/m12-primary-beta-waivers.json");
        let waivers = read_beta_waivers(&path).expect("read source-owned M12 beta waivers");

        assert_eq!(waivers.schema_version, 1);
        assert_eq!(waivers.rows.len(), 3);
    }

    fn row(
        id: &str,
        runner: Runner,
        status: &str,
        pass_fail_status: &str,
        ratio: Option<f64>,
    ) -> CompareRowResult {
        CompareRowResult {
            id: id.to_string(),
            milestone: Milestone::M12,
            threshold_class: "performance-gate".to_string(),
            runner,
            comparability: ComparabilityClass::DirectMatch,
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
            stab_median_seconds: ratio,
            relative_ratio: ratio,
            measurement_ratios: Vec::new(),
            stab_allocation_count_max: None,
            stab_allocation_bytes_max: None,
            pass_fail_status: pass_fail_status.to_string(),
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
