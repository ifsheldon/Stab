use std::collections::{BTreeMap, BTreeSet};

use super::artifacts::{
    validate_baseline_report_path, validate_binding, validate_compare_report_path,
    validate_profile_artifact_path, validate_revision, validate_semantic_witness_path,
};
use super::{
    CROSSING_RATIO, FocusedEvidenceLedger, MAX_DIAGNOSTICS, MAX_PHASES, SCHEMA_VERSION,
    focused_error, validate_positive_seconds, validate_profile, validate_ratio,
    validate_row_and_measurement,
};
use crate::error::BenchError;
use crate::manifest::is_safe_benchmark_id;

pub(super) fn validate_structure(ledger: &FocusedEvidenceLedger) -> Result<(), BenchError> {
    let mut issues = Vec::new();
    if ledger.schema_version != SCHEMA_VERSION {
        issues.push(format!(
            "schema_version={} expected {SCHEMA_VERSION}",
            ledger.schema_version
        ));
    }
    validate_revision(&ledger.source_revision, &mut issues);
    validate_binding("baseline_report", &ledger.baseline_report, &mut issues);
    validate_baseline_report_path("baseline_report", &ledger.baseline_report.path, &mut issues);
    validate_binding("matrix_report", &ledger.matrix_report, &mut issues);
    validate_compare_report_path("matrix_report", &ledger.matrix_report.path, &mut issues);
    if ledger.baseline_report.path == ledger.matrix_report.path
        || ledger.baseline_report.sha256 == ledger.matrix_report.sha256
    {
        issues.push("baseline_report and matrix_report must bind distinct artifacts".to_string());
    }
    let revision_prefix = ledger.source_revision.chars().take(8).collect::<String>();
    if !ledger.matrix_report.path.contains(&revision_prefix) {
        issues.push(format!(
            "matrix_report path {} does not bind the source revision prefix",
            ledger.matrix_report.path
        ));
    }
    if ledger.phases.is_empty() || ledger.phases.len() > MAX_PHASES {
        issues.push(format!(
            "phases must contain 1..={MAX_PHASES} entries, got {}",
            ledger.phases.len()
        ));
    }
    if ledger.diagnostics.len() > MAX_DIAGNOSTICS {
        issues.push(format!("diagnostics exceeds {MAX_DIAGNOSTICS} entries"));
    }

    let mut phase_keys = BTreeSet::new();
    let mut predecessor_seconds = BTreeMap::new();
    let mut required_crossings = BTreeSet::new();
    for phase in &ledger.phases {
        validate_row_and_measurement(&phase.row_id, &phase.measurement, &mut issues);
        let key = (phase.row_id.clone(), phase.measurement.clone());
        if !phase_keys.insert(key.clone()) {
            issues.push(format!(
                "duplicate phase {}/{}",
                phase.row_id, phase.measurement
            ));
        }
        validate_positive_seconds(
            &format!(
                "{}/{} source_current_seconds",
                phase.row_id, phase.measurement
            ),
            phase.source_current_seconds,
            &mut issues,
        );
        match (
            &phase.predecessor_report,
            phase.predecessor_seconds,
            phase.source_over_predecessor,
            phase.initial_seed_reason.as_deref(),
        ) {
            (Some(binding), Some(previous), Some(ratio), None) => {
                validate_binding("predecessor_report", binding, &mut issues);
                validate_compare_report_path("predecessor_report", &binding.path, &mut issues);
                if binding.path == ledger.matrix_report.path
                    || binding.sha256 == ledger.matrix_report.sha256
                    || binding.path == ledger.baseline_report.path
                    || binding.sha256 == ledger.baseline_report.sha256
                {
                    issues.push(format!(
                        "{}/{} reuses a current matrix or baseline artifact as its predecessor",
                        phase.row_id, phase.measurement
                    ));
                }
                validate_positive_seconds(
                    &format!("{}/{} predecessor_seconds", phase.row_id, phase.measurement),
                    previous,
                    &mut issues,
                );
                validate_ratio(
                    &format!(
                        "{}/{} source_over_predecessor",
                        phase.row_id, phase.measurement
                    ),
                    phase.source_current_seconds,
                    previous,
                    ratio,
                    &mut issues,
                );
                predecessor_seconds.insert(key.clone(), previous);
                if ratio > CROSSING_RATIO {
                    required_crossings.insert(key);
                }
            }
            (None, None, None, Some(reason)) if !reason.trim().is_empty() => {}
            _ => issues.push(format!(
                "{}/{} must be exactly one comparable predecessor triple or one nonempty initial_seed_reason",
                phase.row_id, phase.measurement
            )),
        }
    }

    let mut diagnostic_rows = BTreeSet::new();
    let mut diagnostic_paths = BTreeSet::new();
    let predecessor_paths = ledger
        .phases
        .iter()
        .filter_map(|phase| phase.predecessor_report.as_ref())
        .map(|binding| binding.path.as_str())
        .collect::<BTreeSet<_>>();
    let predecessor_digests = ledger
        .phases
        .iter()
        .filter_map(|phase| phase.predecessor_report.as_ref())
        .map(|binding| binding.sha256.as_str())
        .collect::<BTreeSet<_>>();
    let mut profile_paths = BTreeSet::new();
    let mut profile_digests = BTreeSet::new();
    let mut covered_crossings = BTreeSet::new();
    for diagnostic in &ledger.diagnostics {
        if !is_safe_benchmark_id(&diagnostic.row_id) {
            issues.push(format!(
                "diagnostic row {:?} is not a safe benchmark id",
                diagnostic.row_id
            ));
        }
        if !diagnostic_rows.insert(diagnostic.row_id.clone()) {
            issues.push(format!(
                "duplicate focused diagnostic row {}",
                diagnostic.row_id
            ));
        }
        validate_binding("focused report", &diagnostic.report, &mut issues);
        validate_compare_report_path("focused report", &diagnostic.report.path, &mut issues);
        validate_binding(
            "semantic witness source",
            &diagnostic.semantic_witness_source,
            &mut issues,
        );
        validate_semantic_witness_path(&diagnostic.semantic_witness_source.path, &mut issues);
        if !diagnostic_paths.insert(diagnostic.report.path.clone()) {
            issues.push(format!(
                "focused report path {} is reused by multiple rows",
                diagnostic.report.path
            ));
        }
        if diagnostic.report.path == ledger.matrix_report.path
            || diagnostic.report.sha256 == ledger.matrix_report.sha256
            || diagnostic.report.path == ledger.baseline_report.path
            || diagnostic.report.sha256 == ledger.baseline_report.sha256
            || predecessor_paths.contains(diagnostic.report.path.as_str())
            || predecessor_digests.contains(diagnostic.report.sha256.as_str())
        {
            issues.push(format!(
                "focused report {} reuses a matrix, baseline, or predecessor artifact",
                diagnostic.report.path
            ));
        }
        if !diagnostic.report.path.contains(&revision_prefix) {
            issues.push(format!(
                "focused report path {} does not bind the source revision prefix",
                diagnostic.report.path
            ));
        }
        if diagnostic.internal_timing_count < 8 {
            issues.push(format!(
                "{} retains {} internal timings, expected at least 8",
                diagnostic.row_id, diagnostic.internal_timing_count
            ));
        }
        if diagnostic.owner_action.trim().is_empty() {
            issues.push(format!("{} has an empty owner action", diagnostic.row_id));
        }
        if diagnostic.measurements.is_empty() {
            issues.push(format!("{} has no focused measurements", diagnostic.row_id));
        }

        let mut row_reproduces = false;
        for measurement in &diagnostic.measurements {
            validate_row_and_measurement(&diagnostic.row_id, &measurement.measurement, &mut issues);
            let key = (diagnostic.row_id.clone(), measurement.measurement.clone());
            if !covered_crossings.insert(key.clone()) {
                issues.push(format!(
                    "duplicate focused measurement {}/{}",
                    diagnostic.row_id, measurement.measurement
                ));
            }
            let Some(previous) = predecessor_seconds.get(&key).copied() else {
                issues.push(format!(
                    "focused measurement {}/{} has no comparable predecessor",
                    diagnostic.row_id, measurement.measurement
                ));
                continue;
            };
            validate_positive_seconds(
                &format!(
                    "{}/{} focused_seconds",
                    diagnostic.row_id, measurement.measurement
                ),
                measurement.focused_seconds,
                &mut issues,
            );
            validate_ratio(
                &format!(
                    "{}/{} focused_over_predecessor",
                    diagnostic.row_id, measurement.measurement
                ),
                measurement.focused_seconds,
                previous,
                measurement.focused_over_predecessor,
                &mut issues,
            );
            row_reproduces |= measurement.focused_over_predecessor > CROSSING_RATIO;
        }
        validate_profile(diagnostic, row_reproduces, &mut issues);
        if let Some(artifact) = &diagnostic.profile.artifact {
            validate_profile_artifact_path(&artifact.path, &mut issues);
            if !profile_paths.insert(artifact.path.as_str())
                || !profile_digests.insert(artifact.sha256.as_str())
            {
                issues.push(format!(
                    "{} reuses a hardware profile artifact",
                    diagnostic.row_id
                ));
            }
            if artifact.path == ledger.matrix_report.path
                || artifact.sha256 == ledger.matrix_report.sha256
                || artifact.path == ledger.baseline_report.path
                || artifact.sha256 == ledger.baseline_report.sha256
                || predecessor_paths.contains(artifact.path.as_str())
                || predecessor_digests.contains(artifact.sha256.as_str())
                || artifact.path == diagnostic.report.path
                || artifact.sha256 == diagnostic.report.sha256
                || artifact.path == diagnostic.semantic_witness_source.path
                || artifact.sha256 == diagnostic.semantic_witness_source.sha256
            {
                issues.push(format!(
                    "{} profile artifact collides with another evidence role",
                    diagnostic.row_id
                ));
            }
            if !artifact.path.contains(&revision_prefix) {
                issues.push(format!(
                    "{} profile artifact path {} does not bind the source revision prefix",
                    diagnostic.row_id, artifact.path
                ));
            }
        }
    }

    for missing in required_crossings.difference(&covered_crossings) {
        issues.push(format!(
            "missing focused diagnostic for {}/{}",
            missing.0, missing.1
        ));
    }
    for extra in covered_crossings.difference(&required_crossings) {
        issues.push(format!(
            "focused diagnostic {}/{} is not a source-current crossing",
            extra.0, extra.1
        ));
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(focused_error(issues.join("\n")))
    }
}
