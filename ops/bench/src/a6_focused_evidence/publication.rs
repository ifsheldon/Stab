use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::artifacts::{bind_artifact, normalize_repo_relative_path, validate_compare_report_path};
use super::{
    ArtifactBinding, CROSSING_RATIO, DiagnosticOutcome, FocusedDiagnostic, FocusedEvidenceLedger,
    FocusedMeasurement, INITIAL_SEED_PHASES, MAX_LEDGER_BYTES, MAX_REPORT_BYTES, PhaseEvidence,
    ProfileDisposition, ProfileStatus, SCHEMA_VERSION, find_measurement, focused_error,
    read_bound_baseline, read_bound_report, require_baseline_contract, require_focused_contract,
    require_matrix_contract, require_predecessor_contract, validate_ledger,
};
use crate::error::BenchError;
use crate::manifest::BenchmarkManifest;
use crate::report::{CompareReport, stab_metadata};
use crate::root::RepoRoot;
use crate::source_file::atomic_create_repo_regular_file;

const REQUEST_SCHEMA_VERSION: u32 = 1;
const CHECKED_LEDGER_PATH: &str = "benchmarks/a6-focused-evidence.json";
const INITIAL_SEED_REASON: &str =
    "No semantically identical clean predecessor exists in the source-owned A6 seed list.";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationRequest {
    schema_version: u32,
    matrix_report: PathBuf,
    predecessors: Vec<PredecessorSelection>,
    diagnostics: Vec<DiagnosticSelection>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredecessorSelection {
    report: PathBuf,
    phases: Vec<PhaseSelector>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
struct PhaseSelector {
    row_id: String,
    measurement: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticSelection {
    row_id: String,
    report: PathBuf,
    profile: ProfileSelection,
    owner_action: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileSelection {
    status: ProfileStatus,
    detail: String,
    artifact: Option<PathBuf>,
}

pub(super) fn publish(
    root: &RepoRoot,
    ledger_path: &Path,
    request_path: &Path,
) -> Result<(), BenchError> {
    if ledger_path != Path::new(CHECKED_LEDGER_PATH) {
        return Err(focused_error(format!(
            "publication output must be exactly {CHECKED_LEDGER_PATH}"
        )));
    }
    let request_path = normalize_repo_relative_path(root, request_path)?;
    if !request_path.starts_with("target/benchmarks") {
        return Err(focused_error(
            "publication request must be under target/benchmarks",
        ));
    }
    let request_bytes =
        super::artifacts::read_bounded(&root.resolve_relative(&request_path), MAX_LEDGER_BYTES)?;
    let request: PublicationRequest = serde_json::from_slice(&request_bytes).map_err(|error| {
        focused_error(format!(
            "failed to parse {}: {error}",
            request_path.display()
        ))
    })?;
    if request.schema_version != REQUEST_SCHEMA_VERSION {
        return Err(focused_error(format!(
            "publication request schema_version={} expected {REQUEST_SCHEMA_VERSION}",
            request.schema_version
        )));
    }

    let repository_before = stab_metadata(root)?;
    if repository_before.local_modifications {
        return Err(focused_error(
            "publication requires a clean repository before reading evidence",
        ));
    }

    let (matrix_binding, matrix_bytes) =
        bind_artifact(root, &request.matrix_report, MAX_REPORT_BYTES)?;
    require_compare_path("matrix report", &matrix_binding)?;
    let matrix: CompareReport = serde_json::from_slice(&matrix_bytes).map_err(|error| {
        focused_error(format!("failed to parse {}: {error}", matrix_binding.path))
    })?;
    if matrix.stab.commit != repository_before.commit {
        return Err(focused_error(format!(
            "matrix source revision {} does not match clean HEAD {}",
            matrix.stab.commit, repository_before.commit
        )));
    }

    let manifest = BenchmarkManifest::read(root)?;
    manifest.check(root)?;
    let measurement_contract =
        super::measurement_contract::A6MeasurementContract::read_and_validate(root, &manifest)?;
    require_matrix_contract(
        root,
        &matrix,
        &repository_before.commit,
        &manifest,
        &measurement_contract,
    )?;

    let baseline_path =
        normalize_repo_relative_path(root, Path::new(&matrix.command.baseline_path))?;
    let (baseline_binding, _) = bind_artifact(root, &baseline_path, MAX_REPORT_BYTES)?;
    let baseline = read_bound_baseline(root, &baseline_binding)?;
    require_baseline_contract(&baseline, &matrix, &baseline_binding, &manifest)?;

    let phases = derive_phases(root, &request, &matrix)?;
    let diagnostics = derive_diagnostics(root, &request, &matrix, &phases)?;
    let ledger = FocusedEvidenceLedger {
        schema_version: SCHEMA_VERSION,
        source_revision: repository_before.commit.clone(),
        baseline_report: baseline_binding,
        matrix_report: matrix_binding,
        phases,
        diagnostics,
    };
    validate_ledger(root, &ledger, true)?;

    let repository_after = stab_metadata(root)?;
    if repository_after != repository_before || repository_after.local_modifications {
        return Err(focused_error(
            "repository changed while deriving the A6 evidence ledger",
        ));
    }

    let mut bytes = serde_json::to_vec_pretty(&ledger)?;
    bytes.push(b'\n');
    let output = root.resolve_relative(ledger_path);
    atomic_create_repo_regular_file(root, &output, &bytes)?;
    println!(
        "[stab-bench] published {} from clean revision {}",
        ledger_path.display(),
        ledger.source_revision
    );
    Ok(())
}

fn derive_phases(
    root: &RepoRoot,
    request: &PublicationRequest,
    matrix: &CompareReport,
) -> Result<Vec<PhaseEvidence>, BenchError> {
    let ordered = report_only_phases(matrix)?;
    let initial = INITIAL_SEED_PHASES
        .iter()
        .map(|(row, measurement)| PhaseSelector {
            row_id: (*row).to_string(),
            measurement: (*measurement).to_string(),
        })
        .collect::<BTreeSet<_>>();
    let expected_predecessors = ordered
        .iter()
        .filter(|phase| !initial.contains(*phase))
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut report_paths = BTreeSet::new();
    let mut reports = BTreeMap::new();
    let mut selected = BTreeMap::new();
    for predecessor in &request.predecessors {
        if predecessor.phases.is_empty() {
            return Err(focused_error("predecessor selection has no phases"));
        }
        let (binding, bytes) = bind_artifact(root, &predecessor.report, MAX_REPORT_BYTES)?;
        require_compare_path("predecessor report", &binding)?;
        if !report_paths.insert(binding.path.clone()) {
            return Err(focused_error(format!(
                "publication request repeats predecessor report {}",
                binding.path
            )));
        }
        let report: CompareReport = serde_json::from_slice(&bytes)
            .map_err(|error| focused_error(format!("failed to parse {}: {error}", binding.path)))?;
        reports.insert(binding.path.clone(), (binding.clone(), report));
        for phase in &predecessor.phases {
            if selected
                .insert(phase.clone(), binding.path.clone())
                .is_some()
            {
                return Err(focused_error(format!(
                    "publication request assigns predecessor more than once for {}/{}",
                    phase.row_id, phase.measurement
                )));
            }
        }
    }
    let actual = selected.keys().cloned().collect::<BTreeSet<_>>();
    if actual != expected_predecessors {
        return Err(focused_error(format!(
            "publication predecessor phases differ from the source-owned set: missing={:?}, extra={:?}",
            expected_predecessors
                .difference(&actual)
                .collect::<Vec<_>>(),
            actual
                .difference(&expected_predecessors)
                .collect::<Vec<_>>()
        )));
    }

    let mut phases = Vec::with_capacity(ordered.len());
    for selector in ordered {
        let current = find_measurement(matrix, &selector.row_id, &selector.measurement)?;
        if initial.contains(&selector) {
            phases.push(PhaseEvidence {
                row_id: selector.row_id,
                measurement: selector.measurement,
                source_current_seconds: current.seconds,
                predecessor_report: None,
                predecessor_seconds: None,
                source_over_predecessor: None,
                initial_seed_reason: Some(INITIAL_SEED_REASON.to_string()),
            });
            continue;
        }
        let report_path = selected.get(&selector).ok_or_else(|| {
            focused_error(format!(
                "missing predecessor for {}/{}",
                selector.row_id, selector.measurement
            ))
        })?;
        let (binding, report) = reports.get(report_path).ok_or_else(|| {
            focused_error(format!("missing loaded predecessor report {report_path}"))
        })?;
        let provisional = PhaseEvidence {
            row_id: selector.row_id.clone(),
            measurement: selector.measurement.clone(),
            source_current_seconds: current.seconds,
            predecessor_report: Some(binding.clone()),
            predecessor_seconds: None,
            source_over_predecessor: None,
            initial_seed_reason: None,
        };
        require_predecessor_contract(matrix, report, &provisional, &binding.path)?;
        let predecessor =
            find_measurement(report, &selector.row_id, &selector.measurement)?.seconds;
        phases.push(PhaseEvidence {
            predecessor_seconds: Some(predecessor),
            source_over_predecessor: Some(current.seconds / predecessor),
            ..provisional
        });
    }
    Ok(phases)
}

fn derive_diagnostics(
    root: &RepoRoot,
    request: &PublicationRequest,
    matrix: &CompareReport,
    phases: &[PhaseEvidence],
) -> Result<Vec<FocusedDiagnostic>, BenchError> {
    let mut crossings = BTreeMap::<String, Vec<&PhaseEvidence>>::new();
    for phase in phases {
        if phase
            .source_over_predecessor
            .is_some_and(|ratio| ratio > CROSSING_RATIO)
        {
            crossings
                .entry(phase.row_id.clone())
                .or_default()
                .push(phase);
        }
    }
    let expected_rows = crossings.keys().cloned().collect::<BTreeSet<_>>();
    let mut selections = BTreeMap::new();
    for selection in &request.diagnostics {
        if selections
            .insert(selection.row_id.clone(), selection)
            .is_some()
        {
            return Err(focused_error(format!(
                "publication request repeats diagnostic row {}",
                selection.row_id
            )));
        }
    }
    let actual_rows = selections.keys().cloned().collect::<BTreeSet<_>>();
    if actual_rows != expected_rows {
        return Err(focused_error(format!(
            "publication diagnostic rows differ from current crossings: missing={:?}, extra={:?}",
            expected_rows.difference(&actual_rows).collect::<Vec<_>>(),
            actual_rows.difference(&expected_rows).collect::<Vec<_>>()
        )));
    }

    let mut diagnostics = Vec::with_capacity(crossings.len());
    for (row_id, row_phases) in crossings {
        let selection = selections
            .get(&row_id)
            .ok_or_else(|| focused_error(format!("missing diagnostic selection for {row_id}")))?;
        let (report_binding, _) = bind_artifact(root, &selection.report, MAX_REPORT_BYTES)?;
        require_compare_path("focused report", &report_binding)?;
        let report = read_bound_report(root, &report_binding)?;
        let profile_artifact = selection
            .profile
            .artifact
            .as_ref()
            .map(|path| bind_artifact(root, path, MAX_REPORT_BYTES).map(|(binding, _)| binding))
            .transpose()?;

        let mut timing_count = None;
        let mut measurements = Vec::with_capacity(row_phases.len());
        for phase in row_phases {
            let focused = find_measurement(&report, &row_id, &phase.measurement)?;
            let count = focused.iterations.ok_or_else(|| {
                focused_error(format!(
                    "focused report omits internal timing count for {row_id}/{}",
                    phase.measurement
                ))
            })?;
            match timing_count {
                Some(expected) if expected != count => {
                    return Err(focused_error(format!(
                        "focused row {row_id} mixes internal timing counts {expected} and {count}"
                    )));
                }
                None => timing_count = Some(count),
                _ => {}
            }
            let predecessor = phase.predecessor_seconds.ok_or_else(|| {
                focused_error(format!(
                    "crossing {row_id}/{} has no predecessor",
                    phase.measurement
                ))
            })?;
            measurements.push(FocusedMeasurement {
                measurement: phase.measurement.clone(),
                focused_seconds: focused.seconds,
                focused_over_predecessor: focused.seconds / predecessor,
            });
        }
        let diagnostic = FocusedDiagnostic {
            row_id,
            report: report_binding,
            internal_timing_count: timing_count
                .ok_or_else(|| focused_error("focused diagnostic has no measurements"))?,
            outcome: diagnostic_outcome(&measurements, &selection.profile.status)?,
            measurements,
            profile: ProfileDisposition {
                status: selection.profile.status.clone(),
                detail: selection.profile.detail.clone(),
                artifact: profile_artifact,
            },
            owner_action: selection.owner_action.clone(),
        };
        require_focused_contract(matrix, &report, &diagnostic, &matrix.stab.commit)?;
        diagnostics.push(diagnostic);
    }
    Ok(diagnostics)
}

fn diagnostic_outcome(
    measurements: &[FocusedMeasurement],
    profile_status: &ProfileStatus,
) -> Result<DiagnosticOutcome, BenchError> {
    let reproduces = measurements
        .iter()
        .any(|measurement| measurement.focused_over_predecessor > CROSSING_RATIO);
    match (reproduces, profile_status) {
        (false, ProfileStatus::NotRequired) => Ok(DiagnosticOutcome::ResolvedWithinBoundary),
        (true, ProfileStatus::Captured) => Ok(DiagnosticOutcome::ReproducedProfiled),
        (true, ProfileStatus::Unavailable) => Ok(DiagnosticOutcome::ReproducedProfileUnavailable),
        _ => Err(focused_error(format!(
            "profile status {profile_status:?} is inconsistent with reproduced={reproduces}"
        ))),
    }
}

fn report_only_phases(matrix: &CompareReport) -> Result<Vec<PhaseSelector>, BenchError> {
    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    for row in &matrix.rows {
        if row.comparability != crate::comparability::ComparabilityClass::ReportOnly {
            continue;
        }
        for measurement in &row.stab_measurements {
            let selector = PhaseSelector {
                row_id: row.id.clone(),
                measurement: measurement.name.clone(),
            };
            if !seen.insert(selector.clone()) {
                return Err(focused_error(format!(
                    "matrix repeats report-only phase {}/{}",
                    selector.row_id, selector.measurement
                )));
            }
            ordered.push(selector);
        }
    }
    Ok(ordered)
}

fn require_compare_path(label: &str, binding: &ArtifactBinding) -> Result<(), BenchError> {
    let mut issues = Vec::new();
    validate_compare_report_path(label, &binding.path, &mut issues);
    if issues.is_empty() {
        Ok(())
    } else {
        Err(focused_error(issues.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn focused_measurement(ratio: f64) -> FocusedMeasurement {
        FocusedMeasurement {
            measurement: "stab_work".to_string(),
            focused_seconds: ratio,
            focused_over_predecessor: ratio,
        }
    }

    #[test]
    fn diagnostic_outcome_is_derived_from_evidence_and_profile_availability() {
        assert_eq!(
            diagnostic_outcome(&[focused_measurement(1.15)], &ProfileStatus::NotRequired)
                .expect("resolved outcome"),
            DiagnosticOutcome::ResolvedWithinBoundary
        );
        assert_eq!(
            diagnostic_outcome(&[focused_measurement(1.151)], &ProfileStatus::Captured)
                .expect("profiled outcome"),
            DiagnosticOutcome::ReproducedProfiled
        );
        assert_eq!(
            diagnostic_outcome(&[focused_measurement(1.151)], &ProfileStatus::Unavailable)
                .expect("unavailable outcome"),
            DiagnosticOutcome::ReproducedProfileUnavailable
        );
    }

    #[test]
    fn diagnostic_outcome_rejects_profile_labels_that_contradict_evidence() {
        let error = diagnostic_outcome(&[focused_measurement(1.151)], &ProfileStatus::NotRequired)
            .expect_err("reproduced crossing needs a profile disposition");
        assert!(
            error
                .to_string()
                .contains("inconsistent with reproduced=true")
        );

        let error = diagnostic_outcome(&[focused_measurement(1.1)], &ProfileStatus::Captured)
            .expect_err("resolved crossing cannot claim a captured profile");
        assert!(
            error
                .to_string()
                .contains("inconsistent with reproduced=false")
        );
    }

    #[test]
    fn publication_request_rejects_the_obsolete_source_path_surrogate() {
        let value = serde_json::json!({
            "schema_version": REQUEST_SCHEMA_VERSION,
            "matrix_report": "target/benchmarks/matrix/compare.json",
            "predecessors": [],
            "diagnostics": [{
                "row_id": "row",
                "report": "target/benchmarks/focused/compare.json",
                "semantic_witness_source": "ops/bench/src/baseline/m9.rs",
                "profile": {
                    "status": "not-required",
                    "detail": "resolved",
                    "artifact": null
                },
                "owner_action": "retain"
            }]
        });
        let error = serde_json::from_value::<PublicationRequest>(value)
            .expect_err("source-path existence is not semantic evidence");
        assert!(error.to_string().contains("unknown field"));
    }
}
