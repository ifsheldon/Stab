use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::{CapturedOutput, OracleError, ProcessOutput};
use crate::safe_file::{SafeFileError, SafeFileLocation};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);
pub(super) const OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;
const DIAGNOSTIC_LIMIT_BYTES: usize = 4096;

pub(super) fn run_checked<I, S>(
    program: &str,
    args: I,
    stdin: &[u8],
    working_dir: Option<&Path>,
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
    working_dir: Option<&Path>,
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
    working_dir: Option<&Path>,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_process_with_timeout(program, args, stdin, working_dir, COMMAND_TIMEOUT)
}

pub(super) fn run_process_monitoring_files<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: Option<&Path>,
    monitored_files: &[SafeFileLocation],
    file_limit: u64,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_process_with_timeout_and_monitored_files(
        program,
        args,
        stdin,
        working_dir,
        COMMAND_TIMEOUT,
        monitored_files,
        file_limit,
    )
}

pub(super) fn run_process_with_timeout<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: Option<&Path>,
    timeout: Duration,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_process_with_timeout_and_monitored_files(
        program,
        args,
        stdin,
        working_dir,
        timeout,
        &[],
        u64::MAX,
    )
}

pub(super) fn run_process_with_timeout_and_monitored_files<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: Option<&Path>,
    timeout: Duration,
    monitored_files: &[SafeFileLocation],
    file_limit: u64,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let cancellation = ProcessCancellation::for_signals()?;
    run_process_with_control(
        program,
        None,
        args,
        stdin,
        working_dir,
        timeout,
        monitored_files,
        file_limit,
        None,
        &cancellation,
    )
}

pub(super) fn run_qualification_process_with_timeout<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: Option<&Path>,
    timeout: Duration,
    environment: &[(std::ffi::OsString, std::ffi::OsString)],
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_qualification_process_with_timeout_and_monitored_files(
        program,
        args,
        stdin,
        working_dir,
        timeout,
        &[],
        u64::MAX,
        environment,
    )
}

pub(super) fn run_qualification_process_with_timeout_and_arg0<I, S>(
    program: &Path,
    arg0: &OsStr,
    args: I,
    stdin: &[u8],
    working_dir: Option<&Path>,
    timeout: Duration,
    environment: &[(std::ffi::OsString, std::ffi::OsString)],
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let cancellation = ProcessCancellation::for_signals()?;
    run_process_with_control(
        program,
        Some(arg0),
        args,
        stdin,
        working_dir,
        timeout,
        &[],
        u64::MAX,
        Some(environment),
        &cancellation,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "qualification process contract is explicit"
)]
pub(super) fn run_qualification_process_with_timeout_and_monitored_files<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: Option<&Path>,
    timeout: Duration,
    monitored_files: &[SafeFileLocation],
    file_limit: u64,
    environment: &[(std::ffi::OsString, std::ffi::OsString)],
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let cancellation = ProcessCancellation::for_signals()?;
    run_process_with_control(
        program,
        None,
        args,
        stdin,
        working_dir,
        timeout,
        monitored_files,
        file_limit,
        Some(environment),
        &cancellation,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "process execution contract is explicit"
)]
fn run_process_with_control<I, S>(
    program: &Path,
    arg0: Option<&OsStr>,
    args: I,
    stdin: &[u8],
    working_dir: Option<&Path>,
    timeout: Duration,
    monitored_files: &[SafeFileLocation],
    file_limit: u64,
    environment: Option<&[(std::ffi::OsString, std::ffi::OsString)]>,
    cancellation: &ProcessCancellation,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    if cancellation.is_cancelled() {
        return Err(interrupted_without_output(program));
    }
    let mut command = std::process::Command::new(program);
    #[cfg(unix)]
    if let Some(arg0) = arg0 {
        use std::os::unix::process::CommandExt as _;

        command.arg0(arg0);
    }
    #[cfg(not(unix))]
    let _ = arg0;
    command.args(args);
    if let Some(working_dir) = working_dir {
        command.current_dir(working_dir);
    }
    if let Some(environment) = environment {
        command.env_clear();
        command.envs(environment.iter().map(|(key, value)| (key, value)));
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        command.process_group(0);
    }
    let program_name = program.display().to_string();
    let mut child = command.spawn().map_err(|source| OracleError::Spawn {
        program: program_name.clone(),
        source,
    })?;
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| OracleError::Wait {
            program: program_name.clone(),
            source: std::io::Error::other("process timeout exceeds the monotonic clock range"),
        })?;
    let stdout = child
        .stdout
        .take()
        .map(spawn_output_reader)
        .ok_or_else(|| OracleError::CaptureOutput {
            program: program_name.clone(),
            source: std::io::Error::other("child stdout was not piped"),
        })?;
    let stderr = child
        .stderr
        .take()
        .map(spawn_output_reader)
        .ok_or_else(|| OracleError::CaptureOutput {
            program: program_name.clone(),
            source: std::io::Error::other("child stderr was not piped"),
        })?;
    let stdin = child
        .stdin
        .take()
        .map(|child_stdin| spawn_stdin_writer(child_stdin, stdin));
    let mut status = None;

    loop {
        if status.is_none() {
            let observed_status = child.try_wait().map_err(|source| OracleError::Wait {
                program: program_name.clone(),
                source,
            })?;
            if observed_status.is_some() {
                status = observed_status;
                kill_process_group(&mut child, &program_name)?;
            }
        }
        if cancellation.is_cancelled() {
            terminate_process_tree(&mut child, &program_name)?;
            if let Some(stdin) = stdin {
                drop(stdin.join());
            }
            let stdout = join_output_reader(&program_name, stdout)?;
            let stderr = join_output_reader(&program_name, stderr)?;
            return Err(OracleError::Interrupted {
                program: program_name,
                stdout,
                stderr,
            });
        }
        if let Some(stream) = output_limit_violation(&stdout, &stderr) {
            terminate_process_tree(&mut child, &program_name)?;
            if let Some(stdin) = stdin {
                drop(stdin.join());
            }
            let stdout = join_output_reader(&program_name, stdout)?;
            let stderr = join_output_reader(&program_name, stderr)?;
            return Err(OracleError::OutputLimitExceeded {
                program: program_name,
                stream,
                limit: OUTPUT_LIMIT_BYTES,
                stdout,
                stderr,
            });
        }
        if let Some(violation) = monitored_output_violation(monitored_files, file_limit) {
            terminate_process_tree(&mut child, &program_name)?;
            if let Some(stdin) = stdin {
                drop(stdin.join());
            }
            drop(join_output_reader(&program_name, stdout)?);
            drop(join_output_reader(&program_name, stderr)?);
            return Err(violation.into_oracle_error(program_name, file_limit));
        }
        let stdin_finished = stdin.as_ref().is_none_or(JoinHandle::is_finished);
        if status.is_some() && stdin_finished && stdout.is_finished() && stderr.is_finished() {
            break;
        }

        let now = Instant::now();
        if now >= deadline {
            terminate_process_tree(&mut child, &program_name)?;
            if let Some(stdin) = stdin {
                drop(stdin.join());
            }
            let stdout = join_output_reader(&program_name, stdout)?;
            let stderr = join_output_reader(&program_name, stderr)?;
            return Err(OracleError::TimedOut {
                program: program_name,
                milliseconds: timeout.as_millis(),
                stdout,
                stderr,
            });
        }
        std::thread::sleep(PROCESS_POLL_INTERVAL.min(deadline.duration_since(now)));
    }

    join_stdin_writer(&program_name, stdin)?;
    let stdout = join_output_reader(&program_name, stdout)?;
    let stderr = join_output_reader(&program_name, stderr)?;
    if cancellation.is_cancelled() {
        return Err(OracleError::Interrupted {
            program: program_name,
            stdout,
            stderr,
        });
    }
    let truncated_stream = if stdout.truncated {
        Some("stdout")
    } else if stderr.truncated {
        Some("stderr")
    } else {
        None
    };
    if let Some(stream) = truncated_stream {
        return Err(OracleError::OutputLimitExceeded {
            program: program_name,
            stream,
            limit: OUTPUT_LIMIT_BYTES,
            stdout,
            stderr,
        });
    }
    Ok(ProcessOutput {
        status: status.and_then(|status| status.code()),
        stdout,
        stderr,
    })
}

struct ProcessCancellation {
    cancelled: Arc<AtomicBool>,
}

impl ProcessCancellation {
    fn for_signals() -> Result<Self, OracleError> {
        #[cfg(unix)]
        {
            static CANCELLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
            static INSTALLATION: OnceLock<Result<(), String>> = OnceLock::new();
            let cancelled = Arc::clone(CANCELLED.get_or_init(|| Arc::new(AtomicBool::new(false))));
            let installation = INSTALLATION.get_or_init(|| {
                for signal in [
                    signal_hook::consts::signal::SIGINT,
                    signal_hook::consts::signal::SIGTERM,
                ] {
                    signal_hook::flag::register(signal, Arc::clone(&cancelled))
                        .map_err(|source| source.to_string())?;
                }
                Ok(())
            });
            if let Err(reason) = installation {
                return Err(OracleError::InstallCancellationHandler(
                    std::io::Error::other(reason.clone()),
                ));
            }
            Ok(Self { cancelled })
        }
        #[cfg(not(unix))]
        {
            static CANCELLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
            Ok(Self {
                cancelled: Arc::clone(CANCELLED.get_or_init(|| Arc::new(AtomicBool::new(false)))),
            })
        }
    }

    #[cfg(test)]
    fn from_flag(cancelled: Arc<AtomicBool>) -> Self {
        Self { cancelled }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

pub(crate) fn ensure_qualification_active() -> Result<(), OracleError> {
    let cancellation = ProcessCancellation::for_signals()?;
    if cancellation.is_cancelled() {
        return Err(interrupted_without_output(Path::new(
            "qualification controller",
        )));
    }
    Ok(())
}

fn interrupted_without_output(program: &Path) -> OracleError {
    let empty = CapturedOutput {
        bytes: Vec::new(),
        truncated: false,
    };
    OracleError::Interrupted {
        program: program.display().to_string(),
        stdout: empty.clone(),
        stderr: empty,
    }
}

enum MonitoredOutputViolation {
    TooLarge {
        path: std::path::PathBuf,
    },
    Unsafe {
        path: std::path::PathBuf,
        reason: String,
    },
}

impl MonitoredOutputViolation {
    fn into_oracle_error(self, program: String, limit: u64) -> OracleError {
        match self {
            Self::TooLarge { path } => OracleError::AuxiliaryOutputLimitExceeded {
                program,
                path,
                limit,
            },
            Self::Unsafe { path, reason } => OracleError::UnsafeAuxiliaryOutput {
                program,
                path,
                reason,
            },
        }
    }
}

fn monitored_output_violation(
    files: &[SafeFileLocation],
    limit: u64,
) -> Option<MonitoredOutputViolation> {
    for location in files {
        let file = match location.open_regular_file() {
            Ok(file) => file,
            Err(SafeFileError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }
            Err(error) => {
                return Some(MonitoredOutputViolation::Unsafe {
                    path: location.display_path().to_path_buf(),
                    reason: error.to_string(),
                });
            }
        };
        match file.metadata() {
            Ok(metadata) if metadata.len() > limit => {
                return Some(MonitoredOutputViolation::TooLarge {
                    path: location.display_path().to_path_buf(),
                });
            }
            Ok(_) => {}
            Err(error) => {
                return Some(MonitoredOutputViolation::Unsafe {
                    path: location.display_path().to_path_buf(),
                    reason: error.to_string(),
                });
            }
        }
    }
    None
}

fn spawn_stdin_writer(
    mut writer: std::process::ChildStdin,
    stdin: &[u8],
) -> JoinHandle<Result<(), std::io::Error>> {
    let stdin = stdin.to_vec();
    std::thread::spawn(move || writer.write_all(&stdin))
}

fn join_stdin_writer(
    program: &str,
    writer: Option<JoinHandle<Result<(), std::io::Error>>>,
) -> Result<(), OracleError> {
    let Some(writer) = writer else {
        return Ok(());
    };
    match writer.join() {
        Ok(Ok(())) => Ok(()),
        // The child may intentionally reject its arguments before reading stdin.
        Ok(Err(source)) if source.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Ok(Err(source)) => Err(OracleError::WriteStdin {
            program: program.to_string(),
            source,
        }),
        Err(_panic) => Err(OracleError::WriteStdin {
            program: program.to_string(),
            source: std::io::Error::other("stdin writer thread panicked"),
        }),
    }
}

fn terminate_process_tree(
    child: &mut std::process::Child,
    program: &str,
) -> Result<(), OracleError> {
    kill_process_group(child, program)?;
    child.wait().map_err(|source| OracleError::Wait {
        program: program.to_string(),
        source,
    })?;
    Ok(())
}

fn kill_process_group(child: &mut std::process::Child, program: &str) -> Result<(), OracleError> {
    #[cfg(unix)]
    {
        let raw_pid =
            i32::try_from(child.id()).map_err(|_| OracleError::TerminateProcessGroup {
                program: program.to_string(),
                source: std::io::Error::other("child process id exceeds the Unix pid range"),
            })?;
        let pid = rustix::process::Pid::from_raw(raw_pid).ok_or_else(|| {
            OracleError::TerminateProcessGroup {
                program: program.to_string(),
                source: std::io::Error::other("child process id is zero"),
            }
        })?;
        if let Err(source) = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL)
            && source != rustix::io::Errno::SRCH
        {
            return Err(OracleError::TerminateProcessGroup {
                program: program.to_string(),
                source: source.into(),
            });
        }
    }
    #[cfg(not(unix))]
    child
        .kill()
        .map_err(|source| OracleError::TerminateProcessGroup {
            program: program.to_string(),
            source,
        })?;
    Ok(())
}

struct OutputReader {
    handle: JoinHandle<Result<CapturedOutput, std::io::Error>>,
    exceeded: Arc<AtomicBool>,
}

impl OutputReader {
    fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    fn exceeded(&self) -> bool {
        self.exceeded.load(Ordering::Acquire)
    }
}

fn output_limit_violation(stdout: &OutputReader, stderr: &OutputReader) -> Option<&'static str> {
    if stdout.exceeded() {
        Some("stdout")
    } else if stderr.exceeded() {
        Some("stderr")
    } else {
        None
    }
}

fn spawn_output_reader<R>(mut reader: R) -> OutputReader
where
    R: Read + Send + 'static,
{
    let exceeded = Arc::new(AtomicBool::new(false));
    let reader_exceeded = Arc::clone(&exceeded);
    let handle = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 8192];
        let mut truncated = false;
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            let chunk = buffer.get(..bytes_read).ok_or_else(|| {
                std::io::Error::other("bounded output reader exceeded buffer bounds")
            })?;
            let remaining = OUTPUT_LIMIT_BYTES.saturating_sub(bytes.len());
            if bytes_read <= remaining {
                bytes.extend_from_slice(chunk);
            } else {
                let kept = chunk.get(..remaining).ok_or_else(|| {
                    std::io::Error::other("bounded output reader exceeded keep bounds")
                })?;
                bytes.extend_from_slice(kept);
                truncated = true;
                reader_exceeded.store(true, Ordering::Release);
            }
        }
        Ok(CapturedOutput { bytes, truncated })
    });
    OutputReader { handle, exceeded }
}

fn join_output_reader(program: &str, reader: OutputReader) -> Result<CapturedOutput, OracleError> {
    match reader.handle.join() {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(source)) => Err(OracleError::CaptureOutput {
            program: program.to_string(),
            source,
        }),
        Err(_panic) => Err(OracleError::CaptureOutput {
            program: program.to_string(),
            source: std::io::Error::other("output reader thread panicked"),
        }),
    }
}

pub(super) fn render_bytes_for_diagnostics(bytes: &[u8], truncated: bool) -> String {
    let mut rendered = String::new();
    let display_len = bytes.len().min(DIAGNOSTIC_LIMIT_BYTES);
    let display_bytes = bytes.get(..display_len).unwrap_or(bytes);
    for byte in display_bytes {
        for escaped in std::ascii::escape_default(*byte) {
            rendered.push(char::from(escaped));
        }
    }
    if truncated || bytes.len() > display_len {
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

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::ffi::OsString;
    #[cfg(unix)]
    use std::path::Path;
    #[cfg(unix)]
    use std::sync::Arc;
    #[cfg(unix)]
    use std::sync::atomic::{AtomicBool, Ordering};
    #[cfg(unix)]
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    use super::{
        OUTPUT_LIMIT_BYTES, ProcessCancellation, run_process_with_control,
        run_process_with_timeout, run_process_with_timeout_and_monitored_files,
        run_qualification_process_with_timeout,
    };
    #[cfg(unix)]
    use crate::OracleError;

    #[cfg(unix)]
    #[test]
    fn output_limit_failure_retains_raw_captured_streams() {
        let error = run_process_with_timeout(
            Path::new("/bin/sh"),
            [
                "-c",
                "dd if=/dev/zero bs=1048577 count=1 2>/dev/null; printf '\\377\\000' >&2",
            ],
            &[],
            None,
            Duration::from_secs(5),
        )
        .expect_err("stdout beyond the process limit must fail closed");

        assert!(
            matches!(&error, OracleError::OutputLimitExceeded { .. }),
            "unexpected process error: {error}"
        );
        if let OracleError::OutputLimitExceeded {
            stream,
            limit,
            stdout,
            stderr,
            ..
        } = error
        {
            assert_eq!(stream, "stdout");
            assert_eq!(limit, OUTPUT_LIMIT_BYTES);
            assert_eq!(stdout.bytes.len(), OUTPUT_LIMIT_BYTES);
            assert!(stdout.truncated);
            assert_eq!(stderr.bytes, [0xff, 0]);
            assert!(!stderr.truncated);
        }
    }

    #[cfg(unix)]
    #[test]
    fn infinite_output_is_terminated_when_the_capture_limit_is_crossed() {
        let started = Instant::now();
        let error = run_process_with_timeout(
            Path::new("/bin/sh"),
            ["-c", "while :; do printf 0123456789abcdef; done"],
            &[],
            None,
            Duration::from_secs(30),
        )
        .expect_err("unbounded stdout must be terminated at the capture limit");

        assert!(matches!(error, OracleError::OutputLimitExceeded { .. }));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn direct_child_exit_terminates_closed_stdio_descendants() {
        let directory = tempfile::tempdir().expect("temporary marker directory");
        let marker = directory.path().join("escaped-descendant");
        let args = vec![
            OsString::from("-c"),
            OsString::from("exec 1>&- 2>&-; (sleep 0.3; printf escaped > \"$1\") & exit 0"),
            OsString::from("qualification-child"),
            marker.as_os_str().to_owned(),
        ];

        let output = run_process_with_timeout(
            Path::new("/bin/sh"),
            args,
            &[],
            None,
            Duration::from_secs(2),
        )
        .expect("direct child exit should be collected");
        assert_eq!(output.status, Some(0));
        std::thread::sleep(Duration::from_millis(500));
        assert!(!marker.exists(), "background descendant escaped cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn controller_cancellation_terminates_the_process_group() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancellation = ProcessCancellation::from_flag(Arc::clone(&cancelled));
        let trigger = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            cancelled.store(true, Ordering::Release);
        });
        let started = Instant::now();

        let error = run_process_with_control(
            Path::new("/bin/sh"),
            None,
            ["-c", "sleep 30 & wait"],
            &[],
            None,
            Duration::from_secs(30),
            &[],
            u64::MAX,
            None,
            &cancellation,
        )
        .expect_err("controller cancellation must terminate the child tree");
        trigger.join().expect("cancellation trigger");

        assert!(matches!(error, OracleError::Interrupted { .. }));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn sticky_cancellation_prevents_a_later_process_from_starting() {
        let directory = tempfile::tempdir().expect("temporary marker directory");
        let marker = directory.path().join("must-not-start");
        let cancellation = ProcessCancellation::from_flag(Arc::new(AtomicBool::new(true)));

        let error = run_process_with_control(
            Path::new("/bin/sh"),
            None,
            [
                OsString::from("-c"),
                OsString::from("printf started > \"$1\""),
                OsString::from("qualification-child"),
                marker.as_os_str().to_owned(),
            ],
            &[],
            None,
            Duration::from_secs(2),
            &[],
            u64::MAX,
            None,
            &cancellation,
        )
        .expect_err("sticky cancellation must reject a later spawn");

        assert!(matches!(error, OracleError::Interrupted { .. }));
        assert!(!marker.exists());
    }

    #[cfg(unix)]
    #[test]
    fn qualification_process_inherits_only_the_explicit_environment() {
        let environment = vec![(OsString::from("CQ_VISIBLE"), OsString::from("bound"))];

        let output = run_qualification_process_with_timeout(
            Path::new("/bin/sh"),
            [
                "-c",
                "printf '%s:%s' \"${CQ_VISIBLE-unset}\" \"${HOME-unset}\"",
            ],
            &[],
            None,
            Duration::from_secs(2),
            &environment,
        )
        .expect("qualification environment probe");

        assert_eq!(output.stdout.bytes, b"bound:unset");
    }

    #[cfg(unix)]
    #[test]
    fn process_timeout_terminates_descendants_holding_output_pipes() {
        let started = Instant::now();
        let error = run_process_with_timeout(
            Path::new("/bin/sh"),
            ["-c", "sleep 30 & wait"],
            &[],
            None,
            Duration::from_millis(200),
        )
        .expect_err("process tree should time out");

        assert!(matches!(error, OracleError::TimedOut { .. }));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn auxiliary_output_limit_terminates_process_group_promptly() {
        let directory = tempfile::tempdir().expect("temporary output directory");
        let output = directory.path().join("side-output");
        std::fs::write(&output, b"too large").expect("oversized side output");
        let monitored_output = crate::safe_file::SafeFileLocation::path(output);
        let started = Instant::now();
        let error = run_process_with_timeout_and_monitored_files(
            Path::new("/bin/sh"),
            ["-c", "sleep 30 & wait"],
            &[],
            None,
            Duration::from_secs(5),
            &[monitored_output],
            1,
        )
        .expect_err("oversized auxiliary output must terminate the child");

        assert!(matches!(
            error,
            OracleError::AuxiliaryOutputLimitExceeded { .. }
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn parent_exit_terminates_descendants_holding_output_pipes() {
        let started = Instant::now();
        let output = run_process_with_timeout(
            Path::new("/bin/sh"),
            ["-c", "sleep 30 & exit 0"],
            &[],
            None,
            Duration::from_secs(5),
        )
        .expect("direct-child success should terminate descendant-held pipes");

        assert_eq!(output.status, Some(0));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn process_timeout_includes_blocked_stdin_writes() {
        let started = Instant::now();
        let stdin = vec![0; 1024 * 1024];
        let error = run_process_with_timeout(
            Path::new("/bin/sh"),
            ["-c", "exec 1>&- 2>&-; sleep 30"],
            &stdin,
            None,
            Duration::from_millis(200),
        )
        .expect_err("blocked stdin writes should remain subject to the process deadline");

        assert!(matches!(error, OracleError::TimedOut { .. }));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn early_child_exit_preserves_process_output_after_broken_stdin_pipe() {
        let stdin = vec![0; 1024 * 1024];
        let output = run_process_with_timeout(
            Path::new("/bin/sh"),
            ["-c", "exec 0<&-; printf rejected >&2; exit 3"],
            &stdin,
            None,
            Duration::from_secs(2),
        )
        .expect("an early argument rejection should remain observable");

        assert_eq!(output.status, Some(3));
        assert_eq!(output.stderr.bytes, b"rejected");
    }
}
