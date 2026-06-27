use std::path::{Path, PathBuf};

use crate::baseline::{
    compare_note, read_baseline_report, run_stab_compare_row, summarize_measurements,
    summarize_stab_measurements, validate_baseline_metadata,
};
use crate::config::PREFIX;
use crate::error::BenchError;
use crate::manifest::{BenchmarkManifest, BenchmarkRow, Runner};
use crate::report::{
    BaselineReport, CompareCommandMetadata, CompareReport, CompareRowResult, Measurement,
    machine_metadata, render_compare_markdown_report, stab_metadata, unix_epoch_seconds,
};
use crate::root::RepoRoot;

#[derive(Clone, Debug)]
pub(crate) struct CompareOptions {
    pub(crate) baseline: PathBuf,
    pub(crate) milestone: Option<String>,
    pub(crate) profile: String,
    pub(crate) primary: bool,
    pub(crate) report: Option<PathBuf>,
    pub(crate) require_profiler_notes: bool,
    pub(crate) strict: bool,
}

pub(crate) fn run_compare(
    root: &RepoRoot,
    manifest: &BenchmarkManifest,
    options: &CompareOptions,
) -> Result<(), BenchError> {
    if options.require_profiler_notes && options.report.is_none() {
        return Err(BenchError::ProfilerNotesRequireReport);
    }
    let baseline_path = root.resolve_relative(&options.baseline);
    let baseline_report = read_baseline_report(&baseline_path)?;
    validate_baseline_metadata(&baseline_report)?;
    let rows = manifest.compare_rows(options.milestone.as_deref(), options.primary)?;
    println!(
        "[{PREFIX}] comparing {} row(s) against {}",
        rows.len(),
        baseline_path.display()
    );
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
        match run_stab_compare_row(row)? {
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
                    contract_only_without_measurements.push(row.id.clone());
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
    let profiler_note_findings = if let Some(report_dir) = &options.report {
        write_compare_report(
            root,
            &baseline_report,
            &baseline_path,
            report_dir,
            options,
            report_rows,
        )?
    } else {
        ProfilerNoteFindings::default()
    };
    if options.require_profiler_notes && !profiler_note_findings.blockers.is_empty() {
        return Err(BenchError::ProfilerNotesMissing {
            details: profiler_note_findings.blockers.join("\n").into_boxed_str(),
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

fn write_compare_report(
    root: &RepoRoot,
    baseline_report: &BaselineReport,
    baseline_path: &Path,
    report_dir: &Path,
    options: &CompareOptions,
    mut rows: Vec<CompareRowResult>,
) -> Result<ProfilerNoteFindings, BenchError> {
    let out_dir = root.create_benchmark_output_dir(report_dir)?;
    let profiler_note_findings = apply_profiler_notes(&mut rows, &out_dir.join("profiler-notes"));
    let report = CompareReport {
        schema_version: 1,
        generated_unix_epoch_seconds: unix_epoch_seconds(),
        machine: machine_metadata(root)?,
        stim: baseline_report.stim.clone(),
        stab: stab_metadata(root)?,
        command: CompareCommandMetadata {
            baseline_path: baseline_path.display().to_string(),
            profile: options.profile.clone(),
            milestone: options.milestone.clone(),
            primary: options.primary,
            require_profiler_notes: options.require_profiler_notes,
            strict: options.strict,
        },
        rows,
    };
    let json_path = out_dir.join("compare.json");
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&json_path, json).map_err(|source| BenchError::WriteOutput {
        path: json_path.clone(),
        source,
    })?;
    let report_path = out_dir.join("report.md");
    std::fs::write(&report_path, render_compare_markdown_report(&report)).map_err(|source| {
        BenchError::WriteOutput {
            path: report_path.clone(),
            source,
        }
    })?;
    println!("[{PREFIX}] wrote {}", json_path.display());
    println!("[{PREFIX}] wrote {}", report_path.display());
    Ok(profiler_note_findings)
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ProfilerNoteFindings {
    blockers: Vec<String>,
}

const HOT_PATH_PROFILER_NOTE_RATIO: f64 = 1.5;

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
    let stim_median_seconds = median_seconds(&stim_measurements);
    let stab_median_seconds = median_seconds(&stab_measurements);
    let relative_ratio = match (stim_median_seconds, stab_median_seconds) {
        (Some(stim_seconds), Some(stab_seconds)) if stim_seconds > 0.0 => {
            Some(stab_seconds / stim_seconds)
        }
        _ => None,
    };
    CompareRowResult {
        id: row.id.clone(),
        milestone: row.milestone,
        threshold_class: row.threshold_class.clone(),
        runner: row.runner,
        upstream_source: row.upstream_source.clone(),
        phase: row.phase.clone(),
        measurement: row.measurement.clone(),
        status: status.to_string(),
        baseline_summary: baseline_summary.to_string(),
        stab_summary: stab_summary.to_string(),
        note,
        stim_measurements,
        stab_measurements,
        stim_median_seconds,
        stab_median_seconds,
        relative_ratio,
        pass_fail_status: compare_pass_fail_status(status, baseline_status, relative_ratio),
        profiler_note_status: "not-required".to_string(),
        profiler_note_path: None,
        profiler_note_error: None,
    }
}

fn median_seconds(measurements: &[Measurement]) -> Option<f64> {
    if measurements.is_empty() {
        return None;
    }
    let mut seconds = measurements
        .iter()
        .map(|measurement| measurement.seconds)
        .collect::<Vec<_>>();
    seconds.sort_by(f64::total_cmp);
    seconds.get(seconds.len() / 2).copied()
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
        Some(ratio) if ratio <= 2.0 => "pass".to_string(),
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

fn apply_profiler_notes(rows: &mut [CompareRowResult], notes_dir: &Path) -> ProfilerNoteFindings {
    let mut findings = ProfilerNoteFindings::default();
    for row in rows {
        if !row
            .relative_ratio
            .is_some_and(|ratio| ratio > HOT_PATH_PROFILER_NOTE_RATIO)
        {
            row.profiler_note_status = "not-required".to_string();
            continue;
        }
        let relative_path = format!("profiler-notes/{}.md", row.id);
        let note_path = notes_dir.join(format!("{}.md", row.id));
        row.profiler_note_path = Some(relative_path);
        match read_and_validate_profiler_note(&note_path) {
            Ok(()) => {
                row.profiler_note_status = "present".to_string();
            }
            Err(error) => {
                row.profiler_note_status = error.status().to_string();
                row.profiler_note_error = Some(error.message().to_string());
                findings
                    .blockers
                    .push(format!("{}: {}", row.id, error.message()));
            }
        }
    }
    findings
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProfilerNoteError {
    Missing,
    Invalid(String),
}

impl ProfilerNoteError {
    fn status(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Invalid(_) => "invalid",
        }
    }

    fn message(&self) -> &str {
        match self {
            Self::Missing => "profiler note is missing",
            Self::Invalid(message) => message,
        }
    }
}

fn read_and_validate_profiler_note(path: &Path) -> Result<(), ProfilerNoteError> {
    let content = std::fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ProfilerNoteError::Missing
        } else {
            ProfilerNoteError::Invalid(format!("failed to read profiler note: {error}"))
        }
    })?;
    validate_profiler_note_content(&content)
}

fn validate_profiler_note_content(content: &str) -> Result<(), ProfilerNoteError> {
    if content.trim().is_empty() {
        return Err(ProfilerNoteError::Invalid(
            "profiler note is empty".to_string(),
        ));
    }
    if !has_named_nonempty_field(content, "Dominant cost:") {
        return Err(ProfilerNoteError::Invalid(
            "profiler note must include `Dominant cost:`".to_string(),
        ));
    }
    if !has_named_nonempty_field(content, "Next owner action:") {
        return Err(ProfilerNoteError::Invalid(
            "profiler note must include `Next owner action:`".to_string(),
        ));
    }
    Ok(())
}

fn has_named_nonempty_field(content: &str, field: &str) -> bool {
    content.lines().any(|line| {
        line.trim_start()
            .strip_prefix(field)
            .is_some_and(|value| !value.trim().is_empty())
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        BaselineCompareStatus, CompareRowBuild, HOT_PATH_PROFILER_NOTE_RATIO, apply_profiler_notes,
        build_compare_row_result, validate_profiler_note_content,
    };
    use crate::manifest::{BenchmarkRow, Milestone, Runner};
    use crate::report::Measurement;

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

        let findings = apply_profiler_notes(&mut rows, notes.path());

        assert!(findings.blockers.is_empty());
        let fast = rows.first().expect("fast row");
        let slow = rows.get(1).expect("slow row");
        assert_eq!(fast.profiler_note_status, "not-required");
        assert_eq!(slow.profiler_note_status, "present");
        assert_eq!(
            slow.profiler_note_path.as_deref(),
            Some("profiler-notes/slow-row.md")
        );

        slow_row.relative_ratio = Some(HOT_PATH_PROFILER_NOTE_RATIO + 0.2);
        let missing_notes = tempdir().expect("missing note dir");
        let mut rows = vec![slow_row];
        let findings = apply_profiler_notes(&mut rows, missing_notes.path());

        let slow = rows.first().expect("slow row");
        assert_eq!(slow.profiler_note_status, "missing");
        assert_eq!(
            findings.blockers,
            vec!["slow-row: profiler note is missing"]
        );
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

    fn compare_row(id: &str, ratio: Option<f64>) -> crate::report::CompareRowResult {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::M12,
            threshold_class: "performance-gate".to_string(),
            runner: Runner::StimPerf,
            upstream_source: "future/performance-primary-matrix".to_string(),
            stim_perf_filter: "test".to_string(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "performance-hardening".to_string(),
            measurement: "primary-matrix".to_string(),
            description: "test row".to_string(),
        };
        let stim_measurements = ratio.map_or_else(Vec::new, |_| {
            vec![Measurement {
                name: "stim".to_string(),
                seconds: 1.0,
                variance_seconds: None,
                iterations: None,
            }]
        });
        let stab_measurements = ratio.map_or_else(Vec::new, |ratio| {
            vec![Measurement {
                name: "stab".to_string(),
                seconds: ratio,
                variance_seconds: Some(0.0),
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
}
