use std::ffi::{OsStr, OsString};
use std::num::NonZeroUsize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::comparability::ComparabilityClass;
use crate::error::BenchError;
use crate::manifest::{Milestone, Runner};
use crate::process::{check_success, run_process};
use crate::root::RepoRoot;

pub(crate) const BETA_GATE_MAX_RELATIVE_RATIO: f64 = 1.25;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct BaselineReport {
    pub(crate) schema_version: u32,
    pub(crate) generated_unix_epoch_seconds: u64,
    pub(crate) machine: MachineMetadata,
    pub(crate) stim: StimMetadata,
    pub(crate) command: BaselineCommandMetadata,
    pub(crate) rows: Vec<BaselineRowResult>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct MachineMetadata {
    os: String,
    arch: String,
    family: String,
    available_parallelism: usize,
    rustc_version: String,
    cmake_version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct StimMetadata {
    pub(crate) source_path: String,
    pub(crate) expected_tag: String,
    pub(crate) expected_commit: String,
    pub(crate) actual_tag: String,
    pub(crate) actual_commit: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct StabMetadata {
    pub(crate) commit: String,
    pub(crate) local_modifications: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct BaselineCommandMetadata {
    pub(crate) target_seconds: f64,
    pub(crate) cli_iterations: u32,
    pub(crate) filters: Vec<String>,
    #[serde(default)]
    pub(crate) primary: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct BaselineRowResult {
    pub(crate) id: String,
    pub(crate) milestone: Milestone,
    pub(crate) threshold_class: String,
    pub(crate) runner: Runner,
    pub(crate) upstream_source: String,
    pub(crate) phase: String,
    pub(crate) measurement: String,
    pub(crate) status: String,
    pub(crate) command: RowCommandMetadata,
    pub(crate) measurements: Vec<Measurement>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RowCommandMetadata {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) stdin_path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct Measurement {
    pub(crate) name: String,
    pub(crate) seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) variance_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) allocation: Option<AllocationMeasurement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) resident_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) resident_delta_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) observations: Vec<MeasurementObservation>,
    pub(crate) iterations: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct MeasurementObservation {
    pub(crate) name: String,
    pub(crate) value: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct MeasurementRatio {
    pub(crate) stim_name: String,
    pub(crate) stab_name: String,
    pub(crate) stim_seconds: f64,
    pub(crate) stab_seconds: f64,
    pub(crate) relative_ratio: f64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct AllocationMeasurement {
    pub(crate) count_total: u64,
    pub(crate) count_current: i64,
    pub(crate) count_max: u64,
    pub(crate) bytes_total: u64,
    pub(crate) bytes_current: i64,
    pub(crate) bytes_max: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct CompareReport {
    pub(crate) schema_version: u32,
    pub(crate) generated_unix_epoch_seconds: u64,
    pub(crate) machine: MachineMetadata,
    pub(crate) stim: StimMetadata,
    pub(crate) stab: StabMetadata,
    pub(crate) command: CompareCommandMetadata,
    pub(crate) rows: Vec<CompareRowResult>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct CompareCommandMetadata {
    pub(crate) baseline_path: String,
    pub(crate) profile: String,
    pub(crate) milestone: Option<String>,
    pub(crate) primary: bool,
    #[serde(default)]
    pub(crate) filters: Vec<String>,
    pub(crate) require_profiler_notes: bool,
    pub(crate) require_beta_gate: bool,
    #[serde(default)]
    pub(crate) beta_waivers_path: Option<String>,
    #[serde(default)]
    pub(crate) regression_waivers_path: Option<String>,
    pub(crate) require_memory_gate: bool,
    pub(crate) memory_baseline_path: Option<String>,
    pub(crate) thresholds_path: Option<String>,
    #[serde(default)]
    pub(crate) profiler_notes_path: Option<String>,
    pub(crate) track_allocations: bool,
    #[serde(default)]
    pub(crate) warmup: bool,
    #[serde(default = "default_measurement_runs")]
    pub(crate) measurement_runs: usize,
    pub(crate) strict: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct CompareRowResult {
    pub(crate) id: String,
    pub(crate) milestone: Milestone,
    pub(crate) threshold_class: String,
    pub(crate) runner: Runner,
    #[serde(default)]
    pub(crate) comparability: ComparabilityClass,
    pub(crate) upstream_source: String,
    pub(crate) phase: String,
    pub(crate) measurement: String,
    pub(crate) status: String,
    pub(crate) baseline_summary: String,
    pub(crate) stab_summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) note: Option<String>,
    pub(crate) stim_measurements: Vec<Measurement>,
    pub(crate) stab_measurements: Vec<Measurement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stim_median_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stab_median_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) relative_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) measurement_ratios: Vec<MeasurementRatio>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stab_allocation_count_max: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stab_allocation_bytes_max: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stab_resident_bytes_max: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stab_resident_delta_bytes_max: Option<u64>,
    pub(crate) pass_fail_status: String,
    #[serde(default)]
    pub(crate) beta_gate_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) beta_gate_waiver_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) beta_gate_waiver_follow_up: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) beta_gate_error: Option<String>,
    pub(crate) memory_gate_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memory_gate_baseline_bytes_max: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memory_gate_allowed_bytes_max: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memory_gate_baseline_resident_bytes_max: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memory_gate_allowed_resident_bytes_max: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memory_gate_baseline_resident_delta_bytes_max: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memory_gate_allowed_resident_delta_bytes_max: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memory_gate_error: Option<String>,
    pub(crate) regression_threshold_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) regression_threshold_max_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) regression_threshold_waiver_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) regression_threshold_waiver_follow_up: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) regression_threshold_error: Option<String>,
    pub(crate) profiler_note_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) profiler_note_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) profiler_note_error: Option<String>,
}

impl CompareRowResult {
    pub(crate) fn refresh_measured_ratio_status_from_measurement_ratios(&mut self) {
        let worst_measurement_ratio = self
            .measurement_ratios
            .iter()
            .map(|ratio| ratio.relative_ratio)
            .max_by(f64::total_cmp);
        if let Some(worst_measurement_ratio) = worst_measurement_ratio {
            self.relative_ratio = Some(
                if self.comparability.uses_paired_ratios_without_mixed_median() {
                    worst_measurement_ratio
                } else {
                    self.relative_ratio
                        .map_or(worst_measurement_ratio, |ratio| {
                            ratio.max(worst_measurement_ratio)
                        })
                },
            );
        }
        if self.status == "measured"
            && matches!(
                self.pass_fail_status.as_str(),
                "pass" | "fail" | "not-comparable"
            )
        {
            self.pass_fail_status = match self.relative_ratio {
                Some(ratio) if ratio <= BETA_GATE_MAX_RELATIVE_RATIO => "pass".to_string(),
                Some(_) => "fail".to_string(),
                None => "not-comparable".to_string(),
            };
        }
    }
}

pub(crate) fn machine_metadata(root: &RepoRoot) -> Result<MachineMetadata, BenchError> {
    Ok(MachineMetadata {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        family: std::env::consts::FAMILY.to_string(),
        available_parallelism: std::thread::available_parallelism().map_or(1, NonZeroUsize::get),
        rustc_version: command_first_line("rustc", ["--version"], &root.path)?,
        cmake_version: command_first_line("cmake", ["--version"], &root.path)?,
    })
}

pub(crate) fn stab_metadata(root: &RepoRoot) -> Result<StabMetadata, BenchError> {
    let status_args = [OsString::from("status"), OsString::from("--short")];
    let status = run_process(Path::new("git"), &status_args, b"", &root.path, true)?;
    check_success(Path::new("git"), &status)?;
    Ok(StabMetadata {
        commit: command_first_line("git", ["rev-parse", "HEAD"], &root.path)?,
        local_modifications: !status.stdout.is_empty(),
    })
}

pub(crate) fn unix_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

pub(crate) fn render_markdown_report(report: &BaselineReport) -> String {
    let mut out = String::new();
    out.push_str("# Stab Benchmark Baseline\n\n");
    out.push_str(&format!(
        "- Generated Unix epoch seconds: {}\n",
        report.generated_unix_epoch_seconds
    ));
    out.push_str(&format!(
        "- Stim: {} ({})\n",
        report.stim.actual_tag, report.stim.actual_commit
    ));
    out.push_str(&format!(
        "- Target seconds: {:.6}\n",
        report.command.target_seconds
    ));
    out.push_str(&format!(
        "- CLI iterations: {}\n",
        report.command.cli_iterations
    ));
    out.push_str(&format!(
        "- Filters: {}\n",
        if report.command.filters.is_empty() {
            "none".to_string()
        } else {
            report.command.filters.join(", ")
        }
    ));
    out.push_str(&format!("- Primary matrix: {}\n", report.command.primary));
    out.push_str(&format!(
        "- Machine: {} {} with {} worker(s)\n\n",
        report.machine.os, report.machine.arch, report.machine.available_parallelism
    ));
    out.push_str("| Benchmark | Milestone | Runner | Status | Measurements |\n");
    out.push_str("| --- | --- | --- | --- | --- |\n");
    for row in &report.rows {
        let measurement_summary = if row.measurements.is_empty() {
            String::new()
        } else {
            row.measurements
                .iter()
                .map(|measurement| {
                    let observations = measurement
                        .observations
                        .iter()
                        .map(|observation| format!("{}={}", observation.name, observation.value))
                        .collect::<Vec<_>>()
                        .join(",");
                    if observations.is_empty() {
                        format!("{}={:.6}s", measurement.name, measurement.seconds)
                    } else {
                        format!(
                            "{}={:.6}s [{}]",
                            measurement.name, measurement.seconds, observations
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join("<br>")
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            row.id,
            row.milestone.as_str(),
            row.runner.as_str(),
            row.status,
            measurement_summary
        ));
    }
    out
}

pub(crate) fn render_compare_markdown_report(report: &CompareReport) -> String {
    let mut out = String::new();
    out.push_str("# Stab Benchmark Compare\n\n");
    out.push_str(&format!(
        "- Generated Unix epoch seconds: {}\n",
        report.generated_unix_epoch_seconds
    ));
    out.push_str(&format!(
        "- Stim: {} ({})\n",
        report.stim.actual_tag, report.stim.actual_commit
    ));
    out.push_str(&format!("- Stab commit: {}\n", report.stab.commit));
    out.push_str(&format!(
        "- Stab local modifications: {}\n",
        report.stab.local_modifications
    ));
    out.push_str(&format!("- Profile: {}\n", report.command.profile));
    out.push_str(&format!("- Baseline: {}\n", report.command.baseline_path));
    out.push_str(&format!(
        "- Filters: {}\n",
        if report.command.filters.is_empty() {
            "none".to_string()
        } else {
            report.command.filters.join(", ")
        }
    ));
    out.push_str(&format!("- Primary matrix: {}\n", report.command.primary));
    if let Some(memory_baseline_path) = &report.command.memory_baseline_path {
        out.push_str(&format!("- Memory baseline: {memory_baseline_path}\n"));
    }
    if let Some(beta_waivers_path) = &report.command.beta_waivers_path {
        out.push_str(&format!("- Beta waivers: {beta_waivers_path}\n"));
    }
    if let Some(thresholds_path) = &report.command.thresholds_path {
        out.push_str(&format!("- Thresholds: {thresholds_path}\n"));
    }
    if let Some(regression_waivers_path) = &report.command.regression_waivers_path {
        out.push_str(&format!(
            "- Regression waivers: {regression_waivers_path}\n"
        ));
    }
    if let Some(profiler_notes_path) = &report.command.profiler_notes_path {
        out.push_str(&format!("- Profiler notes: {profiler_notes_path}\n"));
    }
    out.push_str(&format!("- Warmup: {}\n", report.command.warmup));
    out.push_str(&format!(
        "- Measurement runs: {}\n",
        report.command.measurement_runs
    ));
    out.push_str(&format!(
        "- Machine: {} {} with {} worker(s)\n\n",
        report.machine.os, report.machine.arch, report.machine.available_parallelism
    ));
    out.push_str("| Benchmark | Milestone | Class | Status | Pass/Fail | Beta Gate | Stim Median | Stab Median | Ratio | Ratio Source | Stab Alloc Max | Stab Resident Max | Stab Resident Delta Max | Memory Gate | Regression Threshold | Profiler Note | Note |\n");
    out.push_str(
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for row in &report.rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            row.id,
            row.milestone.as_str(),
            row.comparability.as_str(),
            row.status,
            row.pass_fail_status,
            format_beta_gate(row),
            format_optional_seconds(row.stim_median_seconds),
            format_optional_seconds(row.stab_median_seconds),
            format_optional_ratio(row.relative_ratio),
            format_ratio_source(row),
            format_optional_bytes(row.stab_allocation_bytes_max),
            format_optional_bytes(row.stab_resident_bytes_max),
            format_optional_bytes(row.stab_resident_delta_bytes_max),
            format_memory_gate(row),
            format_regression_threshold(row),
            format_profiler_note(row),
            row.note.as_deref().unwrap_or("")
        ));
    }
    render_report_only_submeasurements(&mut out, report);
    out
}

fn render_report_only_submeasurements(out: &mut String, report: &CompareReport) {
    let rows = report.rows.iter().filter(|row| {
        row.comparability.omits_multi_measurement_median() && row.stab_measurements.len() > 1
    });
    let mut rows = rows.peekable();
    if rows.peek().is_none() {
        return;
    }

    out.push_str("\n## Report-Only Submeasurements\n\n");
    out.push_str("| Benchmark | Measurement | Median | Normalized Rate |\n");
    out.push_str("| --- | --- | ---: | ---: |\n");
    for row in rows {
        for measurement in &row.stab_measurements {
            let normalized_rate = crate::baseline::measurement_rate_work(&row.id, measurement)
                .map_or_else(String::new, |(work, unit)| {
                    let rate = if measurement.seconds > 0.0 {
                        work / measurement.seconds
                    } else {
                        0.0
                    };
                    format!("{rate:.3e} {unit}")
                });
            out.push_str(&format!(
                "| {} | {} | {:.9}s | {} |\n",
                row.id, measurement.name, measurement.seconds, normalized_rate
            ));
        }
    }
}

fn format_beta_gate(row: &CompareRowResult) -> String {
    match (
        &row.beta_gate_waiver_reason,
        &row.beta_gate_waiver_follow_up,
        &row.beta_gate_error,
    ) {
        (Some(reason), Some(follow_up), None) => {
            format!(
                "{} ({reason}; follow-up: {follow_up})",
                row.beta_gate_status
            )
        }
        (_, _, Some(error)) => format!("{} ({error})", row.beta_gate_status),
        _ => row.beta_gate_status.clone(),
    }
}

fn format_optional_seconds(seconds: Option<f64>) -> String {
    seconds.map_or_else(String::new, |seconds| format!("{seconds:.9}s"))
}

fn format_optional_ratio(ratio: Option<f64>) -> String {
    ratio.map_or_else(String::new, |ratio| format!("{ratio:.3}x"))
}

fn format_ratio_source(row: &CompareRowResult) -> String {
    row.measurement_ratios
        .iter()
        .max_by(|left, right| left.relative_ratio.total_cmp(&right.relative_ratio))
        .map_or_else(String::new, |ratio| {
            format!(
                "{} / {} = {:.3}x",
                ratio.stab_name, ratio.stim_name, ratio.relative_ratio
            )
        })
}

fn format_optional_bytes(bytes: Option<u64>) -> String {
    bytes.map_or_else(String::new, |bytes| bytes.to_string())
}

fn format_profiler_note(row: &CompareRowResult) -> String {
    match (&row.profiler_note_path, &row.profiler_note_error) {
        (Some(path), None) => format!("{} ({path})", row.profiler_note_status),
        (Some(path), Some(error)) => format!("{} ({path}: {error})", row.profiler_note_status),
        (None, Some(error)) => format!("{} ({error})", row.profiler_note_status),
        (None, None) => row.profiler_note_status.clone(),
    }
}

fn format_memory_gate(row: &CompareRowResult) -> String {
    let allocation_allowed = row
        .memory_gate_allowed_bytes_max
        .map_or_else(String::new, |bytes| format!(" alloc<={bytes}"));
    let resident_allowed = row
        .memory_gate_allowed_resident_bytes_max
        .map_or_else(String::new, |bytes| format!(" rss<={bytes}"));
    let resident_delta_allowed = row
        .memory_gate_allowed_resident_delta_bytes_max
        .map_or_else(String::new, |bytes| format!(" rss_delta<={bytes}"));
    let allowed = format!("{allocation_allowed}{resident_allowed}{resident_delta_allowed}");
    match &row.memory_gate_error {
        Some(error) => format!("{}{} ({error})", row.memory_gate_status, allowed),
        None => format!("{}{}", row.memory_gate_status, allowed),
    }
}

fn format_regression_threshold(row: &CompareRowResult) -> String {
    let max_ratio = row
        .regression_threshold_max_ratio
        .map_or_else(String::new, |ratio| format!(" <= {ratio:.3}x"));
    match (
        &row.regression_threshold_waiver_reason,
        &row.regression_threshold_waiver_follow_up,
        &row.regression_threshold_error,
    ) {
        (Some(reason), Some(follow_up), None) => format!(
            "{}{} ({reason}; follow-up: {follow_up})",
            row.regression_threshold_status, max_ratio
        ),
        (_, _, Some(error)) => {
            format!("{}{} ({error})", row.regression_threshold_status, max_ratio)
        }
        _ => format!("{}{}", row.regression_threshold_status, max_ratio),
    }
}

fn command_first_line<I, S>(
    program: &str,
    args: I,
    working_dir: &Path,
) -> Result<String, BenchError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = args
        .into_iter()
        .map(|arg| OsString::from(arg.as_ref()))
        .collect::<Vec<_>>();
    let output = run_process(Path::new(program), &args, b"", working_dir, true)?;
    check_success(Path::new(program), &output)?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_string())
}

fn default_measurement_runs() -> usize {
    1
}
