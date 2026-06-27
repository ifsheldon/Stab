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
    pub(crate) strict: bool,
}

pub(crate) fn run_compare(
    root: &RepoRoot,
    manifest: &BenchmarkManifest,
    options: &CompareOptions,
) -> Result<(), BenchError> {
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
    if let Some(report_dir) = &options.report {
        write_compare_report(
            root,
            &baseline_report,
            &baseline_path,
            report_dir,
            options,
            report_rows,
        )?;
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
    rows: Vec<CompareRowResult>,
) -> Result<(), BenchError> {
    let out_dir = root.create_benchmark_output_dir(report_dir)?;
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
    Ok(())
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
