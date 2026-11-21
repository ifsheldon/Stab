//! Supervised invocations of the pinned binaries, including the inputs that
//! benchmark cases derive from them.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use super::data::PinnedInputs;
use super::error::{AgreedOutput, CommandError};
use super::model::Policy;
use crate::process::{
    OutputPolicy, ProcessLimits, ProcessRequest, ProcessResult, bench_launcher, run_bounded_process,
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

    fn run(&self, binary: &Path, args: &[String], stdin: &[u8]) -> Result<Vec<u8>, CommandError> {
        let args = args.iter().map(OsString::from).collect::<Vec<_>>();
        let result = run_command(
            binary,
            &args,
            stdin,
            self.working_directory,
            true,
            self.cpu,
            self.policy,
        )?;
        checked_success(binary, &result)?;
        Ok(result.stdout)
    }

    /// Runs both binaries and requires byte-identical deterministic output.
    fn agreed(
        &self,
        args: &[String],
        stdin: &[u8],
        what: AgreedOutput,
    ) -> Result<Vec<u8>, CommandError> {
        let stim = self.run(self.stim, args, stdin)?;
        let stab = self.run(self.stab, args, stdin)?;
        if stim != stab {
            return Err(CommandError::Disagreement { what });
        }
        Ok(stim)
    }
}

impl PinnedInputs for PinnedCommands<'_> {
    fn circuit(&mut self, args: &[String]) -> Result<Vec<u8>, CommandError> {
        self.agreed(args, b"", AgreedOutput::GeneratedCircuit)
    }

    fn dem(&mut self, circuit: &[String], analyze: &[String]) -> Result<Vec<u8>, CommandError> {
        let circuit = self.circuit(circuit)?;
        self.agreed(analyze, &circuit, AgreedOutput::AnalyzedDem)
    }

    fn stim_records(
        &mut self,
        circuit: &[String],
        sample: &[String],
    ) -> Result<Vec<u8>, CommandError> {
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
) -> Result<ProcessResult, CommandError> {
    let output_policy = if capture {
        OutputPolicy::Capture {
            maximum_bytes: policy.maximum_output_bytes,
        }
    } else {
        OutputPolicy::Discard
    };
    let launcher = bench_launcher().map_err(CommandError::Launcher)?;
    let limits = ProcessLimits {
        stdout: output_policy,
        stderr: OutputPolicy::Capture {
            maximum_bytes: STDERR_LIMIT_BYTES,
        },
        ..ProcessLimits::capture(0, Duration::from_secs(policy.command_timeout_seconds))
    };
    run_bounded_process(
        &ProcessRequest::direct(binary, args.to_vec(), working_directory, limits)
            .with_stdin(stdin)
            .with_affinity(cpu)
            .measured(launcher, Some(policy.maximum_output_bytes as u64)),
    )
    .map_err(CommandError::from)
}

pub(super) fn checked_success(binary: &Path, result: &ProcessResult) -> Result<(), CommandError> {
    if result.status.success() && result.stderr_bytes == 0 {
        return Ok(());
    }
    Err(CommandError::Unclean {
        program: binary.to_path_buf(),
        status: result.status,
        stderr: String::from_utf8_lossy(&result.stderr).into(),
    })
}

pub(super) fn command_output(
    binary: &Path,
    args: &[&str],
    stdin: &[u8],
    working_directory: &Path,
    maximum_output: usize,
    cpu: Option<usize>,
    timeout_seconds: u64,
) -> Result<Vec<u8>, CommandError> {
    let args = args.iter().map(OsString::from).collect::<Vec<_>>();
    let limits = ProcessLimits::capture(maximum_output, Duration::from_secs(timeout_seconds));
    let result = run_bounded_process(
        &ProcessRequest::direct(binary, args, working_directory, limits)
            .with_stdin(stdin)
            .with_affinity(cpu),
    )?;
    if !result.status.success() {
        return Err(CommandError::Failed {
            program: binary.to_path_buf(),
            stderr: String::from_utf8_lossy(&result.stderr).into(),
        });
    }
    Ok(result.stdout)
}
