//! Supervised invocations of the pinned binaries, including the inputs that
//! benchmark cases derive from them.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use super::data::PinnedInputs;
use super::model::Policy;
use crate::error::BenchError;
use crate::process::{
    OutputPolicy, ProcessEnvironment, ProcessLaunch, ProcessLimits, ProcessRequest, ProcessResult,
    UnreadStdin, bench_launcher, run_bounded_process,
};

const STDERR_LIMIT_BYTES: usize = 64 << 10;

/// Derives case inputs from the pinned Stim and Stab binaries outside any
/// timed region.
pub(super) struct PinnedCommands<'a> {
    stim: &'a Path,
    stab: &'a Path,
    working_directory: &'a Path,
    policy: &'a Policy,
    cpu: Option<usize>,
}

impl<'a> PinnedCommands<'a> {
    pub(super) fn new(
        stim: &'a Path,
        stab: &'a Path,
        working_directory: &'a Path,
        policy: &'a Policy,
        cpu: Option<usize>,
    ) -> Self {
        Self {
            stim,
            stab,
            working_directory,
            policy,
            cpu,
        }
    }

    fn run(&self, binary: &Path, args: &[String], stdin: &[u8]) -> Result<Vec<u8>, String> {
        let args = args.iter().map(OsString::from).collect::<Vec<_>>();
        let result = run_command(
            binary,
            &args,
            stdin,
            self.working_directory,
            true,
            self.cpu,
            self.policy,
        )
        .map_err(|source| source.to_string())?;
        checked_success(binary, &result).map_err(|source| source.to_string())?;
        Ok(result.stdout)
    }

    /// Runs both binaries and requires byte-identical deterministic output.
    fn agreed(&self, args: &[String], stdin: &[u8], what: &str) -> Result<Vec<u8>, String> {
        let stim = self.run(self.stim, args, stdin)?;
        let stab = self.run(self.stab, args, stdin)?;
        if stim != stab {
            return Err(format!("{what} differs from pinned Stim"));
        }
        Ok(stim)
    }
}

impl PinnedInputs for PinnedCommands<'_> {
    fn circuit(&mut self, args: &[String]) -> Result<Vec<u8>, String> {
        self.agreed(args, b"", "generated circuit")
    }

    fn dem(&mut self, circuit: &[String], analyze: &[String]) -> Result<Vec<u8>, String> {
        let circuit = self.circuit(circuit)?;
        self.agreed(analyze, &circuit, "analyzed detector error model")
    }

    fn stim_records(&mut self, circuit: &[String], sample: &[String]) -> Result<Vec<u8>, String> {
        let circuit = self.circuit(circuit)?;
        self.run(self.stim, sample, &circuit)
    }
}

pub(super) fn run_command(
    binary: &Path,
    args: &[OsString],
    stdin: &[u8],
    working_directory: &Path,
    capture: bool,
    cpu: Option<usize>,
    policy: &Policy,
) -> Result<ProcessResult, BenchError> {
    let output_policy = if capture {
        OutputPolicy::Capture {
            maximum_bytes: policy.maximum_output_bytes,
        }
    } else {
        OutputPolicy::Discard
    };
    let launcher = bench_launcher()
        .map_err(|source| BenchError::E2e(format!("locate measurement launcher: {source}")))?;
    run_bounded_process(&ProcessRequest {
        launch: ProcessLaunch::Measured(launcher),
        program: binary.to_path_buf(),
        args: args.to_vec(),
        stdin: stdin.to_vec(),
        unread_stdin: UnreadStdin::Reject,
        working_directory: working_directory.to_path_buf(),
        environment: ProcessEnvironment::Inherit,
        affinity_cpu: cpu,
        limits: ProcessLimits {
            stdin_bytes: stdin.len(),
            stdout: output_policy,
            stderr: OutputPolicy::Capture {
                maximum_bytes: STDERR_LIMIT_BYTES,
            },
            regular_file_bytes: Some(policy.maximum_output_bytes as u64),
            timeout: Duration::from_secs(policy.command_timeout_seconds),
        },
    })
    .map_err(BenchError::from)
}

pub(super) fn checked_success(binary: &Path, result: &ProcessResult) -> Result<(), BenchError> {
    if result.status == Some(0) && result.stderr_bytes == 0 {
        return Ok(());
    }
    Err(BenchError::E2e(format!(
        "{} failed with status {:?} and stderr {}",
        binary.display(),
        result.status,
        String::from_utf8_lossy(&result.stderr)
    )))
}

pub(super) fn command_output(
    binary: &Path,
    args: &[&str],
    stdin: &[u8],
    working_directory: &Path,
    maximum_output: usize,
    cpu: Option<usize>,
    timeout_seconds: u64,
) -> Result<Vec<u8>, BenchError> {
    let args = args.iter().map(OsString::from).collect::<Vec<_>>();
    let result = run_bounded_process(&ProcessRequest {
        launch: ProcessLaunch::Direct,
        program: binary.to_path_buf(),
        args,
        stdin: stdin.to_vec(),
        unread_stdin: UnreadStdin::Reject,
        working_directory: working_directory.to_path_buf(),
        environment: ProcessEnvironment::Inherit,
        affinity_cpu: cpu,
        limits: ProcessLimits {
            stdin_bytes: stdin.len(),
            stdout: OutputPolicy::Capture {
                maximum_bytes: maximum_output,
            },
            stderr: OutputPolicy::Capture {
                maximum_bytes: maximum_output,
            },
            regular_file_bytes: None,
            timeout: Duration::from_secs(timeout_seconds),
        },
    })?;
    if result.status != Some(0) {
        return Err(BenchError::E2e(format!(
            "{} failed: {}",
            binary.display(),
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    Ok(result.stdout)
}
