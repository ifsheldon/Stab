use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::model::{
    QualificationStatus, QualificationSuite, RowClassification, RowDecision, ThresholdPolicy,
};
use crate::error::BenchError;
use crate::root::RepoRoot;

const LEDGER_SCHEMA_VERSION: u32 = 2;
const MAX_LEDGER_BYTES: usize = 64 << 10;
const LEDGER_PATH: &str = "benchmarks/qualification-threshold-migrations.json";
const MEMORY_BASELINE_PATH: &str = "benchmarks/m12-primary-memory-baseline.json";
const REVIEWED_MIGRATIONS: [ReviewedMigration; 3] = [
    ReviewedMigration {
        id: "m6-clifford-string-identity-small",
        legacy_guard_kind: LegacyGuardKind::TimingThreshold,
        legacy_row_id: "m6-clifford-string",
        legacy_stim_measurement: Some("CliffordString_multiplication_10K"),
        legacy_stab_measurement: Some("stab_clifford_string_multiplication_10K"),
        replacement_group_id: "PERFQ-M6-CLIFFORD-STRING",
        replacement_measurement_id: "right-multiply-identity",
        replacement_scale_id: Some("small"),
        authorization_revision: "127d6661a9e00872fc4aa4c0b0d27171e005afa5",
        authorization_performance_inventory_sha256: "0ee3639389860799298164c94c647fcab45b03c9d67b941b1aad12c6e5e06df5",
        authorization_completion_path: "target/benchmarks/qualification/pq2-clifford-pre-migration-127d6661-identity-completion",
        authorization_completion_report_sha256: "78fc10ca29e432641f3d978ed871c4b96d1ba344d714c20bf726f574239d2126",
        authorization_completion_preflight_sha256: "1acb62cf0606a6abc705bc9ac83068b80099a39585b67732de5aa6443e66d1a0",
        migration_revision: "91f62d0a78659da2e8e264a6968b3c6cd32456de",
        migration_performance_inventory_sha256: "a76090c996ad404c1cb8bfa85066e286c6f40b32754b3750e984375f7ca90025",
        retained_memory_baseline_id: "m6-clifford-string",
    },
    ReviewedMigration {
        id: "m10-dem-parse-scale-family",
        legacy_guard_kind: LegacyGuardKind::TimingThreshold,
        legacy_row_id: "m10-dem-parse-contract",
        legacy_stim_measurement: Some("m10-dem-parse-contract"),
        legacy_stab_measurement: Some("stab_dem_parse_sample"),
        replacement_group_id: "PERFQ-M10-DEM-PARSE-CONTRACT",
        replacement_measurement_id: "parse",
        replacement_scale_id: None,
        authorization_revision: "d9e2405d18cfff05d9b5d908525394476b0edcbc",
        authorization_performance_inventory_sha256: "a98f57cf194f3a021d321266656cf688c9f7780fb39fa337475e8132411eb88a",
        authorization_completion_path: "target/benchmarks/qualification/pq2-dem-d9e2405-parse-completion",
        authorization_completion_report_sha256: "3445c71b5453c7b1c31906bf0021e905271f317320333ecaf7592df5864e31e9",
        authorization_completion_preflight_sha256: "62a6124b861d95865245199dc90b0d80c51703323a4e59ad7c34fda742c0972d",
        migration_revision: "1cfecd64cde4a5effdf07fdaabdbe51017e25a4a",
        migration_performance_inventory_sha256: "3f51801b592b0cb8dc3b340cced3dc3b7644b913168073c7d4106188e444d83d",
        retained_memory_baseline_id: "m10-dem-parse-contract",
    },
    ReviewedMigration {
        id: "m10-dem-print-scale-family",
        legacy_guard_kind: LegacyGuardKind::NoRatioWaiver,
        legacy_row_id: "m10-dem-print-contract",
        legacy_stim_measurement: None,
        legacy_stab_measurement: None,
        replacement_group_id: "PERFQ-M10-DEM-PRINT-CONTRACT",
        replacement_measurement_id: "serialize",
        replacement_scale_id: None,
        authorization_revision: "d9e2405d18cfff05d9b5d908525394476b0edcbc",
        authorization_performance_inventory_sha256: "a98f57cf194f3a021d321266656cf688c9f7780fb39fa337475e8132411eb88a",
        authorization_completion_path: "target/benchmarks/qualification/pq2-dem-d9e2405-print-completion",
        authorization_completion_report_sha256: "4597e10933cf211a5f7984de377b8946a1ab4f1f4569a77983e86df22e67c38b",
        authorization_completion_preflight_sha256: "2b2a8c2691a4caba611e0a2a18f8bdd78f584b5ae221966abbc4621a949a33f9",
        migration_revision: "1cfecd64cde4a5effdf07fdaabdbe51017e25a4a",
        migration_performance_inventory_sha256: "3f51801b592b0cb8dc3b340cced3dc3b7644b913168073c7d4106188e444d83d",
        retained_memory_baseline_id: "m10-dem-print-contract",
    },
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum LegacyGuardKind {
    TimingThreshold,
    NoRatioWaiver,
}

struct ReviewedMigration {
    id: &'static str,
    legacy_guard_kind: LegacyGuardKind,
    legacy_row_id: &'static str,
    legacy_stim_measurement: Option<&'static str>,
    legacy_stab_measurement: Option<&'static str>,
    replacement_group_id: &'static str,
    replacement_measurement_id: &'static str,
    replacement_scale_id: Option<&'static str>,
    authorization_revision: &'static str,
    authorization_performance_inventory_sha256: &'static str,
    authorization_completion_path: &'static str,
    authorization_completion_report_sha256: &'static str,
    authorization_completion_preflight_sha256: &'static str,
    migration_revision: &'static str,
    migration_performance_inventory_sha256: &'static str,
    retained_memory_baseline_id: &'static str,
}

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
    legacy_guard_kind: LegacyGuardKind,
    legacy_row_id: String,
    legacy_stim_measurement: Option<String>,
    legacy_stab_measurement: Option<String>,
    replacement_group_id: String,
    replacement_measurement_id: String,
    replacement_scale_id: Option<String>,
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
    if ledger.migrations.len() != REVIEWED_MIGRATIONS.len() {
        return qualification_error(format!(
            "threshold-migration ledger has {} records, expected exactly {} reviewed records",
            ledger.migrations.len(),
            REVIEWED_MIGRATIONS.len()
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
        validate_legacy_guard(migration)?;
        validate_current_migration(root, suite, migration)?;
        require_memory_baseline(root, &migration.retained_memory_baseline_id)?;
    }

    for reviewed in &REVIEWED_MIGRATIONS {
        let migration = ledger
            .migrations
            .iter()
            .find(|migration| migration.id == reviewed.id)
            .ok_or_else(|| {
                BenchError::Qualification(format!(
                    "threshold-migration ledger omits reviewed migration {}",
                    reviewed.id
                ))
            })?;
        if !reviewed.matches(migration) {
            return qualification_error(format!(
                "threshold migration {} differs from the reviewed authorization",
                reviewed.id
            ));
        }
    }
    Ok(())
}

impl ReviewedMigration {
    fn matches(&self, migration: &ThresholdMigration) -> bool {
        migration.id == self.id
            && migration.legacy_guard_kind == self.legacy_guard_kind
            && migration.legacy_row_id == self.legacy_row_id
            && migration.legacy_stim_measurement.as_deref() == self.legacy_stim_measurement
            && migration.legacy_stab_measurement.as_deref() == self.legacy_stab_measurement
            && migration.replacement_group_id == self.replacement_group_id
            && migration.replacement_measurement_id == self.replacement_measurement_id
            && migration.replacement_scale_id.as_deref() == self.replacement_scale_id
            && migration.authorization_revision == self.authorization_revision
            && migration.authorization_performance_inventory_sha256
                == self.authorization_performance_inventory_sha256
            && migration.authorization_completion_path == self.authorization_completion_path
            && migration.authorization_completion_report_sha256
                == self.authorization_completion_report_sha256
            && migration.authorization_completion_preflight_sha256
                == self.authorization_completion_preflight_sha256
            && migration.migration_revision == self.migration_revision
            && migration.migration_performance_inventory_sha256
                == self.migration_performance_inventory_sha256
            && migration.retained_memory_baseline_id == self.retained_memory_baseline_id
    }
}

fn validate_legacy_guard(migration: &ThresholdMigration) -> Result<(), BenchError> {
    let named_measurements = migration
        .legacy_stim_measurement
        .as_deref()
        .zip(migration.legacy_stab_measurement.as_deref());
    match (migration.legacy_guard_kind, named_measurements) {
        (LegacyGuardKind::TimingThreshold, Some((stim, stab)))
            if !stim.is_empty() && !stab.is_empty() =>
        {
            Ok(())
        }
        (LegacyGuardKind::NoRatioWaiver, None)
            if migration.legacy_stim_measurement.is_none()
                && migration.legacy_stab_measurement.is_none() =>
        {
            Ok(())
        }
        (LegacyGuardKind::TimingThreshold, _) => qualification_error(format!(
            "threshold migration {} must name both legacy timing measurements",
            migration.id
        )),
        (LegacyGuardKind::NoRatioWaiver, _) => qualification_error(format!(
            "waiver migration {} must not invent legacy timing measurements",
            migration.id
        )),
    }
}

fn validate_current_migration(
    root: &RepoRoot,
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
        || !row.waiver_refs.is_empty()
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
        || migration
            .replacement_scale_id
            .as_ref()
            .is_some_and(|scale_id| {
                !group
                    .workload_family
                    .scales
                    .iter()
                    .any(|scale| scale.id == *scale_id)
            })
        || group.workload_family.scales.is_empty()
        || migration.replacement_measurement_id.is_empty()
        || migration.authorization_completion_path.is_empty()
    {
        return qualification_error(format!(
            "threshold migration {} lacks an implemented exact replacement",
            migration.id
        ));
    }
    super::runtime::validate_migration_target(
        root,
        &suite.semantic_digest,
        &migration.replacement_group_id,
        &migration.replacement_measurement_id,
        migration.replacement_scale_id.as_deref(),
    )
    .map_err(BenchError::Qualification)?;
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

    #[test]
    fn dem_parse_migration_requires_the_complete_scale_family() {
        let (root, suite, mut ledger) = fixture();
        ledger
            .migrations
            .iter_mut()
            .find(|migration| migration.id == "m10-dem-parse-scale-family")
            .expect("DEM parse migration")
            .replacement_scale_id = Some("small".to_string());

        assert!(
            validate(&root, &suite, &ledger)
                .expect_err("a single scale cannot authorize the family migration")
                .to_string()
                .contains("differs from the reviewed authorization")
        );
    }

    #[test]
    fn dem_print_waiver_migration_rejects_invented_timing_measurements() {
        let (root, suite, mut ledger) = fixture();
        ledger
            .migrations
            .iter_mut()
            .find(|migration| migration.id == "m10-dem-print-scale-family")
            .expect("DEM print migration")
            .legacy_stim_measurement = Some("invented".to_string());

        assert!(
            validate(&root, &suite, &ledger)
                .expect_err("a no-ratio waiver has no legacy Stim measurement")
                .to_string()
                .contains("must not invent legacy timing measurements")
        );
    }

    #[test]
    fn migration_ledger_rejects_a_missing_runtime_measurement() {
        let (root, suite, mut ledger) = fixture();
        ledger
            .migrations
            .iter_mut()
            .find(|migration| migration.id == "m10-dem-parse-scale-family")
            .expect("DEM parse migration")
            .replacement_measurement_id = "missing-measurement".to_string();

        assert!(
            validate(&root, &suite, &ledger)
                .expect_err("replacement measurement must exist in the runtime contract")
                .to_string()
                .contains("does not exist at the requested scale")
        );
    }

    #[test]
    fn migration_ledger_rejects_an_unreviewed_extra_record() {
        let (root, suite, mut ledger) = fixture();
        let mut extra = ledger
            .migrations
            .last()
            .expect("reviewed migration")
            .clone();
        extra.id = "unreviewed-extra".to_string();
        extra.legacy_row_id = "m7-convert-stim-canonical".to_string();
        ledger.migrations.push(extra);

        assert!(
            validate(&root, &suite, &ledger)
                .expect_err("unreviewed migrations must fail closed")
                .to_string()
                .contains("expected exactly 3 reviewed records")
        );
    }
}
