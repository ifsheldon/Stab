use std::path::{Path, PathBuf};

use super::CompareOptions;
use super::profiler_notes::{
    ProfilerNoteFindings, apply_profiler_notes, profiler_note_report_metadata,
};
use crate::config::PREFIX;
use crate::error::BenchError;
use crate::report::{
    BaselineReport, COMPARE_REPORT_SCHEMA_VERSION, COMPARE_TIMING_BOUNDARY, CompareCommandMetadata,
    CompareReport, CompareRowResult, active_benchmark_features, machine_metadata,
    render_compare_markdown_report, stab_metadata, unix_epoch_seconds,
};
use crate::root::RepoRoot;

pub(super) struct CompareReportWrite<'a> {
    pub(super) root: &'a RepoRoot,
    pub(super) baseline_report: &'a BaselineReport,
    pub(super) baseline_path: &'a Path,
    pub(super) baseline_sha256: &'a str,
    pub(super) measurement_contract_path: Option<&'a Path>,
    pub(super) measurement_contract_sha256: &'a str,
    pub(super) beta_waivers_path: Option<&'a Path>,
    pub(super) regression_waivers_path: Option<&'a Path>,
    pub(super) memory_baseline_path: Option<&'a Path>,
    pub(super) threshold_path: Option<&'a Path>,
    pub(super) report_dir: &'a Path,
    pub(super) options: &'a CompareOptions,
    pub(super) rows: Vec<CompareRowResult>,
}

pub(super) fn write_compare_report(
    input: CompareReportWrite<'_>,
) -> Result<ProfilerNoteFindings, BenchError> {
    let CompareReportWrite {
        root,
        baseline_report,
        baseline_path,
        baseline_sha256,
        measurement_contract_path,
        measurement_contract_sha256,
        beta_waivers_path,
        regression_waivers_path,
        memory_baseline_path,
        threshold_path,
        report_dir,
        options,
        mut rows,
    } = input;
    let out_dir = if options.new_output {
        root.create_new_benchmark_output_dir(report_dir)?
    } else {
        root.create_benchmark_output_dir(report_dir)?
    };
    let profiler_note_dirs = if options.profiler_notes_dirs.is_empty() {
        vec![(
            out_dir.join("profiler-notes"),
            PathBuf::from("profiler-notes"),
        )]
    } else {
        options
            .profiler_notes_dirs
            .iter()
            .map(|path| (root.resolve_relative(path), path.clone()))
            .collect()
    };
    let profiler_note_findings = apply_profiler_notes(&mut rows, &profiler_note_dirs);
    let (profiler_notes_path, profiler_notes_paths) =
        profiler_note_report_metadata(&options.profiler_notes_dirs);
    let report = CompareReport {
        schema_version: COMPARE_REPORT_SCHEMA_VERSION,
        generated_unix_epoch_seconds: unix_epoch_seconds(),
        machine: machine_metadata(root)?,
        stim: baseline_report.stim.clone(),
        stab: stab_metadata(root)?,
        command: CompareCommandMetadata {
            baseline_path: baseline_path.display().to_string(),
            baseline_sha256: baseline_sha256.to_string(),
            profile: options.profile.clone(),
            milestone: options.milestone.clone(),
            primary: options.primary,
            filters: options.only.clone(),
            cargo_features: active_benchmark_features(),
            timing_boundary: COMPARE_TIMING_BOUNDARY.to_string(),
            measurement_contract_path: measurement_contract_path
                .map(|path| path.display().to_string()),
            measurement_contract_sha256: measurement_contract_sha256.to_string(),
            require_profiler_notes: options.require_profiler_notes,
            require_beta_gate: options.require_beta_gate,
            beta_waivers_path: beta_waivers_path.map(|path| path.display().to_string()),
            regression_waivers_path: regression_waivers_path.map(|path| path.display().to_string()),
            require_memory_gate: options.require_memory_gate,
            memory_baseline_path: memory_baseline_path.map(|path| path.display().to_string()),
            thresholds_path: threshold_path.map(|path| path.display().to_string()),
            profiler_notes_path,
            profiler_notes_paths,
            track_allocations: options.track_allocations,
            warmup: options.warmup,
            measurement_runs: options.measurement_runs,
            strict: options.strict,
            new_output: options.new_output,
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
