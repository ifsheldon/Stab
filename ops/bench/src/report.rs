use std::ffi::{OsStr, OsString};
use std::num::NonZeroUsize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::BenchError;
use crate::manifest::{Milestone, Runner};
use crate::process::{check_success, run_process};
use crate::root::RepoRoot;

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
pub(crate) struct BaselineCommandMetadata {
    pub(crate) target_seconds: f64,
    pub(crate) cli_iterations: u32,
    pub(crate) filters: Vec<String>,
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
    pub(crate) iterations: Option<usize>,
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
                .map(|measurement| format!("{}={:.6}s", measurement.name, measurement.seconds))
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
