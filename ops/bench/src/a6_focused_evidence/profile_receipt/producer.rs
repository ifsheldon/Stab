use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::{Args, Subcommand};
use sha2::{Digest as _, Sha256};

use super::{
    ArtifactBinding, MAX_PROBE_STDERR_BYTES, ProfileDataBinding, ProfileProbeDiagnostics,
    ProfileReceipt, ProfileReceiptIdentity, RECEIPT_FILE_NAME, profile_error, read_and_validate,
    validate_against, validate_target_benchmark_path,
};
use crate::a6_focused_evidence::artifacts::{
    bind_artifact, normalize_repo_relative_path, read_bounded,
};
use crate::error::BenchError;
use crate::process::{
    OutputPolicy, ProcessLimits, ProcessRequest, ProcessResult, run_bounded_process,
};
use crate::report::CompareReport;
use crate::root::RepoRoot;
use crate::source_file::atomic_create_repo_regular_file;

const PERF: &str = "/usr/bin/perf";
const TRUE: &str = "/usr/bin/true";
const PERF_EVENT_PARANOID: &str = "/proc/sys/kernel/perf_event_paranoid";
const PERF_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Args)]
pub(crate) struct A6ProfileReceiptArgs {
    /// Exact focused compare report that the profile disposition explains.
    #[arg(long, value_name = "PATH")]
    focused_report: PathBuf,

    /// New target/benchmarks/.../profile-receipt.json output.
    #[arg(long, value_name = "PATH")]
    out: PathBuf,

    #[command(subcommand)]
    outcome: ProfileReceiptCommand,
}

#[derive(Debug, Subcommand)]
enum ProfileReceiptCommand {
    /// Validate and bind an existing Linux perf-record capture.
    Captured {
        /// Existing sibling target/benchmarks/.../perf.data capture.
        #[arg(long, value_name = "PATH")]
        data: PathBuf,
    },

    /// Run a fixed perf capability probe and record a kernel-policy denial.
    Unavailable,
}

pub(crate) fn produce(root: &RepoRoot, args: A6ProfileReceiptArgs) -> Result<(), BenchError> {
    let (focused_report_binding, focused_report_bytes) =
        bind_artifact(root, &args.focused_report, super::super::MAX_REPORT_BYTES)?;
    let focused_report: CompareReport =
        serde_json::from_slice(&focused_report_bytes).map_err(|error| {
            profile_error(format!(
                "failed to parse {}: {error}",
                focused_report_binding.path
            ))
        })?;
    let identity =
        ProfileReceiptIdentity::from_focused_report(&focused_report_binding, &focused_report)?;
    let probe_policy = read_perf_event_paranoid()?;

    let receipt = match args.outcome {
        ProfileReceiptCommand::Captured { data } => {
            let data = normalize_repo_relative_path(root, &data)?;
            let validation = validate_captured_perf_data(root, &data)?;
            let probe = ProfileProbeDiagnostics::from_stderr(
                validation
                    .status
                    .ok_or_else(|| profile_error("perf report terminated by signal"))?,
                probe_policy,
                &validation.stderr,
            )?;
            let data = ProfileDataBinding::bind(root, &data)?;
            ProfileReceipt::captured(identity, probe, data)?
        }
        ProfileReceiptCommand::Unavailable => {
            let probe = probe_perf_availability(root)?;
            let status = probe
                .status
                .ok_or_else(|| profile_error("perf capability probe terminated by signal"))?;
            if status == 0 {
                return Err(profile_error(
                    "perf capability probe succeeded; capture a profile instead of recording unavailability",
                ));
            }
            let diagnostics =
                ProfileProbeDiagnostics::from_stderr(status, probe_policy, &probe.stderr)?;
            ProfileReceipt::unavailable(identity, diagnostics)?
        }
    };

    let output = publish_receipt(
        root,
        &args.out,
        &receipt,
        &focused_report_binding,
        &focused_report,
    )?;
    println!(
        "[stab-bench] published typed A6 profile receipt {}",
        output.display()
    );
    Ok(())
}

fn validate_captured_perf_data(root: &RepoRoot, data: &Path) -> Result<ProcessResult, BenchError> {
    let absolute = root.resolve_relative(data);
    let output = run_perf(
        root,
        [
            OsString::from("report"),
            OsString::from("--header-only"),
            OsString::from("--input"),
            absolute.into_os_string(),
        ],
    )?;
    if output.status != Some(0) {
        return Err(profile_error(format!(
            "perf report rejected captured data with status {:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output)
}

fn probe_perf_availability(root: &RepoRoot) -> Result<ProcessResult, BenchError> {
    run_perf(
        root,
        [
            OsString::from("stat"),
            OsString::from("--event"),
            OsString::from("cycles"),
            OsString::from("--"),
            OsString::from(TRUE),
        ],
    )
}

fn run_perf<const N: usize>(
    root: &RepoRoot,
    args: [OsString; N],
) -> Result<ProcessResult, BenchError> {
    Ok(run_bounded_process(&ProcessRequest {
        program: PathBuf::from(PERF),
        args: args.into_iter().collect(),
        stdin: Vec::new(),
        working_directory: root.path.clone(),
        environment: vec![
            (OsString::from("HOME"), OsString::from("/nonexistent")),
            (
                OsString::from("XDG_CONFIG_HOME"),
                OsString::from("/nonexistent"),
            ),
            (OsString::from("LANG"), OsString::from("C")),
            (OsString::from("LC_ALL"), OsString::from("C")),
            (OsString::from("PATH"), OsString::from("/usr/bin:/bin")),
        ]
        .into(),
        affinity_cpu: None,
        limits: ProcessLimits {
            stdin_bytes: 0,
            stdout: OutputPolicy::Discard,
            stderr: OutputPolicy::Capture {
                maximum_bytes: MAX_PROBE_STDERR_BYTES,
            },
            regular_file_bytes: None,
            timeout: PERF_TIMEOUT,
        },
    })?)
}

fn read_perf_event_paranoid() -> Result<Option<i32>, BenchError> {
    let bytes = read_bounded(Path::new(PERF_EVENT_PARANOID), 64)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| profile_error(format!("perf_event_paranoid is not UTF-8: {error}")))?;
    let value = text
        .trim()
        .parse::<i32>()
        .map_err(|error| profile_error(format!("failed to parse perf_event_paranoid: {error}")))?;
    Ok(Some(value))
}

pub(super) fn publish_receipt(
    root: &RepoRoot,
    output: &Path,
    receipt: &ProfileReceipt,
    focused_report_binding: &ArtifactBinding,
    focused_report: &CompareReport,
) -> Result<PathBuf, BenchError> {
    let output = normalize_repo_relative_path(root, output)?;
    let output_text = output.to_str().ok_or_else(|| {
        profile_error(format!(
            "profile receipt output {} is not valid UTF-8",
            output.display()
        ))
    })?;
    validate_target_benchmark_path(
        "profile receipt output",
        output_text,
        Some(RECEIPT_FILE_NAME),
    )?;
    let bytes = receipt.to_pretty_json()?;
    let binding = ArtifactBinding {
        path: output_text.to_string(),
        sha256: hex::encode(Sha256::digest(&bytes)),
    };
    validate_against(
        receipt,
        root,
        &binding,
        focused_report_binding,
        focused_report,
        &[],
    )?;
    let absolute = root.resolve_relative(&output);
    atomic_create_repo_regular_file(root, &absolute, &bytes)?;
    read_and_validate(root, &binding, focused_report_binding, focused_report, &[])?;
    Ok(output)
}
