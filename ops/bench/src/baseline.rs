use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::config::{PREFIX, STIM_COMMIT, STIM_TAG};
use crate::error::BenchError;
use crate::manifest::{BenchmarkManifest, BenchmarkRow, Runner};
use crate::process::{check_success, run_process};
use crate::report::{
    BaselineCommandMetadata, BaselineReport, BaselineRowResult, Measurement, RowCommandMetadata,
    StimMetadata, machine_metadata, render_markdown_report, unix_epoch_seconds,
};
use crate::root::RepoRoot;
use crate::stim::{ensure_stim_binaries, validate_stim_source};

#[derive(Clone, Debug)]
pub(crate) struct BaselineOptions {
    pub(crate) stim: PathBuf,
    pub(crate) out: PathBuf,
    pub(crate) target_seconds: f64,
    pub(crate) cli_iterations: u32,
    pub(crate) only: Vec<String>,
    pub(crate) rebuild_stim: bool,
}

pub(crate) fn run_baseline(
    root: &RepoRoot,
    manifest: &BenchmarkManifest,
    options: &BaselineOptions,
) -> Result<(), BenchError> {
    if !options.target_seconds.is_finite() || options.target_seconds <= 0.0 {
        return Err(BenchError::InvalidTargetSeconds);
    }
    if options.cli_iterations == 0 {
        return Err(BenchError::InvalidCliIterations);
    }
    let stim_source = root.resolve_relative(&options.stim);
    let version = validate_stim_source(&stim_source)?;
    let rows = manifest.filtered(&options.only)?;
    let needs_stim_perf = rows.iter().any(|row| row.runner == Runner::StimPerf);
    let needs_stim_cli = rows.iter().any(|row| row.runner == Runner::StimCli);
    if needs_stim_perf || needs_stim_cli {
        ensure_stim_binaries(
            root,
            &stim_source,
            needs_stim_perf,
            needs_stim_cli,
            options.rebuild_stim,
        )?;
    }

    let mut results = Vec::new();
    for row in rows {
        results.push(run_baseline_row(
            root,
            row,
            options.target_seconds,
            options.cli_iterations,
        )?);
    }

    let out_dir = root.create_benchmark_output_dir(&options.out)?;
    let report = BaselineReport {
        schema_version: 1,
        generated_unix_epoch_seconds: unix_epoch_seconds(),
        machine: machine_metadata(root)?,
        stim: StimMetadata {
            source_path: stim_source.display().to_string(),
            expected_tag: STIM_TAG.to_string(),
            expected_commit: STIM_COMMIT.to_string(),
            actual_tag: version.tag,
            actual_commit: version.commit,
        },
        command: BaselineCommandMetadata {
            target_seconds: options.target_seconds,
            cli_iterations: options.cli_iterations,
            filters: options.only.clone(),
        },
        rows: results,
    };
    let json_path = out_dir.join("baseline.json");
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&json_path, json).map_err(|source| BenchError::WriteOutput {
        path: json_path.clone(),
        source,
    })?;
    let report_path = out_dir.join("report.md");
    std::fs::write(&report_path, render_markdown_report(&report)).map_err(|source| {
        BenchError::WriteOutput {
            path: report_path.clone(),
            source,
        }
    })?;
    println!("[{PREFIX}] wrote {}", json_path.display());
    println!("[{PREFIX}] wrote {}", report_path.display());
    Ok(())
}

pub(crate) fn run_compare(
    manifest: &BenchmarkManifest,
    milestone: Option<&str>,
    strict: bool,
) -> Result<(), BenchError> {
    let rows = manifest
        .rows
        .iter()
        .filter(|row| milestone.is_none_or(|milestone| milestone == row.milestone.as_str()))
        .collect::<Vec<_>>();
    println!(
        "[{PREFIX}] Stab-vs-Stim comparison runners are pending; {} row(s) planned.",
        rows.len()
    );
    for row in rows {
        println!(
            "- {} {} {} {}",
            row.milestone.as_str(),
            row.id,
            row.threshold_class,
            row.measurement
        );
    }
    if strict {
        Err(BenchError::ComparePending)
    } else {
        Ok(())
    }
}

fn run_baseline_row(
    root: &RepoRoot,
    row: &BenchmarkRow,
    target_seconds: f64,
    cli_iterations: u32,
) -> Result<BaselineRowResult, BenchError> {
    match row.runner {
        Runner::ContractOnly => Ok(BaselineRowResult {
            id: row.id.clone(),
            milestone: row.milestone,
            threshold_class: row.threshold_class.clone(),
            runner: row.runner,
            upstream_source: row.upstream_source.clone(),
            phase: row.phase.clone(),
            measurement: row.measurement.clone(),
            status: "contract-only".to_string(),
            command: RowCommandMetadata {
                program: String::new(),
                args: Vec::new(),
                stdin_path: row.stdin_path.clone(),
            },
            measurements: Vec::new(),
        }),
        Runner::StimPerf => run_stim_perf_row(root, row, target_seconds),
        Runner::StimCli => run_stim_cli_row(root, row, cli_iterations),
    }
}

fn run_stim_perf_row(
    root: &RepoRoot,
    row: &BenchmarkRow,
    target_seconds: f64,
) -> Result<BaselineRowResult, BenchError> {
    let args = vec![
        OsString::from("--only"),
        OsString::from(row.stim_perf_filter.as_str()),
        OsString::from("--target_seconds"),
        OsString::from(format!("{target_seconds}")),
    ];
    let output = run_process(&root.stim_perf_binary(), &args, b"", &root.path, true)?;
    check_success(&root.stim_perf_binary(), &output)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let measurements = parse_stim_perf_output(&stdout);
    if measurements.is_empty() {
        return Err(BenchError::MissingPerfMeasurements {
            row_id: row.id.clone(),
        });
    }
    Ok(BaselineRowResult {
        id: row.id.clone(),
        milestone: row.milestone,
        threshold_class: row.threshold_class.clone(),
        runner: row.runner,
        upstream_source: row.upstream_source.clone(),
        phase: row.phase.clone(),
        measurement: row.measurement.clone(),
        status: "measured".to_string(),
        command: RowCommandMetadata {
            program: root.stim_perf_binary().display().to_string(),
            args: args
                .iter()
                .map(|arg| arg.to_string_lossy().to_string())
                .collect(),
            stdin_path: row.stdin_path.clone(),
        },
        measurements,
    })
}

fn run_stim_cli_row(
    root: &RepoRoot,
    row: &BenchmarkRow,
    cli_iterations: u32,
) -> Result<BaselineRowResult, BenchError> {
    let iterations = usize::try_from(cli_iterations)
        .map_err(|_| BenchError::CliIterationsOverflow(cli_iterations))?;
    let args = row
        .argv_tokens()
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let stdin = row.stdin(root)?;
    let mut timings = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let output = run_process(&root.stim_binary(), &args, &stdin, &root.path, false)?;
        check_success(&root.stim_binary(), &output)?;
        timings.push(start.elapsed());
    }
    timings.sort();
    let median = timings
        .get(timings.len() / 2)
        .map(Duration::as_secs_f64)
        .unwrap_or_default();
    Ok(BaselineRowResult {
        id: row.id.clone(),
        milestone: row.milestone,
        threshold_class: row.threshold_class.clone(),
        runner: row.runner,
        upstream_source: row.upstream_source.clone(),
        phase: row.phase.clone(),
        measurement: row.measurement.clone(),
        status: "measured".to_string(),
        command: RowCommandMetadata {
            program: root.stim_binary().display().to_string(),
            args: args
                .iter()
                .map(|arg| arg.to_string_lossy().to_string())
                .collect(),
            stdin_path: row.stdin_path.clone(),
        },
        measurements: vec![Measurement {
            name: row.id.clone(),
            seconds: median,
            iterations: Some(iterations),
        }],
    })
}

fn parse_stim_perf_output(stdout: &str) -> Vec<Measurement> {
    stdout.lines().filter_map(parse_stim_perf_line).collect()
}

fn parse_stim_perf_line(line: &str) -> Option<Measurement> {
    if !line.contains("(vs") {
        return None;
    }
    let name = line.split_whitespace().last()?.to_string();
    let before_expected = line.split("(vs").next()?.trim();
    let mut parts = before_expected.split_whitespace().rev();
    let unit = parts.next()?;
    let value = parts.next()?.parse::<f64>().ok()?;
    let seconds = duration_seconds(value, unit)?;
    Some(Measurement {
        name,
        seconds,
        iterations: None,
    })
}

fn duration_seconds(value: f64, unit: &str) -> Option<f64> {
    match unit {
        "s" => Some(value),
        "ms" => Some(value / 1_000.0),
        "us" => Some(value / 1_000_000.0),
        "ns" => Some(value / 1_000_000_000.0),
        "ps" => Some(value / 1_000_000_000_000.0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_stim_perf_line;
    use crate::report::Measurement;

    #[test]
    fn parses_stim_perf_measurement_line() {
        let measurement = parse_stim_perf_line(
            "[..................*<|....................] 1.3 us (vs 950 ns) circuit_parse",
        )
        .expect("parse line");

        assert_eq!(
            measurement,
            Measurement {
                name: "circuit_parse".to_string(),
                seconds: 0.0000013,
                iterations: None,
            }
        );
    }
}
