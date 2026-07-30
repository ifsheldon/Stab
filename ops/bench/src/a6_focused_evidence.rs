//! Checked A6 report-only crossing and focused-diagnostic evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use clap::Args;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::BenchError;
use crate::manifest::is_safe_benchmark_id;
use crate::report::{CompareReport, Measurement};
use crate::root::RepoRoot;
use crate::source_file::open_regular_file_bounded_descriptor;

const SCHEMA_VERSION: u32 = 1;
const MAX_LEDGER_BYTES: u64 = 1 << 20;
const MAX_REPORT_BYTES: u64 = 64 << 20;
const MAX_PHASES: usize = 256;
const MAX_DIAGNOSTICS: usize = 128;
const CROSSING_RATIO: f64 = 1.15;
const RATIO_TOLERANCE: f64 = 1e-9;

#[derive(Debug, Args)]
pub(crate) struct A6FocusedEvidenceArgs {
    /// Checked source ledger describing every report-only phase and focused crossing.
    #[arg(
        long,
        default_value = "benchmarks/a6-focused-evidence.json",
        value_name = "PATH"
    )]
    ledger: PathBuf,

    /// Reopen every bound report and source file and verify its SHA-256 and recorded values.
    #[arg(long)]
    verify_artifacts: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FocusedEvidenceLedger {
    schema_version: u32,
    source_revision: String,
    matrix_report: ArtifactBinding,
    phases: Vec<PhaseEvidence>,
    diagnostics: Vec<FocusedDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactBinding {
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PhaseEvidence {
    row_id: String,
    measurement: String,
    source_current_seconds: f64,
    predecessor_report: Option<ArtifactBinding>,
    predecessor_seconds: Option<f64>,
    source_over_predecessor: Option<f64>,
    initial_seed_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FocusedDiagnostic {
    row_id: String,
    report: ArtifactBinding,
    semantic_witness_source: ArtifactBinding,
    internal_timing_count: usize,
    measurements: Vec<FocusedMeasurement>,
    profile: ProfileDisposition,
    disposition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FocusedMeasurement {
    measurement: String,
    focused_seconds: f64,
    focused_over_predecessor: f64,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ProfileStatus {
    Captured,
    Unavailable,
    NotRequired,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileDisposition {
    status: ProfileStatus,
    detail: String,
    artifact: Option<ArtifactBinding>,
}

pub(crate) fn check(root: &RepoRoot, args: A6FocusedEvidenceArgs) -> Result<(), BenchError> {
    let ledger_path = root.resolve_relative(&args.ledger);
    let bytes = read_bounded(&ledger_path, MAX_LEDGER_BYTES)?;
    let ledger: FocusedEvidenceLedger = serde_json::from_slice(&bytes).map_err(|error| {
        focused_error(format!(
            "failed to parse {}: {error}",
            args.ledger.display()
        ))
    })?;
    validate_structure(&ledger)?;
    if args.verify_artifacts {
        verify_artifacts(root, &ledger)?;
    }
    println!(
        "[stab-bench] A6 focused evidence OK: {} phases, {} focused row(s), artifacts_verified={}",
        ledger.phases.len(),
        ledger.diagnostics.len(),
        args.verify_artifacts
    );
    Ok(())
}

fn validate_structure(ledger: &FocusedEvidenceLedger) -> Result<(), BenchError> {
    let mut issues = Vec::new();
    if ledger.schema_version != SCHEMA_VERSION {
        issues.push(format!(
            "schema_version={} expected {SCHEMA_VERSION}",
            ledger.schema_version
        ));
    }
    validate_revision(&ledger.source_revision, &mut issues);
    validate_binding("matrix_report", &ledger.matrix_report, &mut issues);
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
                validate_positive_seconds(
                    &format!("{}/{} predecessor_seconds", phase.row_id, phase.measurement),
                    previous,
                    &mut issues,
                );
                validate_ratio(
                    &format!("{}/{} source_over_predecessor", phase.row_id, phase.measurement),
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
        validate_binding(
            "semantic witness source",
            &diagnostic.semantic_witness_source,
            &mut issues,
        );
        if !diagnostic_paths.insert(diagnostic.report.path.clone()) {
            issues.push(format!(
                "focused report path {} is reused by multiple rows",
                diagnostic.report.path
            ));
        }
        let revision_prefix = ledger.source_revision.chars().take(8).collect::<String>();
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
        if diagnostic.disposition.trim().is_empty() {
            issues.push(format!("{} has an empty disposition", diagnostic.row_id));
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

fn validate_profile(
    diagnostic: &FocusedDiagnostic,
    row_reproduces: bool,
    issues: &mut Vec<String>,
) {
    if diagnostic.profile.detail.trim().is_empty() {
        issues.push(format!("{} has an empty profile detail", diagnostic.row_id));
    }
    match (&diagnostic.profile.status, &diagnostic.profile.artifact) {
        (ProfileStatus::Captured, Some(binding)) => {
            validate_binding("hardware profile artifact", binding, issues);
        }
        (ProfileStatus::Captured, None) => issues.push(format!(
            "{} says a profile was captured but has no artifact",
            diagnostic.row_id
        )),
        (ProfileStatus::Unavailable | ProfileStatus::NotRequired, Some(_)) => issues.push(format!(
            "{} has a profile artifact with status {:?}",
            diagnostic.row_id, diagnostic.profile.status
        )),
        (ProfileStatus::NotRequired, None) if row_reproduces => issues.push(format!(
            "{} reproduces above {CROSSING_RATIO:.2}x but marks profiling not required",
            diagnostic.row_id
        )),
        _ => {}
    }
}

fn verify_artifacts(root: &RepoRoot, ledger: &FocusedEvidenceLedger) -> Result<(), BenchError> {
    let matrix = read_bound_report(root, &ledger.matrix_report)?;
    require_matrix_contract(&matrix, &ledger.source_revision)?;
    for phase in &ledger.phases {
        let current = find_measurement(&matrix, &phase.row_id, &phase.measurement)?;
        require_recorded_seconds(
            &format!("{}/{} matrix", phase.row_id, phase.measurement),
            current.seconds,
            phase.source_current_seconds,
        )?;
        if let (Some(binding), Some(expected)) =
            (&phase.predecessor_report, phase.predecessor_seconds)
        {
            let report = read_bound_report(root, binding)?;
            if report.stab.local_modifications {
                return Err(focused_error(format!(
                    "predecessor report {} has local modifications",
                    binding.path
                )));
            }
            let measurement = find_measurement(&report, &phase.row_id, &phase.measurement)?;
            require_recorded_seconds(
                &format!("{}/{} predecessor", phase.row_id, phase.measurement),
                measurement.seconds,
                expected,
            )?;
        }
    }

    for diagnostic in &ledger.diagnostics {
        verify_binding(root, &diagnostic.semantic_witness_source, MAX_REPORT_BYTES)?;
        if let Some(artifact) = &diagnostic.profile.artifact {
            verify_binding(root, artifact, MAX_REPORT_BYTES)?;
        }
        let report = read_bound_report(root, &diagnostic.report)?;
        require_focused_contract(&report, diagnostic, &ledger.source_revision)?;
    }
    Ok(())
}

fn require_matrix_contract(report: &CompareReport, revision: &str) -> Result<(), BenchError> {
    if report.stab.commit != revision
        || report.stab.local_modifications
        || report.command.profile != "release"
        || !report.command.warmup
        || report.command.measurement_runs != 3
        || report.rows.len() != 166
    {
        return Err(focused_error(
            "matrix report does not bind the clean 166-row warmed three-run A6 contract",
        ));
    }
    Ok(())
}

fn require_focused_contract(
    report: &CompareReport,
    diagnostic: &FocusedDiagnostic,
    revision: &str,
) -> Result<(), BenchError> {
    if report.stab.commit != revision
        || report.stab.local_modifications
        || report.command.profile != "release"
        || !report.command.warmup
        || report.command.measurement_runs != 1
        || report.command.filters != [diagnostic.row_id.as_str()]
        || report.rows.len() != 1
    {
        return Err(focused_error(format!(
            "{} does not bind one clean warmed outer run for {}",
            diagnostic.report.path, diagnostic.row_id
        )));
    }
    for expected in &diagnostic.measurements {
        let measurement = find_measurement(report, &diagnostic.row_id, &expected.measurement)?;
        require_recorded_seconds(
            &format!("{}/{} focused", diagnostic.row_id, expected.measurement),
            measurement.seconds,
            expected.focused_seconds,
        )?;
        if measurement.iterations != Some(diagnostic.internal_timing_count) {
            return Err(focused_error(format!(
                "{}/{} records {:?} internal timings, expected {}",
                diagnostic.row_id,
                expected.measurement,
                measurement.iterations,
                diagnostic.internal_timing_count
            )));
        }
    }
    Ok(())
}

fn find_measurement<'a>(
    report: &'a CompareReport,
    row_id: &str,
    name: &str,
) -> Result<&'a Measurement, BenchError> {
    let row = report
        .rows
        .iter()
        .find(|row| row.id == row_id)
        .ok_or_else(|| focused_error(format!("report omits row {row_id}")))?;
    row.stab_measurements
        .iter()
        .find(|measurement| measurement.name == name)
        .ok_or_else(|| focused_error(format!("report omits measurement {row_id}/{name}")))
}

fn read_bound_report(
    root: &RepoRoot,
    binding: &ArtifactBinding,
) -> Result<CompareReport, BenchError> {
    let bytes = verify_binding(root, binding, MAX_REPORT_BYTES)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| focused_error(format!("failed to parse {}: {error}", binding.path)))
}

fn verify_binding(
    root: &RepoRoot,
    binding: &ArtifactBinding,
    max_bytes: u64,
) -> Result<Vec<u8>, BenchError> {
    let path = root.resolve_relative(Path::new(&binding.path));
    let bytes = read_bounded(&path, max_bytes)?;
    let actual = hex::encode(Sha256::digest(&bytes));
    if actual != binding.sha256 {
        return Err(focused_error(format!(
            "{} SHA-256 is {actual}, expected {}",
            binding.path, binding.sha256
        )));
    }
    Ok(bytes)
}

fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, BenchError> {
    use std::io::Read as _;

    let file = open_regular_file_bounded_descriptor(path, max_bytes)?;
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| focused_error(format!("failed to read {}: {error}", path.display())))?;
    let too_large = match u64::try_from(bytes.len()) {
        Ok(len) => len > max_bytes,
        Err(_) => true,
    };
    if too_large {
        return Err(focused_error(format!(
            "{} grew beyond {max_bytes} bytes while reading",
            path.display()
        )));
    }
    Ok(bytes)
}

fn validate_binding(label: &str, binding: &ArtifactBinding, issues: &mut Vec<String>) {
    if !valid_relative_path(&binding.path) {
        issues.push(format!(
            "{label} path {:?} is not safe and relative",
            binding.path
        ));
    }
    if !valid_sha256(&binding.sha256) {
        issues.push(format!("{label} has invalid SHA-256 {:?}", binding.sha256));
    }
}

fn validate_revision(revision: &str, issues: &mut Vec<String>) {
    if revision.len() != 40
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        issues.push("source_revision must be a lowercase 40-byte Git object id".to_string());
    }
}

fn validate_row_and_measurement(row_id: &str, measurement: &str, issues: &mut Vec<String>) {
    if !is_safe_benchmark_id(row_id) {
        issues.push(format!("row {row_id:?} is not a safe benchmark id"));
    }
    if measurement.trim().is_empty() || measurement.len() > 256 {
        issues.push(format!(
            "{row_id} has an invalid measurement {measurement:?}"
        ));
    }
}

fn validate_positive_seconds(label: &str, seconds: f64, issues: &mut Vec<String>) {
    if !seconds.is_finite() || seconds <= 0.0 {
        issues.push(format!("{label} must be positive and finite"));
    }
}

fn validate_ratio(
    label: &str,
    numerator: f64,
    denominator: f64,
    recorded: f64,
    issues: &mut Vec<String>,
) {
    if !recorded.is_finite() || recorded <= 0.0 {
        issues.push(format!("{label} must be positive and finite"));
        return;
    }
    let expected = numerator / denominator;
    let scale = expected.abs().max(1.0);
    if (expected - recorded).abs() > RATIO_TOLERANCE * scale {
        issues.push(format!(
            "{label}={recorded} does not match recomputed ratio {expected}"
        ));
    }
}

fn require_recorded_seconds(label: &str, actual: f64, recorded: f64) -> Result<(), BenchError> {
    let scale = actual.abs().max(1.0);
    if (actual - recorded).abs() <= RATIO_TOLERANCE * scale {
        Ok(())
    } else {
        Err(focused_error(format!(
            "{label} records {recorded} seconds, bound report contains {actual}"
        )))
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !path.is_absolute()
        && path.components().all(|component| {
            matches!(component, Component::Normal(_))
                && component.as_os_str().to_string_lossy() != "."
        })
}

fn focused_error(message: impl Into<String>) -> BenchError {
    BenchError::Qualification(format!(
        "A6 focused evidence validation failed:\n{}",
        message.into()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(path: &str) -> ArtifactBinding {
        ArtifactBinding {
            path: path.to_string(),
            sha256: "a".repeat(64),
        }
    }

    fn phase(ratio: f64) -> PhaseEvidence {
        PhaseEvidence {
            row_id: "row".to_string(),
            measurement: "stab_measurement".to_string(),
            source_current_seconds: ratio,
            predecessor_report: Some(binding("target/benchmarks/old/compare.json")),
            predecessor_seconds: Some(1.0),
            source_over_predecessor: Some(ratio),
            initial_seed_reason: None,
        }
    }

    fn diagnostic(ratio: f64, profile: ProfileStatus) -> FocusedDiagnostic {
        FocusedDiagnostic {
            row_id: "row".to_string(),
            report: binding("target/benchmarks/focused-aaaaaaaa/compare.json"),
            semantic_witness_source: binding("ops/bench/src/baseline/m9.rs"),
            internal_timing_count: 8,
            measurements: vec![FocusedMeasurement {
                measurement: "stab_measurement".to_string(),
                focused_seconds: ratio,
                focused_over_predecessor: ratio,
            }],
            profile: ProfileDisposition {
                status: profile,
                detail: "source-owned disposition".to_string(),
                artifact: None,
            },
            disposition: "retain after review".to_string(),
        }
    }

    fn ledger(phase_ratio: f64, focused_ratio: f64) -> FocusedEvidenceLedger {
        FocusedEvidenceLedger {
            schema_version: SCHEMA_VERSION,
            source_revision: "a".repeat(40),
            matrix_report: binding("target/benchmarks/matrix/compare.json"),
            phases: vec![phase(phase_ratio)],
            diagnostics: vec![diagnostic(focused_ratio, ProfileStatus::NotRequired)],
        }
    }

    #[test]
    fn structure_requires_exact_crossing_coverage() {
        validate_structure(&ledger(1.151, 1.149)).expect("valid crossing");

        let mut missing = ledger(1.151, 1.149);
        missing.diagnostics.clear();
        let error = validate_structure(&missing).expect_err("missing diagnostic");
        assert!(error.to_string().contains("missing focused diagnostic"));

        let extra = ledger(1.149, 1.149);
        let error = validate_structure(&extra).expect_err("extra diagnostic");
        assert!(
            error
                .to_string()
                .contains("is not a source-current crossing")
        );
    }

    #[test]
    fn reproducing_crossing_requires_profile_disposition() {
        let error = validate_structure(&ledger(1.2, 1.151)).expect_err("profile required");
        assert!(error.to_string().contains("marks profiling not required"));

        let mut unavailable = ledger(1.2, 1.151);
        unavailable
            .diagnostics
            .first_mut()
            .expect("fixture has a diagnostic")
            .profile
            .status = ProfileStatus::Unavailable;
        validate_structure(&unavailable).expect("explicit unavailable profile");
    }

    #[test]
    fn structure_rejects_weak_samples_and_inconsistent_ratios() {
        let mut invalid = ledger(1.2, 1.1);
        invalid
            .diagnostics
            .first_mut()
            .expect("fixture has a diagnostic")
            .internal_timing_count = 7;
        invalid
            .phases
            .first_mut()
            .expect("fixture has a phase")
            .source_over_predecessor = Some(1.3);
        let error = validate_structure(&invalid).expect_err("invalid ledger");
        let message = error.to_string();
        assert!(message.contains("expected at least 8"));
        assert!(message.contains("does not match recomputed ratio"));
    }
}
