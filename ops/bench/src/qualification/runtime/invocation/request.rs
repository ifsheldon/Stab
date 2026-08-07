//! One owner for qualification-worker request construction.
//!
//! [`WorkerRequestSpec`] owns the worker argv spelling and order (including
//! the Stab `qualification-worker` subcommand prefix), the bounded process
//! limits and locale-pinned environment shared by formal invocations, probes,
//! and rejection preflights, and the shared comparison rule for rejection
//! expectations. Call sites choose values; this module chooses how they reach
//! the worker.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use super::super::process::{ProcessLimits, ProcessRequest, ProcessResult};
use super::super::protocol::{EvidenceMode, Implementation};

const WORKER_STDOUT_LIMIT: usize = 1 << 20;
const WORKER_STDERR_LIMIT: usize = 64 << 10;
const STAB_WORKER_SUBCOMMAND: &str = "qualification-worker";

/// A fully described qualification-worker request: workload identity, work
/// shape, evidence mode, optional protocol flags, barrier policy, and the
/// invocation timeout.
#[derive(Clone, Debug)]
pub(in crate::qualification::runtime) struct WorkerRequestSpec {
    workload: String,
    measurement_id: String,
    iterations: String,
    work_items: String,
    evidence_mode: EvidenceMode,
    start_barrier: Option<bool>,
    expected_cpu: Option<u32>,
    input_descriptor_hex: Option<String>,
    input_family: Option<String>,
    release_barrier: bool,
    timeout: Duration,
}

impl WorkerRequestSpec {
    pub(in crate::qualification::runtime) fn new(
        workload: impl Into<String>,
        measurement_id: impl Into<String>,
        iterations: impl ToString,
        work_items: impl ToString,
        evidence_mode: EvidenceMode,
        timeout: Duration,
    ) -> Self {
        Self {
            workload: workload.into(),
            measurement_id: measurement_id.into(),
            iterations: iterations.to_string(),
            work_items: work_items.to_string(),
            evidence_mode,
            start_barrier: None,
            expected_cpu: None,
            input_descriptor_hex: None,
            input_family: None,
            release_barrier: false,
            timeout,
        }
    }

    /// Passes `--start-barrier <value>` so the worker waits for a stdin
    /// newline before measured work. Rejection preflights combine this with
    /// an empty stdin: the worker must fail validation before the barrier.
    pub(in crate::qualification::runtime) fn start_barrier(mut self, value: bool) -> Self {
        self.start_barrier = Some(value);
        self
    }

    /// Sends the one-newline stdin payload that releases the start barrier.
    pub(in crate::qualification::runtime) fn release_barrier(mut self) -> Self {
        self.release_barrier = true;
        self
    }

    pub(in crate::qualification::runtime) fn expected_cpu(mut self, cpu: u32) -> Self {
        self.expected_cpu = Some(cpu);
        self
    }

    pub(in crate::qualification::runtime) fn input_descriptor_hex(
        mut self,
        descriptor: impl Into<String>,
    ) -> Self {
        self.input_descriptor_hex = Some(descriptor.into());
        self
    }

    pub(in crate::qualification::runtime) fn input_family(
        mut self,
        family: impl Into<String>,
    ) -> Self {
        self.input_family = Some(family.into());
        self
    }

    /// Builds the worker argv, prefixing the Stab worker subcommand for the
    /// private Stab binary.
    pub(in crate::qualification::runtime) fn arguments(
        &self,
        implementation: Implementation,
    ) -> Vec<OsString> {
        let mut arguments = Vec::with_capacity(19);
        if implementation == Implementation::Stab {
            arguments.push(OsString::from(STAB_WORKER_SUBCOMMAND));
        }
        arguments.extend([
            OsString::from("--workload"),
            OsString::from(&self.workload),
            OsString::from("--measurement-id"),
            OsString::from(&self.measurement_id),
            OsString::from("--iterations"),
            OsString::from(&self.iterations),
            OsString::from("--work-items"),
            OsString::from(&self.work_items),
            OsString::from("--evidence-mode"),
            OsString::from(evidence_mode_argument(self.evidence_mode)),
        ]);
        if let Some(start_barrier) = self.start_barrier {
            arguments.push(OsString::from("--start-barrier"));
            arguments.push(OsString::from(if start_barrier { "true" } else { "false" }));
        }
        if let Some(expected_cpu) = self.expected_cpu {
            arguments.push(OsString::from("--expected-cpu"));
            arguments.push(OsString::from(expected_cpu.to_string()));
        }
        if let Some(descriptor) = &self.input_descriptor_hex {
            arguments.push(OsString::from("--input-descriptor-hex"));
            arguments.push(OsString::from(descriptor));
        }
        if let Some(family) = &self.input_family {
            arguments.push(OsString::from("--input-family"));
            arguments.push(OsString::from(family));
        }
        arguments
    }

    /// Assembles the bounded process request for one implementation.
    pub(in crate::qualification::runtime) fn process_request(
        &self,
        implementation: Implementation,
        program: PathBuf,
        working_directory: PathBuf,
        affinity_cpu: Option<usize>,
    ) -> ProcessRequest {
        let stdin = if self.release_barrier {
            vec![b'\n']
        } else {
            Vec::new()
        };
        ProcessRequest {
            program,
            args: self.arguments(implementation),
            stdin,
            working_directory,
            environment: worker_environment().into(),
            affinity_cpu,
            limits: ProcessLimits {
                stdin_bytes: usize::from(self.release_barrier),
                stdout: WORKER_STDOUT_LIMIT.into(),
                stderr: WORKER_STDERR_LIMIT.into(),
                regular_file_bytes: None,
                timeout: self.timeout,
            },
        }
    }
}

/// Parent-side spelling of the `--evidence-mode` argument.
const fn evidence_mode_argument(mode: EvidenceMode) -> &'static str {
    match mode {
        EvidenceMode::Contract => "contract",
        EvidenceMode::Timing => "timing",
        EvidenceMode::Memory => "memory",
    }
}

/// Locale- and timezone-pinned environment shared by every worker process.
pub(in crate::qualification::runtime) fn worker_environment() -> Vec<(OsString, OsString)> {
    vec![
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (OsString::from("TZ"), OsString::from("UTC")),
    ]
}

/// Shared rejection expectation: the worker must exit with the expected
/// status, produce no stdout, and match the expected stderr byte-for-byte.
pub(in crate::qualification::runtime) fn matches_rejection(
    output: &ProcessResult,
    expected_status: i32,
    expected_stderr: &str,
) -> bool {
    output.status == Some(expected_status)
        && output.stdout.is_empty()
        && output.stderr == expected_stderr.as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(arguments: &[OsString]) -> Vec<String> {
        arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn spec_owns_argv_order_prefix_and_optional_flags() {
        let spec = WorkerRequestSpec::new(
            "dem-parse",
            "parse",
            4_u64,
            256_u64,
            EvidenceMode::Timing,
            Duration::from_secs(30),
        )
        .start_barrier(true)
        .expected_cpu(2)
        .input_descriptor_hex("00ff")
        .input_family("dem-r6");
        assert_eq!(
            text(&spec.arguments(Implementation::Stab)),
            [
                "qualification-worker",
                "--workload",
                "dem-parse",
                "--measurement-id",
                "parse",
                "--iterations",
                "4",
                "--work-items",
                "256",
                "--evidence-mode",
                "timing",
                "--start-barrier",
                "true",
                "--expected-cpu",
                "2",
                "--input-descriptor-hex",
                "00ff",
                "--input-family",
                "dem-r6",
            ]
        );
        let stim = spec.arguments(Implementation::Stim);
        assert_eq!(
            stim.first().and_then(|value| value.to_str()),
            Some("--workload")
        );
        assert_eq!(stim.len(), 18);
    }

    #[test]
    fn process_request_bounds_every_protocol_stream_and_owns_the_barrier_payload() {
        let spec = WorkerRequestSpec::new(
            "protocol-smoke",
            "main",
            "1",
            "64",
            EvidenceMode::Contract,
            Duration::from_secs(5),
        )
        .start_barrier(true);
        let unreleased = spec.clone().process_request(
            Implementation::Stim,
            PathBuf::from("/adapter"),
            PathBuf::from("/repo"),
            None,
        );
        assert!(unreleased.stdin.is_empty());
        assert_eq!(unreleased.limits.stdin_bytes, 0);
        assert_eq!(unreleased.limits.stdout, WORKER_STDOUT_LIMIT.into());
        assert_eq!(unreleased.limits.stderr, WORKER_STDERR_LIMIT.into());
        assert_eq!(unreleased.limits.regular_file_bytes, None);
        assert_eq!(unreleased.limits.timeout, Duration::from_secs(5));

        let released = spec.release_barrier().process_request(
            Implementation::Stab,
            PathBuf::from("/worker"),
            PathBuf::from("/repo"),
            Some(3),
        );
        assert_eq!(released.stdin, vec![b'\n']);
        assert_eq!(released.limits.stdin_bytes, 1);
        assert_eq!(released.affinity_cpu, Some(3));
    }

    #[test]
    fn rejection_rule_requires_exact_status_empty_stdout_and_exact_stderr() {
        let output = |status, stdout: &str, stderr: &str| ProcessResult {
            status,
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
            parent_observed_peak_rss_bytes: None,
            wall_elapsed: Duration::from_millis(1),
        };
        assert!(matches_rejection(
            &output(Some(2), "", "denied\n"),
            2,
            "denied\n"
        ));
        assert!(!matches_rejection(
            &output(Some(1), "", "denied\n"),
            2,
            "denied\n"
        ));
        assert!(!matches_rejection(
            &output(None, "", "denied\n"),
            2,
            "denied\n"
        ));
        assert!(!matches_rejection(
            &output(Some(2), "row\n", "denied\n"),
            2,
            "denied\n"
        ));
        assert!(!matches_rejection(
            &output(Some(2), "", "denied\nextra\n"),
            2,
            "denied\n"
        ));
    }
}
