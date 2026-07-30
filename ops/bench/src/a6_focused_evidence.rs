//! Checked A6 report-only crossing and focused-diagnostic evidence.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use clap::Args;
use serde::{Deserialize, Serialize};

use self::artifacts::{
    normalize_repo_relative_path, path_ends_with, valid_revision, validate_binding, verify_binding,
};
use self::policy::require_matrix_policies;
use self::structure::validate_structure;
use crate::comparability::ComparabilityClass;
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::error::BenchError;
use crate::manifest::{
    BenchmarkManifest, BenchmarkRow, Milestone, Runner, ThresholdClass, is_safe_benchmark_id,
};
use crate::report::{
    BaselineReport, BaselineRowResult, COMPARE_REPORT_SCHEMA_VERSION, COMPARE_TIMING_BOUNDARY,
    CompareReport, CompareRowResult, Measurement,
};
use crate::root::RepoRoot;

mod artifacts;
mod measurement_contract;
mod policy;
mod predecessors;
pub(crate) mod profile_receipt;
mod publication;
mod revision;
mod storage;
mod structure;

const SCHEMA_VERSION: u32 = 4;
const MAX_LEDGER_BYTES: u64 = 1 << 20;
const MAX_REPORT_BYTES: u64 = 64 << 20;
const MAX_PHASES: usize = 256;
const MAX_DIAGNOSTICS: usize = 128;
const MAX_OWNER_ACTION_BYTES: usize = 4 << 10;
const MAX_LEDGER_PROSE_BYTES: usize = 64 << 10;
const CROSSING_RATIO: f64 = 1.15;
const RATIO_TOLERANCE: f64 = 1e-9;
const A6_BASELINE_SCHEMA_VERSION: u32 = 3;
const A6_BASELINE_TARGET_SECONDS: f64 = 0.01;
const A6_BASELINE_CLI_ITERATIONS: u32 = 3;
const MAX_POLICY_BYTES: usize = 8 << 20;
const A6_PRIMARY_PROFILER_NOTE_ROOT: &str = "benchmarks/profiler-notes/m12";
const A6_PROFILER_NOTE_ROOTS: [&str; 2] = [
    A6_PRIMARY_PROFILER_NOTE_ROOT,
    "benchmarks/profiler-notes/pfm-b5",
];
const A6_SELECTED_PAIR_GATES: [(&str, &str, &str); 2] = [
    (
        "m5-simd-bits",
        "simd_bits_xor_10K",
        "stab_simd_bits_xor_10K",
    ),
    (
        "m6-clifford-string",
        "CliffordString_multiplication_10K",
        "stab_clifford_string_multiplication_10K",
    ),
];
const INITIAL_SEED_PHASES: [(&str, &str); 19] = [
    ("pf3-m2d-sweep-b8", "stab_pf3_m2d_sweep_b8"),
    ("pf3-m2d-sweep-ptb64-input", "stab_pf3_m2d_sweep_ptb64"),
    (
        "pf3-detect-sweep-sampling",
        "stab_detect_sweep_default_false",
    ),
    (
        "pf3-detect-sweep-sampling",
        "stab_detect_frame_sweep_default_false",
    ),
    (
        "pfm-b5-analyzer-cycle-folding",
        "stab_pfm_b5_analyzer_transient",
    ),
    (
        "pfm-b5-analyzer-cycle-folding",
        "stab_pfm_b5_analyzer_short_period",
    ),
    (
        "pfm-b5-analyzer-cycle-folding",
        "stab_pfm_b5_analyzer_long_period",
    ),
    (
        "pfm-b5-analyzer-cycle-folding",
        "stab_pfm_b5_analyzer_nested",
    ),
    (
        "pfm-b5-analyzer-cycle-folding",
        "stab_pfm_b5_analyzer_gauge",
    ),
    (
        "pfm-b5-analyzer-cycle-folding",
        "stab_pfm_b5_analyzer_coordinate",
    ),
    (
        "pfm-b5-analyzer-generated-qec",
        "stab_pfm_b5_analyzer_repetition_qec",
    ),
    (
        "pfm-b5-analyzer-generated-qec",
        "stab_pfm_b5_analyzer_surface_qec",
    ),
    (
        "pfm-b5-graphlike-search-direct-dem",
        "stab_pfm_b5_graphlike_direct_dem",
    ),
    (
        "pfm-b5-hypergraph-search-direct-dem",
        "stab_pfm_b5_hypergraph_direct_dem",
    ),
    ("pf7-cli-m2d-sweep-b8", "stab_pf7_cli_m2d_sweep_b8"),
    (
        "pf7-cli-m2d-feedback-inline",
        "stab_pf7_cli_m2d_feedback_inline",
    ),
    (
        "pf7-cli-analyze-errors-generated",
        "stab_pf7_cli_analyze_errors_generated",
    ),
    (
        "pf7-cli-analyze-errors-decompose",
        "stab_pf7_cli_analyze_errors_decompose",
    ),
    (
        "pf7-cli-legacy-dispatch-startup",
        "stab_pf7_cli_legacy_gen_d3_r3",
    ),
];

#[derive(Debug, Args)]
pub(crate) struct A6FocusedEvidenceArgs {
    /// Checked source ledger describing every report-only phase and focused crossing.
    #[arg(long, value_name = "PATH", conflicts_with = "publish_from")]
    ledger: Option<PathBuf>,

    /// Deprecated compatibility flag; full artifact verification is now mandatory.
    #[arg(long, hide = true, conflicts_with = "publish_from")]
    verify_artifacts: bool,

    /// Derive and atomically create the checked ledger from a source-owned publication request.
    #[arg(long, value_name = "PATH", conflicts_with = "verify_artifacts")]
    publish_from: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FocusedEvidenceLedger {
    schema_version: u32,
    source_revision: String,
    predecessor_registry_sha256: String,
    baseline_report: ArtifactBinding,
    matrix_report: ArtifactBinding,
    phases: Vec<PhaseEvidence>,
    diagnostics: Vec<FocusedDiagnostic>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ArtifactBinding {
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FocusedDiagnostic {
    row_id: String,
    report: ArtifactBinding,
    internal_timing_count: usize,
    measurements: Vec<FocusedMeasurement>,
    outcome: DiagnosticOutcome,
    profile: ProfileDisposition,
    owner_action: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FocusedMeasurement {
    measurement: String,
    focused_seconds: f64,
    focused_over_predecessor: f64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DiagnosticOutcome {
    ResolvedWithinBoundary,
    ReproducedProfiled,
    ReproducedProfileUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
enum ProfileDisposition {
    NotRequired,
    Captured { receipt: ArtifactBinding },
    Unavailable { receipt: ArtifactBinding },
}

impl ProfileDisposition {
    fn receipt(&self) -> Option<&ArtifactBinding> {
        match self {
            Self::NotRequired => None,
            Self::Captured { receipt } | Self::Unavailable { receipt } => Some(receipt),
        }
    }
}

pub(crate) fn check(root: &RepoRoot, args: A6FocusedEvidenceArgs) -> Result<(), BenchError> {
    if let Some(request) = &args.publish_from {
        return publication::publish(root, request);
    }
    let _deprecated_verify_artifacts = args.verify_artifacts;
    let (ledger_path, ledger) = load_source_current_ledger(root, args.ledger.as_deref())?;
    validate_ledger(root, &ledger, true, Some(&ledger_path))?;
    println!(
        "[stab-bench] A6 focused evidence OK: {}, {} phases, {} focused row(s), all_artifacts_verified=true",
        ledger_path.display(),
        ledger.phases.len(),
        ledger.diagnostics.len()
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
struct FocusedEvidenceHeader {
    source_revision: String,
}

fn load_source_current_ledger(
    root: &RepoRoot,
    selected: Option<&Path>,
) -> Result<(PathBuf, FocusedEvidenceLedger), BenchError> {
    if let Some(selected) = selected {
        let selected = normalize_repo_relative_path(root, selected)?;
        let object = storage::read_explicit_tracked(root, &selected)?;
        let ledger = parse_ledger(&object.relative_path, &object.bytes)?;
        require_object_identity(&object, &ledger)?;
        return Ok((object.relative_path, ledger));
    }

    let objects = storage::discover_tracked(root)?;
    if objects.is_empty() {
        return Err(focused_error(
            "no tracked A6 focused-evidence object exists; pass --publish-from after producing the required reports",
        ));
    }
    let mut source_revisions = BTreeSet::new();
    let mut current = Vec::new();
    for object in objects {
        let header: FocusedEvidenceHeader =
            serde_json::from_slice(&object.bytes).map_err(|error| {
                focused_error(format!(
                    "failed to parse A6 evidence header {}: {error}",
                    object.relative_path.display()
                ))
            })?;
        if !valid_revision(&header.source_revision) {
            return Err(focused_error(format!(
                "A6 evidence object {} has an invalid source revision",
                object.relative_path.display()
            )));
        }
        if let storage::EvidenceObjectName::Canonical {
            source_revision,
            sha256: _,
        } = &object.name
        {
            if source_revision != &header.source_revision {
                return Err(focused_error(format!(
                    "A6 evidence object {} names source revision {source_revision} but records {}",
                    object.relative_path.display(),
                    header.source_revision
                )));
            }
            if !source_revisions.insert(source_revision.clone()) {
                return Err(focused_error(format!(
                    "tracked A6 evidence contains more than one object for source revision {source_revision}"
                )));
            }
        }
        if revision::source_revision_is_current(
            root,
            &header.source_revision,
            Some(&object.relative_path),
        )? {
            current.push(object);
        }
    }
    let [object] = current.as_slice() else {
        return Err(focused_error(format!(
            "tracked A6 evidence has {} source-current objects, expected exactly one; select an object explicitly only for diagnosis",
            current.len()
        )));
    };
    let ledger = parse_ledger(&object.relative_path, &object.bytes)?;
    require_object_identity(object, &ledger)?;
    Ok((object.relative_path.clone(), ledger))
}

fn parse_ledger(relative_path: &Path, bytes: &[u8]) -> Result<FocusedEvidenceLedger, BenchError> {
    serde_json::from_slice(bytes).map_err(|error| {
        focused_error(format!(
            "failed to parse {}: {error}",
            relative_path.display()
        ))
    })
}

fn require_object_identity(
    object: &storage::TrackedEvidenceObject,
    ledger: &FocusedEvidenceLedger,
) -> Result<(), BenchError> {
    if let storage::EvidenceObjectName::Canonical {
        source_revision,
        sha256: _,
    } = &object.name
        && source_revision != &ledger.source_revision
    {
        return Err(focused_error(format!(
            "A6 evidence object {} names source revision {source_revision} but records {}",
            object.relative_path.display(),
            ledger.source_revision
        )));
    }
    Ok(())
}

fn validate_ledger(
    root: &RepoRoot,
    ledger: &FocusedEvidenceLedger,
    verify_remaining_artifacts: bool,
    evidence_path: Option<&Path>,
) -> Result<(), BenchError> {
    validate_structure(ledger)?;
    revision::validate_source_revision(root, &ledger.source_revision, evidence_path)?;
    let manifest = BenchmarkManifest::read(root)?;
    manifest.check(root)?;
    let measurement_contract =
        measurement_contract::A6MeasurementContract::read_and_validate(root, &manifest)?;
    let matrix = read_bound_report(root, &ledger.matrix_report)?;
    require_matrix_contract(
        root,
        &matrix,
        &ledger.source_revision,
        &manifest,
        &measurement_contract,
    )?;
    let baseline = read_bound_baseline(root, &ledger.baseline_report)?;
    require_baseline_contract(&baseline, &matrix, &ledger.baseline_report, &manifest)?;
    let predecessor_phases = expected_predecessor_phases(&matrix)?;
    let predecessor_registry =
        predecessors::read_and_validate(root, &ledger.source_revision, &predecessor_phases)?;
    if ledger.predecessor_registry_sha256 != predecessor_registry.source_sha256() {
        return Err(focused_error(format!(
            "ledger predecessor_registry_sha256={} expected {}",
            ledger.predecessor_registry_sha256,
            predecessor_registry.source_sha256()
        )));
    }
    validate_matrix_phase_coverage(ledger, &matrix)?;
    verify_matrix_phase_values(ledger, &matrix)?;
    if verify_remaining_artifacts {
        verify_artifacts(root, ledger, &matrix, &predecessor_registry)?;
    }
    Ok(())
}

fn expected_predecessor_phases(
    matrix: &CompareReport,
) -> Result<BTreeSet<predecessors::PhaseKey>, BenchError> {
    let initial = INITIAL_SEED_PHASES
        .iter()
        .map(|(row, measurement)| predecessors::PhaseKey::new(*row, *measurement))
        .collect::<BTreeSet<_>>();
    let mut expected = BTreeSet::new();
    for row in &matrix.rows {
        if row.comparability != ComparabilityClass::ReportOnly {
            continue;
        }
        for measurement in &row.stab_measurements {
            let phase = predecessors::PhaseKey::new(&row.id, &measurement.name);
            if !initial.contains(&phase) && !expected.insert(phase.clone()) {
                return Err(focused_error(format!(
                    "matrix repeats predecessor phase {}/{}",
                    phase.row_id, phase.measurement
                )));
            }
        }
    }
    Ok(expected)
}

fn validate_matrix_phase_coverage(
    ledger: &FocusedEvidenceLedger,
    matrix: &CompareReport,
) -> Result<(), BenchError> {
    let mut expected = BTreeSet::new();
    let mut issues = Vec::new();
    for row in &matrix.rows {
        if row.comparability != ComparabilityClass::ReportOnly {
            continue;
        }
        for measurement in &row.stab_measurements {
            let key = (row.id.clone(), measurement.name.clone());
            if !expected.insert(key.clone()) {
                issues.push(format!(
                    "matrix repeats report-only phase {}/{}",
                    key.0, key.1
                ));
            }
        }
    }
    let initial = INITIAL_SEED_PHASES
        .iter()
        .map(|(row, measurement)| ((*row).to_string(), (*measurement).to_string()))
        .collect::<BTreeSet<_>>();
    validate_phase_coverage(ledger, &expected, &initial, &mut issues);
    if issues.is_empty() {
        Ok(())
    } else {
        Err(focused_error(issues.join("\n")))
    }
}

fn validate_phase_coverage(
    ledger: &FocusedEvidenceLedger,
    expected: &BTreeSet<(String, String)>,
    initial: &BTreeSet<(String, String)>,
    issues: &mut Vec<String>,
) {
    let actual = ledger
        .phases
        .iter()
        .map(|phase| (phase.row_id.clone(), phase.measurement.clone()))
        .collect::<BTreeSet<_>>();
    for missing in expected.difference(&actual) {
        issues.push(format!(
            "ledger omits report-only matrix phase {}/{}",
            missing.0, missing.1
        ));
    }
    for extra in actual.difference(expected) {
        issues.push(format!(
            "ledger invents non-report-only matrix phase {}/{}",
            extra.0, extra.1
        ));
    }
    for missing in initial.difference(expected) {
        issues.push(format!(
            "source-owned initial seed {}/{} is absent from the matrix",
            missing.0, missing.1
        ));
    }
    for phase in &ledger.phases {
        let key = (phase.row_id.clone(), phase.measurement.clone());
        let has_predecessor = phase.predecessor_report.is_some()
            && phase.predecessor_seconds.is_some()
            && phase.source_over_predecessor.is_some();
        let has_seed_reason = phase
            .initial_seed_reason
            .as_deref()
            .is_some_and(|reason| !reason.trim().is_empty());
        if initial.contains(&key) {
            if has_predecessor || !has_seed_reason {
                issues.push(format!(
                    "initial seed {}/{} must have only a nonempty initial_seed_reason",
                    phase.row_id, phase.measurement
                ));
            }
        } else if !has_predecessor || phase.initial_seed_reason.is_some() {
            issues.push(format!(
                "comparable phase {}/{} must have only a complete predecessor triple",
                phase.row_id, phase.measurement
            ));
        }
    }
}

fn verify_matrix_phase_values(
    ledger: &FocusedEvidenceLedger,
    matrix: &CompareReport,
) -> Result<(), BenchError> {
    for phase in &ledger.phases {
        let current = find_measurement(matrix, &phase.row_id, &phase.measurement)?;
        require_recorded_seconds(
            &format!("{}/{} matrix", phase.row_id, phase.measurement),
            current.seconds,
            phase.source_current_seconds,
        )?;
    }
    Ok(())
}

fn validate_profile(
    diagnostic: &FocusedDiagnostic,
    row_reproduces: bool,
    issues: &mut Vec<String>,
) {
    match (row_reproduces, &diagnostic.outcome, &diagnostic.profile) {
        (true, DiagnosticOutcome::ReproducedProfiled, ProfileDisposition::Captured { receipt })
        | (
            true,
            DiagnosticOutcome::ReproducedProfileUnavailable,
            ProfileDisposition::Unavailable { receipt },
        ) => validate_binding("hardware profile receipt", receipt, issues),
        (false, DiagnosticOutcome::ResolvedWithinBoundary, ProfileDisposition::NotRequired) => {}
        _ => issues.push(format!(
            "{} has outcome/profile state {:?}/{:?} inconsistent with reproduced={row_reproduces}",
            diagnostic.row_id, diagnostic.outcome, diagnostic.profile
        )),
    }
}

fn verify_artifacts(
    root: &RepoRoot,
    ledger: &FocusedEvidenceLedger,
    matrix: &CompareReport,
    predecessor_registry: &predecessors::ValidatedPredecessorRegistry,
) -> Result<(), BenchError> {
    let mut verified_predecessor_commits = BTreeSet::new();
    for phase in &ledger.phases {
        if let (Some(binding), Some(expected)) =
            (&phase.predecessor_report, phase.predecessor_seconds)
        {
            let report = read_bound_report(root, binding)?;
            if verified_predecessor_commits.insert(report.stab.commit.clone()) {
                revision::validate_preserved_commit(root, &report.stab.commit)?;
            }
            let phase_key = predecessors::PhaseKey::new(&phase.row_id, &phase.measurement);
            let identity =
                predecessor_registry.require_report_commit(&phase_key, &report.stab.commit)?;
            require_predecessor_contract(matrix, &report, phase, &binding.path, identity)?;
            let measurement = find_measurement(&report, &phase.row_id, &phase.measurement)?;
            require_recorded_seconds(
                &format!("{}/{} predecessor", phase.row_id, phase.measurement),
                measurement.seconds,
                expected,
            )?;
        }
    }

    for diagnostic in &ledger.diagnostics {
        let report = read_bound_report(root, &diagnostic.report)?;
        require_focused_contract(matrix, &report, diagnostic, &ledger.source_revision)?;
        if let Some(receipt) = diagnostic.profile.receipt() {
            let forbidden = profile_forbidden_bindings(ledger, diagnostic);
            let parsed = profile_receipt::read_and_validate(
                root,
                receipt,
                &diagnostic.report,
                &report,
                &forbidden,
            )?;
            match (&diagnostic.profile, parsed.outcome()) {
                (
                    ProfileDisposition::Captured { .. },
                    profile_receipt::ProfileOutcome::Captured { .. },
                )
                | (
                    ProfileDisposition::Unavailable { .. },
                    profile_receipt::ProfileOutcome::Unavailable { .. },
                ) => {}
                _ => {
                    return Err(focused_error(format!(
                        "{} profile disposition disagrees with its typed receipt",
                        diagnostic.row_id
                    )));
                }
            }
        }
    }
    Ok(())
}

fn profile_forbidden_bindings(
    ledger: &FocusedEvidenceLedger,
    diagnostic: &FocusedDiagnostic,
) -> Vec<ArtifactBinding> {
    let mut forbidden = vec![ledger.baseline_report.clone(), ledger.matrix_report.clone()];
    forbidden.extend(
        ledger
            .phases
            .iter()
            .filter_map(|phase| phase.predecessor_report.clone()),
    );
    for other in &ledger.diagnostics {
        if other.row_id != diagnostic.row_id {
            forbidden.push(other.report.clone());
            if let Some(receipt) = other.profile.receipt() {
                forbidden.push(receipt.clone());
            }
        }
    }
    forbidden
}

fn require_baseline_contract(
    baseline: &BaselineReport,
    matrix: &CompareReport,
    binding: &ArtifactBinding,
    manifest: &BenchmarkManifest,
) -> Result<(), BenchError> {
    let expected_rows = a6_manifest_rows(manifest);
    if baseline.schema_version != A6_BASELINE_SCHEMA_VERSION
        || baseline.generated_unix_epoch_seconds > matrix.generated_unix_epoch_seconds
        || baseline.machine != matrix.machine
        || !matrix.machine.has_private_host_fingerprint()
        || !matrix.stab.has_bound_executable()
        || baseline.stim != matrix.stim
        || baseline.stim.expected_tag != STIM_TAG
        || baseline.stim.actual_tag != STIM_TAG
        || baseline.stim.expected_commit != STIM_COMMIT
        || baseline.stim.actual_commit != STIM_COMMIT
        || baseline.command.target_seconds.to_bits() != A6_BASELINE_TARGET_SECONDS.to_bits()
        || baseline.command.cli_iterations != A6_BASELINE_CLI_ITERATIONS
        || baseline.command.primary
        || !baseline.command.new_output
        || !path_ends_with(&matrix.command.baseline_path, &binding.path)
        || matrix.command.baseline_sha256 != binding.sha256
        || baseline.rows.len() != expected_rows.len()
        || expected_rows.len() != 166
    {
        return Err(focused_error(
            "baseline report does not bind the fresh same-host pinned-Stim 166-row A6 contract",
        ));
    }
    for (actual, expected) in baseline.rows.iter().zip(expected_rows) {
        require_baseline_manifest_row_contract(actual, expected)?;
        require_baseline_evidence_status(actual)?;
        let matrix_row = find_row(matrix, &actual.id)?;
        if matrix_row.stim_measurements != actual.measurements {
            return Err(focused_error(format!(
                "matrix does not preserve baseline measurements for {}",
                actual.id
            )));
        }
    }
    Ok(())
}

fn require_matrix_contract(
    root: &RepoRoot,
    report: &CompareReport,
    revision: &str,
    manifest: &BenchmarkManifest,
    measurement_contract: &measurement_contract::A6MeasurementContract,
) -> Result<(), BenchError> {
    let expected_rows = a6_manifest_rows(manifest);
    let profiler_roots_match = report.command.profiler_notes_paths.len()
        == A6_PROFILER_NOTE_ROOTS.len()
        && report
            .command
            .profiler_notes_paths
            .iter()
            .zip(A6_PROFILER_NOTE_ROOTS)
            .all(|(actual, expected)| path_ends_with(actual, expected));
    if report.schema_version != COMPARE_REPORT_SCHEMA_VERSION
        || report.stab.commit != revision
        || report.stab.local_modifications
        || report.stim.expected_tag != STIM_TAG
        || report.stim.actual_tag != STIM_TAG
        || report.stim.expected_commit != STIM_COMMIT
        || report.stim.actual_commit != STIM_COMMIT
        || report.command.profile != "release"
        || report.command.milestone.is_some()
        || report.command.primary
        || !report.command.cargo_features.is_empty()
        || report.command.timing_boundary != COMPARE_TIMING_BOUNDARY
        || report.command.measurement_contract_path.as_deref()
            != Some("benchmarks/a6-measurement-contract.json")
        || report.command.measurement_contract_sha256 != measurement_contract.source_sha256()
        || !report.command.require_profiler_notes
        || !profiler_roots_match
        || report.command.require_beta_gate
        || report.command.beta_waivers_path.is_some()
        || report.command.require_memory_gate
        || report.command.memory_baseline_path.is_some()
        || report.command.track_allocations
        || !report.command.strict
        || !report.command.warmup
        || report.command.measurement_runs != 3
        || !report.command.new_output
        || !report
            .command
            .thresholds_path
            .as_deref()
            .is_some_and(|path| path_ends_with(path, "benchmarks/m12-primary-thresholds.json"))
        || !report
            .command
            .regression_waivers_path
            .as_deref()
            .is_some_and(|path| {
                path_ends_with(path, "benchmarks/m12-primary-regression-waivers.json")
            })
        || report.rows.len() != expected_rows.len()
        || expected_rows.len() != 166
    {
        return Err(focused_error(
            "matrix report does not bind the clean 166-row warmed three-run A6 contract",
        ));
    }
    for (actual, expected) in report.rows.iter().zip(expected_rows) {
        require_manifest_row_contract(actual, expected)?;
        require_matrix_evidence_status(actual, expected.threshold_class)?;
    }
    measurement_contract.require_report(report)?;
    require_matrix_policies(root, report)?;
    Ok(())
}

fn a6_manifest_rows(manifest: &BenchmarkManifest) -> Vec<&BenchmarkRow> {
    manifest
        .rows
        .iter()
        .filter(|row| row.milestone != Milestone::M12)
        .collect()
}

fn require_baseline_evidence_status(actual: &BaselineRowResult) -> Result<(), BenchError> {
    let valid = match actual.runner {
        Runner::ContractOnly => actual.status == "contract-only" && actual.measurements.is_empty(),
        Runner::StimCli | Runner::StimPerf => {
            actual.status == "measured" && !actual.measurements.is_empty()
        }
    };
    if valid {
        Ok(())
    } else {
        Err(focused_error(format!(
            "{} has invalid pinned-Stim baseline evidence for runner {}",
            actual.id,
            actual.runner.as_str()
        )))
    }
}

fn require_baseline_manifest_row_contract(
    actual: &BaselineRowResult,
    expected: &BenchmarkRow,
) -> Result<(), BenchError> {
    let expected_program = match expected.runner {
        Runner::ContractOnly => "",
        Runner::StimCli => "stim",
        Runner::StimPerf => "stim_perf",
    };
    let actual_program = Path::new(&actual.command.program)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let expected_args = match expected.runner {
        Runner::ContractOnly => Vec::new(),
        Runner::StimCli => expected.argv_tokens(),
        Runner::StimPerf => vec![
            "--only".to_string(),
            expected.stim_perf_filter.clone(),
            "--target_seconds".to_string(),
            A6_BASELINE_TARGET_SECONDS.to_string(),
        ],
    };
    if actual.id != expected.id
        || actual.milestone != expected.milestone
        || actual.threshold_class != expected.threshold_class.as_str()
        || actual.runner != expected.runner
        || actual.upstream_source != expected.upstream_source
        || actual.phase != expected.phase
        || actual.measurement != expected.measurement
        || actual.command.stdin_path != expected.stdin_path
        || actual_program != expected_program
        || actual.command.args != expected_args
    {
        return Err(focused_error(format!(
            "baseline row {} disagrees with the source-owned benchmark manifest",
            actual.id
        )));
    }
    Ok(())
}

fn require_matrix_evidence_status(
    actual: &CompareRowResult,
    threshold_class: ThresholdClass,
) -> Result<(), BenchError> {
    match threshold_class {
        ThresholdClass::BaselineMetadata => {
            if actual.status != "contract-only" || !actual.stab_measurements.is_empty() {
                return Err(focused_error(format!(
                    "{} is the metadata anchor but has runtime evidence",
                    actual.id
                )));
            }
        }
        _ => {
            if actual.status != "measured" || actual.stab_measurements.is_empty() {
                return Err(focused_error(format!(
                    "{} is an executable A6 row without measured Stab evidence",
                    actual.id
                )));
            }
        }
    }
    if !matches!(
        actual.regression_threshold_status.as_str(),
        "pass" | "not-configured" | "waived-not-thresholdable"
    ) {
        return Err(focused_error(format!(
            "{} has unacceptable regression threshold status {}",
            actual.id, actual.regression_threshold_status
        )));
    }
    if !matches!(
        actual.profiler_note_status.as_str(),
        "present" | "not-required"
    ) {
        return Err(focused_error(format!(
            "{} has unacceptable profiler-note status {}",
            actual.id, actual.profiler_note_status
        )));
    }
    Ok(())
}

fn require_manifest_row_contract(
    actual: &CompareRowResult,
    expected: &BenchmarkRow,
) -> Result<(), BenchError> {
    if actual.id != expected.id
        || actual.milestone != expected.milestone
        || actual.threshold_class != expected.threshold_class.as_str()
        || actual.runner != expected.runner
        || actual.comparability != expected.comparability
        || actual.upstream_source != expected.upstream_source
        || actual.phase != expected.phase
        || actual.measurement != expected.measurement
    {
        return Err(focused_error(format!(
            "matrix row {} disagrees with the source-owned benchmark manifest",
            actual.id
        )));
    }
    Ok(())
}

fn require_predecessor_contract(
    matrix: &CompareReport,
    predecessor: &CompareReport,
    phase: &PhaseEvidence,
    path: &str,
    identity: &predecessors::PredecessorIdentity,
) -> Result<(), BenchError> {
    if predecessor.schema_version != COMPARE_REPORT_SCHEMA_VERSION
        || predecessor.stab.local_modifications
        || !predecessor.stab.has_bound_executable()
        || predecessor.stab.commit != identity.instrumentation_backport_commit
        || predecessor.stab.commit == matrix.stab.commit
        || !valid_revision(&predecessor.stab.commit)
        || predecessor.generated_unix_epoch_seconds > matrix.generated_unix_epoch_seconds
        || predecessor.machine != matrix.machine
        || predecessor.stim.expected_tag != STIM_TAG
        || predecessor.stim.actual_tag != STIM_TAG
        || predecessor.stim.expected_commit != STIM_COMMIT
        || predecessor.stim.actual_commit != STIM_COMMIT
        || predecessor.command.profile != "release"
        || predecessor.command.cargo_features != matrix.command.cargo_features
        || predecessor.command.timing_boundary != matrix.command.timing_boundary
        || predecessor.command.measurement_contract_path != matrix.command.measurement_contract_path
        || predecessor.command.measurement_contract_sha256
            != matrix.command.measurement_contract_sha256
        || !predecessor.command.warmup
        || predecessor.command.measurement_runs != 1
        || !predecessor.command.strict
        || predecessor.command.track_allocations
        || !predecessor.command.new_output
    {
        return Err(focused_error(format!(
            "predecessor report {path} is not a clean non-instrumented release report"
        )));
    }
    let current_row = find_row(matrix, &phase.row_id)?;
    let predecessor_row = find_row(predecessor, &phase.row_id)?;
    require_same_row_contract(current_row, predecessor_row, path)?;
    require_same_row_measurement_contract(
        current_row,
        matrix.command.measurement_runs,
        predecessor_row,
        predecessor.command.measurement_runs,
        path,
    )?;
    let current_measurement = find_measurement(matrix, &phase.row_id, &phase.measurement)?;
    let predecessor_measurement = find_measurement(predecessor, &phase.row_id, &phase.measurement)?;
    require_same_row_native_iterations(
        current_measurement,
        matrix.command.measurement_runs,
        predecessor_measurement,
        predecessor.command.measurement_runs,
        &phase.row_id,
        &phase.measurement,
        path,
    )
}

fn require_focused_contract(
    matrix: &CompareReport,
    report: &CompareReport,
    diagnostic: &FocusedDiagnostic,
    revision: &str,
) -> Result<(), BenchError> {
    if report.schema_version != COMPARE_REPORT_SCHEMA_VERSION
        || report.stab != matrix.stab
        || report.stab.commit != revision
        || report.generated_unix_epoch_seconds < matrix.generated_unix_epoch_seconds
        || report.machine != matrix.machine
        || report.stim != matrix.stim
        || report.stim.expected_tag != STIM_TAG
        || report.stim.actual_tag != STIM_TAG
        || report.stim.expected_commit != STIM_COMMIT
        || report.stim.actual_commit != STIM_COMMIT
        || report.command.profile != "release"
        || report.command.baseline_sha256 != matrix.command.baseline_sha256
        || report.command.cargo_features != matrix.command.cargo_features
        || report.command.timing_boundary != matrix.command.timing_boundary
        || report.command.measurement_contract_path != matrix.command.measurement_contract_path
        || report.command.measurement_contract_sha256 != matrix.command.measurement_contract_sha256
        || !report.command.warmup
        || report.command.measurement_runs != 1
        || !report.command.strict
        || report.command.track_allocations
        || !report.command.new_output
        || report.command.filters != [diagnostic.row_id.as_str()]
        || report.rows.len() != 1
    {
        return Err(focused_error(format!(
            "{} does not bind one clean warmed outer run for {}",
            diagnostic.report.path, diagnostic.row_id
        )));
    }
    let matrix_row = find_row(matrix, &diagnostic.row_id)?;
    let focused_row = find_row(report, &diagnostic.row_id)?;
    require_same_row_contract(matrix_row, focused_row, &diagnostic.report.path)?;
    require_same_row_measurement_contract(
        matrix_row,
        matrix.command.measurement_runs,
        focused_row,
        report.command.measurement_runs,
        &diagnostic.report.path,
    )?;
    if focused_row.stim_measurements != matrix_row.stim_measurements {
        return Err(focused_error(format!(
            "{} does not preserve pinned-Stim baseline measurements for {}",
            diagnostic.report.path, diagnostic.row_id
        )));
    }
    if focused_row.status != "measured" {
        return Err(focused_error(format!(
            "{} did not produce measured focused evidence",
            diagnostic.report.path
        )));
    }
    for expected in &diagnostic.measurements {
        let measurement = find_measurement(report, &diagnostic.row_id, &expected.measurement)?;
        let matrix_measurement =
            find_measurement(matrix, &diagnostic.row_id, &expected.measurement)?;
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
        require_same_row_native_iterations(
            matrix_measurement,
            matrix.command.measurement_runs,
            measurement,
            report.command.measurement_runs,
            &diagnostic.row_id,
            &expected.measurement,
            &diagnostic.report.path,
        )?;
    }
    Ok(())
}

fn require_same_row_native_iterations(
    current: &Measurement,
    current_outer_runs: usize,
    other: &Measurement,
    other_outer_runs: usize,
    row_id: &str,
    measurement: &str,
    other_path: &str,
) -> Result<(), BenchError> {
    let current_iterations = normalized_iterations(current.iterations, current_outer_runs);
    let other_iterations = normalized_iterations(other.iterations, other_outer_runs);
    match (current_iterations, other_iterations) {
        (Some(current), Some(other)) if current == other => Ok(()),
        _ => Err(focused_error(format!(
            "{other_path} changes row-native iterations for {row_id}/{measurement}: matrix {:?} over {current_outer_runs} outer runs, other {:?} over {other_outer_runs}",
            current.iterations, other.iterations
        ))),
    }
}

fn normalized_iterations(iterations: Option<usize>, outer_runs: usize) -> Option<usize> {
    let iterations = iterations?;
    if outer_runs == 0 || !iterations.is_multiple_of(outer_runs) {
        return None;
    }
    Some(iterations / outer_runs)
}

fn require_same_row_contract(
    current: &CompareRowResult,
    other: &CompareRowResult,
    other_path: &str,
) -> Result<(), BenchError> {
    if current.id != other.id
        || current.milestone != other.milestone
        || current.threshold_class != other.threshold_class
        || current.runner != other.runner
        || current.comparability != other.comparability
        || current.upstream_source != other.upstream_source
        || current.phase != other.phase
        || current.measurement != other.measurement
    {
        return Err(focused_error(format!(
            "{other_path} does not preserve the row contract for {}",
            current.id
        )));
    }
    Ok(())
}

fn require_same_row_measurement_contract(
    current: &CompareRowResult,
    current_outer_runs: usize,
    other: &CompareRowResult,
    other_outer_runs: usize,
    other_path: &str,
) -> Result<(), BenchError> {
    if current.stab_measurements.len() != other.stab_measurements.len() {
        return Err(focused_error(format!(
            "{other_path} changes the measurement set for {}",
            current.id
        )));
    }
    for (current_measurement, other_measurement) in current
        .stab_measurements
        .iter()
        .zip(&other.stab_measurements)
    {
        if current_measurement.name != other_measurement.name
            || current_measurement.observations != other_measurement.observations
        {
            return Err(focused_error(format!(
                "{other_path} changes measurement identity or workload observations for {}",
                current.id
            )));
        }
        require_same_row_native_iterations(
            current_measurement,
            current_outer_runs,
            other_measurement,
            other_outer_runs,
            &current.id,
            &current_measurement.name,
            other_path,
        )?;
    }
    Ok(())
}

fn find_row<'a>(
    report: &'a CompareReport,
    row_id: &str,
) -> Result<&'a CompareRowResult, BenchError> {
    report
        .rows
        .iter()
        .find(|row| row.id == row_id)
        .ok_or_else(|| focused_error(format!("report omits row {row_id}")))
}

fn find_measurement<'a>(
    report: &'a CompareReport,
    row_id: &str,
    name: &str,
) -> Result<&'a Measurement, BenchError> {
    let row = find_row(report, row_id)?;
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

fn read_bound_baseline(
    root: &RepoRoot,
    binding: &ArtifactBinding,
) -> Result<BaselineReport, BenchError> {
    let bytes = verify_binding(root, binding, MAX_REPORT_BYTES)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| focused_error(format!("failed to parse {}: {error}", binding.path)))
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
    if actual.to_bits() == recorded.to_bits() {
        Ok(())
    } else {
        Err(focused_error(format!(
            "{label} records {recorded} seconds, bound report contains {actual}"
        )))
    }
}

fn focused_error(message: impl Into<String>) -> BenchError {
    BenchError::Qualification(format!(
        "A6 focused evidence validation failed:\n{}",
        message.into()
    ))
}

#[cfg(test)]
use policy::{require_a6_selected_pair_gates, require_raw_derived_fields};

#[cfg(test)]
mod tests;
