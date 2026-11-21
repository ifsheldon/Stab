//! Oracle conveniences over the shared `stab-process` supervisor.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::time::Duration;

use stab_process::{
    Launcher, ProcessExit, ProcessLimits, ProcessRequest, UnreadStdin, run_bounded_process,
};

use super::{CapturedOutput, OracleError, ProcessOutput};

pub(super) const OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;
const BUILD_OUTPUT_LIMIT_BYTES: usize = 64 * 1024 * 1024;
const DIAGNOSTIC_LIMIT_BYTES: usize = 4096;
/// Wall-clock and capture bounds for one supervised command, chosen at the
/// request site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessBudget {
    timeout: Duration,
    output_limit_bytes: usize,
}

impl ProcessBudget {
    /// A Stim or Stab fixture run, a pinned helper probe, or a quick tool such
    /// as `git`.
    pub(crate) const FIXTURE: Self = Self {
        timeout: Duration::from_secs(120),
        output_limit_bytes: OUTPUT_LIMIT_BYTES,
    };

    /// A CMake or Cargo build, a C++ probe compile, or a Cargo test-harness
    /// listing or run, which may compile the workspace first.
    pub(crate) const BUILD: Self = Self {
        timeout: Duration::from_secs(2 * 60 * 60),
        output_limit_bytes: BUILD_OUTPUT_LIMIT_BYTES,
    };
}

/// The hidden `stab-oracle` subcommand whose entry point must call
/// `stab_process::run_launcher`; the launcher applies a file-size limit before
/// the supervised program starts.
const LAUNCH_COMMAND: &str = "process-launch";

pub(super) fn run_checked<I, S>(
    program: &str,
    args: I,
    stdin: &[u8],
    working_dir: &Path,
    budget: ProcessBudget,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_checked_path(Path::new(program), args, stdin, working_dir, budget)
}

pub(super) fn run_checked_path<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: &Path,
    budget: ProcessBudget,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let (output, exit) = run(program, args, stdin, working_dir, budget, None)?;
    if output.success() {
        return Ok(output);
    }
    Err(OracleError::CommandFailed {
        program: program.display().to_string(),
        status: exit.to_string(),
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
    run(
        program,
        args,
        stdin,
        working_dir,
        ProcessBudget::FIXTURE,
        None,
    )
    .map(|(output, _)| output)
}

/// Runs a program under `budget` and returns its output whatever its status.
#[cfg(test)]
pub(super) fn run_process_with_budget<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: &Path,
    budget: ProcessBudget,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run(program, args, stdin, working_dir, budget, None).map(|(output, _)| output)
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
    run(
        program,
        args,
        stdin,
        working_dir,
        ProcessBudget::FIXTURE,
        Some(file_limit),
    )
    .map(|(output, _)| output)
}

/// Runs `program`. A file limit runs it through this binary's hidden launcher
/// subcommand, which applies the limit before the program starts.
fn run<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: &Path,
    budget: ProcessBudget,
    regular_file_bytes: Option<u64>,
) -> Result<(ProcessOutput, ProcessExit), OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = args
        .into_iter()
        .map(|arg| OsString::from(arg.as_ref()))
        .collect();
    let mut request = ProcessRequest::direct(
        program,
        args,
        working_dir,
        ProcessLimits::capture(budget.output_limit_bytes, budget.timeout),
    )
    .with_stdin(stdin)
    // Stim and Stab may reject their arguments before reading stdin.
    .with_unread_stdin(UnreadStdin::Allow);
    if let Some(maximum) = regular_file_bytes {
        let launcher = Launcher {
            program: std::env::current_exe().map_err(|source| {
                stab_process::ProcessError::Spawn {
                    program: program.to_path_buf(),
                    source,
                }
            })?,
            args: vec![OsString::from(LAUNCH_COMMAND)],
        };
        request = request.measured(launcher, Some(maximum));
    }
    let result = run_bounded_process(&request)?;
    Ok((
        ProcessOutput {
            status: result.status.code(),
            stdout: CapturedOutput {
                bytes: result.stdout,
            },
            stderr: CapturedOutput {
                bytes: result.stderr,
            },
        },
        result.status,
    ))
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
