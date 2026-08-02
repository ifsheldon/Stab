use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::ReleaseError;

const POLL_INTERVAL: Duration = Duration::from_millis(20);

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
    let program_text = program.to_string_lossy().into_owned();
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(environment.iter().map(|(key, value)| (key, value)));
    clear_git_overrides(&mut command);
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }

    let mut child = command.spawn().map_err(|source| ReleaseError::CommandIo {
        program: program_text.clone(),
        source,
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ReleaseError::CommandCapture {
            program: program_text.clone(),
            source: std::io::Error::other("child stdout was not piped"),
        })?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ReleaseError::CommandCapture {
            program: program_text.clone(),
            source: std::io::Error::other("child stderr was not piped"),
        })?;
    let exceeded = Arc::new(AtomicBool::new(false));
    let stdout_reader = spawn_reader(stdout, output_limit, Arc::clone(&exceeded));
    let stderr_reader = spawn_reader(stderr, output_limit, Arc::clone(&exceeded));
    let started = Instant::now();

    let (status, termination) = loop {
        if exceeded.load(Ordering::Acquire) {
            terminate_process_group(&mut child, &program_text)?;
            let status = child.wait().map_err(|source| ReleaseError::CommandIo {
                program: program_text.clone(),
                source,
            })?;
            break (status, Some(Termination::OutputLimit));
        }
        if started.elapsed() >= timeout {
            terminate_process_group(&mut child, &program_text)?;
            let status = child.wait().map_err(|source| ReleaseError::CommandIo {
                program: program_text.clone(),
                source,
            })?;
            break (status, Some(Termination::Timeout));
        }
        match child.try_wait().map_err(|source| ReleaseError::CommandIo {
            program: program_text.clone(),
            source,
        })? {
            Some(status) => {
                terminate_remaining_group(&mut child);
                break (status, None);
            }
            None => thread::sleep(POLL_INTERVAL),
        }
    };

    let stdout = join_reader(stdout_reader, &program_text)?;
    let stderr = join_reader(stderr_reader, &program_text)?;
    if matches!(termination, Some(Termination::OutputLimit)) || stdout.exceeded || stderr.exceeded {
        return Err(ReleaseError::CommandOutputLimit {
            program: program_text,
            limit: output_limit,
        });
    }
    if matches!(termination, Some(Termination::Timeout)) {
        return Err(ReleaseError::CommandTimeout {
            program: program_text,
            timeout,
        });
    }
    require_success(&program_text, status, &stderr.bytes)?;
    Ok(ProcessOutput {
        stdout: stdout.bytes,
    })
}

fn clear_git_overrides(command: &mut Command) {
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CONFIG",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_SYSTEM",
        "GIT_CONFIG_COUNT",
    ] {
        command.env_remove(key);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Termination {
    OutputLimit,
    Timeout,
}

#[derive(Debug)]
struct CapturedOutput {
    bytes: Vec<u8>,
    exceeded: bool,
}

fn spawn_reader<R>(
    mut reader: R,
    limit: usize,
    any_exceeded: Arc<AtomicBool>,
) -> thread::JoinHandle<Result<CapturedOutput, std::io::Error>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut bytes = Vec::with_capacity(limit.min(64 << 10));
        let mut exceeded = false;
        let mut buffer = [0_u8; 16 << 10];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            let remaining = limit.saturating_sub(bytes.len());
            let retained = remaining.min(read);
            let kept = buffer.get(..retained).ok_or_else(|| {
                std::io::Error::other("bounded output slice exceeded the read buffer")
            })?;
            bytes.extend_from_slice(kept);
            if retained != read {
                exceeded = true;
                any_exceeded.store(true, Ordering::Release);
            }
        }
        Ok(CapturedOutput { bytes, exceeded })
    })
}

fn join_reader(
    reader: thread::JoinHandle<Result<CapturedOutput, std::io::Error>>,
    program: &str,
) -> Result<CapturedOutput, ReleaseError> {
    match reader.join() {
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

fn terminate_remaining_group(child: &mut std::process::Child) {
    drop(terminate_process_group(child, "completed release command"));
}

#[cfg(test)]
mod tests {
    use super::*;

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
            (OsString::from("STAB_RELEASE_HELPER"), OsString::from(mode)),
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
    #[ignore = "executed only as a subprocess by release process tests"]
    fn subprocess_helper() {
        match std::env::var("STAB_RELEASE_HELPER").as_deref() {
            Ok("flood") => print!("{}", "x".repeat(1 << 20)),
            Ok("sleep") => thread::sleep(Duration::from_secs(5)),
            Ok("git-env") => {
                if std::env::var_os("GIT_DIR").is_none() {
                    println!("git-env-absent");
                } else {
                    std::process::exit(9);
                }
            }
            _ => std::process::exit(10),
        }
    }
}
