use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Stdio;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::{CapturedOutput, OracleError, ProcessOutput};

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
    let output = run_process(Path::new(program), args, stdin, working_dir)?;
    if output.success() {
        return Ok(output);
    }
    Err(OracleError::CommandFailed {
        program: program.to_string(),
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
    monitored_paths: &[&Path],
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
        monitored_paths,
        file_limit,
    )
}

fn run_process_with_timeout<I, S>(
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

fn run_process_with_timeout_and_monitored_files<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: Option<&Path>,
    timeout: Duration,
    monitored_paths: &[&Path],
    file_limit: u64,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = std::process::Command::new(program);
    command.args(args);
    if let Some(working_dir) = working_dir {
        command.current_dir(working_dir);
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
            status = child.try_wait().map_err(|source| OracleError::Wait {
                program: program_name.clone(),
                source,
            })?;
        }
        if let Some(violation) = monitored_output_violation(monitored_paths, file_limit) {
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
                stdout: stdout.render_for_diagnostics().into_boxed_str(),
                stderr: stderr.render_for_diagnostics().into_boxed_str(),
            });
        }
        std::thread::sleep(PROCESS_POLL_INTERVAL.min(deadline.duration_since(now)));
    }

    join_stdin_writer(&program_name, stdin)?;
    let stdout = join_output_reader(&program_name, stdout)?;
    let stderr = join_output_reader(&program_name, stderr)?;
    reject_truncated_output(&program_name, "stdout", &stdout)?;
    reject_truncated_output(&program_name, "stderr", &stderr)?;
    Ok(ProcessOutput {
        status: status.and_then(|status| status.code()),
        stdout,
        stderr,
    })
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

fn monitored_output_violation(paths: &[&Path], limit: u64) -> Option<MonitoredOutputViolation> {
    for path in paths {
        match std::fs::symlink_metadata(path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Some(MonitoredOutputViolation::Unsafe {
                    path: path.to_path_buf(),
                    reason: error.to_string(),
                });
            }
        }
        let file = match crate::safe_file::open_regular_file(path) {
            Ok(file) => file,
            Err(error) => {
                return Some(MonitoredOutputViolation::Unsafe {
                    path: path.to_path_buf(),
                    reason: error.to_string(),
                });
            }
        };
        match file.metadata() {
            Ok(metadata) if metadata.len() > limit => {
                return Some(MonitoredOutputViolation::TooLarge {
                    path: path.to_path_buf(),
                });
            }
            Ok(_) => {}
            Err(error) => {
                return Some(MonitoredOutputViolation::Unsafe {
                    path: path.to_path_buf(),
                    reason: error.to_string(),
                });
            }
        }
    }
    None
}

fn reject_truncated_output(
    program: &str,
    stream: &'static str,
    output: &CapturedOutput,
) -> Result<(), OracleError> {
    if output.truncated {
        return Err(OracleError::OutputLimitExceeded {
            program: program.to_string(),
            stream,
            limit: OUTPUT_LIMIT_BYTES,
        });
    }
    Ok(())
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

    child.wait().map_err(|source| OracleError::Wait {
        program: program.to_string(),
        source,
    })?;
    Ok(())
}

fn spawn_output_reader<R>(mut reader: R) -> JoinHandle<Result<CapturedOutput, std::io::Error>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
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
            }
        }
        Ok(CapturedOutput { bytes, truncated })
    })
}

fn join_output_reader(
    program: &str,
    handle: JoinHandle<Result<CapturedOutput, std::io::Error>>,
) -> Result<CapturedOutput, OracleError> {
    match handle.join() {
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
    use std::path::Path;
    #[cfg(unix)]
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    use super::{
        OUTPUT_LIMIT_BYTES, reject_truncated_output, run_process_with_timeout,
        run_process_with_timeout_and_monitored_files,
    };
    use crate::CapturedOutput;
    #[cfg(unix)]
    use crate::OracleError;

    #[test]
    fn truncated_process_output_fails_closed_before_comparison() {
        let output = CapturedOutput {
            bytes: vec![b'x'; OUTPUT_LIMIT_BYTES],
            truncated: true,
        };

        assert!(matches!(
            reject_truncated_output("fixture", "stdout", &output),
            Err(crate::OracleError::OutputLimitExceeded {
                program,
                stream: "stdout",
                limit: OUTPUT_LIMIT_BYTES,
            }) if program == "fixture"
        ));
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
        let started = Instant::now();
        let error = run_process_with_timeout_and_monitored_files(
            Path::new("/bin/sh"),
            ["-c", "sleep 30 & wait"],
            &[],
            None,
            Duration::from_secs(5),
            &[output.as_path()],
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
    fn process_timeout_terminates_descendants_after_parent_exits() {
        let started = Instant::now();
        let error = run_process_with_timeout(
            Path::new("/bin/sh"),
            ["-c", "sleep 30 & exit 0"],
            &[],
            None,
            Duration::from_millis(200),
        )
        .expect_err("descendant-held pipes should keep the process tree subject to the deadline");

        assert!(matches!(error, OracleError::TimedOut { .. }));
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
}
