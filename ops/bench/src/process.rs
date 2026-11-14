//! Benchmark-runner conveniences over the shared `stab-process` supervisor.

use std::ffi::{OsStr, OsString};
use std::path::Path;

pub(crate) use stab_process::{
    Launcher, OutputPolicy, ProcessEnvironment, ProcessError, ProcessLaunch, ProcessLimits,
    ProcessRequest, ProcessResult, UnreadStdin, run_bounded_process, run_launcher,
};

use crate::config::COMMAND_TIMEOUT;
use crate::error::BenchError;

const BASELINE_CAPTURE_LIMIT_BYTES: usize = 8 << 20;

/// The running `stab-bench` binary's hidden `e2e-launch` command.
pub(crate) fn bench_launcher() -> Result<Launcher, std::io::Error> {
    Ok(Launcher {
        program: std::env::current_exe()?,
        args: vec![OsString::from("e2e-launch")],
    })
}

pub(crate) fn run_process(
    program: &Path,
    args: &[OsString],
    stdin: &[u8],
    working_dir: &Path,
    capture_stdout: bool,
) -> Result<ProcessResult, BenchError> {
    run_bounded_process(&ProcessRequest {
        launch: ProcessLaunch::Direct,
        program: program.to_path_buf(),
        args: args.to_vec(),
        stdin: stdin.to_vec(),
        unread_stdin: UnreadStdin::Reject,
        working_directory: working_dir.to_path_buf(),
        environment: ProcessEnvironment::Inherit,
        affinity_cpu: None,
        limits: ProcessLimits {
            stdin_bytes: stdin.len(),
            stdout: if capture_stdout {
                BASELINE_CAPTURE_LIMIT_BYTES.into()
            } else {
                OutputPolicy::Discard
            },
            stderr: BASELINE_CAPTURE_LIMIT_BYTES.into(),
            regular_file_bytes: None,
            timeout: COMMAND_TIMEOUT,
        },
    })
    .map_err(BenchError::from)
}

pub(crate) fn run_checked_status<I, S>(
    program: &str,
    args: I,
    working_dir: &Path,
) -> Result<(), BenchError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = args
        .into_iter()
        .map(|arg| OsString::from(arg.as_ref()))
        .collect::<Vec<_>>();
    let output = run_process(Path::new(program), &args, b"", working_dir, false)?;
    check_success(Path::new(program), &output)
}

pub(crate) fn check_success(program: &Path, output: &ProcessResult) -> Result<(), BenchError> {
    if output.status == Some(0) {
        return Ok(());
    }
    Err(BenchError::CommandFailed {
        program: program.display().to_string(),
        status: output
            .status
            .map_or_else(|| "signal".to_string(), |status| status.to_string()),
        stderr: String::from_utf8_lossy(&output.stderr)
            .into_owned()
            .into_boxed_str(),
    })
}
