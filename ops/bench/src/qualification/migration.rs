use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::model::{
    QualificationStatus, QualificationSuite, RowClassification, RowDecision, ThresholdPolicy,
};
use crate::error::BenchError;
use crate::root::RepoRoot;

const LEDGER_SCHEMA_VERSION: u32 = 1;
const MAX_LEDGER_BYTES: usize = 64 << 10;
const LEDGER_PATH: &str = "benchmarks/qualification-threshold-migrations.json";
const MEMORY_BASELINE_PATH: &str = "benchmarks/m12-primary-memory-baseline.json";
const CLIFFORD_MIGRATION_ID: &str = "m6-clifford-string-identity-small";
const CLIFFORD_AUTHORIZATION_REVISION: &str = "127d6661a9e00872fc4aa4c0b0d27171e005afa5";
const CLIFFORD_AUTHORIZATION_INVENTORY_SHA256: &str =
    "0ee3639389860799298164c94c647fcab45b03c9d67b941b1aad12c6e5e06df5";
const CLIFFORD_AUTHORIZATION_REPORT_SHA256: &str =
    "78fc10ca29e432641f3d978ed871c4b96d1ba344d714c20bf726f574239d2126";
const CLIFFORD_AUTHORIZATION_PREFLIGHT_SHA256: &str =
    "1acb62cf0606a6abc705bc9ac83068b80099a39585b67732de5aa6443e66d1a0";
const CLIFFORD_MIGRATION_REVISION: &str = "91f62d0a78659da2e8e264a6968b3c6cd32456de";
const CLIFFORD_MIGRATION_INVENTORY_SHA256: &str =
    "a76090c996ad404c1cb8bfa85066e286c6f40b32754b3750e984375f7ca90025";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ThresholdMigrationLedger {
    schema_version: u32,
    migrations: Vec<ThresholdMigration>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ThresholdMigration {
    id: String,
    legacy_row_id: String,
    legacy_stim_measurement: String,
    legacy_stab_measurement: String,
    replacement_group_id: String,
    replacement_measurement_id: String,
    replacement_scale_id: String,
    authorization_revision: String,
    authorization_performance_inventory_sha256: String,
    authorization_completion_path: String,
    authorization_completion_report_sha256: String,
    authorization_completion_preflight_sha256: String,
    migration_revision: String,
    migration_performance_inventory_sha256: String,
    retained_memory_baseline_id: String,
}

pub(super) fn check(root: &RepoRoot, suite: &QualificationSuite) -> Result<(), BenchError> {
    let path = root.path.join(LEDGER_PATH);
    let bytes = crate::source_file::read_repo_regular_file_bounded(root, &path, MAX_LEDGER_BYTES)?;
    super::io::preflight_json_shape(&bytes)?;
    let ledger: ThresholdMigrationLedger = serde_json::from_slice(&bytes)?;
    let mut canonical = serde_json::to_vec_pretty(&ledger)?;
    canonical.push(b'\n');
    if bytes != canonical {
        return qualification_error("threshold-migration ledger is not canonical JSON");
    }
    validate(root, suite, &ledger)
}

fn validate(
    root: &RepoRoot,
    suite: &QualificationSuite,
    ledger: &ThresholdMigrationLedger,
) -> Result<(), BenchError> {
    if ledger.schema_version != LEDGER_SCHEMA_VERSION {
        return qualification_error(format!(
            "threshold-migration schema is {}, expected {LEDGER_SCHEMA_VERSION}",
            ledger.schema_version
        ));
    }
    let mut ids = BTreeSet::new();
    let mut legacy_rows = BTreeSet::new();
    for migration in &ledger.migrations {
        if !ids.insert(migration.id.as_str()) {
            return qualification_error(format!(
                "threshold-migration ledger repeats id {}",
                migration.id
            ));
        }
        if !legacy_rows.insert(migration.legacy_row_id.as_str()) {
            return qualification_error(format!(
                "threshold-migration ledger repeats legacy row {}",
                migration.legacy_row_id
            ));
        }
        validate_digest(
            "authorization performance inventory",
            &migration.authorization_performance_inventory_sha256,
        )?;
        validate_digest(
            "authorization completion report",
            &migration.authorization_completion_report_sha256,
        )?;
        validate_digest(
            "authorization completion preflight",
            &migration.authorization_completion_preflight_sha256,
        )?;
        validate_digest(
            "migration performance inventory",
            &migration.migration_performance_inventory_sha256,
        )?;
        validate_revision("authorization revision", &migration.authorization_revision)?;
        validate_revision("migration revision", &migration.migration_revision)?;
        validate_current_migration(suite, migration)?;
        require_memory_baseline(root, &migration.retained_memory_baseline_id)?;
    }

    let clifford = ledger
        .migrations
        .iter()
        .find(|migration| migration.id == CLIFFORD_MIGRATION_ID)
        .ok_or_else(|| {
            BenchError::Qualification(
                "threshold-migration ledger omits the Clifford timing retirement".to_string(),
            )
        })?;
    if clifford.authorization_revision != CLIFFORD_AUTHORIZATION_REVISION
        || clifford.authorization_performance_inventory_sha256
            != CLIFFORD_AUTHORIZATION_INVENTORY_SHA256
        || clifford.authorization_completion_report_sha256 != CLIFFORD_AUTHORIZATION_REPORT_SHA256
        || clifford.authorization_completion_preflight_sha256
            != CLIFFORD_AUTHORIZATION_PREFLIGHT_SHA256
        || clifford.authorization_completion_path
            != "target/benchmarks/qualification/pq2-clifford-pre-migration-127d6661-identity-completion"
        || clifford.migration_revision != CLIFFORD_MIGRATION_REVISION
        || clifford.migration_performance_inventory_sha256 != CLIFFORD_MIGRATION_INVENTORY_SHA256
        || clifford.legacy_row_id != "m6-clifford-string"
        || clifford.legacy_stim_measurement != "CliffordString_multiplication_10K"
        || clifford.legacy_stab_measurement != "stab_clifford_string_multiplication_10K"
        || clifford.replacement_group_id != "PERFQ-M6-CLIFFORD-STRING"
        || clifford.replacement_measurement_id != "right-multiply-identity"
        || clifford.replacement_scale_id != "small"
        || clifford.retained_memory_baseline_id != "m6-clifford-string"
    {
        return qualification_error(
            "Clifford threshold migration differs from the reviewed authorization",
        );
    }
    Ok(())
}

fn validate_current_migration(
    suite: &QualificationSuite,
    migration: &ThresholdMigration,
) -> Result<(), BenchError> {
    let row = suite
        .manifest_rows
        .iter()
        .find(|row| row.id == migration.legacy_row_id)
        .ok_or_else(|| {
            BenchError::Qualification(format!(
                "threshold migration {} references missing legacy row {}",
                migration.id, migration.legacy_row_id
            ))
        })?;
    if row.decision != RowDecision::Superseded
        || row.primary_group_id != migration.replacement_group_id
        || !row.classifications.contains(&RowClassification::Duplicate)
        || !row.threshold_refs.is_empty()
        || row.threshold_max_relative_ratio.is_some()
        || !row.threshold_measurement_pairs.is_empty()
        || !row.replacement_contracts.is_empty()
    {
        return qualification_error(format!(
            "threshold migration {} does not match the retired legacy row",
            migration.id
        ));
    }
    let group = suite
        .qualification_groups
        .iter()
        .find(|group| group.id == migration.replacement_group_id)
        .ok_or_else(|| {
            BenchError::Qualification(format!(
                "threshold migration {} references missing replacement group {}",
                migration.id, migration.replacement_group_id
            ))
        })?;
    if group.status != QualificationStatus::Implemented
        || group.threshold_policy != ThresholdPolicy::Primary1_25
        || !group
            .workload_family
            .scales
            .iter()
            .any(|scale| scale.id == migration.replacement_scale_id)
        || migration.replacement_measurement_id.is_empty()
        || migration.legacy_stim_measurement.is_empty()
        || migration.legacy_stab_measurement.is_empty()
        || migration.authorization_completion_path.is_empty()
    {
        return qualification_error(format!(
            "threshold migration {} lacks an implemented exact replacement",
            migration.id
        ));
    }
    Ok(())
}

fn require_memory_baseline(root: &RepoRoot, expected_id: &str) -> Result<(), BenchError> {
    let path = root.path.join(MEMORY_BASELINE_PATH);
    let bytes = crate::source_file::read_repo_regular_file_bounded(root, &path, 1 << 20)?;
    super::io::preflight_json_shape(&bytes)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let rows = value
        .get("rows")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            BenchError::Qualification("memory baseline lacks a rows array".to_string())
        })?;
    let matches = rows
        .iter()
        .filter(|row| row.get("id").and_then(serde_json::Value::as_str) == Some(expected_id))
        .count();
    if matches != 1 {
        return qualification_error(format!(
            "threshold migration requires exactly one retained memory baseline {expected_id}, found {matches}"
        ));
    }
    Ok(())
}

fn validate_digest(name: &str, value: &str) -> Result<(), BenchError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        qualification_error(format!(
            "threshold-migration {name} is not a SHA-256 digest"
        ))
    }
}

fn validate_revision(name: &str, value: &str) -> Result<(), BenchError> {
    if value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        qualification_error(format!("threshold-migration {name} is not a Git revision"))
    }
}

fn qualification_error<T>(message: impl Into<String>) -> Result<T, BenchError> {
    Err(BenchError::Qualification(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (RepoRoot, QualificationSuite, ThresholdMigrationLedger) {
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repository root");
        let root = RepoRoot::resolve(repository).expect("resolve repository");
        let suite: QualificationSuite = serde_json::from_slice(
            &std::fs::read(root.performance_qualification()).expect("qualification suite"),
        )
        .expect("parse qualification suite");
        let ledger: ThresholdMigrationLedger = serde_json::from_slice(
            &std::fs::read(root.path.join(LEDGER_PATH)).expect("migration ledger"),
        )
        .expect("parse migration ledger");
        (root, suite, ledger)
    }

    #[test]
    fn source_owned_migration_ledger_matches_the_current_suite() {
        let (root, suite, _) = fixture();

        check(&root, &suite).expect("valid migration ledger");
    }

    #[test]
    fn migration_ledger_rejects_a_refingerprinted_authorization() {
        let (root, suite, mut ledger) = fixture();
        ledger
            .migrations
            .first_mut()
            .expect("Clifford migration")
            .authorization_completion_report_sha256 = "f".repeat(64);

        assert!(
            validate(&root, &suite, &ledger)
                .expect_err("refingerprinted authorization must fail")
                .to_string()
                .contains("reviewed authorization")
        );
    }

    #[test]
    fn migration_ledger_rejects_a_reopened_legacy_row() {
        let (root, mut suite, ledger) = fixture();
        suite
            .manifest_rows
            .iter_mut()
            .find(|row| row.id == "m6-clifford-string")
            .expect("Clifford row")
            .decision = RowDecision::Reworked;

        assert!(
            validate(&root, &suite, &ledger)
                .expect_err("reopened legacy row must fail")
                .to_string()
                .contains("retired legacy row")
        );
    }
}
