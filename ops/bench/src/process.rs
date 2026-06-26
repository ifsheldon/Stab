use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::Path;
use std::process::Stdio;
use std::thread::JoinHandle;

use wait_timeout::ChildExt;

use crate::config::COMMAND_TIMEOUT;
use crate::error::BenchError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProcessOutput {
    pub(crate) status: Option<i32>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

pub(crate) fn run_process(
    program: &Path,
    args: &[OsString],
    stdin: &[u8],
    working_dir: &Path,
    capture_stdout: bool,
) -> Result<ProcessOutput, BenchError> {
    let mut command = std::process::Command::new(program);
    command.args(args).current_dir(working_dir);
    command
        .stdin(Stdio::piped())
        .stdout(if capture_stdout {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(Stdio::piped());
    let program_name = program.display().to_string();
    let mut child = command.spawn().map_err(|source| BenchError::Spawn {
        program: program_name.clone(),
        source,
    })?;
    if let Some(mut child_stdin) = child.stdin.take() {
        child_stdin
            .write_all(stdin)
            .map_err(|source| BenchError::WriteStdin {
                program: program_name.clone(),
                source,
            })?;
    }
    let stdout_reader = child.stdout.take().map(spawn_pipe_reader);
    let stderr_reader = child.stderr.take().map(spawn_pipe_reader);
    let status = match child
        .wait_timeout(COMMAND_TIMEOUT)
        .map_err(|source| BenchError::Wait {
            program: program_name.clone(),
            source,
        })? {
        Some(status) => status,
        None => {
            let _kill_result = child.kill();
            let _wait_result = child.wait();
            return Err(BenchError::TimedOut {
                program: program_name,
                seconds: COMMAND_TIMEOUT.as_secs(),
            });
        }
    };
    let stdout = join_pipe_reader(stdout_reader, &program_name)?;
    let stderr = join_pipe_reader(stderr_reader, &program_name)?;
    Ok(ProcessOutput {
        status: status.code(),
        stdout,
        stderr,
    })
}

type PipeReader = JoinHandle<Result<Vec<u8>, std::io::Error>>;

fn spawn_pipe_reader(mut pipe: impl Read + Send + 'static) -> PipeReader {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

fn join_pipe_reader(reader: Option<PipeReader>, program: &str) -> Result<Vec<u8>, BenchError> {
    let Some(reader) = reader else {
        return Ok(Vec::new());
    };
    match reader.join() {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(source)) => Err(BenchError::Wait {
            program: program.to_string(),
            source,
        }),
        Err(_) => Err(BenchError::Wait {
            program: program.to_string(),
            source: std::io::Error::other("pipe reader thread panicked"),
        }),
    }
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
