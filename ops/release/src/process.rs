use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::{ReleaseError, cancellation::ReleaseCancellation, safe_fs};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
const READER_DRAIN_GRACE: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub(crate) struct ProcessOutput {
    pub(crate) stdout: Vec<u8>,
}

pub(crate) fn run<I, S>(
    working_directory: &Path,
    program: &OsStr,
    args: I,
    environment: &[(OsString, OsString)],
    timeout: Duration,
    output_limit: usize,
) -> Result<ProcessOutput, ReleaseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let cancellation = ReleaseCancellation::for_signals()?;
    run_with_cancellation(
        working_directory,
        program,
        args,
        environment,
        timeout,
        output_limit,
        &cancellation,
    )
}

fn run_with_cancellation<I, S>(
    working_directory: &Path,
    program: &OsStr,
    args: I,
    environment: &[(OsString, OsString)],
    timeout: Duration,
    output_limit: usize,
    cancellation: &ReleaseCancellation,
) -> Result<ProcessOutput, ReleaseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let program_text = program.to_string_lossy().into_owned();
    if cancellation.is_cancelled() {
        return Err(ReleaseError::CommandInterrupted {
            program: program_text,
        });
    }
    let retained_program = if is_descriptor_program(program) {
        None
    } else {
        Some(retain_program(program)?)
    };
    let execution_program = retained_program
        .as_ref()
        .map_or_else(|| Path::new(program), safe_fs::DescriptorProgram::path);
    let mut command = Command::new(execution_program);
    command
        .args(args)
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear();
    for (key, value) in environment {
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
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;

        if !is_descriptor_program(program)
            && let Some(name) = Path::new(program).file_name()
        {
            command.arg0(name);
        }
        command.process_group(0);
    }

    let child = command.spawn().map_err(|source| ReleaseError::CommandIo {
        program: program_text.clone(),
        source,
    })?;
    let mut child = ManagedChild::new(child, program_text.clone());
    child.start_readers(output_limit)?;
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
        if started.elapsed() >= timeout {
            child.close_group()?;
            break Termination::Timeout;
        }
        match child.try_wait()? {
            Some(_) => {
                child.close_group()?;
                break Termination::Completed;
            }
            None => thread::sleep(POLL_INTERVAL),
        }
    };

    let (stdout, stderr) = child.join_readers()?;
    if termination == Termination::Interrupted {
        return Err(ReleaseError::CommandInterrupted {
            program: program_text,
        });
    }
    if termination == Termination::OutputLimit || stdout.exceeded || stderr.exceeded {
        return Err(ReleaseError::CommandOutputLimit {
            program: program_text,
            limit: output_limit,
        });
    }
    if termination == Termination::Timeout {
        return Err(ReleaseError::CommandTimeout {
            program: program_text,
            timeout,
        });
    }
    let status = child.status().ok_or_else(|| ReleaseError::CommandIo {
        program: program_text.clone(),
        source: std::io::Error::other("release child completed without an exit status"),
    })?;
    require_success(&program_text, status, &stderr.bytes)?;
    Ok(ProcessOutput {
        stdout: stdout.bytes,
    })
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

fn is_descriptor_program(program: &OsStr) -> bool {
    let path = Path::new(program);
    let parent = path.parent();
    let descriptor = path
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()));
    descriptor
        && matches!(
            parent,
            Some(parent) if parent == Path::new("/proc/self/fd") || parent == Path::new("/dev/fd")
        )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Termination {
    Completed,
    Interrupted,
    OutputLimit,
    Timeout,
}

struct ManagedChild {
    child: std::process::Child,
    program: String,
    stdout: Option<OutputReader>,
    stderr: Option<OutputReader>,
    status: Option<ExitStatus>,
    group_closed: bool,
}

impl ManagedChild {
    fn new(child: std::process::Child, program: String) -> Self {
        Self {
            child,
            program,
            stdout: None,
            stderr: None,
            status: None,
            group_closed: false,
        }
    }

    fn start_readers(&mut self, output_limit: usize) -> Result<(), ReleaseError> {
        let stdout = self
            .child
            .stdout
            .take()
            .ok_or_else(|| self.capture_error("child stdout was not piped"))?;
        self.stdout = Some(
            spawn_reader(stdout, output_limit)
                .map_err(|source| self.capture_source_error(source))?,
        );
        let stderr = self
            .child
            .stderr
            .take()
            .ok_or_else(|| self.capture_error("child stderr was not piped"))?;
        self.stderr = Some(
            spawn_reader(stderr, output_limit)
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

    fn try_wait(&mut self) -> Result<Option<ExitStatus>, ReleaseError> {
        let status = self
            .child
            .try_wait()
            .map_err(|source| ReleaseError::CommandIo {
                program: self.program.clone(),
                source,
            })?;
        if let Some(status) = status {
            self.status = Some(status);
        }
        Ok(status)
    }

    fn output_exceeded(&self) -> bool {
        self.stdout.as_ref().is_some_and(OutputReader::exceeded)
            || self.stderr.as_ref().is_some_and(OutputReader::exceeded)
    }

    fn close_group(&mut self) -> Result<(), ReleaseError> {
        self.stop_readers();
        if !self.group_closed {
            terminate_process_group(&mut self.child, &self.program)?;
            self.group_closed = true;
        }
        if self.status.is_none() {
            self.status = Some(
                self.child
                    .wait()
                    .map_err(|source| ReleaseError::CommandIo {
                        program: self.program.clone(),
                        source,
                    })?,
            );
        }
        Ok(())
    }

    fn stop_readers(&self) {
        if let Some(reader) = &self.stdout {
            reader.stop();
        }
        if let Some(reader) = &self.stderr {
            reader.stop();
        }
    }

    fn join_readers(&mut self) -> Result<(CapturedOutput, CapturedOutput), ReleaseError> {
        self.stop_readers();
        let stdout = self
            .stdout
            .take()
            .ok_or_else(|| self.capture_error("child stdout reader was not started"))?;
        let stderr = self
            .stderr
            .take()
            .ok_or_else(|| self.capture_error("child stderr reader was not started"))?;
        let stdout = join_reader(stdout, &self.program);
        let stderr = join_reader(stderr, &self.program);
        match (stdout, stderr) {
            (Ok(stdout), Ok(stderr)) => Ok((stdout, stderr)),
            (Err(error), _) | (_, Err(error)) => Err(error),
        }
    }

    fn status(&self) -> Option<ExitStatus> {
        self.status
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        self.stop_readers();
        if !self.group_closed {
            if terminate_process_group(&mut self.child, &self.program).is_err() {
                drop(self.child.kill());
            }
            self.group_closed = true;
        }
        if self.status.is_none() {
            drop(self.child.wait());
        }
        if let Some(stdout) = self.stdout.take() {
            drop(join_reader(stdout, &self.program));
        }
        if let Some(stderr) = self.stderr.take() {
            drop(join_reader(stderr, &self.program));
        }
    }
}

#[derive(Debug)]
struct CapturedOutput {
    bytes: Vec<u8>,
    exceeded: bool,
}

struct OutputReader {
    handle: JoinHandle<Result<CapturedOutput, std::io::Error>>,
    stop: Arc<AtomicBool>,
    exceeded: Arc<AtomicBool>,
}

impl OutputReader {
    fn stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    fn exceeded(&self) -> bool {
        self.exceeded.load(Ordering::Acquire)
    }
}

#[cfg(unix)]
trait ReadPipe: Read + Send + std::os::fd::AsFd {}

#[cfg(unix)]
impl<T> ReadPipe for T where T: Read + Send + std::os::fd::AsFd {}

#[cfg(not(unix))]
trait ReadPipe: Read + Send {}

#[cfg(not(unix))]
impl<T> ReadPipe for T where T: Read + Send {}

fn spawn_reader<R>(mut reader: R, limit: usize) -> Result<OutputReader, std::io::Error>
where
    R: ReadPipe + 'static,
{
    set_nonblocking(&reader)?;
    let stop = Arc::new(AtomicBool::new(false));
    let reader_stop = Arc::clone(&stop);
    let exceeded = Arc::new(AtomicBool::new(false));
    let reader_exceeded = Arc::clone(&exceeded);
    let handle = thread::spawn(move || {
        let mut bytes = Vec::with_capacity(limit.min(64 << 10));
        let mut exceeded = false;
        let mut buffer = [0_u8; 16 << 10];
        let mut stop_deadline = None;
        loop {
            if reader_stop.load(Ordering::Acquire) && stop_deadline.is_none() {
                let now = Instant::now();
                stop_deadline = Some(now.checked_add(READER_DRAIN_GRACE).unwrap_or(now));
            }
            if stop_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                break;
            }
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    let remaining = limit.saturating_sub(bytes.len());
                    let retained = remaining.min(read);
                    let kept = buffer.get(..retained).ok_or_else(|| {
                        std::io::Error::other("bounded output slice exceeded the read buffer")
                    })?;
                    bytes.extend_from_slice(kept);
                    if retained != read {
                        exceeded = true;
                        reader_exceeded.store(true, Ordering::Release);
                    }
                }
                Err(source) if source.kind() == std::io::ErrorKind::WouldBlock => {
                    if reader_stop.load(Ordering::Acquire) {
                        break;
                    }
                    thread::sleep(POLL_INTERVAL);
                }
                Err(source) if source.kind() == std::io::ErrorKind::Interrupted => {}
                Err(source) => return Err(source),
            }
        }
        Ok(CapturedOutput { bytes, exceeded })
    });
    Ok(OutputReader {
        handle,
        stop,
        exceeded,
    })
}

#[cfg(unix)]
fn set_nonblocking(reader: &impl std::os::fd::AsFd) -> Result<(), std::io::Error> {
    let flags = rustix::fs::fcntl_getfl(reader)?;
    rustix::fs::fcntl_setfl(reader, flags | rustix::fs::OFlags::NONBLOCK).map_err(Into::into)
}

#[cfg(not(unix))]
fn set_nonblocking<R>(_reader: &R) -> Result<(), std::io::Error> {
    Ok(())
}

fn join_reader(reader: OutputReader, program: &str) -> Result<CapturedOutput, ReleaseError> {
    match reader.handle.join() {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(source)) => Err(ReleaseError::CommandCapture {
            program: program.to_string(),
            source,
        }),
        Err(_) => Err(ReleaseError::CommandCapture {
            program: program.to_string(),
            source: std::io::Error::other("output reader thread panicked"),
        }),
    }
}

fn require_success(program: &str, status: ExitStatus, stderr: &[u8]) -> Result<(), ReleaseError> {
    if status.success() {
        return Ok(());
    }
    Err(ReleaseError::CommandFailed {
        program: program.to_string(),
        status: status.to_string(),
        stderr: String::from_utf8_lossy(stderr).trim().to_string(),
    })
}

#[cfg(unix)]
fn terminate_process_group(
    child: &mut std::process::Child,
    program: &str,
) -> Result<(), ReleaseError> {
    let raw_pid = i32::try_from(child.id()).map_err(|_| ReleaseError::InvalidProcessIdentity {
        program: program.to_string(),
    })?;
    let pid = rustix::process::Pid::from_raw(raw_pid).ok_or_else(|| {
        ReleaseError::InvalidProcessIdentity {
            program: program.to_string(),
        }
    })?;
    if let Err(source) = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL)
        && source != rustix::io::Errno::SRCH
    {
        return Err(ReleaseError::CommandIo {
            program: program.to_string(),
            source: source.into(),
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn terminate_process_group(
    child: &mut std::process::Child,
    program: &str,
) -> Result<(), ReleaseError> {
    child.kill().map_err(|source| ReleaseError::CommandIo {
        program: program.to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::path::PathBuf;

    const HELPER_ENV: &str = "STAB_RELEASE_HELPER";
    const STARTED_PATH_ENV: &str = "STAB_RELEASE_STARTED_PATH";
    const PUBLISHED_PATH_ENV: &str = "STAB_RELEASE_PUBLISHED_PATH";

    fn helper_request(mode: &str) -> (OsString, Vec<OsString>, Vec<(OsString, OsString)>) {
        let executable = std::env::current_exe()
            .expect("test executable")
            .into_os_string();
        let args = vec![
            OsString::from("--ignored"),
            OsString::from("--exact"),
            OsString::from("process::tests::subprocess_helper"),
            OsString::from("--nocapture"),
        ];
        let environment = vec![
            (OsString::from(HELPER_ENV), OsString::from(mode)),
            (
                OsString::from("GIT_DIR"),
                OsString::from("/tmp/attacker-controlled-git-dir"),
            ),
        ];
        (executable, args, environment)
    }

    #[test]
    fn child_output_is_bounded_before_capture_completes() {
        let (program, args, environment) = helper_request("flood");
        assert!(matches!(
            run(
                Path::new(env!("CARGO_MANIFEST_DIR")),
                &program,
                &args,
                &environment,
                Duration::from_secs(5),
                4096,
            ),
            Err(ReleaseError::CommandOutputLimit { limit: 4096, .. })
        ));
    }

    #[test]
    fn child_timeout_terminates_the_process_group() {
        let (program, args, environment) = helper_request("sleep");
        assert!(matches!(
            run(
                Path::new(env!("CARGO_MANIFEST_DIR")),
                &program,
                &args,
                &environment,
                Duration::from_millis(100),
                4096,
            ),
            Err(ReleaseError::CommandTimeout { .. })
        ));
    }

    #[test]
    fn inherited_and_explicit_git_overrides_are_cleared() {
        let (program, args, environment) = helper_request("git-env");
        let output = run(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &program,
            &args,
            &environment,
            Duration::from_secs(5),
            4096,
        )
        .expect("helper succeeds");
        assert!(String::from_utf8_lossy(&output.stdout).contains("git-env-absent"));
    }

    #[test]
    fn ambient_release_credentials_do_not_cross_the_supervisor_boundary() {
        let (program, args, environment) = helper_request("secret-supervisor");
        let status = Command::new(program)
            .args(args)
            .envs(environment)
            .env("CARGO_REGISTRY_TOKEN", "must-not-reach-release-children")
            .env(
                "CARGO_REGISTRIES_CRATES_IO_TOKEN",
                "must-not-reach-release-children",
            )
            .env("GITHUB_TOKEN", "must-not-reach-release-children")
            .env("GH_TOKEN", "must-not-reach-release-children")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("spawn secret-scope supervisor");
        assert!(
            status.success(),
            "secret-scope supervisor failed with {status}"
        );
    }

    #[test]
    fn successful_child_output_is_fully_drained() {
        let (program, args, environment) = helper_request("bounded-output");
        let output = run(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &program,
            &args,
            &environment,
            Duration::from_secs(5),
            256 << 10,
        )
        .expect("bounded helper output succeeds");

        assert_eq!(
            output.stdout.iter().filter(|byte| **byte == 0xa5).count(),
            128 << 10
        );
    }

    #[test]
    fn explicit_cancellation_prevents_mock_publication() {
        let directory = tempfile::tempdir().expect("temporary marker directory");
        let started = directory.path().join("started");
        let published = directory.path().join("published");
        let (program, args, mut environment) = helper_request("mock-publication");
        environment.extend(marker_environment(&started, &published));
        let cancellation = ReleaseCancellation::for_test();
        let trigger = cancellation.clone();
        let started_for_trigger = started.clone();
        let canceller = thread::spawn(move || {
            wait_for_path(&started_for_trigger, Duration::from_secs(5));
            trigger.cancel();
        });

        let result = run_with_cancellation(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &program,
            &args,
            &environment,
            Duration::from_secs(10),
            4096,
            &cancellation,
        );
        canceller.join().expect("cancellation thread");

        assert!(matches!(
            result,
            Err(ReleaseError::CommandInterrupted { .. })
        ));
        assert_process_gone(read_pid(&started)).expect("mock publication process is gone");
        assert!(
            !published.exists(),
            "cancelled publication wrote its marker"
        );
    }

    #[cfg(unix)]
    #[test]
    fn sigint_and_sigterm_cancel_mock_publication() {
        for (name, signal) in [
            ("sigint", rustix::process::Signal::INT),
            ("sigterm", rustix::process::Signal::TERM),
        ] {
            let directory = tempfile::tempdir().expect("temporary marker directory");
            let started = directory.path().join(format!("{name}-started"));
            let published = directory.path().join(format!("{name}-published"));
            let (program, args, mut environment) = helper_request("signal-supervisor");
            environment.extend(marker_environment(&started, &published));
            let mut supervisor = Command::new(&program)
                .args(&args)
                .envs(environment.iter().map(|(key, value)| (key, value)))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn signal supervisor");
            wait_for_path(&started, Duration::from_secs(5));

            rustix::process::kill_process(process_id(supervisor.id()), signal)
                .expect("signal release supervisor");
            let status = wait_for_child(&mut supervisor, Duration::from_secs(5))
                .expect("signal supervisor completes");

            assert!(status.success(), "{name} supervisor failed with {status}");
            assert_process_gone(read_pid(&started)).expect("mock publication process is gone");
            assert!(
                !published.exists(),
                "{name} cancellation allowed mock publication"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn escaped_descendant_cannot_hold_reader_joins_open() {
        let (program, args, environment) = helper_request("leader-exit-with-descendant");
        let started = Instant::now();

        let output = run(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &program,
            &args,
            &environment,
            Duration::from_secs(5),
            4096,
        )
        .expect("leader exit must complete despite escaped pipe holder");

        assert!(
            started.elapsed() < Duration::from_secs(2),
            "reader join waited for an escaped descendant"
        );
        let descendant = String::from_utf8_lossy(&output.stdout)
            .lines()
            .find_map(|line| line.strip_prefix("descendant-pid="))
            .expect("descendant pid marker")
            .parse::<u32>()
            .expect("numeric descendant pid");
        terminate_test_group(descendant);
        assert_process_gone(descendant).expect("escaped descendant is gone");
    }

    #[cfg(unix)]
    #[test]
    fn managed_child_drop_cancels_mock_publication() {
        let directory = tempfile::tempdir().expect("temporary marker directory");
        let started = directory.path().join("drop-started");
        let published = directory.path().join("drop-published");
        let (program, args, mut environment) = helper_request("mock-publication");
        environment.extend(marker_environment(&started, &published));
        let mut command = Command::new(&program);
        command
            .args(&args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(environment.iter().map(|(key, value)| (key, value)));
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
        let child = command.spawn().expect("spawn managed mock publication");
        let mut managed = ManagedChild::new(child, program.to_string_lossy().into_owned());
        managed.start_readers(4096).expect("start bounded readers");
        wait_for_path(&started, Duration::from_secs(5));
        let pid = read_pid(&started);

        let dropped = Instant::now();
        drop(managed);

        assert!(
            dropped.elapsed() < Duration::from_secs(2),
            "managed-child cleanup exceeded its reader bound"
        );
        assert_process_gone(pid).expect("managed child is gone");
        assert!(!published.exists(), "dropped guard allowed publication");
    }

    fn marker_environment(started: &Path, published: &Path) -> Vec<(OsString, OsString)> {
        vec![
            (
                OsString::from(STARTED_PATH_ENV),
                started.as_os_str().to_os_string(),
            ),
            (
                OsString::from(PUBLISHED_PATH_ENV),
                published.as_os_str().to_os_string(),
            ),
        ]
    }

    fn wait_for_path(path: &Path, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while !path.exists() {
            assert!(Instant::now() < deadline, "timed out waiting for {path:?}");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_for_child(
        child: &mut std::process::Child,
        timeout: Duration,
    ) -> Result<ExitStatus, String> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = child.try_wait().expect("poll helper") {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                drop(child.kill());
                let status = child.wait().expect("reap timed-out helper");
                return Err(format!("helper timed out and was killed with {status}"));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn read_pid(path: &Path) -> u32 {
        std::fs::read_to_string(path)
            .expect("read process marker")
            .parse::<u32>()
            .expect("numeric process marker")
    }

    #[cfg(unix)]
    fn process_id(pid: u32) -> rustix::process::Pid {
        let raw = i32::try_from(pid).expect("test pid fits i32");
        rustix::process::Pid::from_raw(raw).expect("nonzero test pid")
    }

    #[cfg(unix)]
    fn terminate_test_group(pid: u32) {
        if let Err(source) =
            rustix::process::kill_process_group(process_id(pid), rustix::process::Signal::KILL)
        {
            assert_eq!(source, rustix::io::Errno::SRCH);
        }
    }

    #[cfg(unix)]
    fn assert_process_gone(pid: u32) -> Result<(), String> {
        let pid = process_id(pid);
        for _ in 0..200 {
            match rustix::process::test_kill_process(pid) {
                Err(rustix::io::Errno::SRCH) => return Ok(()),
                Ok(()) => thread::sleep(Duration::from_millis(10)),
                Err(source) => {
                    return Err(format!("failed to inspect test process {pid:?}: {source}"));
                }
            }
        }
        Err(format!("test process {pid:?} survived supervisor cleanup"))
    }

    #[cfg(not(unix))]
    fn assert_process_gone(_pid: u32) -> Result<(), String> {
        Ok(())
    }

    #[test]
    #[ignore = "executed only as a subprocess by release process tests"]
    fn subprocess_helper() {
        match std::env::var(HELPER_ENV).as_deref() {
            Ok("flood") => print!("{}", "x".repeat(1 << 20)),
            Ok("sleep") => thread::sleep(Duration::from_secs(5)),
            Ok("git-env") => {
                if std::env::var_os("GIT_DIR").is_none() {
                    println!("git-env-absent");
                } else {
                    std::process::exit(9);
                }
            }
            Ok("secret-env") => {
                if [
                    "CARGO_REGISTRY_TOKEN",
                    "CARGO_REGISTRIES_CRATES_IO_TOKEN",
                    "GITHUB_TOKEN",
                    "GH_TOKEN",
                ]
                .iter()
                .all(|name| std::env::var_os(name).is_none())
                {
                    println!("release-credentials-absent");
                } else {
                    std::process::exit(10);
                }
            }
            Ok("secret-supervisor") => {
                let (program, args, environment) = helper_request("secret-env");
                let output = run(
                    Path::new(env!("CARGO_MANIFEST_DIR")),
                    &program,
                    &args,
                    &environment,
                    Duration::from_secs(5),
                    4096,
                )
                .expect("isolated secret probe");
                if !output
                    .stdout
                    .windows(b"release-credentials-absent".len())
                    .any(|window| window == b"release-credentials-absent")
                {
                    std::process::exit(11);
                }
            }
            Ok("bounded-output") => {
                std::io::stdout()
                    .write_all(&vec![0xa5; 128 << 10])
                    .expect("write bounded helper output");
            }
            Ok("mock-publication") => {
                let started = required_marker_path(STARTED_PATH_ENV);
                let published = required_marker_path(PUBLISHED_PATH_ENV);
                std::fs::write(started, std::process::id().to_string())
                    .expect("write publication start marker");
                thread::sleep(Duration::from_secs(30));
                std::fs::write(published, b"published").expect("write mock publication marker");
            }
            Ok("signal-supervisor") => {
                let (program, args, mut environment) = helper_request("mock-publication");
                let started = required_marker_path(STARTED_PATH_ENV);
                let published = required_marker_path(PUBLISHED_PATH_ENV);
                environment.extend(marker_environment(&started, &published));
                match run(
                    Path::new(env!("CARGO_MANIFEST_DIR")),
                    &program,
                    &args,
                    &environment,
                    Duration::from_secs(30),
                    4096,
                ) {
                    Err(ReleaseError::CommandInterrupted { .. }) => {}
                    Ok(_) => std::process::exit(11),
                    Err(error) => {
                        eprintln!("unexpected supervisor error: {error}");
                        std::process::exit(12);
                    }
                }
            }
            #[cfg(unix)]
            Ok("leader-exit-with-descendant") => {
                let (program, args, mut environment) = helper_request("pipe-holder");
                environment.retain(|(key, _)| key != "GIT_DIR");
                let mut command = Command::new(program);
                command
                    .args(args)
                    .envs(environment.iter().map(|(key, value)| (key, value)));
                use std::os::unix::process::CommandExt as _;
                command.process_group(0);
                let child = command.spawn().expect("spawn escaped pipe holder");
                println!("descendant-pid={}", child.id());
                std::io::stdout().flush().expect("flush descendant pid");
                drop(child);
            }
            Ok("pipe-holder") => thread::sleep(Duration::from_secs(30)),
            _ => std::process::exit(10),
        }
    }

    fn required_marker_path(key: &str) -> PathBuf {
        std::env::var_os(key)
            .map(PathBuf::from)
            .expect("required release marker path")
    }
}
