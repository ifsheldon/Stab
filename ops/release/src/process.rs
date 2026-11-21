//! The release supervisor.
//!
//! It runs on the shared lower layer of `stab-process` (process-group leader,
//! stoppable bounded readers, sticky cancellation) and keeps the publication
//! contract above it: it executes a retained program descriptor that it
//! revalidates around spawn, replaces the environment with a fixed locale,
//! `PATH`, and Git configuration while dropping inherited `GIT_*` overrides,
//! and gives the child no stdin. Once the leader exits, the rest of its group
//! is killed while the leader still pins the group id; readers then drain
//! everything readable and give a descendant outside the group a short grace
//! to close an inherited pipe. Output still arriving after that grace makes
//! the capture incomplete and fails the command.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use stab_process::group::ProcessGroupLeader;
use stab_process::pipe::{CapturedStream, DrainPolicy, OutputPolicy, OutputReader, StreamEnd};

use crate::{ReleaseError, cancellation::ReleaseCancellation, safe_fs};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
const READER_DRAIN_GRACE: Duration = Duration::from_millis(100);
const DRAIN: DrainPolicy = DrainPolicy::StopAfterGrace(READER_DRAIN_GRACE);
const DIAGNOSTIC_PREFIX_BYTES: usize = 4 << 10;

#[derive(Debug)]
pub(crate) struct ProcessOutput {
    pub(crate) stdout: Vec<u8>,
}

/// The program a release command executes.
#[derive(Clone, Copy)]
pub(crate) enum Program<'a> {
    /// An absolute path or a single tool name from `/usr/bin:/bin`, which the
    /// supervisor retains as a descriptor before spawning.
    Resolve(&'a OsStr),
    /// A descriptor the caller already retained.
    Retained(&'a safe_fs::DescriptorProgram),
}

/// One bounded release command.
pub(crate) struct Invocation<'a> {
    program: Program<'a>,
    label: String,
    arg0: Option<&'a OsStr>,
    args: Vec<OsString>,
    environment: &'a [(OsString, OsString)],
    timeout: Duration,
    output_limit: usize,
}

impl<'a> Invocation<'a> {
    /// A command whose errors name `program`: the resolved path, or the
    /// retained program's source path instead of its `/proc/self/fd` alias.
    pub(crate) fn new<I, S>(
        program: Program<'a>,
        args: I,
        timeout: Duration,
        output_limit: usize,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let label = match program {
            Program::Resolve(path) => path.to_string_lossy().into_owned(),
            Program::Retained(retained) => retained.source_path().display().to_string(),
        };
        Self {
            program,
            label,
            arg0: None,
            args: args
                .into_iter()
                .map(|arg| arg.as_ref().to_os_string())
                .collect(),
            environment: &[],
            timeout,
            output_limit,
        }
    }

    /// Passes `name` as the program's `argv[0]`.
    pub(crate) const fn arg0(mut self, name: &'a OsStr) -> Self {
        self.arg0 = Some(name);
        self
    }

    /// Names the command in errors, for example by its Cargo subcommand.
    pub(crate) fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Variables added to the fixed release environment; `GIT_*` entries are
    /// dropped.
    pub(crate) const fn environment(mut self, environment: &'a [(OsString, OsString)]) -> Self {
        self.environment = environment;
        self
    }
}

pub(crate) fn run(
    working_directory: &Path,
    invocation: &Invocation<'_>,
) -> Result<ProcessOutput, ReleaseError> {
    let cancellation = ReleaseCancellation::for_signals()?;
    run_with_cancellation(working_directory, invocation, &cancellation)
}

fn run_with_cancellation(
    working_directory: &Path,
    invocation: &Invocation<'_>,
    cancellation: &ReleaseCancellation,
) -> Result<ProcessOutput, ReleaseError> {
    let label = invocation.label.as_str();
    if cancellation.is_cancelled() {
        return Err(ReleaseError::CommandInterrupted {
            program: label.to_string(),
        });
    }
    // A program the supervisor resolves is revalidated around spawn here; a
    // retained descriptor is executed as retained, and its owner decides
    // whether its source path must still match.
    let resolved;
    let (program, default_arg0, supervisor_owned) = match invocation.program {
        Program::Resolve(path) => {
            resolved = retain_program(path)?;
            (&resolved, Path::new(path).file_name(), true)
        }
        Program::Retained(retained) => (retained, None, false),
    };
    let revalidate = || {
        if supervisor_owned {
            program.revalidate()
        } else {
            Ok(())
        }
    };
    let mut command = Command::new(program.path());
    command
        .args(&invocation.args)
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear();
    for (key, value) in invocation.environment {
        if !key.to_string_lossy().starts_with("GIT_") {
            command.env(key, value);
        }
    }
    command
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0");
    if let Some(name) = invocation.arg0.or(default_arg0) {
        use std::os::unix::process::CommandExt as _;

        command.arg0(name);
    }

    revalidate()?;
    let spawned = ProcessGroupLeader::spawn(&mut command);
    if spawned.is_err() {
        revalidate()?;
    }
    let leader = spawned.map_err(|source| ReleaseError::CommandIo {
        program: label.to_string(),
        source,
    })?;
    let mut child = ManagedChild::new(leader, label.to_string());
    revalidate()?;
    child.start_readers(invocation.output_limit)?;
    let started = Instant::now();

    let termination = loop {
        if cancellation.is_cancelled() {
            child.close_group()?;
            break Termination::Interrupted;
        }
        if child.output_exceeded() {
            child.close_group()?;
            break Termination::OutputLimit;
        }
        let elapsed = started.elapsed();
        if elapsed >= invocation.timeout {
            child.close_group()?;
            break Termination::Timeout;
        }
        if child.leader_exited(POLL_INTERVAL.min(invocation.timeout.saturating_sub(elapsed)))? {
            child.close_group()?;
            break Termination::Completed;
        }
    };

    let (stdout, stderr) = child.join_readers()?;
    if termination == Termination::Interrupted {
        return Err(ReleaseError::CommandInterrupted {
            program: label.to_string(),
        });
    }
    if termination == Termination::OutputLimit || stdout.exceeded || stderr.exceeded {
        return Err(ReleaseError::CommandOutputLimit {
            program: label.to_string(),
            limit: invocation.output_limit,
        });
    }
    if termination == Termination::Timeout {
        return Err(ReleaseError::CommandTimeout {
            program: label.to_string(),
            timeout: invocation.timeout,
        });
    }
    let status = child.status().ok_or_else(|| ReleaseError::CommandIo {
        program: label.to_string(),
        source: std::io::Error::other("release child completed without an exit status"),
    })?;
    require_success(label, status, &stderr.bytes)?;
    require_complete_capture(label, &stdout, &stderr)?;
    Ok(ProcessOutput {
        stdout: stdout.bytes,
    })
}

/// A pipe still held open by an escaped descendant was drained up to the
/// moment nothing was readable; output still arriving when the reader stopped
/// was not, so a truncated capture never passes as complete output.
fn require_complete_capture(
    program: &str,
    stdout: &CapturedStream,
    stderr: &CapturedStream,
) -> Result<(), ReleaseError> {
    for (stream, captured) in [("stdout", stdout), ("stderr", stderr)] {
        if captured.end == StreamEnd::Truncated {
            return Err(ReleaseError::CommandCapture {
                program: program.to_string(),
                source: std::io::Error::other(format!(
                    "{stream} was still being written when the drain grace expired, so the capture is incomplete"
                )),
            });
        }
    }
    Ok(())
}

fn retain_program(program: &OsStr) -> Result<safe_fs::DescriptorProgram, ReleaseError> {
    let path = Path::new(program);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else if path.components().count() == 1 {
        [Path::new("/usr/bin"), Path::new("/bin")]
            .into_iter()
            .map(|directory| directory.join(path))
            .find(|candidate| candidate.is_file())
            .ok_or_else(|| ReleaseError::CommandIo {
                program: program.to_string_lossy().into_owned(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "program is not present in the fixed release tool path",
                ),
            })?
    } else {
        return Err(ReleaseError::CommandIo {
            program: program.to_string_lossy().into_owned(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "release program paths must be absolute or a single tool name",
            ),
        });
    };
    let file = safe_fs::open_regular_file(&resolved)?;
    safe_fs::descriptor_program(&file, &resolved)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Termination {
    Completed,
    Interrupted,
    OutputLimit,
    Timeout,
}

/// The supervised group and its readers. Dropping it kills the group before
/// the leader is reaped and then stops and joins both readers.
struct ManagedChild {
    leader: ProcessGroupLeader,
    program: String,
    stdout: Option<OutputReader>,
    stderr: Option<OutputReader>,
}

impl ManagedChild {
    fn new(leader: ProcessGroupLeader, program: String) -> Self {
        Self {
            leader,
            program,
            stdout: None,
            stderr: None,
        }
    }

    fn start_readers(&mut self, output_limit: usize) -> Result<(), ReleaseError> {
        let policy = OutputPolicy::Capture {
            maximum_bytes: output_limit,
        };
        let child = self.leader.child_mut();
        let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
        let stdout = stdout.ok_or_else(|| self.capture_error("child stdout was not piped"))?;
        let stderr = stderr.ok_or_else(|| self.capture_error("child stderr was not piped"))?;
        self.stdout = Some(
            OutputReader::spawn(stdout, policy, DRAIN)
                .map_err(|source| self.capture_source_error(source))?,
        );
        self.stderr = Some(
            OutputReader::spawn(stderr, policy, DRAIN)
                .map_err(|source| self.capture_source_error(source))?,
        );
        Ok(())
    }

    fn capture_error(&self, message: &str) -> ReleaseError {
        self.capture_source_error(std::io::Error::other(message))
    }

    fn capture_source_error(&self, source: std::io::Error) -> ReleaseError {
        ReleaseError::CommandCapture {
            program: self.program.clone(),
            source,
        }
    }

    fn leader_exited(&self, timeout: Duration) -> Result<bool, ReleaseError> {
        self.leader
            .wait_for_exit(timeout)
            .map_err(|source| ReleaseError::CommandIo {
                program: self.program.clone(),
                source,
            })
    }

    fn output_exceeded(&self) -> bool {
        self.stdout.as_ref().is_some_and(OutputReader::exceeded)
            || self.stderr.as_ref().is_some_and(OutputReader::exceeded)
    }

    /// Kills the group while the leader is still unreaped, reaps the leader,
    /// and lets the readers drain under their grace.
    fn close_group(&mut self) -> Result<(), ReleaseError> {
        let closed = self
            .leader
            .kill_group_and_reap()
            .map_err(|source| ReleaseError::CommandIo {
                program: self.program.clone(),
                source,
            });
        self.stop_readers();
        closed.map(drop)
    }

    fn stop_readers(&self) {
        for reader in [&self.stdout, &self.stderr].into_iter().flatten() {
            reader.stop();
        }
    }

    fn join_readers(&mut self) -> Result<(CapturedStream, CapturedStream), ReleaseError> {
        self.stop_readers();
        let stdout = self
            .stdout
            .take()
            .ok_or_else(|| self.capture_error("child stdout reader was not started"))?;
        let stderr = self
            .stderr
            .take()
            .ok_or_else(|| self.capture_error("child stderr reader was not started"))?;
        let stdout = stdout.join();
        let stderr = stderr.join();
        match (stdout, stderr) {
            (Ok(stdout), Ok(stderr)) => Ok((stdout, stderr)),
            (Err(source), _) | (_, Err(source)) => Err(self.capture_source_error(source)),
        }
    }

    const fn status(&self) -> Option<ExitStatus> {
        self.leader.status()
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        drop(self.leader.kill_group_and_reap());
        self.stop_readers();
        for reader in [self.stdout.take(), self.stderr.take()]
            .into_iter()
            .flatten()
        {
            drop(reader.join());
        }
    }
}

fn require_success(program: &str, status: ExitStatus, stderr: &[u8]) -> Result<(), ReleaseError> {
    if status.success() {
        return Ok(());
    }
    Err(ReleaseError::CommandFailed {
        program: program.to_string(),
        status: status.to_string(),
        stderr: diagnostic_prefix(stderr),
    })
}

/// A bounded, lossy, trimmed prefix of a failed command's stderr.
fn diagnostic_prefix(stderr: &[u8]) -> String {
    let kept = stderr
        .get(..stderr.len().min(DIAGNOSTIC_PREFIX_BYTES))
        .unwrap_or(stderr);
    let mut diagnostic = String::from_utf8_lossy(kept).trim().to_string();
    if kept.len() < stderr.len() {
        diagnostic.push_str("\n[stderr truncated]");
    }
    diagnostic
}

#[cfg(test)]
mod tests;
