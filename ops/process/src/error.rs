//! Supervisor results and errors.

use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

const DIAGNOSTIC_PREFIX_BYTES: usize = 4 << 10;

/// How a supervised program terminated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessExit {
    /// The program exited with this status code.
    Exited(i32),
    /// The program was terminated by this signal number.
    Signaled(i32),
}

impl ProcessExit {
    #[must_use]
    pub const fn success(self) -> bool {
        matches!(self, Self::Exited(0))
    }

    /// The exit code, when the program exited rather than being signalled.
    #[must_use]
    pub const fn code(self) -> Option<i32> {
        match self {
            Self::Exited(code) => Some(code),
            Self::Signaled(_) => None,
        }
    }

    #[must_use]
    pub const fn signal(self) -> Option<i32> {
        match self {
            Self::Exited(_) => None,
            Self::Signaled(signal) => Some(signal),
        }
    }

    pub(crate) fn from_status(status: std::process::ExitStatus) -> Option<Self> {
        use std::os::unix::process::ExitStatusExt as _;

        status
            .code()
            .map(Self::Exited)
            .or_else(|| status.signal().map(Self::Signaled))
    }
}

impl std::fmt::Display for ProcessExit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exited(code) => write!(formatter, "{code}"),
            Self::Signaled(signal) => write!(formatter, "signal {signal}"),
        }
    }
}

/// The output a failed run captured before the supervisor stopped it.
#[derive(Debug)]
pub struct CapturedFailure {
    pub program: PathBuf,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_diagnostic: Box<str>,
    pub stderr_diagnostic: Box<str>,
    /// Standard streams that a process outside the killed group still held
    /// open when the supervisor stopped waiting for them.
    pub held_streams: Vec<&'static str>,
}

impl CapturedFailure {
    pub(crate) fn new(
        program: PathBuf,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        held_streams: Vec<&'static str>,
    ) -> Box<Self> {
        Box::new(Self {
            stdout_diagnostic: diagnostic_prefix(&stdout),
            stderr_diagnostic: diagnostic_prefix(&stderr),
            program,
            stdout,
            stderr,
            held_streams,
        })
    }

    pub(crate) fn empty(program: PathBuf) -> Box<Self> {
        Self::new(program, Vec::new(), Vec::new(), Vec::new())
    }
}

impl std::fmt::Display for CapturedFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.held_streams.is_empty() {
            write!(
                formatter,
                "\nprocesses outside its process group still held its {} pipe",
                self.held_streams.join(" and ")
            )?;
        }
        write!(
            formatter,
            "\nstdout (bounded prefix):\n{}\nstderr (bounded prefix):\n{}",
            self.stdout_diagnostic, self.stderr_diagnostic
        )
    }
}

fn diagnostic_prefix(bytes: &[u8]) -> Box<str> {
    let kept = bytes
        .get(..bytes.len().min(DIAGNOSTIC_PREFIX_BYTES))
        .unwrap_or(bytes);
    let mut diagnostic = String::from_utf8_lossy(kept).into_owned();
    if kept.len() < bytes.len() {
        diagnostic.push_str("\n[diagnostic truncated]");
    }
    diagnostic.into_boxed_str()
}

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("bounded process supervision requires Linux")]
    UnsupportedHost,
    #[error("failed to measure {program} through the launcher: {reason}")]
    Launch { program: PathBuf, reason: Box<str> },
    #[error("process stdin is {actual} bytes, exceeding {maximum}")]
    StdinLimit { actual: usize, maximum: usize },
    #[error("process timeout must be positive")]
    ZeroTimeout,
    #[error("process regular-file limit must be positive when configured")]
    ZeroFileLimit,
    #[error(
        "{program} requests a regular-file limit, which only a launched run can apply before the program starts"
    )]
    FileLimitRequiresLauncher { program: PathBuf },
    #[error(
        "{program} requests a {maximum}-byte regular-file limit, below the launcher's {minimum}-byte report bound"
    )]
    FileLimitBelowLauncherReport {
        program: PathBuf,
        maximum: u64,
        minimum: u64,
    },
    #[error("process timeout exceeds the monotonic clock range")]
    DeadlineOverflow,
    #[error("failed to install process-cancellation signal handlers: {0}")]
    InstallSignals(String),
    #[error("failed to spawn {program}: {source}")]
    Spawn {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to poll completion of {program}: {source}")]
    PollCompletion {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to pin {program} to CPU {cpu}: {source}")]
    SetAffinity {
        program: PathBuf,
        cpu: usize,
        source: std::io::Error,
    },
    #[error("spawned process is missing its piped {0}")]
    MissingPipe(&'static str),
    #[error("failed to start pipe I/O for {program}: {source}")]
    StartIo {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("discarded process stream {stream} unexpectedly exceeded an output limit")]
    DiscardExceeded { stream: &'static str },
    #[error("failed to wait for {program}: {source}")]
    Wait {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write stdin for {program}: {source}")]
    WriteStdin {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to capture output from {program}: {source}")]
    Capture {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("{program} {stream} exceeded {maximum} bytes{failure}", program = failure.program.display())]
    OutputLimit {
        stream: &'static str,
        maximum: usize,
        failure: Box<CapturedFailure>,
    },
    #[error("{program} timed out after {timeout:?}{failure}", program = failure.program.display())]
    TimedOut {
        timeout: Duration,
        failure: Box<CapturedFailure>,
    },
    #[error(
        "{program} exceeded the regular-file limit of {maximum} bytes{failure}",
        program = failure.program.display()
    )]
    FileLimit {
        maximum: u64,
        failure: Box<CapturedFailure>,
    },
    #[error("{program} was interrupted{failure}", program = .0.program.display(), failure = .0)]
    Interrupted(Box<CapturedFailure>),
    #[error(
        "{program} exited but its output could not be drained completely{failure}",
        program = .0.program.display(),
        failure = .0
    )]
    PipesHeld(Box<CapturedFailure>),
    #[error("{program} completed without an exit status: {source}")]
    MissingStatus {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("completion time for {0} preceded its start time")]
    CompletionClock(PathBuf),
    #[error("failed to terminate the process group for {program}: {source}")]
    Kill {
        program: PathBuf,
        source: std::io::Error,
    },
}

impl ProcessError {
    /// The output captured before the failure, when the failure captured any.
    #[must_use]
    pub fn captured(&self) -> Option<&CapturedFailure> {
        match self {
            Self::OutputLimit { failure, .. }
            | Self::TimedOut { failure, .. }
            | Self::FileLimit { failure, .. }
            | Self::Interrupted(failure)
            | Self::PipesHeld(failure) => Some(failure),
            _ => None,
        }
    }
}
