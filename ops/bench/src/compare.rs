#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

use sha2::Digest as _;

use crate::allocations::AllocationTrackingGuard;
use crate::baseline::{
    compare_note, run_stab_compare_row_with_root, summarize_measurements,
    summarize_stab_measurements, validate_baseline_metadata,
};
use crate::beta_gate::{apply_beta_gate, read_beta_waivers};
use crate::compare_evidence::aggregate_measurement_runs;
use crate::config::PREFIX;
use crate::error::BenchError;
use crate::manifest::{BenchmarkManifest, BenchmarkRow, Runner, ThresholdClass};
use crate::memory_gate::{apply_memory_gate, read_memory_baseline};
use crate::regression_waivers::{apply_regression_waivers, read_regression_waivers};
use crate::report::{
    BETA_GATE_MAX_RELATIVE_RATIO, BaselineReport, CompareRowResult, Measurement, StabMetadata,
    stab_metadata,
};
use crate::root::RepoRoot;
use crate::source_file::read_repo_regular_file_bounded;
use crate::thresholds::{apply_regression_thresholds, read_thresholds};

mod options;
mod profiler_notes;
mod report_output;

const MAX_COMPARE_SOURCE_BYTES: usize = 64 << 20;

use options::validate_compare_options;
use profiler_notes::ProfilerNoteFindings;
pub(crate) use profiler_notes::{HOT_PATH_PROFILER_NOTE_RATIO, validate_profiler_note_content};
use report_output::{CompareReportWrite, write_compare_report};

#[derive(Clone, Debug)]
pub(crate) struct CompareOptions {
    pub(crate) baseline: PathBuf,
    pub(crate) milestone: Option<String>,
    pub(crate) profile: String,
    pub(crate) primary: bool,
    pub(crate) only: Vec<String>,
    pub(crate) report: Option<PathBuf>,
    pub(crate) require_profiler_notes: bool,
    pub(crate) profiler_notes_dirs: Vec<PathBuf>,
    pub(crate) require_beta_gate: bool,
    pub(crate) beta_waivers: Option<PathBuf>,
    pub(crate) require_memory_gate: bool,
    pub(crate) memory_baseline: Option<PathBuf>,
    pub(crate) thresholds: Option<PathBuf>,
    pub(crate) regression_waivers: Option<PathBuf>,
    pub(crate) track_allocations: bool,
    pub(crate) warmup: bool,
    pub(crate) measurement_runs: usize,
    pub(crate) strict: bool,
    pub(crate) new_output: bool,
    pub(crate) measurement_contract: Option<PathBuf>,
}

pub(crate) fn run_compare(
    root: &RepoRoot,
    manifest: &BenchmarkManifest,
    options: &CompareOptions,
) -> Result<(), BenchError> {
    validate_compare_options(options)?;
    let execution_identity = stab_metadata(root)?;
    let _allocation_tracking = AllocationTrackingGuard::set(options.track_allocations)?;
    let baseline_path = root.resolve_relative(&options.baseline);
    let baseline_bytes =
        read_repo_regular_file_bounded(root, &baseline_path, MAX_COMPARE_SOURCE_BYTES)?;
    let baseline_sha256 = hex::encode(sha2::Sha256::digest(&baseline_bytes));
    let baseline_report: BaselineReport = serde_json::from_slice(&baseline_bytes)?;
    validate_baseline_metadata(&baseline_report)?;
    let measurement_contract_sha256 = options
        .measurement_contract
        .as_ref()
        .map(|path| {
            let resolved = root.resolve_relative(path);
            let bytes = read_repo_regular_file_bounded(root, &resolved, MAX_COMPARE_SOURCE_BYTES)?;
            Ok::<_, BenchError>(hex::encode(sha2::Sha256::digest(&bytes)))
        })
        .transpose()?
        .unwrap_or_default();
    let threshold_path = options
        .thresholds
        .as_ref()
        .map(|path| root.resolve_relative(path));
    let thresholds = threshold_path.as_deref().map(read_thresholds).transpose()?;
    let regression_waivers_path = options
        .regression_waivers
        .as_ref()
        .map(|path| root.resolve_relative(path));
    let regression_waivers = regression_waivers_path
        .as_deref()
        .map(read_regression_waivers)
        .transpose()?;
    let beta_waivers_path = options
        .beta_waivers
        .as_ref()
        .map(|path| root.resolve_relative(path));
    let beta_waivers = beta_waivers_path
        .as_deref()
        .map(read_beta_waivers)
        .transpose()?;
    let memory_baseline_path = options
        .memory_baseline
        .as_ref()
        .map(|path| root.resolve_relative(path));
    let memory_baseline = memory_baseline_path
        .as_deref()
        .map(read_memory_baseline)
        .transpose()?;
    let mut rows = manifest.compare_rows(options.milestone.as_deref(), options.primary)?;
    if !options.only.is_empty() {
        rows.retain(|row| {
            options
                .only
                .iter()
                .any(|filter| row.id == *filter || row.milestone.as_str() == filter)
        });
        if rows.is_empty() {
            return Err(BenchError::UnmatchedFilter(options.only.join(",")));
        }
    }
    println!(
        "[{PREFIX}] comparing {} row(s) against {}",
        rows.len(),
        baseline_path.display()
    );
    if options.warmup {
        run_warmup_rows_with_root(root, &rows, &options.profile)?;
    }
    let mut pending = Vec::new();
    let mut missing_baselines = Vec::new();
    let mut invalid_baselines = Vec::new();
    let mut contract_only_without_measurements = Vec::new();
    let mut report_rows = Vec::new();
    for row in rows {
        let mut baseline_status = BaselineCompareStatus::Comparable;
        let stim_summary = match summarize_baseline_row(&baseline_report, row) {
            BaselineSummary::Present(summary) => summary,
            BaselineSummary::Missing => {
                missing_baselines.push(row.id.clone());
                baseline_status = BaselineCompareStatus::Missing;
                "missing-baseline".to_string()
            }
            BaselineSummary::Invalid(reason) => {
                invalid_baselines.push(format!("{} ({reason})", row.id));
                baseline_status = BaselineCompareStatus::Invalid;
                format!("invalid-baseline({reason})")
            }
        };
        let stim_measurements = baseline_measurements(&baseline_report, row);
        let note = compare_note(&row.id).map(str::to_string);
        match run_recorded_stab_compare_row(root, row, &options.profile, options.measurement_runs)?
        {
            Some(measurements) => {
                let printed_note = note
                    .as_deref()
                    .map(|note| format!(" note={note}"))
                    .unwrap_or_default();
                if row.runner == Runner::ContractOnly && measurements.is_empty() {
                    println!(
                        "- {} {} status=contract-only stab=no-runner stim={}{}",
                        row.milestone.as_str(),
                        row.id,
                        stim_summary,
                        printed_note
                    );
                    if strict_requires_contract_measurement(row) {
                        contract_only_without_measurements.push(row.id.clone());
                    }
                    report_rows.push(build_compare_row_result(CompareRowBuild {
                        row,
                        status: "contract-only",
                        baseline_summary: &stim_summary,
                        stab_summary: "no-runner",
                        note,
                        stim_measurements,
                        stab_measurements: measurements,
                        baseline_status,
                    }));
                } else {
                    let stab_summary = summarize_stab_measurements(&row.id, &measurements);
                    println!(
                        "- {} {} status=measured stab={} stim={}{}",
                        row.milestone.as_str(),
                        row.id,
                        stab_summary,
                        stim_summary,
                        printed_note
                    );
                    report_rows.push(build_compare_row_result(CompareRowBuild {
                        row,
                        status: "measured",
                        baseline_summary: &stim_summary,
                        stab_summary: &stab_summary,
                        note,
                        stim_measurements,
                        stab_measurements: measurements,
                        baseline_status,
                    }));
                }
            }
            None => {
                println!(
                    "- {} {} status=pending stab=no-runner stim={}",
                    row.milestone.as_str(),
                    row.id,
                    stim_summary
                );
                pending.push(row.id.clone());
                report_rows.push(build_compare_row_result(CompareRowBuild {
                    row,
                    status: "pending",
                    baseline_summary: &stim_summary,
                    stab_summary: "no-runner",
                    note,
                    stim_measurements,
                    stab_measurements: Vec::new(),
                    baseline_status,
                }));
            }
        }
    }
    let regression_threshold_findings = thresholds
        .as_ref()
        .map_or_else(Default::default, |thresholds| {
            apply_regression_thresholds(&mut report_rows, thresholds)
        });
    let regression_waiver_findings = regression_waivers
        .as_ref()
        .map_or_else(Default::default, |regression_waivers| {
            apply_regression_waivers(&mut report_rows, regression_waivers)
        });
    let memory_gate_findings = memory_baseline
        .as_ref()
        .map_or_else(Default::default, |memory_baseline| {
            apply_memory_gate(&mut report_rows, memory_baseline)
        });
    let beta_gate_findings = if options.track_allocations {
        mark_instrumented_timing_not_evaluated(&mut report_rows);
        Default::default()
    } else {
        apply_beta_gate(&mut report_rows, beta_waivers.as_ref())
    };
    require_unchanged_execution_identity(root, &execution_identity)?;
    let profiler_note_findings = if let Some(report_dir) = &options.report {
        write_compare_report(CompareReportWrite {
            root,
            baseline_report: &baseline_report,
            baseline_path: &baseline_path,
            baseline_sha256: &baseline_sha256,
            measurement_contract_path: options.measurement_contract.as_deref(),
            measurement_contract_sha256: &measurement_contract_sha256,
            beta_waivers_path: beta_waivers_path.as_deref(),
            regression_waivers_path: regression_waivers_path.as_deref(),
            memory_baseline_path: memory_baseline_path.as_deref(),
            threshold_path: threshold_path.as_deref(),
            report_dir,
            options,
            stab: execution_identity.clone(),
            rows: report_rows,
        })?
    } else {
        ProfilerNoteFindings::default()
    };
    require_unchanged_execution_identity(root, &execution_identity)?;
    if options.require_profiler_notes && !profiler_note_findings.blockers.is_empty() {
        return Err(BenchError::ProfilerNotesMissing {
            details: profiler_note_findings.blockers.join("\n").into_boxed_str(),
        });
    }
    if options.require_beta_gate && !beta_gate_findings.blockers.is_empty() {
        return Err(BenchError::BetaGateFailed {
            details: beta_gate_findings.blockers.join("\n").into_boxed_str(),
        });
    }
    if options.require_memory_gate && !memory_gate_findings.blockers.is_empty() {
        return Err(BenchError::MemoryGateFailed {
            details: memory_gate_findings.blockers.join("\n").into_boxed_str(),
        });
    }
    if !regression_threshold_findings.blockers.is_empty()
        || !regression_waiver_findings.blockers.is_empty()
    {
        let details = regression_threshold_findings
            .blockers
            .iter()
            .chain(&regression_waiver_findings.blockers)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(BenchError::RegressionThresholdFailed {
            details: details.into_boxed_str(),
        });
    }
    if options.strict
        && (!pending.is_empty()
            || !missing_baselines.is_empty()
            || !invalid_baselines.is_empty()
            || !contract_only_without_measurements.is_empty())
    {
        Err(BenchError::CompareIncomplete {
            details: compare_incomplete_details(
                &pending,
                &missing_baselines,
                &invalid_baselines,
                &contract_only_without_measurements,
            )
            .into_boxed_str(),
        })
    } else {
        Ok(())
    }
}

fn require_unchanged_execution_identity(
    root: &RepoRoot,
    expected: &StabMetadata,
) -> Result<(), BenchError> {
    let actual = stab_metadata(root)?;
    if actual == *expected {
        Ok(())
    } else {
        Err(BenchError::SourceInput(format!(
            "benchmark source or executable changed during comparison: before={expected:?}, after={actual:?}"
        )))
    }
}

fn mark_instrumented_timing_not_evaluated(rows: &mut [CompareRowResult]) {
    for row in rows {
        if row.status != "measured" {
            continue;
        }
        if matches!(row.pass_fail_status.as_str(), "pass" | "fail") {
            row.pass_fail_status = "not-evaluated-instrumented".to_string();
        }
        row.beta_gate_status = "not-evaluated-instrumented".to_string();
        row.beta_gate_waiver_reason = None;
        row.beta_gate_waiver_follow_up = None;
        row.beta_gate_error = None;
    }
}

fn run_warmup_rows_with_root(
    root: &RepoRoot,
    rows: &[&BenchmarkRow],
    profile: &str,
) -> Result<(), BenchError> {
    println!(
        "[{PREFIX}] warming {} Stab compare row(s) before recording measurements",
        rows.len()
    );
    for row in rows {
        drop(run_stab_compare_row_with_root(root, profile, row)?);
    }
    Ok(())
}

fn run_recorded_stab_compare_row(
    root: &RepoRoot,
    row: &BenchmarkRow,
    profile: &str,
    measurement_runs: usize,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    if measurement_runs == 1 {
        return run_stab_compare_row_with_root(root, profile, row);
    }
    let mut runs = Vec::with_capacity(measurement_runs);
    for _ in 0..measurement_runs {
        let Some(measurements) = run_stab_compare_row_with_root(root, profile, row)? else {
            return Ok(None);
        };
        runs.push(measurements);
    }
    aggregate_measurement_runs(&row.id, runs).map(Some)
}

#[cfg(test)]
fn run_warmup_rows(rows: &[&BenchmarkRow]) -> Result<(), BenchError> {
    let root = RepoRoot::resolve(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("repository root"),
    )?;
    run_warmup_rows_with_root(&root, rows, "release")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BaselineSummary {
    Present(String),
    Missing,
    Invalid(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BaselineCompareStatus {
    Comparable,
    Missing,
    Invalid,
}

pub(crate) struct CompareRowBuild<'a> {
    pub(crate) row: &'a BenchmarkRow,
    pub(crate) status: &'a str,
    pub(crate) baseline_summary: &'a str,
    pub(crate) stab_summary: &'a str,
    pub(crate) note: Option<String>,
    pub(crate) stim_measurements: Vec<Measurement>,
    pub(crate) stab_measurements: Vec<Measurement>,
    pub(crate) baseline_status: BaselineCompareStatus,
}

pub(crate) fn summarize_baseline_row(
    report: &BaselineReport,
    row: &BenchmarkRow,
) -> BaselineSummary {
    let Some(baseline_row) = report
        .rows
        .iter()
        .find(|baseline_row| baseline_row.id == row.id)
    else {
        return BaselineSummary::Missing;
    };
    if baseline_row.milestone != row.milestone {
        return BaselineSummary::Invalid(format!(
            "milestone={} expected {}",
            baseline_row.milestone.as_str(),
            row.milestone.as_str()
        ));
    }
    if baseline_row.runner != row.runner {
        return BaselineSummary::Invalid(format!(
            "runner={} expected {}",
            baseline_row.runner.as_str(),
            row.runner.as_str()
        ));
    }
    if baseline_row.upstream_source != row.upstream_source {
        return BaselineSummary::Invalid(format!(
            "upstream_source={} expected {}",
            baseline_row.upstream_source, row.upstream_source
        ));
    }
    match row.runner {
        Runner::ContractOnly => {
            if baseline_row.measurements.is_empty() {
                BaselineSummary::Present(baseline_row.status.clone())
            } else {
                BaselineSummary::Present(summarize_measurements(&baseline_row.measurements))
            }
        }
        Runner::StimCli | Runner::StimPerf => {
            if baseline_row.status != "measured" {
                return BaselineSummary::Invalid(format!(
                    "status={} expected measured",
                    baseline_row.status
                ));
            }
            if baseline_row.measurements.is_empty() {
                return BaselineSummary::Invalid(
                    "measured runnable row has no measurements".to_string(),
                );
            }
            BaselineSummary::Present(summarize_measurements(&baseline_row.measurements))
        }
    }
}

fn baseline_measurements(report: &BaselineReport, row: &BenchmarkRow) -> Vec<Measurement> {
    report
        .rows
        .iter()
        .find(|baseline_row| baseline_row.id == row.id)
        .map_or_else(Vec::new, |baseline_row| baseline_row.measurements.clone())
}

pub(crate) fn build_compare_row_result(input: CompareRowBuild<'_>) -> CompareRowResult {
    let CompareRowBuild {
        row,
        status,
        baseline_summary,
        stab_summary,
        note,
        stim_measurements,
        stab_measurements,
        baseline_status,
    } = input;
    let comparability = if row.comparability
        == crate::comparability::ComparabilityClass::Unspecified
    {
        crate::comparability::ComparabilityClass::from_note_and_runner(note.as_deref(), row.runner)
    } else {
        row.comparability
    };
    let derived = crate::compare_evidence::derive_measurement_evidence(
        &stim_measurements,
        &stab_measurements,
        comparability,
    );
    let stab_allocation_count_max = max_stab_allocation_count(&stab_measurements);
    let stab_allocation_bytes_max = max_stab_allocation_bytes(&stab_measurements);
    let stab_resident_bytes_max = max_stab_resident_bytes(&stab_measurements);
    let stab_resident_delta_bytes_max = max_stab_resident_delta_bytes(&stab_measurements);
    CompareRowResult {
        id: row.id.clone(),
        milestone: row.milestone,
        threshold_class: row.threshold_class.as_str().to_string(),
        runner: row.runner,
        comparability,
        upstream_source: row.upstream_source.clone(),
        phase: row.phase.clone(),
        measurement: row.measurement.clone(),
        status: status.to_string(),
        baseline_summary: baseline_summary.to_string(),
        stab_summary: stab_summary.to_string(),
        note,
        stim_measurements,
        stab_measurements,
        stim_median_seconds: derived.stim_median_seconds,
        stab_median_seconds: derived.stab_median_seconds,
        relative_ratio: derived.relative_ratio,
        measurement_ratios: derived.measurement_ratios,
        stab_allocation_count_max,
        stab_allocation_bytes_max,
        stab_resident_bytes_max,
        stab_resident_delta_bytes_max,
        pass_fail_status: compare_pass_fail_status(status, baseline_status, derived.relative_ratio),
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

pub(crate) fn rebuild_compare_row_raw_evidence(row: &mut CompareRowResult) {
    let derived = crate::compare_evidence::derive_measurement_evidence(
        &row.stim_measurements,
        &row.stab_measurements,
        row.comparability,
    );
    row.stim_median_seconds = derived.stim_median_seconds;
    row.stab_median_seconds = derived.stab_median_seconds;
    row.relative_ratio = derived.relative_ratio;
    row.measurement_ratios = derived.measurement_ratios;
    row.stab_allocation_count_max = max_stab_allocation_count(&row.stab_measurements);
    row.stab_allocation_bytes_max = max_stab_allocation_bytes(&row.stab_measurements);
    row.stab_resident_bytes_max = max_stab_resident_bytes(&row.stab_measurements);
    row.stab_resident_delta_bytes_max = max_stab_resident_delta_bytes(&row.stab_measurements);
    row.pass_fail_status = compare_pass_fail_status(
        &row.status,
        BaselineCompareStatus::Comparable,
        row.relative_ratio,
    );
}

fn max_stab_allocation_count(measurements: &[Measurement]) -> Option<u64> {
    measurements
        .iter()
        .filter_map(|measurement| {
            measurement
                .allocation
                .as_ref()
                .map(|allocation| allocation.count_max)
        })
        .max()
}

fn max_stab_allocation_bytes(measurements: &[Measurement]) -> Option<u64> {
    measurements
        .iter()
        .filter_map(|measurement| {
            measurement
                .allocation
                .as_ref()
                .map(|allocation| allocation.bytes_max)
        })
        .max()
}

fn max_stab_resident_bytes(measurements: &[Measurement]) -> Option<u64> {
    measurements
        .iter()
        .filter_map(|measurement| measurement.resident_bytes)
        .max()
}

fn max_stab_resident_delta_bytes(measurements: &[Measurement]) -> Option<u64> {
    measurements
        .iter()
        .filter_map(|measurement| measurement.resident_delta_bytes)
        .max()
}

fn compare_pass_fail_status(
    status: &str,
    baseline_status: BaselineCompareStatus,
    relative_ratio: Option<f64>,
) -> String {
    match baseline_status {
        BaselineCompareStatus::Missing => return "missing-baseline".to_string(),
        BaselineCompareStatus::Invalid => return "invalid-baseline".to_string(),
        BaselineCompareStatus::Comparable => {}
    }
    if status == "pending" {
        return "pending".to_string();
    }
    if status == "contract-only" {
        return "not-comparable".to_string();
    }
    match relative_ratio {
        Some(ratio) if ratio <= BETA_GATE_MAX_RELATIVE_RATIO => "pass".to_string(),
        Some(_) => "fail".to_string(),
        None => "not-comparable".to_string(),
    }
}

pub(crate) fn compare_incomplete_details(
    pending: &[String],
    missing_baselines: &[String],
    invalid_baselines: &[String],
    contract_only_without_measurements: &[String],
) -> String {
    let mut details = Vec::new();
    if !pending.is_empty() {
        details.push(format!(
            "pending Stab comparison runner(s): {}",
            pending.join(", ")
        ));
    }
    if !missing_baselines.is_empty() {
        details.push(format!(
            "missing baseline row(s): {}",
            missing_baselines.join(", ")
        ));
    }
    if !invalid_baselines.is_empty() {
        details.push(format!(
            "invalid baseline row(s): {}",
            invalid_baselines.join(", ")
        ));
    }
    if !contract_only_without_measurements.is_empty() {
        details.push(format!(
            "contract-only row(s) without Stab measurement(s): {}",
            contract_only_without_measurements.join(", ")
        ));
    }
    details.join("\n")
}

fn strict_requires_contract_measurement(row: &BenchmarkRow) -> bool {
    row.runner == Runner::ContractOnly && row.threshold_class != ThresholdClass::BaselineMetadata
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::profiler_notes::{
        HOT_PATH_PROFILER_NOTE_RATIO, apply_profiler_notes, profiler_note_report_metadata,
        validate_profiler_note_content,
    };
    use super::{
        BaselineCompareStatus, CompareRowBuild, build_compare_row_result,
        mark_instrumented_timing_not_evaluated, run_warmup_rows,
        strict_requires_contract_measurement,
    };
    use crate::manifest::{BenchmarkRow, Milestone, Runner, ThresholdClass};
    use crate::report::{AllocationMeasurement, Measurement};

    #[test]
    fn profiler_notes_are_required_only_for_rows_slower_than_hot_path_ratio() {
        let notes = tempdir().expect("note dir");
        let fast_row = compare_row("fast-row", Some(HOT_PATH_PROFILER_NOTE_RATIO));
        let mut slow_row = compare_row("slow-row", Some(HOT_PATH_PROFILER_NOTE_RATIO + 0.1));
        let note_path = notes.path().join("slow-row.md");
        std::fs::write(
            &note_path,
            "Dominant cost: parsing allocations\nNext owner action: profile parser arena reuse\n",
        )
        .expect("write note");
        let mut rows = vec![fast_row, slow_row.clone()];

        let note_dirs = [(
            notes.path().to_path_buf(),
            PathBuf::from("benchmarks/profiler-notes/m12"),
        )];
        let findings = apply_profiler_notes(&mut rows, &note_dirs);

        assert!(findings.blockers.is_empty());
        let fast = rows.first().expect("fast row");
        let slow = rows.get(1).expect("slow row");
        assert_eq!(fast.profiler_note_status, "not-required");
        assert_eq!(slow.profiler_note_status, "present");
        assert_eq!(
            slow.profiler_note_path.as_deref(),
            Some("benchmarks/profiler-notes/m12/slow-row.md")
        );

        slow_row.relative_ratio = Some(HOT_PATH_PROFILER_NOTE_RATIO + 0.2);
        let missing_notes = tempdir().expect("missing note dir");
        let mut rows = vec![slow_row];
        let note_dirs = [(
            missing_notes.path().to_path_buf(),
            PathBuf::from("profiler-notes"),
        )];
        let findings = apply_profiler_notes(&mut rows, &note_dirs);

        let slow = rows.first().expect("slow row");
        assert_eq!(slow.profiler_note_status, "missing");
        assert_eq!(
            findings.blockers,
            vec!["slow-row: profiler note is missing"]
        );
    }

    #[test]
    fn profiler_notes_search_multiple_directories_without_ambiguity() {
        let first = tempdir().expect("first note dir");
        let second = tempdir().expect("second note dir");
        let note = "Dominant cost: graph frontier\nNext owner action: intern states\n";
        std::fs::write(second.path().join("slow-row.md"), note).expect("write second note");
        let note_dirs = [
            (
                first.path().to_path_buf(),
                PathBuf::from("benchmarks/profiler-notes/m12"),
            ),
            (
                second.path().to_path_buf(),
                PathBuf::from("benchmarks/profiler-notes/pfm-b5"),
            ),
        ];
        let mut rows = vec![compare_row(
            "slow-row",
            Some(HOT_PATH_PROFILER_NOTE_RATIO + 0.1),
        )];

        let findings = apply_profiler_notes(&mut rows, &note_dirs);

        assert!(findings.blockers.is_empty());
        let row = rows.first().expect("slow row");
        assert_eq!(
            row.profiler_note_path.as_deref(),
            Some("benchmarks/profiler-notes/pfm-b5/slow-row.md")
        );

        std::fs::write(first.path().join("slow-row.md"), note).expect("write duplicate note");
        rows.first_mut().expect("slow row").relative_ratio = Some(1.0);
        let findings = apply_profiler_notes(&mut rows, &note_dirs);
        let row = rows.first().expect("slow row");
        assert_eq!(row.profiler_note_status, "invalid");
        assert_eq!(findings.blockers.len(), 1);
        let blocker = findings.blockers.first().expect("duplicate blocker");
        assert!(blocker.contains(&first.path().display().to_string()));
        assert!(blocker.contains(&second.path().display().to_string()));
    }

    #[test]
    fn profiler_note_report_metadata_keeps_a_legacy_first_root() {
        let paths = [
            PathBuf::from("benchmarks/profiler-notes/m12"),
            PathBuf::from("benchmarks/profiler-notes/pfm-b5"),
        ];

        let (legacy, structured) = profiler_note_report_metadata(&paths);

        assert_eq!(legacy.as_deref(), Some("benchmarks/profiler-notes/m12"));
        assert_eq!(
            structured,
            [
                "benchmarks/profiler-notes/m12",
                "benchmarks/profiler-notes/pfm-b5"
            ]
        );
    }

    #[test]
    fn strict_compare_allows_metadata_only_rows_without_runtime_measurements() {
        let mut row = benchmark_row("metadata");
        row.runner = Runner::ContractOnly;
        row.threshold_class = ThresholdClass::BaselineMetadata;
        assert!(!strict_requires_contract_measurement(&row));

        row.threshold_class = ThresholdClass::ReportOnly;
        assert!(strict_requires_contract_measurement(&row));
    }

    #[test]
    fn profiler_notes_must_name_dominant_cost_and_next_owner_action() {
        assert!(
            validate_profiler_note_content(
                "Dominant cost: bit transpose\nNext owner action: tune portable SIMD lanes\n"
            )
            .is_ok()
        );
        assert!(validate_profiler_note_content("Dominant cost: bit transpose\n").is_err());
        assert!(
            validate_profiler_note_content("Dominant cost:\nNext owner action: rerun profiler\n")
                .is_err()
        );
    }

    #[test]
    fn compare_row_result_records_stab_memory_maxima() {
        let row = BenchmarkRow {
            id: "allocation-row".to_string(),
            milestone: Milestone::M12,
            threshold_class: crate::manifest::ThresholdClass::PerformanceGate,
            runner: Runner::StimPerf,
            upstream_source: "future/performance-primary-matrix".to_string(),
            stim_perf_filter: "test".to_string(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "performance-hardening".to_string(),
            measurement: "primary-matrix".to_string(),
            description: "test row".to_string(),
            comparability: crate::comparability::ComparabilityClass::Unspecified,
        };

        let result = build_compare_row_result(CompareRowBuild {
            row: &row,
            status: "measured",
            baseline_summary: "stim",
            stab_summary: "stab",
            note: None,
            stim_measurements: Vec::new(),
            stab_measurements: vec![Measurement {
                name: "stab".to_string(),
                seconds: 1.0,
                variance_seconds: Some(0.0),
                allocation: Some(AllocationMeasurement {
                    count_total: 7,
                    count_current: 0,
                    count_max: 3,
                    bytes_total: 4096,
                    bytes_current: 0,
                    bytes_max: 2048,
                }),
                resident_bytes: Some(8192),
                resident_delta_bytes: None,
                observations: Vec::new(),
                iterations: Some(1),
            }],
            baseline_status: BaselineCompareStatus::Comparable,
        });

        assert_eq!(result.stab_allocation_count_max, Some(3));
        assert_eq!(result.stab_allocation_bytes_max, Some(2048));
        assert_eq!(result.stab_resident_bytes_max, Some(8192));
    }

    #[test]
    fn compare_row_result_uses_worst_exact_submeasurement_ratio() {
        let row = benchmark_row("paired-row");

        let result = build_compare_row_result(CompareRowBuild {
            row: &row,
            status: "measured",
            baseline_summary: "stim",
            stab_summary: "stab",
            note: None,
            stim_measurements: vec![measurement("foo_bar", 1.0), measurement("tiny_case", 0.1)],
            stab_measurements: vec![
                measurement("stab_foo_bar", 1.2),
                measurement("stab_tiny_case", 0.4),
            ],
            baseline_status: BaselineCompareStatus::Comparable,
        });

        assert_eq!(result.stim_median_seconds, Some(1.0));
        assert_eq!(result.stab_median_seconds, Some(1.2));
        assert_eq!(result.relative_ratio, Some(4.0));
        assert_eq!(result.pass_fail_status, "fail");
        assert_eq!(result.measurement_ratios.len(), 2);
    }

    #[test]
    fn direct_match_rows_use_positional_submeasurement_ratios_when_names_differ() {
        let row = benchmark_row("m5-sparse-xor");

        let result = build_compare_row_result(CompareRowBuild {
            row: &row,
            status: "measured",
            baseline_summary: "stim",
            stab_summary: "stab",
            note: Some("direct-match: same sparse xor filters".to_string()),
            stim_measurements: vec![
                measurement("SparseXorTable_SmallRowXor_1000", 0.000015),
                measurement("SparseXorVec_XorItem", 0.000000015),
            ],
            stab_measurements: vec![
                measurement("stab_sparse_table_row_xor_1000", 0.000019),
                measurement("stab_sparse_xor_item_7", 0.000000080),
            ],
            baseline_status: BaselineCompareStatus::Comparable,
        });

        assert_eq!(result.stim_median_seconds, Some(0.000015));
        assert_eq!(result.stab_median_seconds, Some(0.000019));
        assert!(result.relative_ratio.is_some_and(|ratio| ratio > 5.0));
        assert_eq!(result.pass_fail_status, "fail");
        assert_eq!(
            result
                .measurement_ratios
                .iter()
                .map(|ratio| ratio.stab_name.as_str())
                .collect::<Vec<_>>(),
            vec!["stab_sparse_table_row_xor_1000", "stab_sparse_xor_item_7"]
        );
    }

    #[test]
    fn compare_row_result_keeps_median_ratio_when_it_exceeds_paired_ratio() {
        let row = benchmark_row("paired-row");

        let result = build_compare_row_result(CompareRowBuild {
            row: &row,
            status: "measured",
            baseline_summary: "stim",
            stab_summary: "stab",
            note: None,
            stim_measurements: vec![
                measurement("fast_case", 1.0),
                measurement("slow_case", 10.0),
            ],
            stab_measurements: vec![
                measurement("stab_fast_case", 1.1),
                measurement("stab_slow_case", 12.0),
            ],
            baseline_status: BaselineCompareStatus::Comparable,
        });

        assert_eq!(result.relative_ratio, Some(1.2));
        assert_eq!(result.pass_fail_status, "pass");
    }

    #[test]
    fn partial_match_row_result_uses_paired_ratio_without_mixed_median() {
        let row = benchmark_row("m5-simd-bits");

        let result = build_compare_row_result(CompareRowBuild {
            row: &row,
            status: "measured",
            baseline_summary: "stim",
            stab_summary: "stab",
            note: Some("partial-match: one direct pair plus unmatched contract extras".to_string()),
            stim_measurements: vec![
                measurement("simd_bits_randomize_10K", 0.1),
                measurement("simd_bits_xor_10K", 1.0),
            ],
            stab_measurements: vec![
                measurement("stab_simd_bits_xor_10K", 0.5),
                measurement("stab_bitvec_range_xor_4096_contract", 10.0),
            ],
            baseline_status: BaselineCompareStatus::Comparable,
        });

        assert_eq!(result.stim_median_seconds, Some(1.0));
        assert_eq!(result.stab_median_seconds, Some(10.0));
        assert_eq!(result.relative_ratio, Some(0.5));
        assert_eq!(result.pass_fail_status, "pass");
        assert_eq!(result.stab_measurements.len(), 2);
    }

    #[test]
    fn error_decomp_partial_match_excludes_failed_iterate_witnesses() {
        let row = benchmark_row("m10-error-decomp");

        let result = build_compare_row_result(CompareRowBuild {
            row: &row,
            status: "measured",
            baseline_summary: "stim",
            stab_summary: "stab",
            note: Some(
                "partial-match: successful public conversions pair; failed attempts are report-only"
                    .to_string(),
            ),
            stim_measurements: vec![
                measurement("disjoint_to_independent_xyz_errors_approx_exact", 1.0),
                measurement("disjoint_to_independent_xyz_errors_approx_p10", 1.0),
                measurement("disjoint_to_independent_xyz_errors_approx_p100", 1.0),
                measurement("independent_to_disjoint_xyz_errors", 1.0),
            ],
            stab_measurements: vec![
                measurement("stab_independent_to_disjoint_xyz_errors", 0.5),
                measurement("stab_disjoint_to_independent_xyz_errors_approx_exact", 0.75),
                measurement(
                    "stab_disjoint_to_independent_xyz_errors_public_none_p10",
                    0.25,
                ),
                measurement(
                    "stab_disjoint_to_independent_xyz_errors_public_none_p100",
                    0.25,
                ),
            ],
            baseline_status: BaselineCompareStatus::Comparable,
        });

        assert_eq!(result.relative_ratio, Some(0.75));
        assert_eq!(result.measurement_ratios.len(), 2);
        assert_eq!(
            result
                .measurement_ratios
                .iter()
                .map(|ratio| ratio.stab_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "stab_disjoint_to_independent_xyz_errors_approx_exact",
                "stab_independent_to_disjoint_xyz_errors",
            ]
        );
    }

    #[test]
    fn allocation_instrumentation_preserves_ratios_without_claiming_timing_evidence() {
        let mut rows = vec![
            compare_row("fast-row", Some(0.5)),
            compare_row("slow-row", Some(2.0)),
        ];

        mark_instrumented_timing_not_evaluated(&mut rows);

        assert_eq!(rows.first().map(|row| row.relative_ratio), Some(Some(0.5)));
        assert_eq!(rows.get(1).map(|row| row.relative_ratio), Some(Some(2.0)));
        for row in rows {
            assert_eq!(row.pass_fail_status, "not-evaluated-instrumented");
            assert_eq!(row.beta_gate_status, "not-evaluated-instrumented");
            assert!(row.beta_gate_error.is_none());
        }
    }

    #[test]
    fn warmup_rows_run_selected_stab_compare_workloads() {
        let mut row = benchmark_row("m4-circuit-parse");
        row.milestone = Milestone::M4;

        run_warmup_rows(&[&row]).expect("warm up M4 parser row");
    }

    fn compare_row(id: &str, ratio: Option<f64>) -> crate::report::CompareRowResult {
        let row = benchmark_row(id);
        let stim_measurements = ratio.map_or_else(Vec::new, |_| {
            vec![Measurement {
                name: "stim".to_string(),
                seconds: 1.0,
                variance_seconds: None,
                allocation: None,
                resident_bytes: None,
                resident_delta_bytes: None,
                observations: Vec::new(),
                iterations: None,
            }]
        });
        let stab_measurements = ratio.map_or_else(Vec::new, |ratio| {
            vec![Measurement {
                name: "stab".to_string(),
                seconds: ratio,
                variance_seconds: Some(0.0),
                allocation: None,
                resident_bytes: None,
                resident_delta_bytes: None,
                observations: Vec::new(),
                iterations: Some(1),
            }]
        });
        build_compare_row_result(CompareRowBuild {
            row: &row,
            status: "measured",
            baseline_summary: "stim",
            stab_summary: "stab",
            note: None,
            stim_measurements,
            stab_measurements,
            baseline_status: BaselineCompareStatus::Comparable,
        })
    }

    fn benchmark_row(id: &str) -> BenchmarkRow {
        BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::M12,
            threshold_class: crate::manifest::ThresholdClass::PerformanceGate,
            runner: Runner::StimPerf,
            upstream_source: "future/performance-primary-matrix".to_string(),
            stim_perf_filter: "test".to_string(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "performance-hardening".to_string(),
            measurement: "primary-matrix".to_string(),
            description: "test row".to_string(),
            comparability: crate::comparability::ComparabilityClass::Unspecified,
        }
    }

    fn measurement(name: &str, seconds: f64) -> Measurement {
        measurement_with_iterations(name, seconds, None)
    }

    fn measurement_with_iterations(
        name: &str,
        seconds: f64,
        iterations: Option<usize>,
    ) -> Measurement {
        Measurement {
            name: name.to_string(),
            seconds,
            variance_seconds: None,
            allocation: None,
            resident_bytes: None,
            resident_delta_bytes: None,
            observations: Vec::new(),
            iterations,
        }
    }
}
