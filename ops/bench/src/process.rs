use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::config::COMMAND_TIMEOUT;
use crate::error::BenchError;

const POLL_INTERVAL: Duration = Duration::from_millis(5);
const MAX_AFFINITY_PASSES: usize = 8;
const MAX_CHILD_TASKS: usize = 4_096;
const BASELINE_CAPTURE_LIMIT_BYTES: usize = 8 << 20;
const DIAGNOSTIC_PREFIX_BYTES: usize = 4 << 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutputPolicy {
    Capture { maximum_bytes: usize },
    Discard,
}

impl OutputPolicy {
    const fn maximum_bytes(self) -> Option<usize> {
        match self {
            Self::Capture { maximum_bytes } => Some(maximum_bytes),
            Self::Discard => None,
        }
    }
}

impl From<usize> for OutputPolicy {
    fn from(maximum_bytes: usize) -> Self {
        Self::Capture { maximum_bytes }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProcessEnvironment {
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
pub(crate) struct ProcessLimits {
    pub(crate) stdin_bytes: usize,
    pub(crate) stdout: OutputPolicy,
    pub(crate) stderr: OutputPolicy,
    pub(crate) regular_file_bytes: Option<u64>,
    pub(crate) timeout: Duration,
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessRequest {
    pub(crate) program: PathBuf,
    pub(crate) args: Vec<OsString>,
    pub(crate) stdin: Vec<u8>,
    pub(crate) working_directory: PathBuf,
    pub(crate) environment: ProcessEnvironment,
    pub(crate) affinity_cpu: Option<usize>,
    pub(crate) limits: ProcessLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProcessResult {
    pub(crate) status: Option<i32>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) parent_observed_peak_rss_bytes: Option<u64>,
    pub(crate) wall_elapsed: Duration,
}

pub(crate) type ProcessOutput = ProcessResult;

pub(crate) fn run_process(
    program: &Path,
    args: &[OsString],
    stdin: &[u8],
    working_dir: &Path,
    capture_stdout: bool,
) -> Result<ProcessOutput, BenchError> {
    run_bounded_process(&ProcessRequest {
        program: program.to_path_buf(),
        args: args.to_vec(),
        stdin: stdin.to_vec(),
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

pub(crate) fn check_success(program: &Path, output: &ProcessOutput) -> Result<(), BenchError> {
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

pub(crate) fn run_bounded_process(request: &ProcessRequest) -> Result<ProcessResult, ProcessError> {
    let cancellation = ProcessCancellation::for_signals()?;
    run_bounded_process_with_cancellation(request, &cancellation)
}

fn run_bounded_process_with_cancellation(
    request: &ProcessRequest,
    cancellation: &ProcessCancellation,
) -> Result<ProcessResult, ProcessError> {
    let wall_started = Instant::now();
    ensure_linux()?;
    if request.stdin.len() > request.limits.stdin_bytes {
        return Err(ProcessError::StdinLimit {
            actual: request.stdin.len(),
            maximum: request.limits.stdin_bytes,
        });
    }
    if request.limits.timeout.is_zero() {
        return Err(ProcessError::ZeroTimeout);
    }
    if request.limits.regular_file_bytes == Some(0) {
        return Err(ProcessError::ZeroFileLimit);
    }
    let deadline = wall_started
        .checked_add(request.limits.timeout)
        .ok_or(ProcessError::DeadlineOverflow)?;
    if cancellation.is_cancelled() {
        return Err(ProcessError::Interrupted(Box::new(InterruptedError {
            program: request.program.clone(),
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdout_diagnostic: Box::default(),
            stderr_diagnostic: Box::default(),
        })));
    }

    let mut command = std::process::Command::new(&request.program);
    command
        .args(&request.args)
        .current_dir(&request.working_directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let ProcessEnvironment::ClearAndSet(environment) = &request.environment {
        command
            .env_clear()
            .envs(environment.iter().map(|(key, value)| (key, value)));
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let child = command.spawn().map_err(|source| ProcessError::Spawn {
        program: request.program.clone(),
        source,
    })?;
    let mut child = ManagedChild::new(child, request.program.clone());
    if let Some(cpu) = request.affinity_cpu
        && let Err(source) = set_child_affinity(child.id(), cpu)
    {
        child.close_group()?;
        return Err(ProcessError::SetAffinity {
            program: request.program.clone(),
            cpu,
            source,
        });
    }
    if let Some(maximum) = request.limits.regular_file_bytes
        && let Err(source) = set_child_file_limit(child.id(), maximum)
    {
        child.close_group()?;
        return Err(ProcessError::SetFileLimit {
            program: request.program.clone(),
            maximum,
            source,
        });
    }
    let pid = child.id();
    child.start_io(request)?;
    let mut status = None;
    let mut peak_rss = None;

    loop {
        if let Some(rss) = process_rss_bytes(pid)? {
            peak_rss = Some(peak_rss.map_or(rss, |peak: u64| peak.max(rss)));
        }
        if status.is_none() {
            status = child.try_wait()?;
            if status.is_some() {
                child.close_group()?;
            }
        }
        if cancellation.is_cancelled() {
            child.close_group()?;
            let captured = child.join_io_after_termination()?;
            let (stdout_diagnostic, stderr_diagnostic) = diagnostics(&captured);
            return Err(ProcessError::Interrupted(Box::new(InterruptedError {
                program: request.program.clone(),
                stdout: captured.stdout,
                stderr: captured.stderr,
                stdout_diagnostic,
                stderr_diagnostic,
            })));
        }
        if child.stdout_exceeded()? || child.stderr_exceeded()? {
            let stream = if child.stdout_exceeded()? {
                "stdout"
            } else {
                "stderr"
            };
            let policy = if stream == "stdout" {
                request.limits.stdout
            } else {
                request.limits.stderr
            };
            let maximum = policy
                .maximum_bytes()
                .ok_or(ProcessError::DiscardExceeded { stream })?;
            child.close_group()?;
            let captured = child.join_io_after_termination()?;
            let (stdout_diagnostic, stderr_diagnostic) = diagnostics(&captured);
            return Err(ProcessError::OutputLimit(Box::new(OutputLimitError {
                program: request.program.clone(),
                stream,
                maximum,
                stdout: captured.stdout,
                stderr: captured.stderr,
                stdout_diagnostic,
                stderr_diagnostic,
            })));
        }
        let now = Instant::now();
        if now >= deadline {
            child.close_group()?;
            let captured = child.join_io_after_termination()?;
            let (stdout_diagnostic, stderr_diagnostic) = diagnostics(&captured);
            return Err(ProcessError::TimedOut(Box::new(TimedOutError {
                program: request.program.clone(),
                timeout: request.limits.timeout,
                stdout: captured.stdout,
                stderr: captured.stderr,
                stdout_diagnostic,
                stderr_diagnostic,
            })));
        }
        if status.is_some() && child.io_finished()? {
            break;
        }
        std::thread::sleep(POLL_INTERVAL.min(deadline.duration_since(now)));
    }

    let captured = child.join_io()?;
    let wall_elapsed = wall_started.elapsed();
    if wall_elapsed > request.limits.timeout {
        let (stdout_diagnostic, stderr_diagnostic) = diagnostics(&captured);
        return Err(ProcessError::TimedOut(Box::new(TimedOutError {
            program: request.program.clone(),
            timeout: request.limits.timeout,
            stdout: captured.stdout,
            stderr: captured.stderr,
            stdout_diagnostic,
            stderr_diagnostic,
        })));
    }
    let status = status.ok_or_else(|| ProcessError::MissingStatus(request.program.clone()))?;
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if let Some(maximum) = request.limits.regular_file_bytes
            && status.signal() == Some(signal_hook::consts::signal::SIGXFSZ)
        {
            let (stdout_diagnostic, stderr_diagnostic) = diagnostics(&captured);
            return Err(ProcessError::FileLimit(Box::new(FileLimitError {
                program: request.program.clone(),
                maximum,
                stdout: captured.stdout,
                stderr: captured.stderr,
                stdout_diagnostic,
                stderr_diagnostic,
            })));
        }
    }
    Ok(ProcessResult {
        status: status.code(),
        stdout: captured.stdout,
        stderr: captured.stderr,
        parent_observed_peak_rss_bytes: peak_rss,
        wall_elapsed,
    })
}

struct ManagedChild {
    child: std::process::Child,
    program: PathBuf,
    stdin: Option<Writer>,
    stdout: Option<OutputReader>,
    stderr: Option<OutputReader>,
    reaped: bool,
    group_closed: bool,
}

impl ManagedChild {
    fn new(child: std::process::Child, program: PathBuf) -> Self {
        Self {
            child,
            program,
            stdin: None,
            stdout: None,
            stderr: None,
            reaped: false,
            group_closed: false,
        }
    }

    fn id(&self) -> u32 {
        self.child.id()
    }

    fn start_io(&mut self, request: &ProcessRequest) -> Result<(), ProcessError> {
        let stdout = self
            .child
            .stdout
            .take()
            .ok_or(ProcessError::MissingPipe("stdout"))?;
        self.stdout = Some(spawn_reader(stdout, request.limits.stdout));
        let stderr = self
            .child
            .stderr
            .take()
            .ok_or(ProcessError::MissingPipe("stderr"))?;
        self.stderr = Some(spawn_reader(stderr, request.limits.stderr));
        let stdin = self
            .child
            .stdin
            .take()
            .ok_or(ProcessError::MissingPipe("stdin"))?;
        self.stdin = Some(spawn_writer(stdin, request.stdin.clone()));
        Ok(())
    }

    fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, ProcessError> {
        let status = self.child.try_wait().map_err(|source| ProcessError::Wait {
            program: self.program.clone(),
            source,
        })?;
        if status.is_some() {
            self.reaped = true;
        }
        Ok(status)
    }

    fn close_group(&mut self) -> Result<(), ProcessError> {
        if self.group_closed {
            return Ok(());
        }
        kill_process_group(&mut self.child, &self.program)?;
        if !self.reaped {
            self.child.wait().map_err(|source| ProcessError::Wait {
                program: self.program.clone(),
                source,
            })?;
            self.reaped = true;
        }
        self.group_closed = true;
        Ok(())
    }

    fn stdout_exceeded(&self) -> Result<bool, ProcessError> {
        self.stdout
            .as_ref()
            .map(OutputReader::exceeded)
            .ok_or(ProcessError::MissingPipe("stdout"))
    }

    fn stderr_exceeded(&self) -> Result<bool, ProcessError> {
        self.stderr
            .as_ref()
            .map(OutputReader::exceeded)
            .ok_or(ProcessError::MissingPipe("stderr"))
    }

    fn io_finished(&self) -> Result<bool, ProcessError> {
        Ok(self
            .stdin
            .as_ref()
            .ok_or(ProcessError::MissingPipe("stdin"))?
            .is_finished()
            && self
                .stdout
                .as_ref()
                .ok_or(ProcessError::MissingPipe("stdout"))?
                .is_finished()
            && self
                .stderr
                .as_ref()
                .ok_or(ProcessError::MissingPipe("stderr"))?
                .is_finished())
    }

    fn join_io(&mut self) -> Result<JoinedOutput, ProcessError> {
        self.take_and_join_io(false)
    }

    fn join_io_after_termination(&mut self) -> Result<JoinedOutput, ProcessError> {
        self.take_and_join_io(true)
    }

    fn take_and_join_io(&mut self, ignore_stdin_error: bool) -> Result<JoinedOutput, ProcessError> {
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
        join_all(&self.program, stdin, stdout, stderr, ignore_stdin_error)
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        if !self.group_closed {
            drop(kill_process_group(&mut self.child, &self.program));
            if !self.reaped {
                drop(self.child.wait());
            }
        }
        if let Some(stdin) = self.stdin.take() {
            drop(join_writer(&self.program, stdin));
        }
        if let Some(stdout) = self.stdout.take() {
            drop(join_reader(&self.program, stdout));
        }
        if let Some(stderr) = self.stderr.take() {
            drop(join_reader(&self.program, stderr));
        }
    }
}

struct JoinedOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn diagnostics(output: &JoinedOutput) -> (Box<str>, Box<str>) {
    (
        diagnostic_prefix(&output.stdout),
        diagnostic_prefix(&output.stderr),
    )
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

fn join_all(
    program: &std::path::Path,
    stdin: Writer,
    stdout: OutputReader,
    stderr: OutputReader,
    ignore_stdin_error: bool,
) -> Result<JoinedOutput, ProcessError> {
    let writer = join_writer(program, stdin);
    let stdout = join_reader(program, stdout)?;
    let stderr = join_reader(program, stderr)?;
    if !ignore_stdin_error {
        writer?;
    }
    Ok(JoinedOutput {
        stdout: stdout.bytes,
        stderr: stderr.bytes,
    })
}

#[derive(Clone)]
struct ProcessCancellation {
    cancelled: Arc<AtomicBool>,
}

impl ProcessCancellation {
    fn for_signals() -> Result<Self, ProcessError> {
        static CANCELLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
        static INSTALLED: OnceLock<Result<(), String>> = OnceLock::new();
        let cancelled = Arc::clone(CANCELLED.get_or_init(|| Arc::new(AtomicBool::new(false))));
        let installed = INSTALLED.get_or_init(|| {
            for signal in [
                signal_hook::consts::signal::SIGINT,
                signal_hook::consts::signal::SIGTERM,
            ] {
                signal_hook::flag::register(signal, Arc::clone(&cancelled))
                    .map_err(|source| source.to_string())?;
            }
            Ok(())
        });
        if let Err(reason) = installed {
            return Err(ProcessError::InstallSignals(reason.clone()));
        }
        Ok(Self { cancelled })
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    #[cfg(test)]
    fn for_test() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    #[cfg(test)]
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

type Writer = JoinHandle<Result<(), std::io::Error>>;

fn spawn_writer(mut pipe: std::process::ChildStdin, bytes: Vec<u8>) -> Writer {
    std::thread::spawn(move || pipe.write_all(&bytes))
}

fn join_writer(program: &std::path::Path, writer: Writer) -> Result<(), ProcessError> {
    match writer.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(source)) => Err(ProcessError::WriteStdin {
            program: program.to_path_buf(),
            source,
        }),
        Err(_) => Err(ProcessError::WriteStdin {
            program: program.to_path_buf(),
            source: std::io::Error::other("stdin writer thread panicked"),
        }),
    }
}

struct CapturedOutput {
    bytes: Vec<u8>,
}

struct OutputReader {
    handle: JoinHandle<Result<CapturedOutput, std::io::Error>>,
    exceeded: Arc<AtomicBool>,
}

impl OutputReader {
    fn exceeded(&self) -> bool {
        self.exceeded.load(Ordering::Acquire)
    }

    fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }
}

fn spawn_reader(mut pipe: impl Read + Send + 'static, policy: OutputPolicy) -> OutputReader {
    let exceeded = Arc::new(AtomicBool::new(false));
    let reader_exceeded = Arc::clone(&exceeded);
    let handle = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let count = pipe.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            let chunk = buffer
                .get(..count)
                .ok_or_else(|| std::io::Error::other("pipe read exceeded buffer bounds"))?;
            if let OutputPolicy::Capture { maximum_bytes } = policy {
                let remaining = maximum_bytes.saturating_sub(bytes.len());
                if count <= remaining {
                    bytes.extend_from_slice(chunk);
                } else {
                    let kept = chunk
                        .get(..remaining)
                        .ok_or_else(|| std::io::Error::other("pipe keep exceeded buffer bounds"))?;
                    bytes.extend_from_slice(kept);
                    reader_exceeded.store(true, Ordering::Release);
                }
            }
        }
        Ok(CapturedOutput { bytes })
    });
    OutputReader { handle, exceeded }
}

fn join_reader(
    program: &std::path::Path,
    reader: OutputReader,
) -> Result<CapturedOutput, ProcessError> {
    match reader.handle.join() {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(source)) => Err(ProcessError::Capture {
            program: program.to_path_buf(),
            source,
        }),
        Err(_) => Err(ProcessError::Capture {
            program: program.to_path_buf(),
            source: std::io::Error::other("output reader thread panicked"),
        }),
    }
}

fn kill_process_group(
    child: &mut std::process::Child,
    program: &std::path::Path,
) -> Result<(), ProcessError> {
    let raw_pid = i32::try_from(child.id()).map_err(|_| ProcessError::InvalidPid(child.id()))?;
    let pid =
        rustix::process::Pid::from_raw(raw_pid).ok_or(ProcessError::InvalidPid(child.id()))?;
    if let Err(source) = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL)
        && source != rustix::io::Errno::SRCH
    {
        return Err(ProcessError::Kill {
            program: program.to_path_buf(),
            source: source.into(),
        });
    }
    Ok(())
}

fn set_child_affinity(pid: u32, cpu: usize) -> Result<(), std::io::Error> {
    if cpu >= rustix::thread::CpuSet::MAX_CPU {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("CPU {cpu} exceeds the supported affinity mask"),
        ));
    }
    let mut set = rustix::thread::CpuSet::new();
    set.set(cpu);
    let leader = process_id(pid)?;

    // Pin the leader first so newly created threads inherit the requested mask,
    // then close the post-spawn race by pinning threads that already exist.
    set_task_affinity(leader, &set)?;
    for _ in 0..MAX_AFFINITY_PASSES {
        let task_ids = child_task_ids(pid)?;
        for task_id in task_ids {
            if let Err(source) = rustix::thread::sched_setaffinity(Some(task_id), &set)
                && source != rustix::io::Errno::SRCH
            {
                return Err(source.into());
            }
        }

        let mut complete = true;
        for task_id in child_task_ids(pid)? {
            match rustix::thread::sched_getaffinity(Some(task_id)) {
                Ok(actual) if affinity_is_singleton(&actual, cpu) => {}
                Ok(_) | Err(rustix::io::Errno::SRCH) => complete = false,
                Err(source) => return Err(source.into()),
            }
        }
        if complete {
            return Ok(());
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::WouldBlock,
        format!("child process {pid} did not converge to CPU {cpu} affinity"),
    ))
}

fn process_id(pid: u32) -> Result<rustix::process::Pid, std::io::Error> {
    let raw_pid = i32::try_from(pid).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("child process id {pid} exceeds the supported Linux pid range"),
        )
    })?;
    rustix::process::Pid::from_raw(raw_pid).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("child process id {pid} is invalid"),
        )
    })
}

fn set_task_affinity(
    task_id: rustix::process::Pid,
    set: &rustix::thread::CpuSet,
) -> Result<(), std::io::Error> {
    rustix::thread::sched_setaffinity(Some(task_id), set).map_err(Into::into)
}

fn child_task_ids(pid: u32) -> Result<Vec<rustix::process::Pid>, std::io::Error> {
    let path = PathBuf::from(format!("/proc/{pid}/task"));
    let entries = std::fs::read_dir(&path)?;
    let mut task_ids = Vec::new();
    for entry in entries {
        if task_ids.len() == MAX_CHILD_TASKS {
            return Err(std::io::Error::other(format!(
                "child process {pid} exceeds the {MAX_CHILD_TASKS}-task affinity limit"
            )));
        }
        let name = entry?.file_name().into_string().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("child task directory {path:?} contains a non-UTF-8 entry"),
            )
        })?;
        let raw_task_id = name.parse::<i32>().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("child task directory {path:?} contains nonnumeric entry {name:?}"),
            )
        })?;
        let task_id = rustix::process::Pid::from_raw(raw_task_id).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("child task directory {path:?} contains invalid task id {name:?}"),
            )
        })?;
        task_ids.push(task_id);
    }
    if task_ids.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("child process {pid} has no visible tasks"),
        ));
    }
    Ok(task_ids)
}

fn affinity_is_singleton(set: &rustix::thread::CpuSet, cpu: usize) -> bool {
    (0..rustix::thread::CpuSet::MAX_CPU)
        .all(|candidate| set.is_set(candidate) == (candidate == cpu))
}

fn set_child_file_limit(pid: u32, requested_maximum: u64) -> Result<(), std::io::Error> {
    let raw_pid = i32::try_from(pid).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("child process id {pid} exceeds the supported Linux pid range"),
        )
    })?;
    let pid = rustix::process::Pid::from_raw(raw_pid).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("child process id {pid} is invalid"),
        )
    })?;
    let inherited = rustix::process::getrlimit(rustix::process::Resource::Fsize);
    let maximum = [
        Some(requested_maximum),
        inherited.current,
        inherited.maximum,
    ]
    .into_iter()
    .flatten()
    .min()
    .ok_or_else(|| std::io::Error::other("file-size limit has no finite bound"))?;
    rustix::process::prlimit(
        Some(pid),
        rustix::process::Resource::Fsize,
        rustix::process::Rlimit {
            current: Some(maximum),
            maximum: Some(maximum),
        },
    )
    .map(|_| ())
    .map_err(Into::into)
}

fn process_rss_bytes(pid: u32) -> Result<Option<u64>, ProcessError> {
    let path = PathBuf::from(format!("/proc/{pid}/status"));
    let status = match std::fs::read_to_string(&path) {
        Ok(status) => status,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(ProcessError::ReadRss { path, source }),
    };
    let Some(line) = status.lines().find(|line| line.starts_with("VmRSS:")) else {
        // A reaped or zombie process can retain /proc/<pid>/status after VmRSS disappears.
        return Ok(None);
    };
    let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
    let ["VmRSS:", kib, "kB"] = fields.as_slice() else {
        return Err(ProcessError::MalformedRss(path));
    };
    let kib = kib
        .parse::<u64>()
        .map_err(|_| ProcessError::MalformedRss(path.clone()))?;
    kib.checked_mul(1024)
        .map(Some)
        .ok_or(ProcessError::RssOverflow(path))
}

fn ensure_linux() -> Result<(), ProcessError> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(ProcessError::UnsupportedHost)
    }
}

#[derive(Debug, Error)]
#[error(
    "{program} {stream} exceeded {maximum} bytes\nstdout (bounded prefix):\n{stdout_diagnostic}\nstderr (bounded prefix):\n{stderr_diagnostic}"
)]
pub(crate) struct OutputLimitError {
    pub(crate) program: PathBuf,
    pub(crate) stream: &'static str,
    pub(crate) maximum: usize,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) stdout_diagnostic: Box<str>,
    pub(crate) stderr_diagnostic: Box<str>,
}

#[derive(Debug, Error)]
#[error(
    "{program} timed out after {timeout:?}\nstdout (bounded prefix):\n{stdout_diagnostic}\nstderr (bounded prefix):\n{stderr_diagnostic}"
)]
pub(crate) struct TimedOutError {
    pub(crate) program: PathBuf,
    pub(crate) timeout: Duration,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) stdout_diagnostic: Box<str>,
    pub(crate) stderr_diagnostic: Box<str>,
}

#[derive(Debug, Error)]
#[error(
    "{program} exceeded the regular-file limit of {maximum} bytes\nstdout (bounded prefix):\n{stdout_diagnostic}\nstderr (bounded prefix):\n{stderr_diagnostic}"
)]
pub(crate) struct FileLimitError {
    pub(crate) program: PathBuf,
    pub(crate) maximum: u64,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) stdout_diagnostic: Box<str>,
    pub(crate) stderr_diagnostic: Box<str>,
}

#[derive(Debug, Error)]
#[error(
    "{program} was interrupted\nstdout (bounded prefix):\n{stdout_diagnostic}\nstderr (bounded prefix):\n{stderr_diagnostic}"
)]
pub(crate) struct InterruptedError {
    pub(crate) program: PathBuf,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) stdout_diagnostic: Box<str>,
    pub(crate) stderr_diagnostic: Box<str>,
}

#[derive(Debug, Error)]
pub(crate) enum ProcessError {
    #[error("performance qualification process control requires Linux")]
    UnsupportedHost,
    #[error("process stdin is {actual} bytes, exceeding {maximum}")]
    StdinLimit { actual: usize, maximum: usize },
    #[error("process timeout must be positive")]
    ZeroTimeout,
    #[error("process regular-file limit must be positive when configured")]
    ZeroFileLimit,
    #[error("process timeout exceeds the monotonic clock range")]
    DeadlineOverflow,
    #[error("failed to install qualification signal handlers: {0}")]
    InstallSignals(String),
    #[error("failed to spawn {program}: {source}")]
    Spawn {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to pin {program} to CPU {cpu}: {source}")]
    SetAffinity {
        program: PathBuf,
        cpu: usize,
        source: std::io::Error,
    },
    #[error("failed to limit regular files written by {program} to {maximum} bytes: {source}")]
    SetFileLimit {
        program: PathBuf,
        maximum: u64,
        source: std::io::Error,
    },
    #[error("spawned process is missing its piped {0}")]
    MissingPipe(&'static str),
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
    #[error(transparent)]
    OutputLimit(Box<OutputLimitError>),
    #[error(transparent)]
    TimedOut(Box<TimedOutError>),
    #[error(transparent)]
    FileLimit(Box<FileLimitError>),
    #[error(transparent)]
    Interrupted(Box<InterruptedError>),
    #[error("child process id {0} exceeds the supported Linux pid range")]
    InvalidPid(u32),
    #[error("{0} completed without an observable exit status")]
    MissingStatus(PathBuf),
    #[error("failed to terminate the process group for {program}: {source}")]
    Kill {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read process RSS from {path}: {source}")]
    ReadRss {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("process RSS in {0} is malformed")]
    MalformedRss(PathBuf),
    #[error("process RSS in {0} overflows u64 bytes")]
    RssOverflow(PathBuf),
}

#[cfg(test)]
mod tests;
