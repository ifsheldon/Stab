use super::*;
use crate::compare::rebuild_compare_row_raw_evidence;
use crate::report::RowCommandMetadata;

fn binding(path: &str) -> ArtifactBinding {
    ArtifactBinding {
        path: path.to_string(),
        sha256: "a".repeat(64),
    }
}

fn phase(ratio: f64) -> PhaseEvidence {
    let mut predecessor = binding("target/benchmarks/old/compare.json");
    predecessor.sha256 = "b".repeat(64);
    PhaseEvidence {
        row_id: "row".to_string(),
        measurement: "stab_measurement".to_string(),
        source_current_seconds: ratio,
        predecessor_report: Some(predecessor),
        predecessor_seconds: Some(1.0),
        source_over_predecessor: Some(ratio),
        initial_seed_reason: None,
    }
}

fn initial_phase(row_id: &str, measurement: &str) -> PhaseEvidence {
    PhaseEvidence {
        row_id: row_id.to_string(),
        measurement: measurement.to_string(),
        source_current_seconds: 1.0,
        predecessor_report: None,
        predecessor_seconds: None,
        source_over_predecessor: None,
        initial_seed_reason: Some("first clean source-owned seed".to_string()),
    }
}

fn comparable_phase(row_id: &str, measurement: &str) -> PhaseEvidence {
    let mut predecessor = binding("target/benchmarks/old/compare.json");
    predecessor.sha256 = "b".repeat(64);
    PhaseEvidence {
        row_id: row_id.to_string(),
        measurement: measurement.to_string(),
        source_current_seconds: 1.0,
        predecessor_report: Some(predecessor),
        predecessor_seconds: Some(1.0),
        source_over_predecessor: Some(1.0),
        initial_seed_reason: None,
    }
}

fn profile_receipt() -> ArtifactBinding {
    ArtifactBinding {
        path: "target/benchmarks/profile-aaaaaaaa/profile-receipt.json".to_string(),
        sha256: "e".repeat(64),
    }
}

fn diagnostic(ratio: f64, profile: ProfileDisposition) -> FocusedDiagnostic {
    let mut report = binding("target/benchmarks/focused-aaaaaaaa/compare.json");
    report.sha256 = "d".repeat(64);
    let outcome = match (&profile, ratio > CROSSING_RATIO) {
        (ProfileDisposition::Captured { .. }, true) => DiagnosticOutcome::ReproducedProfiled,
        (ProfileDisposition::Unavailable { .. }, true) => {
            DiagnosticOutcome::ReproducedProfileUnavailable
        }
        _ => DiagnosticOutcome::ResolvedWithinBoundary,
    };
    FocusedDiagnostic {
        row_id: "row".to_string(),
        report,
        internal_timing_count: 8,
        measurements: vec![FocusedMeasurement {
            measurement: "stab_measurement".to_string(),
            focused_seconds: ratio,
            focused_over_predecessor: ratio,
        }],
        outcome,
        profile,
        owner_action: "retain after review".to_string(),
    }
}

fn ledger(phase_ratio: f64, focused_ratio: f64) -> FocusedEvidenceLedger {
    FocusedEvidenceLedger {
        schema_version: SCHEMA_VERSION,
        source_revision: "a".repeat(40),
        predecessor_registry_sha256: "f".repeat(64),
        baseline_report: ArtifactBinding {
            path: "target/benchmarks/baseline-aaaaaaaa/baseline.json".to_string(),
            sha256: "c".repeat(64),
        },
        matrix_report: binding("target/benchmarks/matrix-aaaaaaaa/compare.json"),
        phases: vec![phase(phase_ratio)],
        diagnostics: vec![diagnostic(focused_ratio, ProfileDisposition::NotRequired)],
    }
}

fn measured_row() -> CompareRowResult {
    CompareRowResult {
        id: "row".to_string(),
        milestone: Milestone::M7,
        threshold_class: ThresholdClass::ReportOnly.as_str().to_string(),
        runner: Runner::ContractOnly,
        comparability: ComparabilityClass::ReportOnly,
        upstream_source: "src/stim/example.perf.cc".to_string(),
        phase: "throughput".to_string(),
        measurement: "row-work".to_string(),
        status: "measured".to_string(),
        baseline_summary: String::new(),
        stab_summary: String::new(),
        note: None,
        stim_measurements: Vec::new(),
        stab_measurements: vec![measurement("stab_measurement", 128)],
        stim_median_seconds: None,
        stab_median_seconds: Some(1.0),
        relative_ratio: None,
        measurement_ratios: Vec::new(),
        stab_allocation_count_max: None,
        stab_allocation_bytes_max: None,
        stab_resident_bytes_max: None,
        stab_resident_delta_bytes_max: None,
        pass_fail_status: "not-comparable".to_string(),
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
        regression_threshold_status: "not-configured".to_string(),
        regression_threshold_max_ratio: None,
        regression_threshold_waiver_reason: None,
        regression_threshold_waiver_follow_up: None,
        regression_threshold_error: None,
        profiler_note_status: "not-required".to_string(),
        profiler_note_path: None,
        profiler_note_error: None,
    }
}

fn measurement(name: &str, iterations: usize) -> Measurement {
    Measurement {
        name: name.to_string(),
        seconds: 1.0,
        variance_seconds: None,
        allocation: None,
        resident_bytes: None,
        resident_delta_bytes: None,
        observations: Vec::new(),
        iterations: Some(iterations),
    }
}

fn baseline_row(runner: Runner, status: &str, measurements: Vec<Measurement>) -> BaselineRowResult {
    BaselineRowResult {
        id: "row".to_string(),
        milestone: Milestone::M7,
        threshold_class: ThresholdClass::ReportOnly.as_str().to_string(),
        runner,
        upstream_source: "src/stim/example.perf.cc".to_string(),
        phase: "throughput".to_string(),
        measurement: "row-work".to_string(),
        status: status.to_string(),
        command: RowCommandMetadata {
            program: "stim".to_string(),
            args: Vec::new(),
            stdin_path: String::new(),
        },
        measurements,
    }
}

fn compare_report(
    mut row: CompareRowResult,
    commit: &str,
    measurement_runs: usize,
) -> CompareReport {
    let iterations = row
        .stab_measurements
        .first()
        .and_then(|measurement| measurement.iterations)
        .unwrap_or(8);
    if let Some(measurement) = row.stab_measurements.first_mut() {
        measurement.iterations = Some(iterations);
    }
    serde_json::from_value(serde_json::json!({
        "schema_version": COMPARE_REPORT_SCHEMA_VERSION,
        "generated_unix_epoch_seconds": 1,
        "machine": {
            "os": "linux",
            "arch": "x86_64",
            "family": "unix",
            "host_fingerprint": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "available_parallelism": 1,
            "rustc_version": "rustc test",
            "cmake_version": "cmake test"
        },
        "stim": {
            "source_path": "vendor/stim",
            "expected_tag": STIM_TAG,
            "expected_commit": STIM_COMMIT,
            "actual_tag": STIM_TAG,
            "actual_commit": STIM_COMMIT
        },
        "stab": {
            "commit": commit,
            "local_modifications": false,
            "executable_sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
        },
        "command": {
            "baseline_path": "target/benchmarks/baseline/baseline.json",
            "baseline_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "profile": "release",
            "milestone": null,
            "primary": false,
            "filters": ["row"],
            "cargo_features": [],
            "timing_boundary": COMPARE_TIMING_BOUNDARY,
            "measurement_contract_path": "benchmarks/a6-measurement-contract.json",
            "measurement_contract_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "require_profiler_notes": false,
            "require_beta_gate": false,
            "beta_waivers_path": null,
            "regression_waivers_path": null,
            "require_memory_gate": false,
            "memory_baseline_path": null,
            "thresholds_path": null,
            "profiler_notes_path": null,
            "profiler_notes_paths": [],
            "track_allocations": false,
            "warmup": true,
            "measurement_runs": measurement_runs,
            "strict": true,
            "new_output": true
        },
        "rows": [row]
    }))
    .expect("synthetic compare report")
}

fn set_iterations(row: &mut CompareRowResult, iterations: usize) {
    row.stab_measurements
        .first_mut()
        .expect("fixture has one Stab measurement")
        .iterations = Some(iterations);
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
    assert!(
        error
            .to_string()
            .contains("inconsistent with reproduced=true")
    );

    let mut unavailable = ledger(1.2, 1.151);
    let diagnostic = unavailable
        .diagnostics
        .first_mut()
        .expect("fixture has a diagnostic");
    diagnostic.profile = ProfileDisposition::Unavailable {
        receipt: profile_receipt(),
    };
    diagnostic.outcome = DiagnosticOutcome::ReproducedProfileUnavailable;
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

#[test]
fn phase_coverage_rejects_omitted_and_invented_phases() {
    let expected = BTreeSet::from([
        ("seed-row".to_string(), "seed-measurement".to_string()),
        (
            "comparable-row".to_string(),
            "comparable-measurement".to_string(),
        ),
    ]);
    let initial = BTreeSet::from([("seed-row".to_string(), "seed-measurement".to_string())]);
    let ledger = FocusedEvidenceLedger {
        schema_version: SCHEMA_VERSION,
        source_revision: "a".repeat(40),
        predecessor_registry_sha256: "f".repeat(64),
        baseline_report: ArtifactBinding {
            path: "target/benchmarks/baseline-aaaaaaaa/baseline.json".to_string(),
            sha256: "c".repeat(64),
        },
        matrix_report: binding("target/benchmarks/matrix-aaaaaaaa/compare.json"),
        phases: vec![
            initial_phase("seed-row", "seed-measurement"),
            comparable_phase("invented-row", "invented-measurement"),
        ],
        diagnostics: Vec::new(),
    };
    let mut issues = Vec::new();
    validate_phase_coverage(&ledger, &expected, &initial, &mut issues);
    let message = issues.join("\n");
    assert!(message.contains("omits report-only matrix phase comparable-row"));
    assert!(message.contains("invents non-report-only matrix phase invented-row"));
}

#[test]
fn phase_coverage_rejects_wrong_seed_classification() {
    let expected = BTreeSet::from([
        ("seed-row".to_string(), "seed-measurement".to_string()),
        (
            "comparable-row".to_string(),
            "comparable-measurement".to_string(),
        ),
    ]);
    let initial = BTreeSet::from([("seed-row".to_string(), "seed-measurement".to_string())]);
    let ledger = FocusedEvidenceLedger {
        schema_version: SCHEMA_VERSION,
        source_revision: "a".repeat(40),
        predecessor_registry_sha256: "f".repeat(64),
        baseline_report: ArtifactBinding {
            path: "target/benchmarks/baseline-aaaaaaaa/baseline.json".to_string(),
            sha256: "c".repeat(64),
        },
        matrix_report: binding("target/benchmarks/matrix-aaaaaaaa/compare.json"),
        phases: vec![
            comparable_phase("seed-row", "seed-measurement"),
            initial_phase("comparable-row", "comparable-measurement"),
        ],
        diagnostics: Vec::new(),
    };
    let mut issues = Vec::new();
    validate_phase_coverage(&ledger, &expected, &initial, &mut issues);
    let message = issues.join("\n");
    assert!(message.contains("initial seed seed-row/seed-measurement"));
    assert!(message.contains("comparable phase comparable-row/comparable-measurement"));
}

#[test]
fn matrix_evidence_status_rejects_failed_policy_or_missing_work() {
    let valid = measured_row();
    require_matrix_evidence_status(&valid, ThresholdClass::ReportOnly)
        .expect("measured report-only row");

    let mut failed = measured_row();
    failed.regression_threshold_status = "fail".to_string();
    let error = require_matrix_evidence_status(&failed, ThresholdClass::ReportOnly)
        .expect_err("failed threshold policy must be rejected");
    assert!(
        error
            .to_string()
            .contains("regression threshold status fail")
    );

    let mut missing = measured_row();
    missing.stab_measurements.clear();
    let error = require_matrix_evidence_status(&missing, ThresholdClass::ReportOnly)
        .expect_err("runtime row without work must be rejected");
    assert!(error.to_string().contains("without measured Stab evidence"));

    let mut metadata = measured_row();
    metadata.status = "contract-only".to_string();
    let error = require_matrix_evidence_status(&metadata, ThresholdClass::BaselineMetadata)
        .expect_err("metadata anchor cannot contain timings");
    assert!(error.to_string().contains("metadata anchor"));
}

#[test]
fn predecessor_identity_requires_row_native_iteration_equivalence() {
    let matrix = measurement("stab_measurement", 384);
    let predecessor = measurement("stab_measurement", 128);
    require_same_row_native_iterations(
        &matrix,
        3,
        &predecessor,
        1,
        "row",
        "stab_measurement",
        "predecessor.json",
    )
    .expect("three matrix runs and one predecessor run retain 128 timings each");

    let changed = measurement("stab_measurement", 127);
    let error = require_same_row_native_iterations(
        &matrix,
        3,
        &changed,
        1,
        "row",
        "stab_measurement",
        "predecessor.json",
    )
    .expect_err("changed row-native timing count must be rejected");
    assert!(error.to_string().contains("changes row-native iterations"));
}

#[test]
fn structure_rejects_current_matrix_as_predecessor() {
    let mut ledger = ledger(1.2, 1.1);
    let matrix = ledger.matrix_report.clone();
    ledger
        .phases
        .first_mut()
        .expect("fixture phase")
        .predecessor_report = Some(matrix);
    let error = validate_structure(&ledger).expect_err("current matrix is not a predecessor");
    assert!(
        error
            .to_string()
            .contains("reuses a current matrix or baseline artifact")
    );
}

#[test]
fn structure_binds_distinct_revision_named_baseline_and_matrix_reports() {
    let mut collision = ledger(1.2, 1.1);
    collision.baseline_report.sha256 = collision.matrix_report.sha256.clone();
    let error = validate_structure(&collision).expect_err("artifact digest collision");
    assert!(
        error
            .to_string()
            .contains("baseline_report and matrix_report must bind distinct artifacts")
    );

    let mut stale_name = ledger(1.2, 1.1);
    stale_name.matrix_report.path = "target/benchmarks/matrix-other/compare.json".to_string();
    let error = validate_structure(&stale_name).expect_err("revision-free artifact name");
    assert!(
        error
            .to_string()
            .contains("does not bind the source revision prefix")
    );
}

#[test]
fn baseline_evidence_status_matches_runner_contract() {
    require_baseline_evidence_status(&baseline_row(
        Runner::StimCli,
        "measured",
        vec![measurement("row", 3)],
    ))
    .expect("measured CLI baseline");
    require_baseline_evidence_status(&baseline_row(
        Runner::ContractOnly,
        "contract-only",
        Vec::new(),
    ))
    .expect("contract-only baseline");

    let error =
        require_baseline_evidence_status(&baseline_row(Runner::StimPerf, "measured", Vec::new()))
            .expect_err("measured baseline without measurements");
    assert!(error.to_string().contains("invalid pinned-Stim baseline"));
}

#[test]
fn focused_report_requires_exact_row_and_row_native_timing_count() {
    let current_commit = "a".repeat(40);
    let mut matrix_row = measured_row();
    set_iterations(&mut matrix_row, 24);
    let matrix = compare_report(matrix_row.clone(), &current_commit, 3);

    let mut focused_row = matrix_row;
    set_iterations(&mut focused_row, 8);
    let focused = compare_report(focused_row.clone(), &current_commit, 1);
    let diagnostic = diagnostic(1.0, ProfileDisposition::NotRequired);
    require_focused_contract(&matrix, &focused, &diagnostic, &current_commit)
        .expect("exact focused report");

    let mut changed_row = focused_row;
    set_iterations(&mut changed_row, 9);
    let changed = compare_report(changed_row, &current_commit, 1);
    let error = require_focused_contract(&matrix, &changed, &diagnostic, &current_commit)
        .expect_err("changed internal timing count");
    assert!(error.to_string().contains("row-native iterations"));

    let mut changed_features = focused.clone();
    changed_features
        .command
        .cargo_features
        .push("portable-simd".to_string());
    let error = require_focused_contract(&matrix, &changed_features, &diagnostic, &current_commit)
        .expect_err("changed Cargo feature selection");
    assert!(
        error
            .to_string()
            .contains("does not bind one clean warmed outer run")
    );
}

#[test]
fn predecessor_report_requires_distinct_clean_identity_and_same_row_contract() {
    let current_commit = "a".repeat(40);
    let predecessor_commit = "b".repeat(40);
    let mut matrix_row = measured_row();
    set_iterations(&mut matrix_row, 24);
    let matrix = compare_report(matrix_row.clone(), &current_commit, 3);

    let mut predecessor_row = matrix_row;
    set_iterations(&mut predecessor_row, 8);
    let predecessor = compare_report(predecessor_row.clone(), &predecessor_commit, 1);
    let phase = phase(1.0);
    let identity = predecessors::PredecessorIdentity {
        historical_product_commit: "e".repeat(40),
        instrumentation_backport_commit: predecessor_commit.clone(),
        patch_sha256: "f".repeat(64),
    };
    require_predecessor_contract(
        &matrix,
        &predecessor,
        &phase,
        "target/benchmarks/old/compare.json",
        &identity,
    )
    .expect("comparable predecessor");

    let mut cold_predecessor = predecessor.clone();
    cold_predecessor.command.warmup = false;
    let error = require_predecessor_contract(
        &matrix,
        &cold_predecessor,
        &phase,
        "target/benchmarks/cold/compare.json",
        &identity,
    )
    .expect_err("cold predecessor is not comparable");
    assert!(error.to_string().contains("not a clean non-instrumented"));

    let mut aggregated_predecessor = predecessor.clone();
    aggregated_predecessor.command.measurement_runs = 2;
    let error = require_predecessor_contract(
        &matrix,
        &aggregated_predecessor,
        &phase,
        "target/benchmarks/aggregated/compare.json",
        &identity,
    )
    .expect_err("aggregated predecessor is not the source-owned run shape");
    assert!(error.to_string().contains("not a clean non-instrumented"));

    let same_revision = compare_report(predecessor_row.clone(), &current_commit, 1);
    let error = require_predecessor_contract(
        &matrix,
        &same_revision,
        &phase,
        "target/benchmarks/current/compare.json",
        &identity,
    )
    .expect_err("current report is not a predecessor");
    assert!(error.to_string().contains("not a clean non-instrumented"));

    let mut changed_row = predecessor_row;
    changed_row.phase = "different-work".to_string();
    let changed = compare_report(changed_row, &predecessor_commit, 1);
    let error = require_predecessor_contract(
        &matrix,
        &changed,
        &phase,
        "target/benchmarks/changed/compare.json",
        &identity,
    )
    .expect_err("changed row contract");
    assert!(
        error
            .to_string()
            .contains("does not preserve the row contract")
    );

    let mut changed_boundary = predecessor;
    changed_boundary.command.timing_boundary = "different-boundary".to_string();
    let error = require_predecessor_contract(
        &matrix,
        &changed_boundary,
        &phase,
        "target/benchmarks/changed-boundary/compare.json",
        &identity,
    )
    .expect_err("changed timing boundary");
    assert!(error.to_string().contains("not a clean non-instrumented"));
}

#[test]
fn raw_derived_fields_reject_a_serialized_ratio_that_disagrees_with_measurements() {
    let mut actual = measured_row();
    actual.comparability = ComparabilityClass::DirectMatch;
    actual.stim_measurements = vec![measurement("case", 1)];
    actual.stab_measurements = vec![measurement("stab_case", 1)];
    actual.stim_median_seconds = Some(1.0);
    actual.stab_median_seconds = Some(1.0);
    actual.relative_ratio = Some(0.25);
    actual.pass_fail_status = "pass".to_string();

    let mut rebuilt = actual.clone();
    rebuild_compare_row_raw_evidence(&mut rebuilt);
    let error = require_raw_derived_fields(&actual, &rebuilt)
        .expect_err("serialized ratio must reconstruct from raw measurements");

    assert!(error.to_string().contains("do not reconstruct"));
    assert_eq!(rebuilt.relative_ratio, Some(1.0));
}

#[test]
fn a6_selected_pair_gate_includes_the_m6_equal_width_measurement() {
    let mut rows = Vec::new();
    for (row_id, stim_name, stab_name) in A6_SELECTED_PAIR_GATES {
        let mut row = measured_row();
        row.id = row_id.to_string();
        row.comparability = ComparabilityClass::DirectMatch;
        let mut stim = measurement(stim_name, 1);
        stim.seconds = 1.0;
        let mut stab = measurement(stab_name, 1);
        stab.seconds = if row_id == "m6-clifford-string" {
            1.251
        } else {
            1.0
        };
        row.stim_measurements = vec![stim];
        row.stab_measurements = vec![stab];
        rebuild_compare_row_raw_evidence(&mut row);
        rows.push(row);
    }

    let error =
        require_a6_selected_pair_gates(&rows).expect_err("M6 must retain its A6 1.25x gate");
    assert!(error.to_string().contains("m6-clifford-string"));
    assert!(error.to_string().contains("above 1.25x"));
}

#[test]
fn captured_profile_requires_unique_revision_bound_artifact() {
    let mut valid = ledger(1.2, 1.2);
    let diagnostic = valid.diagnostics.first_mut().expect("focused diagnostic");
    diagnostic.profile = ProfileDisposition::Captured {
        receipt: profile_receipt(),
    };
    diagnostic.outcome = DiagnosticOutcome::ReproducedProfiled;
    validate_structure(&valid).expect("distinct captured profile");

    let diagnostic = valid.diagnostics.first_mut().expect("focused diagnostic");
    assert!(matches!(
        diagnostic.profile,
        ProfileDisposition::Captured { .. }
    ));
    if let ProfileDisposition::Captured { receipt } = &mut diagnostic.profile {
        receipt.sha256 = diagnostic.report.sha256.clone();
    }
    let error = validate_structure(&valid).expect_err("profile role collision");
    assert!(
        error
            .to_string()
            .contains("profile receipt collides with another evidence role")
    );
}
