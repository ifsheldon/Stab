use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::artifacts::{bind_artifact, normalize_repo_relative_path, validate_compare_report_path};
use super::{
    ArtifactBinding, CROSSING_RATIO, DiagnosticOutcome, FocusedDiagnostic, FocusedEvidenceLedger,
    FocusedMeasurement, INITIAL_SEED_PHASES, MAX_DIAGNOSTICS, MAX_LEDGER_BYTES,
    MAX_LEDGER_PROSE_BYTES, MAX_OWNER_ACTION_BYTES, MAX_PHASES, MAX_REPORT_BYTES, PhaseEvidence,
    ProfileDisposition, SCHEMA_VERSION, find_measurement, focused_error, read_bound_baseline,
    read_bound_report, require_baseline_contract, require_focused_contract,
    require_matrix_contract, require_predecessor_contract, validate_ledger,
};
use crate::error::BenchError;
use crate::manifest::BenchmarkManifest;
use crate::report::{CompareReport, stab_metadata};
use crate::root::RepoRoot;

const REQUEST_SCHEMA_VERSION: u32 = 2;
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
    profile_receipt: Option<PathBuf>,
    owner_action: String,
}

struct ProfileEvidenceContext<'a> {
    report_binding: &'a ArtifactBinding,
    report: &'a CompareReport,
    baseline_binding: &'a ArtifactBinding,
    matrix_binding: &'a ArtifactBinding,
    phases: &'a [PhaseEvidence],
}

pub(super) fn publish(root: &RepoRoot, request_path: &Path) -> Result<(), BenchError> {
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
    validate_request_metadata(&request)?;

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

    let expected_predecessors = super::expected_predecessor_phases(&matrix)?;
    let predecessor_registry = super::predecessors::read_and_validate(
        root,
        &repository_before.commit,
        &expected_predecessors,
    )?;
    let phases = derive_phases(root, &request, &matrix, &predecessor_registry)?;
    let diagnostics = derive_diagnostics(
        root,
        &request,
        &matrix,
        &phases,
        &baseline_binding,
        &matrix_binding,
    )?;
    let ledger = FocusedEvidenceLedger {
        schema_version: SCHEMA_VERSION,
        source_revision: repository_before.commit.clone(),
        predecessor_registry_sha256: predecessor_registry.source_sha256().to_string(),
        baseline_report: baseline_binding,
        matrix_report: matrix_binding,
        phases,
        diagnostics,
    };
    validate_ledger(root, &ledger, true, None)?;

    let repository_after = stab_metadata(root)?;
    if repository_after != repository_before || repository_after.local_modifications {
        return Err(focused_error(
            "repository changed while deriving the A6 evidence ledger",
        ));
    }

    let prepared = super::storage::prepare(&ledger.source_revision, &ledger)?;
    let output = super::storage::publish(root, &prepared)?;
    println!(
        "[stab-bench] published uncommitted A6 evidence candidate {} from clean revision {}; review and commit this exact object before validation",
        output.display(),
        ledger.source_revision
    );
    Ok(())
}

fn derive_phases(
    root: &RepoRoot,
    request: &PublicationRequest,
    matrix: &CompareReport,
    registry: &super::predecessors::ValidatedPredecessorRegistry,
) -> Result<Vec<PhaseEvidence>, BenchError> {
    let ordered = report_only_phases(matrix)?;
    let initial = INITIAL_SEED_PHASES
        .iter()
        .map(|(row, measurement)| PhaseSelector {
            row_id: (*row).to_string(),
            measurement: (*measurement).to_string(),
        })
        .collect::<BTreeSet<_>>();
    let mut report_paths = BTreeSet::new();
    let mut reports = BTreeMap::new();
    for predecessor in &request.predecessors {
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
        if reports
            .insert(report.stab.commit.clone(), (binding.clone(), report))
            .is_some()
        {
            return Err(focused_error(
                "publication request selects more than one predecessor report for a registered backport commit",
            ));
        }
    }

    let mut expected_commits = BTreeSet::new();
    for phase in registry.phases() {
        expected_commits.insert(
            registry
                .identity_for(phase)?
                .instrumentation_backport_commit
                .clone(),
        );
    }
    let actual_commits = reports.keys().cloned().collect::<BTreeSet<_>>();
    if actual_commits != expected_commits {
        return Err(focused_error(format!(
            "publication predecessor reports differ from registered backport commits: missing={:?}, extra={:?}",
            expected_commits
                .difference(&actual_commits)
                .collect::<Vec<_>>(),
            actual_commits
                .difference(&expected_commits)
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
        let phase_key = super::predecessors::PhaseKey::new(&selector.row_id, &selector.measurement);
        let identity = registry.identity_for(&phase_key)?;
        let (binding, report) = reports
            .get(&identity.instrumentation_backport_commit)
            .ok_or_else(|| {
                focused_error(format!(
                    "missing predecessor report for registered backport {}",
                    identity.instrumentation_backport_commit
                ))
            })?;
        registry.require_report_commit(&phase_key, &report.stab.commit)?;
        let provisional = PhaseEvidence {
            row_id: selector.row_id.clone(),
            measurement: selector.measurement.clone(),
            source_current_seconds: current.seconds,
            predecessor_report: Some(binding.clone()),
            predecessor_seconds: None,
            source_over_predecessor: None,
            initial_seed_reason: None,
        };
        require_predecessor_contract(matrix, report, &provisional, &binding.path, identity)?;
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
    baseline_binding: &ArtifactBinding,
    matrix_binding: &ArtifactBinding,
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
        let row_reproduces = measurements
            .iter()
            .any(|measurement| measurement.focused_over_predecessor > CROSSING_RATIO);
        let profile = derive_profile_disposition(
            root,
            selection,
            row_reproduces,
            ProfileEvidenceContext {
                report_binding: &report_binding,
                report: &report,
                baseline_binding,
                matrix_binding,
                phases,
            },
        )?;
        let diagnostic = FocusedDiagnostic {
            row_id,
            report: report_binding,
            internal_timing_count: timing_count
                .ok_or_else(|| focused_error("focused diagnostic has no measurements"))?,
            outcome: diagnostic_outcome(&measurements, &profile)?,
            measurements,
            profile,
            owner_action: selection.owner_action.clone(),
        };
        require_focused_contract(matrix, &report, &diagnostic, &matrix.stab.commit)?;
        diagnostics.push(diagnostic);
    }
    Ok(diagnostics)
}

fn derive_profile_disposition(
    root: &RepoRoot,
    selection: &DiagnosticSelection,
    row_reproduces: bool,
    context: ProfileEvidenceContext<'_>,
) -> Result<ProfileDisposition, BenchError> {
    let Some(path) = &selection.profile_receipt else {
        return if row_reproduces {
            Err(focused_error(format!(
                "{} reproduces the A6 crossing but has no typed profile receipt",
                selection.row_id
            )))
        } else {
            Ok(ProfileDisposition::NotRequired)
        };
    };
    if !row_reproduces {
        return Err(focused_error(format!(
            "{} resolves within the A6 boundary and must not select a profile receipt",
            selection.row_id
        )));
    }

    let (binding, _) = bind_artifact(
        root,
        path,
        super::profile_receipt::MAX_PROFILE_RECEIPT_BYTES,
    )?;
    let mut forbidden = vec![
        context.baseline_binding.clone(),
        context.matrix_binding.clone(),
    ];
    forbidden.extend(
        context
            .phases
            .iter()
            .filter_map(|phase| phase.predecessor_report.clone()),
    );
    let receipt = super::profile_receipt::read_and_validate(
        root,
        &binding,
        context.report_binding,
        context.report,
        &forbidden,
    )?;
    match receipt.outcome() {
        super::profile_receipt::ProfileOutcome::Captured { .. } => {
            Ok(ProfileDisposition::Captured { receipt: binding })
        }
        super::profile_receipt::ProfileOutcome::Unavailable { .. } => {
            Ok(ProfileDisposition::Unavailable { receipt: binding })
        }
    }
}

fn diagnostic_outcome(
    measurements: &[FocusedMeasurement],
    profile: &ProfileDisposition,
) -> Result<DiagnosticOutcome, BenchError> {
    let reproduces = measurements
        .iter()
        .any(|measurement| measurement.focused_over_predecessor > CROSSING_RATIO);
    match (reproduces, profile) {
        (false, ProfileDisposition::NotRequired) => Ok(DiagnosticOutcome::ResolvedWithinBoundary),
        (true, ProfileDisposition::Captured { .. }) => Ok(DiagnosticOutcome::ReproducedProfiled),
        (true, ProfileDisposition::Unavailable { .. }) => {
            Ok(DiagnosticOutcome::ReproducedProfileUnavailable)
        }
        _ => Err(focused_error(format!(
            "profile disposition {profile:?} is inconsistent with reproduced={reproduces}"
        ))),
    }
}

fn validate_request_metadata(request: &PublicationRequest) -> Result<(), BenchError> {
    if request.predecessors.len() > MAX_PHASES {
        return Err(focused_error(format!(
            "publication request selects {} predecessor reports, maximum is {MAX_PHASES}",
            request.predecessors.len()
        )));
    }
    if request.diagnostics.len() > MAX_DIAGNOSTICS {
        return Err(focused_error(format!(
            "publication request selects {} diagnostics, maximum is {MAX_DIAGNOSTICS}",
            request.diagnostics.len()
        )));
    }
    let mut total = 0usize;
    for diagnostic in &request.diagnostics {
        if !crate::manifest::is_safe_benchmark_id(&diagnostic.row_id) {
            return Err(focused_error(format!(
                "publication diagnostic row {:?} is not a safe benchmark id",
                diagnostic.row_id
            )));
        }
        let action = diagnostic.owner_action.as_bytes();
        if action.is_empty()
            || action.len() > MAX_OWNER_ACTION_BYTES
            || action.contains(&0)
            || diagnostic.owner_action.trim().is_empty()
        {
            return Err(focused_error(format!(
                "{} owner_action must contain 1..={MAX_OWNER_ACTION_BYTES} bytes, non-whitespace text, and no NUL",
                diagnostic.row_id
            )));
        }
        total = total.checked_add(action.len()).ok_or_else(|| {
            focused_error("publication request prose length overflows address space")
        })?;
    }
    if total > MAX_LEDGER_PROSE_BYTES {
        return Err(focused_error(format!(
            "publication request contains {total} owner-action bytes, maximum is {MAX_LEDGER_PROSE_BYTES}"
        )));
    }
    Ok(())
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

    fn request_with_owner_action(owner_action: String) -> PublicationRequest {
        PublicationRequest {
            schema_version: REQUEST_SCHEMA_VERSION,
            matrix_report: PathBuf::from("target/benchmarks/matrix/compare.json"),
            predecessors: Vec::new(),
            diagnostics: vec![DiagnosticSelection {
                row_id: "row".to_string(),
                report: PathBuf::from("target/benchmarks/focused/compare.json"),
                profile_receipt: None,
                owner_action,
            }],
        }
    }

    fn focused_measurement(ratio: f64) -> FocusedMeasurement {
        FocusedMeasurement {
            measurement: "stab_work".to_string(),
            focused_seconds: ratio,
            focused_over_predecessor: ratio,
        }
    }

    #[test]
    fn diagnostic_outcome_is_derived_from_evidence_and_profile_availability() {
        let receipt = ArtifactBinding {
            path: "target/benchmarks/profile-aaaaaaaa/profile-receipt.json".to_string(),
            sha256: "a".repeat(64),
        };
        assert_eq!(
            diagnostic_outcome(
                &[focused_measurement(1.15)],
                &ProfileDisposition::NotRequired
            )
            .expect("resolved outcome"),
            DiagnosticOutcome::ResolvedWithinBoundary
        );
        assert_eq!(
            diagnostic_outcome(
                &[focused_measurement(1.151)],
                &ProfileDisposition::Captured {
                    receipt: receipt.clone()
                }
            )
            .expect("profiled outcome"),
            DiagnosticOutcome::ReproducedProfiled
        );
        assert_eq!(
            diagnostic_outcome(
                &[focused_measurement(1.151)],
                &ProfileDisposition::Unavailable { receipt }
            )
            .expect("unavailable outcome"),
            DiagnosticOutcome::ReproducedProfileUnavailable
        );
    }

    #[test]
    fn diagnostic_outcome_rejects_profile_labels_that_contradict_evidence() {
        let receipt = ArtifactBinding {
            path: "target/benchmarks/profile-aaaaaaaa/profile-receipt.json".to_string(),
            sha256: "a".repeat(64),
        };
        let error = diagnostic_outcome(
            &[focused_measurement(1.151)],
            &ProfileDisposition::NotRequired,
        )
        .expect_err("reproduced crossing needs a profile disposition");
        assert!(
            error
                .to_string()
                .contains("inconsistent with reproduced=true")
        );

        let error = diagnostic_outcome(
            &[focused_measurement(1.1)],
            &ProfileDisposition::Captured { receipt },
        )
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
                "profile_receipt": null,
                "owner_action": "retain"
            }]
        });
        let error = serde_json::from_value::<PublicationRequest>(value)
            .expect_err("source-path existence is not semantic evidence");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn publication_request_rejects_manual_phase_and_profile_claims() {
        let value = serde_json::json!({
            "schema_version": REQUEST_SCHEMA_VERSION,
            "matrix_report": "target/benchmarks/matrix/compare.json",
            "predecessors": [{
                "report": "target/benchmarks/predecessor/compare.json",
                "phases": [{"row_id": "row", "measurement": "work"}]
            }],
            "diagnostics": [{
                "row_id": "row",
                "report": "target/benchmarks/focused/compare.json",
                "profile": {
                    "status": "unavailable",
                    "detail": "operator claim",
                    "artifact": null
                },
                "profile_receipt": null,
                "owner_action": "retain"
            }]
        });
        let error = serde_json::from_value::<PublicationRequest>(value)
            .expect_err("manual phase and profile claims are not selections");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn publication_request_bounds_owner_prose_before_artifact_reads() {
        validate_request_metadata(&request_with_owner_action("retain".to_string()))
            .expect("bounded owner action");

        let error = validate_request_metadata(&request_with_owner_action(
            "x".repeat(MAX_OWNER_ACTION_BYTES + 1),
        ))
        .expect_err("oversized owner action");
        assert!(error.to_string().contains("owner_action"));

        let error =
            validate_request_metadata(&request_with_owner_action("bad\0action".to_string()))
                .expect_err("NUL owner action");
        assert!(error.to_string().contains("no NUL"));

        let mut request = request_with_owner_action("retain".to_string());
        request.diagnostics = (0..=MAX_DIAGNOSTICS)
            .map(|index| DiagnosticSelection {
                row_id: format!("row-{index}"),
                report: PathBuf::from("target/benchmarks/focused/compare.json"),
                profile_receipt: None,
                owner_action: "x".to_string(),
            })
            .collect();
        let error = validate_request_metadata(&request).expect_err("too many diagnostics");
        assert!(error.to_string().contains("maximum"));
    }
}
