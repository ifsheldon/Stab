use std::collections::BTreeSet;
use std::ffi::OsString;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::adapter::{AdapterExecutable, prepare_adapter};
use super::process::{ProcessLimits, ProcessRequest, ProcessResult, run_bounded_process};
use super::protocol::{
    EvidenceMode, GitCommit, Implementation, ProtocolExpectation, ProtocolId, SemanticDigest,
    WorkerMeasurement, parse_worker_json_lines,
};
use crate::config::STIM_COMMIT;
use crate::root::RepoRoot;

const PROTOCOL_OUTPUT_LIMIT: usize = 1 << 20;
const WORKLOAD_ID: &str = "protocol-smoke";
const MEASUREMENT_ID: &str = "main";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerIdentityEvidence {
    pub(super) stim_source_sha256: String,
    pub(super) stim_build_fingerprint: String,
    pub(super) stim_binary_sha256: String,
    pub(super) stab_source_sha256: String,
    pub(super) stab_build_fingerprint: String,
    pub(super) stab_binary_sha256: String,
}

#[derive(Debug)]
pub(crate) struct PreparedWorkers {
    root: PathBuf,
    adapter: AdapterExecutable,
    worker: super::stab_build::StabWorkerExecutable,
    repository_commit: String,
    toolchain: super::toolchain::ToolchainEvidence,
    cpu: Option<usize>,
}

impl PreparedWorkers {
    pub(crate) fn prepare(
        root: &RepoRoot,
        repository_commit: &str,
        toolchain: &super::toolchain::ToolchainEvidence,
    ) -> Result<Self, InvocationError> {
        let adapter = prepare_adapter(root)?;
        let worker =
            super::stab_build::StabWorkerExecutable::prepare(root, repository_commit, toolchain)?;
        let workers = Self {
            root: root.path.clone(),
            adapter,
            worker,
            repository_commit: repository_commit.to_string(),
            toolchain: toolchain.clone(),
            cpu: None,
        };
        workers.verify()?;
        Ok(workers)
    }

    pub(crate) fn pin_to_cpu(&mut self, cpu: usize) {
        self.cpu = Some(cpu);
    }

    pub(crate) fn identity_evidence(&self) -> WorkerIdentityEvidence {
        WorkerIdentityEvidence {
            stim_source_sha256: self.adapter.source_digest.as_str().to_string(),
            stim_build_fingerprint: self.adapter.build_fingerprint.as_str().to_string(),
            stim_binary_sha256: self.adapter.binary_digest.as_str().to_string(),
            stab_source_sha256: self.worker.identity().source_digest.as_str().to_string(),
            stab_build_fingerprint: self
                .worker
                .identity()
                .build_fingerprint
                .as_str()
                .to_string(),
            stab_binary_sha256: self.worker.binary_sha256().to_string(),
        }
    }

    pub(crate) fn adapter_receipt(&self) -> &super::adapter::AdapterBuildReceipt {
        &self.adapter.receipt
    }

    pub(crate) fn stab_build_receipt(&self) -> &super::stab_build::StabBuildReceipt {
        self.worker.receipt()
    }

    pub(crate) fn invoke(
        &self,
        implementation: Implementation,
        evidence_mode: EvidenceMode,
        iterations: NonZeroU64,
        work_items: NonZeroU64,
        expected_output_digest: Option<&SemanticDigest>,
        timeout: Duration,
    ) -> Result<InvocationRecord, InvocationError> {
        let cpu = self.cpu.ok_or(InvocationError::MissingCpu)?;
        let expected_cpu = u32::try_from(cpu).map_err(|_| InvocationError::CpuRange(cpu))?;
        let expected_work_count = checked_work_count(iterations, work_items)?;
        let mut arguments = vec![
            OsString::from("--workload"),
            OsString::from(WORKLOAD_ID),
            OsString::from("--iterations"),
            OsString::from(iterations.get().to_string()),
            OsString::from("--work-items"),
            OsString::from(work_items.get().to_string()),
            OsString::from("--evidence-mode"),
            OsString::from(match evidence_mode {
                EvidenceMode::Timing => "timing",
                EvidenceMode::Memory => "memory",
            }),
            OsString::from("--start-barrier"),
            OsString::from("true"),
            OsString::from("--expected-cpu"),
            OsString::from(expected_cpu.to_string()),
        ];
        let (program, source_digest, build_fingerprint) = match implementation {
            Implementation::Stim => (
                self.adapter.path.clone(),
                self.adapter.source_digest.clone(),
                self.adapter.build_fingerprint.clone(),
            ),
            Implementation::Stab => {
                arguments.insert(0, OsString::from("qualification-worker"));
                (
                    self.worker.program(),
                    self.worker.identity().source_digest.clone(),
                    self.worker.identity().build_fingerprint.clone(),
                )
            }
        };
        let process = run_bounded_process(&ProcessRequest {
            program,
            args: arguments,
            stdin: vec![b'\n'],
            working_directory: self.root.clone(),
            environment: worker_environment(),
            affinity_cpu: Some(cpu),
            limits: ProcessLimits {
                stdin_bytes: 1,
                stdout_bytes: PROTOCOL_OUTPUT_LIMIT,
                stderr_bytes: 64 << 10,
                regular_file_bytes: None,
                timeout,
            },
        })?;
        let process = checked_process(process, implementation)?;
        let rows = parse_worker_json_lines(&process.stdout)?;
        ProtocolExpectation {
            implementation,
            evidence_mode,
            workload_id: ProtocolId::try_new(WORKLOAD_ID)?,
            measurement_ids: BTreeSet::from([ProtocolId::try_new(MEASUREMENT_ID)?]),
            iteration_count: iterations.get(),
            expected_work_count,
            expected_output_digest: expected_output_digest.cloned(),
            affinity_cpu: Some(expected_cpu),
            stim_commit: GitCommit::try_new(STIM_COMMIT)?,
            source_digest,
            build_fingerprint,
        }
        .validate(&rows)?;
        Ok(InvocationRecord {
            implementation,
            evidence_mode,
            process_wall_seconds: process.wall_elapsed.as_secs_f64(),
            parent_observed_peak_rss_bytes: process.parent_observed_peak_rss_bytes,
            rows,
        })
    }

    pub(crate) fn verify(&self) -> Result<(), InvocationError> {
        self.adapter.verify()?;
        self.worker
            .verify(&self.toolchain, &self.repository_commit)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InvocationRecord {
    pub(super) implementation: Implementation,
    pub(super) evidence_mode: EvidenceMode,
    pub(super) process_wall_seconds: f64,
    pub(super) parent_observed_peak_rss_bytes: Option<u64>,
    pub(super) rows: Vec<WorkerMeasurement>,
}

impl InvocationRecord {
    pub(crate) fn measured_duration(&self) -> Result<Duration, InvocationError> {
        let row = self
            .rows
            .first()
            .ok_or(InvocationError::MissingMeasurement)?;
        Duration::try_from_secs_f64(row.elapsed_seconds)
            .map_err(|_| InvocationError::InvalidMeasuredDuration(row.elapsed_seconds))
    }

    pub(crate) fn wall_duration(&self) -> Result<Duration, InvocationError> {
        Duration::try_from_secs_f64(self.process_wall_seconds)
            .map_err(|_| InvocationError::InvalidWallDuration(self.process_wall_seconds))
    }
}

fn checked_process(
    output: ProcessResult,
    implementation: Implementation,
) -> Result<ProcessResult, InvocationError> {
    if output.status != Some(0) {
        return Err(InvocationError::WorkerFailed {
            implementation,
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    if !output.stderr.is_empty() {
        return Err(InvocationError::UnexpectedStderr {
            implementation,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(output)
}

fn worker_environment() -> Vec<(OsString, OsString)> {
    vec![
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (OsString::from("TZ"), OsString::from("UTC")),
    ]
}

fn checked_work_count(
    iterations: NonZeroU64,
    work_items: NonZeroU64,
) -> Result<u64, InvocationError> {
    iterations
        .get()
        .checked_mul(work_items.get())
        .ok_or(InvocationError::WorkOverflow)
}

pub(crate) fn protocol_ids() -> Result<(ProtocolId, ProtocolId), InvocationError> {
    Ok((
        ProtocolId::try_new(WORKLOAD_ID)?,
        ProtocolId::try_new(MEASUREMENT_ID)?,
    ))
}

#[derive(Debug, Error)]
pub(crate) enum InvocationError {
    #[error(transparent)]
    Adapter(#[from] super::adapter::AdapterError),
    #[error(transparent)]
    StabBuild(#[from] super::stab_build::StabBuildError),
    #[error(transparent)]
    Process(#[from] super::process::ProcessError),
    #[error(transparent)]
    Protocol(#[from] super::protocol::ProtocolError),
    #[error("qualification CPU {0} exceeds the shared worker protocol")]
    CpuRange(usize),
    #[error("qualification workers were invoked before selecting a host-policy CPU")]
    MissingCpu,
    #[error("qualification parent semantic work count overflows u64")]
    WorkOverflow,
    #[error(
        "{implementation} qualification worker failed with status {status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    WorkerFailed {
        implementation: Implementation,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("{implementation} qualification worker emitted unexpected stderr: {stderr}")]
    UnexpectedStderr {
        implementation: Implementation,
        stderr: String,
    },
    #[error("qualification invocation returned no measurement")]
    MissingMeasurement,
    #[error("qualification worker measured invalid duration {0}")]
    InvalidMeasuredDuration(f64),
    #[error("qualification process recorded invalid wall duration {0}")]
    InvalidWallDuration(f64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_rejects_semantic_work_overflow_before_invocation() {
        let maximum = NonZeroU64::new(u64::MAX).expect("positive maximum");
        let two = NonZeroU64::new(2).expect("positive two");
        assert!(matches!(
            checked_work_count(maximum, two),
            Err(InvocationError::WorkOverflow)
        ));
    }
}
