use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{focused_error, validate_row_and_measurement};
use crate::error::BenchError;
use crate::manifest::{BenchmarkManifest, Milestone, ThresholdClass};
use crate::report::CompareReport;
use crate::root::RepoRoot;
use crate::source_file::read_repo_regular_file_bounded;

const CONTRACT_PATH: &str = "benchmarks/a6-measurement-contract.json";
const THRESHOLD_PATH: &str = "benchmarks/m12-primary-thresholds.json";
const CONTRACT_SCHEMA_VERSION: u32 = 2;
const SEMANTIC_PREFLIGHT_CONTRACT: &str = "gated-exact-output-v1";
const SELECTED_EXACT_PREFLIGHT_ROW: &str = "m6-clifford-string";
const MAX_CONTRACT_BYTES: usize = 1 << 20;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct A6MeasurementContract {
    schema_version: u32,
    provenance: String,
    semantic_preflight_contract: String,
    exact_preflight_rows: Vec<String>,
    rows: Vec<MeasurementContractRow>,
    #[serde(skip)]
    source_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasurementContractRow {
    id: String,
    measurements: Vec<String>,
}

impl A6MeasurementContract {
    pub(super) fn read_and_validate(
        root: &RepoRoot,
        manifest: &BenchmarkManifest,
    ) -> Result<Self, BenchError> {
        let path = root.resolve_relative(Path::new(CONTRACT_PATH));
        let bytes = read_repo_regular_file_bounded(root, &path, MAX_CONTRACT_BYTES)?;
        let mut contract: Self = serde_json::from_slice(&bytes)
            .map_err(|error| focused_error(format!("failed to parse {CONTRACT_PATH}: {error}")))?;
        contract.source_sha256 = hex::encode(Sha256::digest(&bytes));
        contract.validate(root, manifest)?;
        Ok(contract)
    }

    pub(super) fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    pub(super) fn require_report(&self, report: &CompareReport) -> Result<(), BenchError> {
        if report.rows.len() != self.rows.len() {
            return Err(focused_error(format!(
                "matrix has {} rows but the A6 measurement contract has {}",
                report.rows.len(),
                self.rows.len()
            )));
        }
        for (actual, expected) in report.rows.iter().zip(&self.rows) {
            let actual_names = actual
                .stab_measurements
                .iter()
                .map(|measurement| measurement.name.as_str())
                .collect::<Vec<_>>();
            let expected_names = expected
                .measurements
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            if actual.id != expected.id || actual_names != expected_names {
                return Err(focused_error(format!(
                    "matrix measurement identities for {} differ from the checked A6 contract",
                    expected.id
                )));
            }
        }
        Ok(())
    }

    fn validate(&self, root: &RepoRoot, manifest: &BenchmarkManifest) -> Result<(), BenchError> {
        let expected = manifest
            .rows
            .iter()
            .filter(|row| row.milestone != Milestone::M12)
            .collect::<Vec<_>>();
        let mut issues = Vec::new();
        if self.schema_version != CONTRACT_SCHEMA_VERSION {
            issues.push(format!(
                "measurement contract schema_version={} expected {CONTRACT_SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        if self.provenance.trim().is_empty() {
            issues.push("measurement contract provenance is empty".to_string());
        }
        if self.semantic_preflight_contract != SEMANTIC_PREFLIGHT_CONTRACT {
            issues.push(format!(
                "measurement contract semantic_preflight_contract={} expected {SEMANTIC_PREFLIGHT_CONTRACT}",
                self.semantic_preflight_contract
            ));
        }
        let threshold_path = root.resolve_relative(Path::new(THRESHOLD_PATH));
        let threshold_bytes =
            read_repo_regular_file_bounded(root, &threshold_path, MAX_CONTRACT_BYTES)?;
        let thresholds =
            crate::thresholds::parse_thresholds(Path::new(THRESHOLD_PATH), &threshold_bytes)?;
        let mut expected_preflight_rows = thresholds
            .row_ids()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        expected_preflight_rows.insert(SELECTED_EXACT_PREFLIGHT_ROW.to_string());
        let actual_preflight_rows = self
            .exact_preflight_rows
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if actual_preflight_rows.len() != self.exact_preflight_rows.len() {
            issues.push("measurement contract repeats an exact_preflight_rows id".to_string());
        }
        if actual_preflight_rows != expected_preflight_rows {
            issues.push(format!(
                "measurement contract exact_preflight_rows differ from threshold-owned rows plus {SELECTED_EXACT_PREFLIGHT_ROW}"
            ));
        }
        if self.rows.len() != expected.len() || expected.len() != 166 {
            issues.push(format!(
                "measurement contract has {} rows, expected 166",
                self.rows.len()
            ));
        }
        let mut row_ids = BTreeSet::new();
        for (index, actual) in self.rows.iter().enumerate() {
            if !row_ids.insert(actual.id.as_str()) {
                issues.push(format!("measurement contract repeats row {}", actual.id));
            }
            let Some(expected_row) = expected.get(index) else {
                continue;
            };
            if actual.id != expected_row.id {
                issues.push(format!(
                    "measurement contract row {index} is {}, expected {}",
                    actual.id, expected_row.id
                ));
            }
            if expected_row.threshold_class == ThresholdClass::BaselineMetadata {
                if !actual.measurements.is_empty() {
                    issues.push(format!(
                        "metadata row {} has runtime measurements",
                        actual.id
                    ));
                }
            } else if actual.measurements.is_empty() {
                issues.push(format!(
                    "executable row {} has no measurement identities",
                    actual.id
                ));
            }
            let mut measurements = BTreeSet::new();
            for measurement in &actual.measurements {
                validate_row_and_measurement(&actual.id, measurement, &mut issues);
                if !measurements.insert(measurement.as_str()) {
                    issues.push(format!(
                        "measurement contract repeats {}/{}",
                        actual.id, measurement
                    ));
                }
            }
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(focused_error(issues.join("\n")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_contract() -> (RepoRoot, BenchmarkManifest, A6MeasurementContract) {
        let root = RepoRoot::resolve(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("repository root"),
        )
        .expect("resolve root");
        let manifest = BenchmarkManifest::read(&root).expect("read manifest");
        let contract =
            A6MeasurementContract::read_and_validate(&root, &manifest).expect("read contract");
        (root, manifest, contract)
    }

    #[test]
    fn checked_contract_matches_the_exact_pre_m12_manifest() {
        let (root, manifest, contract) = repository_contract();
        contract.validate(&root, &manifest).expect("valid contract");
        assert_eq!(contract.rows.len(), 166);
        assert_eq!(
            contract
                .rows
                .iter()
                .map(|row| row.measurements.len())
                .sum::<usize>(),
            309
        );
    }

    #[test]
    fn contract_rejects_missing_or_duplicate_measurement_identity() {
        let (root, manifest, mut contract) = repository_contract();
        let row = contract
            .rows
            .iter_mut()
            .find(|row| row.measurements.len() > 1)
            .expect("multi-measurement row");
        row.measurements.pop();
        let duplicate = row
            .measurements
            .first()
            .expect("remaining measurement")
            .clone();
        row.measurements.push(duplicate);
        let error = contract
            .validate(&root, &manifest)
            .expect_err("duplicate measurement identity");
        assert!(error.to_string().contains("measurement contract repeats"));
    }

    #[test]
    fn contract_rejects_a_stale_semantic_preflight_version() {
        let (root, manifest, mut contract) = repository_contract();
        contract.semantic_preflight_contract = "source-path-exists-v0".to_string();
        let error = contract
            .validate(&root, &manifest)
            .expect_err("stale semantic preflight contract");
        assert!(error.to_string().contains("semantic_preflight_contract"));
    }

    #[test]
    fn contract_rejects_missing_or_extra_gated_preflight_rows() {
        let (root, manifest, mut contract) = repository_contract();
        contract.exact_preflight_rows.pop();
        contract
            .exact_preflight_rows
            .push("report-only-not-gated".to_string());
        let error = contract
            .validate(&root, &manifest)
            .expect_err("gated preflight set must be exact");
        assert!(error.to_string().contains("exact_preflight_rows differ"));
    }
}
