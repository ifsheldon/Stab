//! Measurement launcher for benchmark processes.
//!
//! Linux folds the spawning process's high-water RSS into a child's `ru_maxrss`
//! when the child executes: `posix_spawn` shares the parent's memory map until
//! `exec`, and `fork` seeds the child's peak from the parent's current RSS.
//! Spawning a measured program directly from the runner therefore reports at
//! least the runner's own peak, which hid every benchmark's real memory use.
//!
//! The supervisor instead executes a fresh, small launcher: a binary whose
//! entry point calls [`run_launcher`], such as `stab-bench e2e-launch`. The launcher pins itself to the requested CPU so the
//! program inherits the mask before it starts, spawns the program with inherited
//! standard streams, bounds the program's regular-file writes, reaps it with
//! `wait4`, and writes
//! the program's wait status, peak RSS, and monotonic spawn and exit instants to
//! a report file owned by the supervisor. The launcher never writes to its
//! standard streams, which belong to the measured program.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{ProcessError, ProcessRequest, limits, wait_with_rusage};

/// Environment variable carrying the JSON [`LaunchSpec`] to the launcher.
const SPEC_ENV: &str = "STAB_BENCH_LAUNCH_SPEC";
const MAX_REPORT_BYTES: u64 = 4 << 10;
/// Exit status of a launcher that could not measure its program.
const LAUNCH_FAILURE_EXIT: u8 = 125;

/// The launcher executable and the arguments that select its entry point, which
/// must call [`run_launcher`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Launcher {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LaunchSpec {
    program: String,
    args: Vec<String>,
    report: String,
    affinity_cpu: Option<usize>,
    file_limit_bytes: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum LaunchReport {
    Completed {
        wait_status: i32,
        peak_rss_bytes: u64,
        spawn_monotonic_ns: u64,
        exit_monotonic_ns: u64,
    },
    Failed {
        error: String,
    },
}

/// The supervisor's side of one measured launch.
pub(super) struct MeasuredLaunch {
    spec_json: String,
    report: tempfile::TempPath,
}

/// What the launcher observed about its program.
pub(super) struct MeasuredCompletion {
    pub(super) wait_status: i32,
    pub(super) peak_rss_bytes: u64,
    pub(super) elapsed: Duration,
}

impl MeasuredLaunch {
    pub(super) fn prepare(request: &ProcessRequest) -> Result<Self, ProcessError> {
        let report = tempfile::Builder::new()
            .prefix("stab-bench-launch-")
            .tempfile()
            .map_err(|source| launch_error(request, format!("create report file: {source}")))?
            .into_temp_path();
        let utf8 = |value: &std::ffi::OsStr, what: &str| {
            value
                .to_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| launch_error(request, format!("{what} is not UTF-8")))
        };
        let spec = LaunchSpec {
            program: utf8(request.program.as_os_str(), "program path")?,
            args: request
                .args
                .iter()
                .map(|arg| utf8(arg, "argument"))
                .collect::<Result<_, _>>()?,
            report: utf8(report.as_os_str(), "report path")?,
            affinity_cpu: request.affinity_cpu,
            file_limit_bytes: request.limits.regular_file_bytes,
        };
        let spec_json = serde_json::to_string(&spec)
            .map_err(|source| launch_error(request, format!("encode launch spec: {source}")))?;
        Ok(Self { spec_json, report })
    }

    pub(super) fn environment(&self) -> (&'static str, &str) {
        (SPEC_ENV, &self.spec_json)
    }

    /// Reads the launcher's report after the launcher itself exited with `launcher_status`.
    pub(super) fn completion(
        &self,
        request: &ProcessRequest,
        launcher_status: std::process::ExitStatus,
    ) -> Result<MeasuredCompletion, ProcessError> {
        let report = read_report(&self.report);
        match (launcher_status.success(), report) {
            (
                true,
                Ok(LaunchReport::Completed {
                    wait_status,
                    peak_rss_bytes,
                    spawn_monotonic_ns,
                    exit_monotonic_ns,
                }),
            ) => {
                let elapsed = exit_monotonic_ns
                    .checked_sub(spawn_monotonic_ns)
                    .map(Duration::from_nanos)
                    .ok_or_else(|| ProcessError::CompletionClock(request.program.clone()))?;
                Ok(MeasuredCompletion {
                    wait_status,
                    peak_rss_bytes,
                    elapsed,
                })
            }
            (_, Ok(LaunchReport::Failed { error })) => Err(launch_error(request, error)),
            (_, Err(reason)) => Err(launch_error(
                request,
                format!("launcher exited with {launcher_status} without a valid report: {reason}"),
            )),
            (false, Ok(LaunchReport::Completed { .. })) => Err(launch_error(
                request,
                format!("launcher exited with {launcher_status} after reporting completion"),
            )),
        }
    }
}

fn launch_error(request: &ProcessRequest, reason: String) -> ProcessError {
    ProcessError::Launch {
        program: request.program.clone(),
        reason: reason.into_boxed_str(),
    }
}

fn read_report(path: &Path) -> Result<LaunchReport, String> {
    use std::io::Read as _;

    let file = std::fs::File::open(path).map_err(|source| source.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_REPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| source.to_string())?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_REPORT_BYTES {
        return Err(format!("report exceeds {MAX_REPORT_BYTES} bytes"));
    }
    serde_json::from_slice(&bytes).map_err(|source| source.to_string())
}

/// Entry point of the launcher process. Reads its spec from the environment,
/// measures the program, and reports through the spec's report file only.
pub fn run_launcher() -> ExitCode {
    ExitCode::from(launcher_exit_code())
}

pub(super) fn launcher_exit_code() -> u8 {
    let Ok(spec_json) = std::env::var(SPEC_ENV) else {
        return LAUNCH_FAILURE_EXIT;
    };
    let Ok(spec) = serde_json::from_str::<LaunchSpec>(&spec_json) else {
        return LAUNCH_FAILURE_EXIT;
    };
    let (report, code) = match launch(&spec) {
        Ok(report) => (report, 0),
        Err(error) => (LaunchReport::Failed { error }, LAUNCH_FAILURE_EXIT),
    };
    let written = serde_json::to_vec(&report)
        .map_err(|source| source.to_string())
        .and_then(|bytes| std::fs::write(&spec.report, bytes).map_err(|e| e.to_string()));
    if written.is_err() {
        return LAUNCH_FAILURE_EXIT;
    }
    code
}

fn launch(spec: &LaunchSpec) -> Result<LaunchReport, String> {
    use std::os::unix::process::ExitStatusExt as _;

    if let Some(cpu) = spec.affinity_cpu {
        limits::pin_current_thread(cpu).map_err(|source| format!("set affinity: {source}"))?;
    }
    let mut command = Command::new(&spec.program);
    command.args(&spec.args).env_remove(SPEC_ENV);
    let spawn_monotonic_ns = monotonic_ns()?;
    let mut child = command
        .spawn()
        .map_err(|source| format!("spawn {}: {source}", spec.program))?;
    // The file limit binds only the program: the launcher must still be able to
    // write its report after the program exceeds a small limit.
    if let Some(maximum) = spec.file_limit_bytes
        && let Err(source) = limits::set_child_file_limit(child.id(), maximum)
    {
        drop(child.kill());
        drop(child.wait());
        return Err(format!("set file-size limit: {source}"));
    }
    let completed = wait_with_rusage(child.id())
        .map_err(|source| format!("wait for {}: {source}", spec.program))?;
    let exit_monotonic_ns = monotonic_ns()?;
    Ok(LaunchReport::Completed {
        wait_status: completed.status.into_raw(),
        peak_rss_bytes: completed.peak_rss_bytes,
        spawn_monotonic_ns,
        exit_monotonic_ns,
    })
}

fn monotonic_ns() -> Result<u64, String> {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    let seconds = u64::try_from(now.tv_sec).map_err(|_| "negative monotonic clock".to_string())?;
    let nanos = u64::try_from(now.tv_nsec).map_err(|_| "negative monotonic clock".to_string())?;
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(nanos))
        .ok_or_else(|| "monotonic clock overflows nanoseconds".to_string())
}
