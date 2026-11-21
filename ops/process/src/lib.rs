//! Bounded child-process supervisor shared by Stab's operational tools.
//!
//! Every supervised program runs in its own process group with piped standard
//! streams, a wall-clock deadline, bounded stdin and output capture, optional
//! CPU pinning, and sticky SIGINT/SIGTERM cancellation. Any limit violation,
//! cancellation, supervisor exit, or leader exit kills the whole group while
//! the leader is still unreaped. Pipe I/O is nonblocking and stoppable: once
//! the group is killed, readers drain what is readable and then give a process
//! outside the group a short grace to close a pipe it inherited, so such a
//! process is reported instead of holding the supervisor past its deadline.
//! [`ProcessLaunch::Measured`] runs the program through a fresh launcher, which
//! also applies the regular-file size limit before the program starts.
//!
//! The lower layer ([`pipe`], [`group`], [`Cancellation`]) is public so other
//! supervisors with different publication contracts can reuse it.
//!
//! The supervisor requires Linux: it waits on a pidfd and reaps with `wait4`.

#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact diagnostics"
    )
)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

mod cancellation;
mod error;
pub mod group;
mod launcher;
mod limits;
pub mod pipe;

pub use cancellation::Cancellation;
pub use error::{CapturedFailure, ProcessError, ProcessExit};
pub use launcher::{Launcher, run_launcher};
pub use pipe::OutputPolicy;

use group::ProcessGroupLeader;
use launcher::MeasuredLaunch;
use limits::set_child_affinity;
use pipe::{CapturedStream, DrainPolicy, InputWriter, OutputReader, StreamEnd, WriteEnd};

const POLL_INTERVAL: Duration = Duration::from_millis(5);
/// How long pipe ends stay open for processes outside the killed group.
const PIPE_DRAIN_GRACE: Duration = Duration::from_millis(500);
const DRAIN: DrainPolicy = DrainPolicy::StopAfterGrace(PIPE_DRAIN_GRACE);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessEnvironment {
    Inherit,
    ClearAndSet(Vec<(OsString, OsString)>),
}

impl From<Vec<(OsString, OsString)>> for ProcessEnvironment {
    fn from(environment: Vec<(OsString, OsString)>) -> Self {
        Self::ClearAndSet(environment)
    }
}

impl ProcessEnvironment {
    #[cfg(test)]
    fn push(&mut self, entry: (OsString, OsString)) {
        match self {
            Self::ClearAndSet(environment) => environment.push(entry),
            Self::Inherit => {
                *self = Self::ClearAndSet(vec![entry]);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessLimits {
    pub stdin_bytes: usize,
    pub stdout: OutputPolicy,
    pub stderr: OutputPolicy,
    /// Bound on every regular file the program writes. It requires
    /// [`ProcessLaunch::Measured`], whose launcher applies it before the
    /// program starts.
    pub regular_file_bytes: Option<u64>,
    pub timeout: Duration,
}

impl ProcessLimits {
    /// No stdin, both output streams captured up to `maximum_bytes`, and no
    /// regular-file limit.
    #[must_use]
    pub const fn capture(maximum_bytes: usize, timeout: Duration) -> Self {
        Self {
            stdin_bytes: 0,
            stdout: OutputPolicy::Capture { maximum_bytes },
            stderr: OutputPolicy::Capture { maximum_bytes },
            regular_file_bytes: None,
            timeout,
        }
    }
}

/// Whether a program may exit before reading all of its stdin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnreadStdin {
    /// A broken stdin pipe after the program exits is a supervisor error.
    Reject,
    /// The program may exit without reading its stdin, for example when it
    /// rejects its arguments first; its status and output stay observable.
    Allow,
}

/// How the supervisor starts the requested program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessLaunch {
    /// Spawn the program directly. Its reported peak RSS includes the
    /// supervisor's own high-water mark, so it is not a measurement.
    Direct,
    /// Spawn through a fresh launcher so the reported peak RSS and wall
    /// interval belong to the program alone.
    Measured(Launcher),
}

#[derive(Clone, Debug)]
pub struct ProcessRequest {
    pub launch: ProcessLaunch,
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub stdin: Vec<u8>,
    pub unread_stdin: UnreadStdin,
    pub working_directory: PathBuf,
    pub environment: ProcessEnvironment,
    pub affinity_cpu: Option<usize>,
    pub limits: ProcessLimits,
}

impl ProcessRequest {
    /// A directly spawned program with no stdin, the inherited environment, no
    /// CPU pinning, and `limits`.
    #[must_use]
    pub fn direct(
        program: impl Into<PathBuf>,
        args: Vec<OsString>,
        working_directory: impl Into<PathBuf>,
        limits: ProcessLimits,
    ) -> Self {
        Self {
            launch: ProcessLaunch::Direct,
            program: program.into(),
            args,
            stdin: Vec::new(),
            unread_stdin: UnreadStdin::Reject,
            working_directory: working_directory.into(),
            environment: ProcessEnvironment::Inherit,
            affinity_cpu: None,
            limits,
        }
    }

    /// Sends `stdin` and bounds stdin at exactly its length.
    #[must_use]
    pub fn with_stdin(mut self, stdin: &[u8]) -> Self {
        self.stdin = stdin.to_vec();
        self.limits.stdin_bytes = stdin.len();
        self
    }

    #[must_use]
    pub const fn with_unread_stdin(mut self, unread_stdin: UnreadStdin) -> Self {
        self.unread_stdin = unread_stdin;
        self
    }

    #[must_use]
    pub const fn with_affinity(mut self, cpu: Option<usize>) -> Self {
        self.affinity_cpu = cpu;
        self
    }

    /// Runs the program through `launcher`, bounding its regular files at
    /// `regular_file_bytes` before it starts.
    #[must_use]
    pub fn measured(mut self, launcher: Launcher, regular_file_bytes: Option<u64>) -> Self {
        self.launch = ProcessLaunch::Measured(launcher);
        self.limits.regular_file_bytes = regular_file_bytes;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessResult {
    pub status: ProcessExit,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub parent_observed_peak_rss_bytes: Option<u64>,
    pub wall_elapsed: Duration,
}

/// Runs `request` under the process-wide SIGINT/SIGTERM [`Cancellation`].
pub fn run_bounded_process(request: &ProcessRequest) -> Result<ProcessResult, ProcessError> {
    let cancellation = Cancellation::for_signals().map_err(ProcessError::InstallSignals)?;
    run_bounded_process_with_cancellation(request, &cancellation)
}

/// Runs `request`, stopping it as soon as `cancellation` is set. A request
/// whose cancellation is already set never starts.
pub fn run_bounded_process_with_cancellation(
    request: &ProcessRequest,
    cancellation: &Cancellation,
) -> Result<ProcessResult, ProcessError> {
    let wall_started = Instant::now();
    ensure_linux()?;
    validate(request)?;
    let deadline = wall_started
        .checked_add(request.limits.timeout)
        .ok_or(ProcessError::DeadlineOverflow)?;
    if cancellation.is_cancelled() {
        return Err(ProcessError::Interrupted(CapturedFailure::empty(
            request.program.clone(),
        )));
    }

    let measured = match &request.launch {
        ProcessLaunch::Direct => None,
        ProcessLaunch::Measured(_) => Some(MeasuredLaunch::prepare(request)?),
    };
    let mut child = SupervisedChild::spawn(request, measured.as_ref())?;
    // A measured launcher pins itself before spawning the program, so the
    // supervisor pins only direct spawns.
    if measured.is_none()
        && let Some(cpu) = request.affinity_cpu
        && let Err(source) = set_child_affinity(child.leader.id(), cpu)
    {
        child.close_group()?;
        return Err(ProcessError::SetAffinity {
            program: request.program.clone(),
            cpu,
            source,
        });
    }
    child.start_io(request)?;
    let mut completed = None;

    loop {
        if cancellation.is_cancelled() {
            return Err(ProcessError::Interrupted(child.terminate()?));
        }
        if let Some((stream, policy)) = child.exceeded_stream(request)? {
            let maximum = policy
                .maximum_bytes()
                .ok_or(ProcessError::DiscardExceeded { stream })?;
            return Err(ProcessError::OutputLimit {
                stream,
                maximum,
                failure: child.terminate()?,
            });
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(ProcessError::TimedOut {
                timeout: request.limits.timeout,
                failure: child.terminate()?,
            });
        }
        if completed.is_some() && child.io_finished()? {
            break;
        }
        let slice = POLL_INTERVAL.min(deadline.duration_since(now));
        if completed.is_none() {
            if child.leader_exited(slice)? {
                completed = Some(child.reap_completed()?);
            }
        } else {
            std::thread::sleep(slice);
        }
    }

    let joined = child.join_io(request.unread_stdin)?;
    // A reader can cross its limit after the last loop check and still finish
    // before the loop observes it, so the joined totals are checked again.
    for (stream, policy, total) in [
        ("stdout", request.limits.stdout, joined.stdout.total_bytes),
        ("stderr", request.limits.stderr, joined.stderr.total_bytes),
    ] {
        if let Some(maximum) = policy.maximum_bytes()
            && u64::try_from(maximum).is_ok_and(|maximum| total > maximum)
        {
            return Err(ProcessError::OutputLimit {
                stream,
                maximum,
                failure: joined.into_failure(&request.program),
            });
        }
    }
    if !joined.held_streams.is_empty() {
        return Err(ProcessError::PipesHeld(
            joined.into_failure(&request.program),
        ));
    }
    let completed = completed.ok_or_else(|| ProcessError::MissingStatus {
        program: request.program.clone(),
        source: std::io::Error::other("the supervision loop ended before the leader exited"),
    })?;
    let (status, peak_rss, wall_elapsed) = match &measured {
        None => (
            completed.status,
            Some(completed.peak_rss_bytes),
            measured_wall_elapsed(wall_started, completed.completed_at, &request.program)?,
        ),
        Some(measured) => {
            use std::os::unix::process::ExitStatusExt as _;
            let program = measured.completion(request, completed.status)?;
            (
                std::process::ExitStatus::from_raw(program.wait_status),
                Some(program.peak_rss_bytes),
                program.elapsed,
            )
        }
    };
    let status = ProcessExit::from_status(status).ok_or_else(|| ProcessError::MissingStatus {
        program: request.program.clone(),
        source: std::io::Error::other(format!("wait status {status} is neither exit nor signal")),
    })?;
    if wall_elapsed > request.limits.timeout {
        return Err(ProcessError::TimedOut {
            timeout: request.limits.timeout,
            failure: joined.into_failure(&request.program),
        });
    }
    if let Some(maximum) = request.limits.regular_file_bytes
        && status.signal() == Some(signal_hook::consts::signal::SIGXFSZ)
    {
        return Err(ProcessError::FileLimit {
            maximum,
            failure: joined.into_failure(&request.program),
        });
    }
    Ok(ProcessResult {
        status,
        stdout: joined.stdout.bytes,
        stderr: joined.stderr.bytes,
        stdout_bytes: joined.stdout.total_bytes,
        stderr_bytes: joined.stderr.total_bytes,
        parent_observed_peak_rss_bytes: peak_rss,
        wall_elapsed,
    })
}

fn validate(request: &ProcessRequest) -> Result<(), ProcessError> {
    if request.stdin.len() > request.limits.stdin_bytes {
        return Err(ProcessError::StdinLimit {
            actual: request.stdin.len(),
            maximum: request.limits.stdin_bytes,
        });
    }
    if request.limits.timeout.is_zero() {
        return Err(ProcessError::ZeroTimeout);
    }
    match (request.limits.regular_file_bytes, &request.launch) {
        (Some(0), _) => Err(ProcessError::ZeroFileLimit),
        (Some(_), ProcessLaunch::Direct) => Err(ProcessError::FileLimitRequiresLauncher {
            program: request.program.clone(),
        }),
        _ => Ok(()),
    }
}

fn measured_wall_elapsed(
    started_at: Instant,
    completed_at: Instant,
    program: &Path,
) -> Result<Duration, ProcessError> {
    completed_at
        .checked_duration_since(started_at)
        .ok_or_else(|| ProcessError::CompletionClock(program.to_path_buf()))
}

/// The supervised group and its pipe threads. Dropping it kills the group
/// before the leader is reaped and then stops and joins every pipe thread.
struct SupervisedChild {
    leader: ProcessGroupLeader,
    program: PathBuf,
    stdin: Option<InputWriter>,
    stdout: Option<OutputReader>,
    stderr: Option<OutputReader>,
}

impl SupervisedChild {
    fn spawn(
        request: &ProcessRequest,
        measured: Option<&MeasuredLaunch>,
    ) -> Result<Self, ProcessError> {
        let mut command = match &request.launch {
            ProcessLaunch::Direct => {
                let mut command = std::process::Command::new(&request.program);
                command.args(&request.args);
                command
            }
            ProcessLaunch::Measured(launcher) => {
                let mut command = std::process::Command::new(&launcher.program);
                command.args(&launcher.args);
                command
            }
        };
        command
            .current_dir(&request.working_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let ProcessEnvironment::ClearAndSet(environment) = &request.environment {
            command
                .env_clear()
                .envs(environment.iter().map(|(key, value)| (key, value)));
        }
        if let Some(measured) = measured {
            let (key, value) = measured.environment();
            command.env(key, value);
        }
        Self::spawn_command(&mut command, request.program.clone())
    }

    fn spawn_command(
        command: &mut std::process::Command,
        program: PathBuf,
    ) -> Result<Self, ProcessError> {
        let leader = ProcessGroupLeader::spawn(command).map_err(|source| ProcessError::Spawn {
            program: program.clone(),
            source,
        })?;
        Ok(Self {
            leader,
            program,
            stdin: None,
            stdout: None,
            stderr: None,
        })
    }

    fn start_io(&mut self, request: &ProcessRequest) -> Result<(), ProcessError> {
        let child = self.leader.child_mut();
        let stdout = child
            .stdout
            .take()
            .ok_or(ProcessError::MissingPipe("stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(ProcessError::MissingPipe("stderr"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or(ProcessError::MissingPipe("stdin"))?;
        let start_error = |source| ProcessError::StartIo {
            program: self.program.clone(),
            source,
        };
        self.stdout =
            Some(OutputReader::spawn(stdout, request.limits.stdout, DRAIN).map_err(start_error)?);
        self.stderr =
            Some(OutputReader::spawn(stderr, request.limits.stderr, DRAIN).map_err(start_error)?);
        self.stdin =
            Some(InputWriter::spawn(stdin, request.stdin.clone(), DRAIN).map_err(start_error)?);
        Ok(())
    }

    fn exceeded_stream(
        &self,
        request: &ProcessRequest,
    ) -> Result<Option<(&'static str, OutputPolicy)>, ProcessError> {
        if reader(self.stdout.as_ref(), "stdout")?.exceeded() {
            return Ok(Some(("stdout", request.limits.stdout)));
        }
        if reader(self.stderr.as_ref(), "stderr")?.exceeded() {
            return Ok(Some(("stderr", request.limits.stderr)));
        }
        Ok(None)
    }

    fn leader_exited(&self, timeout: Duration) -> Result<bool, ProcessError> {
        self.leader
            .wait_for_exit(timeout)
            .map_err(|source| ProcessError::PollCompletion {
                program: self.program.clone(),
                source,
            })
    }

    /// Kills the rest of the group while the exited leader still pins its
    /// group id, reaps the leader, and lets the pipes drain under their grace.
    fn reap_completed(&mut self) -> Result<group::RusageCompletion, ProcessError> {
        let completed = self
            .leader
            .kill_group_and_reap_with_rusage()
            .map_err(|source| ProcessError::Wait {
                program: self.program.clone(),
                source,
            })?;
        self.stop_io();
        Ok(completed)
    }

    /// Kills and reaps the group, then collects what the pipes captured.
    fn terminate(&mut self) -> Result<Box<CapturedFailure>, ProcessError> {
        self.close_group()?;
        let joined = self.take_and_join_io(StdinOutcome::Ignored)?;
        Ok(joined.into_failure(&self.program))
    }

    fn close_group(&mut self) -> Result<(), ProcessError> {
        let closed = self
            .leader
            .kill_group_and_reap()
            .map_err(|source| ProcessError::Kill {
                program: self.program.clone(),
                source,
            });
        self.stop_io();
        closed.map(drop)
    }

    fn stop_io(&self) {
        if let Some(stdin) = &self.stdin {
            stdin.stop();
        }
        for reader in [&self.stdout, &self.stderr].into_iter().flatten() {
            reader.stop();
        }
    }

    fn io_finished(&self) -> Result<bool, ProcessError> {
        Ok(self
            .stdin
            .as_ref()
            .ok_or(ProcessError::MissingPipe("stdin"))?
            .is_finished()
            && reader(self.stdout.as_ref(), "stdout")?.is_finished()
            && reader(self.stderr.as_ref(), "stderr")?.is_finished())
    }

    fn join_io(&mut self, unread_stdin: UnreadStdin) -> Result<JoinedOutput, ProcessError> {
        self.take_and_join_io(match unread_stdin {
            UnreadStdin::Reject => StdinOutcome::Required,
            UnreadStdin::Allow => StdinOutcome::BrokenPipeAllowed,
        })
    }

    fn take_and_join_io(
        &mut self,
        stdin_outcome: StdinOutcome,
    ) -> Result<JoinedOutput, ProcessError> {
        let stdin = self
            .stdin
            .take()
            .ok_or(ProcessError::MissingPipe("stdin"))?;
        let stdout = self
            .stdout
            .take()
            .ok_or(ProcessError::MissingPipe("stdout"))?;
        let stderr = self
            .stderr
            .take()
            .ok_or(ProcessError::MissingPipe("stderr"))?;
        join_all(&self.program, stdin, stdout, stderr, stdin_outcome)
    }
}

fn reader<'a>(
    reader: Option<&'a OutputReader>,
    stream: &'static str,
) -> Result<&'a OutputReader, ProcessError> {
    reader.ok_or(ProcessError::MissingPipe(stream))
}

impl Drop for SupervisedChild {
    fn drop(&mut self) {
        drop(self.leader.kill_group_and_reap());
        self.stop_io();
        if let Some(stdin) = self.stdin.take() {
            drop(stdin.join());
        }
        for reader in [self.stdout.take(), self.stderr.take()]
            .into_iter()
            .flatten()
        {
            drop(reader.join());
        }
    }
}

struct JoinedOutput {
    stdout: CapturedStream,
    stderr: CapturedStream,
    held_streams: Vec<&'static str>,
}

impl JoinedOutput {
    fn into_failure(self, program: &Path) -> Box<CapturedFailure> {
        CapturedFailure::new(
            program.to_path_buf(),
            self.stdout.bytes,
            self.stderr.bytes,
            self.held_streams,
        )
    }
}

/// Which stdin-writer failures still fail the joined process.
#[derive(Clone, Copy)]
enum StdinOutcome {
    Required,
    BrokenPipeAllowed,
    /// The supervisor killed the group, so a failed write is expected.
    Ignored,
}

fn join_all(
    program: &Path,
    stdin: InputWriter,
    stdout: OutputReader,
    stderr: OutputReader,
    stdin_outcome: StdinOutcome,
) -> Result<JoinedOutput, ProcessError> {
    let writer = stdin.join();
    let capture_error = |source| ProcessError::Capture {
        program: program.to_path_buf(),
        source,
    };
    let stdout = stdout.join().map_err(capture_error)?;
    let stderr = stderr.join().map_err(capture_error)?;
    let mut held_streams = Vec::new();
    match (stdin_outcome, writer) {
        (_, Ok(WriteEnd::HeldOpen)) => held_streams.push("stdin"),
        (StdinOutcome::Ignored, Err(_)) | (_, Ok(WriteEnd::Complete)) => {}
        (StdinOutcome::BrokenPipeAllowed, Err(source))
            if source.kind() == std::io::ErrorKind::BrokenPipe => {}
        (_, Err(source)) => {
            return Err(ProcessError::WriteStdin {
                program: program.to_path_buf(),
                source,
            });
        }
    }
    for (stream, captured) in [("stdout", &stdout), ("stderr", &stderr)] {
        if captured.end != StreamEnd::Eof {
            held_streams.push(stream);
        }
    }
    Ok(JoinedOutput {
        stdout,
        stderr,
        held_streams,
    })
}

fn ensure_linux() -> Result<(), ProcessError> {
    cfg!(target_os = "linux").ok_or(ProcessError::UnsupportedHost)
}

#[cfg(test)]
mod tests;
