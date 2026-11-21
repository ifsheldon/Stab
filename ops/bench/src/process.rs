//! Benchmark-runner conveniences over the shared `stab-process` supervisor.

use std::ffi::{OsStr, OsString};
use std::path::Path;

pub(crate) use stab_process::{
    Launcher, OutputPolicy, ProcessError, ProcessLimits, ProcessRequest, ProcessResult,
    run_bounded_process, run_launcher,
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
    let mut limits = ProcessLimits::capture(BASELINE_CAPTURE_LIMIT_BYTES, COMMAND_TIMEOUT);
    if !capture_stdout {
        limits.stdout = OutputPolicy::Discard;
    }
    run_bounded_process(
        &ProcessRequest::direct(program, args.to_vec(), working_dir, limits).with_stdin(stdin),
    )
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
    if output.status.success() {
        return Ok(());
    }
    Err(BenchError::CommandFailed {
        program: program.display().to_string(),
        status: output.status.to_string(),
        stderr: String::from_utf8_lossy(&output.stderr)
            .into_owned()
            .into_boxed_str(),
    })
}
