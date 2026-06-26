//! Shared helpers for bounded process execution.

use std::process::{ExitCode, Stdio};

use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub(crate) const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 16 * 1024;
pub(crate) const INTERRUPTED_EXIT: u8 = 130;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandOutput {
    pub(crate) status: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) truncated: bool,
}

impl CommandOutput {
    pub(crate) fn success(&self) -> bool {
        self.status == Some(0)
    }
}

#[derive(Debug, Error)]
pub(crate) enum SupportError {
    #[error("failed to start {program}: {message}")]
    ProcessStart { program: String, message: String },

    #[error("failed while waiting for {program}: {message}")]
    ProcessWait { program: String, message: String },
}

pub(crate) async fn run_command(
    mut command: Command,
    program: &str,
    limit_bytes: usize,
) -> Result<CommandOutput, SupportError> {
    let mut child = command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| SupportError::ProcessStart {
            program: program.to_string(),
            message: error.to_string(),
        })?;

    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let (status, stdout, stderr) = tokio::join!(
        child.wait(),
        read_bounded(stdout.as_mut(), limit_bytes),
        read_bounded(stderr.as_mut(), limit_bytes)
    );
    let status = status.map_err(|error| SupportError::ProcessWait {
        program: program.to_string(),
        message: error.to_string(),
    })?;
    let (stdout, stdout_truncated) = stdout.map_err(|error| SupportError::ProcessWait {
        program: program.to_string(),
        message: error.to_string(),
    })?;
    let (stderr, stderr_truncated) = stderr.map_err(|error| SupportError::ProcessWait {
        program: program.to_string(),
        message: error.to_string(),
    })?;

    Ok(CommandOutput {
        status: status.code(),
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        truncated: stdout_truncated || stderr_truncated,
    })
}

async fn read_bounded<R>(
    stream: Option<&mut R>,
    limit_bytes: usize,
) -> Result<(Vec<u8>, bool), std::io::Error>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(stream) = stream else {
        return Ok((Vec::new(), false));
    };
    let mut data = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut truncated = false;
    loop {
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        let remaining = limit_bytes.saturating_sub(data.len());
        let read_chunk = buffer.get(..bytes_read).ok_or_else(|| {
            std::io::Error::other("bounded process-output reader exceeded buffer bounds")
        })?;
        if bytes_read <= remaining {
            data.extend_from_slice(read_chunk);
        } else {
            let kept_chunk = read_chunk.get(..remaining).ok_or_else(|| {
                std::io::Error::other("bounded process-output reader exceeded keep bounds")
            })?;
            data.extend_from_slice(kept_chunk);
            truncated = true;
        }
    }
    Ok((data, truncated))
}

pub(crate) fn interrupted() -> ExitCode {
    ExitCode::from(INTERRUPTED_EXIT)
}
