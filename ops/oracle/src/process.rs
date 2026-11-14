//! Oracle conveniences over the shared `stab-process` supervisor.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::time::Duration;

use stab_process::{
    OutputPolicy, ProcessEnvironment, ProcessLaunch, ProcessLimits, ProcessRequest, UnreadStdin,
    run_bounded_process,
};

use super::{CapturedOutput, OracleError, ProcessOutput};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
pub(super) const OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;
const DIAGNOSTIC_LIMIT_BYTES: usize = 4096;

pub(super) fn run_checked<I, S>(
    program: &str,
    args: I,
    stdin: &[u8],
    working_dir: &Path,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_checked_path(Path::new(program), args, stdin, working_dir)
}

pub(super) fn run_checked_path<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: &Path,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_process(program, args, stdin, working_dir)?;
    if output.success() {
        return Ok(output);
    }
    Err(OracleError::CommandFailed {
        program: program.display().to_string(),
        status: display_status(output.status),
        stdout: output.stdout.render_for_diagnostics().into_boxed_str(),
        stderr: output.stderr.render_for_diagnostics().into_boxed_str(),
    })
}

pub(super) fn run_process<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: &Path,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run(program, args, stdin, working_dir, None)
}

/// Runs a program whose every regular-file write is bounded by `file_limit`
/// bytes, so a runaway side output terminates the program at the limit.
pub(super) fn run_process_with_file_limit<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: &Path,
    file_limit: u64,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run(program, args, stdin, working_dir, Some(file_limit))
}

fn run<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: &Path,
    regular_file_bytes: Option<u64>,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let result = run_bounded_process(&ProcessRequest {
        launch: ProcessLaunch::Direct,
        program: program.to_path_buf(),
        args: args
            .into_iter()
            .map(|arg| OsString::from(arg.as_ref()))
            .collect(),
        stdin: stdin.to_vec(),
        // Stim and Stab may reject their arguments before reading stdin.
        unread_stdin: UnreadStdin::Allow,
        working_directory: working_dir.to_path_buf(),
        environment: ProcessEnvironment::Inherit,
        affinity_cpu: None,
        limits: ProcessLimits {
            stdin_bytes: stdin.len(),
            stdout: OutputPolicy::Capture {
                maximum_bytes: OUTPUT_LIMIT_BYTES,
            },
            stderr: OutputPolicy::Capture {
                maximum_bytes: OUTPUT_LIMIT_BYTES,
            },
            regular_file_bytes,
            timeout: COMMAND_TIMEOUT,
        },
    })?;
    Ok(ProcessOutput {
        status: result.status,
        stdout: CapturedOutput {
            bytes: result.stdout,
        },
        stderr: CapturedOutput {
            bytes: result.stderr,
        },
    })
}

pub(super) fn render_bytes_for_diagnostics(bytes: &[u8]) -> String {
    let mut rendered = String::new();
    let display_len = bytes.len().min(DIAGNOSTIC_LIMIT_BYTES);
    let display_bytes = bytes.get(..display_len).unwrap_or(bytes);
    for byte in display_bytes {
        for escaped in std::ascii::escape_default(*byte) {
            rendered.push(char::from(escaped));
        }
    }
    if bytes.len() > display_len {
        rendered.push_str("\n[truncated]");
    }
    rendered
}

pub(super) fn display_status(status: Option<i32>) -> String {
    match status {
        Some(status) => status.to_string(),
        None => "terminated by signal".to_string(),
    }
}
