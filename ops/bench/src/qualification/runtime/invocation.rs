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
    EvidenceMode, GitCommit, Implementation, InputDigest, ProtocolExpectation, ProtocolId,
    SemanticDigest, WorkerMeasurement, parse_worker_json_lines,
};
use crate::config::STIM_COMMIT;
use crate::root::RepoRoot;

const PROTOCOL_OUTPUT_LIMIT: usize = 1 << 20;
const IDENTITY_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const CAP_REJECTION_TIMEOUT: Duration = Duration::from_secs(5);
const FIRST_UNSUPPORTED_CIRCUIT_INSTRUCTIONS: &str = "1000001";
const EMPTY_PROTOCOL_INPUT_DIGEST: &str =
    "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1";
pub(super) const PQ1_GROUP_ID: &str = "pq1-adapter-protocol-smoke";
pub(super) const CIRCUIT_PARSE_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-PARSE";
pub(super) const CIRCUIT_CANONICAL_PRINT_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-CANONICAL-PRINT";

pub(super) fn supports_group(contract: &super::group::GroupContract) -> bool {
    let identity = (
        contract.id.to_string(),
        contract.workload_id.to_string(),
        contract.measurement_ids.first().map(ToString::to_string),
        contract.measurement_ids.len(),
    );
    matches!(
        identity,
        (group, workload, Some(measurement), 1)
            if (group == PQ1_GROUP_ID
                && workload == "protocol-smoke"
                && measurement == "main")
                || (group == CIRCUIT_PARSE_GROUP_ID
                    && workload == "circuit-parse"
                    && measurement == "parse")
                || (group == CIRCUIT_CANONICAL_PRINT_GROUP_ID
                    && workload == "circuit-canonical-print"
                    && measurement == "serialize")
    )
}

pub(super) const fn registered_group_count() -> usize {
    3
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

pub(super) fn verify_private_worker_reproducibility(
    root: &RepoRoot,
) -> Result<WorkerIdentityEvidence, InvocationError> {
    let repository_before = super::git::repository_state(root)?;
    require_reproducibility_repository(&repository_before, &repository_before)?;
    let toolchain = super::toolchain::collect(root)?;
    let first = PreparedWorkers::prepare(root, &repository_before.commit, &toolchain)?;
    first.verify_identity_handshake()?;
    let first_identity = first.identity_evidence();
    drop(first);
    let second = PreparedWorkers::prepare(root, &repository_before.commit, &toolchain)?;
    second.verify_identity_handshake()?;
    let second_identity = second.identity_evidence();
    drop(second);
    let repository_after = super::git::repository_state(root)?;
    require_reproducibility_repository(&repository_before, &repository_after)?;
    if first_identity != second_identity {
        return Err(InvocationError::NonReproducibleWorkers {
            first: Box::new(first_identity),
            second: Box::new(second_identity),
        });
    }
    Ok(first_identity)
}

fn require_reproducibility_repository(
    before: &super::git::RepositoryState,
    after: &super::git::RepositoryState,
) -> Result<(), InvocationError> {
    if before.local_modifications || after.local_modifications {
        return Err(InvocationError::DirtyReproducibilityRepository);
    }
    if before.commit != after.commit {
        return Err(InvocationError::ReproducibilityRepositoryChanged {
            before: before.commit.clone(),
            after: after.commit.clone(),
        });
    }
    Ok(())
}

pub(super) struct InvocationRequest<'a> {
    pub(super) group: &'a super::group::GroupContract,
    pub(super) implementation: Implementation,
    pub(super) evidence_mode: EvidenceMode,
    pub(super) iterations: NonZeroU64,
    pub(super) scale: &'a super::group::ScaleContract,
    pub(super) expected_output_digest: Option<&'a SemanticDigest>,
    pub(super) timeout: Duration,
}

impl PreparedWorkers {
    pub(crate) fn prepare(
        root: &RepoRoot,
        repository_commit: &str,
        toolchain: &super::toolchain::ToolchainEvidence,
    ) -> Result<Self, InvocationError> {
        let adapter = prepare_adapter(root, repository_commit)?;
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
        request: InvocationRequest<'_>,
    ) -> Result<InvocationRecord, InvocationError> {
        let InvocationRequest {
            group,
            implementation,
            evidence_mode,
            iterations,
            scale,
            expected_output_digest,
            timeout,
        } = request;
        if !supports_group(group) {
            return Err(InvocationError::UnsupportedGroup(group.id.to_string()));
        }
        let measurement_id = group.single_measurement()?;
        let cpu = self.cpu.ok_or(InvocationError::MissingCpu)?;
        let expected_cpu = u32::try_from(cpu).map_err(|_| InvocationError::CpuRange(cpu))?;
        let expected_work_count = checked_work_count(iterations, scale.work_items)?;
        let mut arguments = vec![
            OsString::from("--workload"),
            OsString::from(group.workload_id.to_string()),
            OsString::from("--measurement-id"),
            OsString::from(measurement_id.to_string()),
            OsString::from("--iterations"),
            OsString::from(iterations.get().to_string()),
            OsString::from("--work-items"),
            OsString::from(scale.work_items.get().to_string()),
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
            workload_id: group.workload_id.clone(),
            measurement_ids: BTreeSet::from([measurement_id.clone()]),
            iteration_count: iterations.get(),
            expected_work_count,
            expected_input_bytes: scale.input_bytes,
            expected_input_digest: scale.input_digest.clone(),
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

    fn verify_identity_handshake(&self) -> Result<(), InvocationError> {
        let stim_output = self.invoke_identity_probe(Implementation::Stim, None)?;
        self.invoke_identity_probe(Implementation::Stab, Some(&stim_output))?;
        self.invoke_cap_rejection(Implementation::Stim)?;
        self.invoke_cap_rejection(Implementation::Stab)?;
        Ok(())
    }

    fn invoke_identity_probe(
        &self,
        implementation: Implementation,
        expected_output_digest: Option<&SemanticDigest>,
    ) -> Result<SemanticDigest, InvocationError> {
        let mut arguments = vec![
            OsString::from("--workload"),
            OsString::from("protocol-smoke"),
            OsString::from("--measurement-id"),
            OsString::from("main"),
            OsString::from("--iterations"),
            OsString::from("1"),
            OsString::from("--work-items"),
            OsString::from("1"),
            OsString::from("--evidence-mode"),
            OsString::from("timing"),
            OsString::from("--start-barrier"),
            OsString::from("true"),
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
            affinity_cpu: None,
            limits: ProcessLimits {
                stdin_bytes: 1,
                stdout_bytes: PROTOCOL_OUTPUT_LIMIT,
                stderr_bytes: 64 << 10,
                regular_file_bytes: None,
                timeout: IDENTITY_PROBE_TIMEOUT,
            },
        })?;
        let process = checked_process(process, implementation)?;
        let rows = parse_worker_json_lines(&process.stdout)?;
        ProtocolExpectation {
            implementation,
            evidence_mode: EvidenceMode::Timing,
            workload_id: ProtocolId::try_new("protocol-smoke")?,
            measurement_ids: BTreeSet::from([ProtocolId::try_new("main")?]),
            iteration_count: 1,
            expected_work_count: 1,
            expected_input_bytes: 0,
            expected_input_digest: InputDigest::try_new(EMPTY_PROTOCOL_INPUT_DIGEST)?,
            expected_output_digest: expected_output_digest.cloned(),
            affinity_cpu: None,
            stim_commit: GitCommit::try_new(STIM_COMMIT)?,
            source_digest,
            build_fingerprint,
        }
        .validate(&rows)?;
        rows.into_iter()
            .next()
            .map(|row| row.output_digest)
            .ok_or(InvocationError::MissingMeasurement)
    }

    fn invoke_cap_rejection(&self, implementation: Implementation) -> Result<(), InvocationError> {
        let mut arguments = vec![
            OsString::from("--workload"),
            OsString::from("circuit-parse"),
            OsString::from("--measurement-id"),
            OsString::from("parse"),
            OsString::from("--iterations"),
            OsString::from("1"),
            OsString::from("--work-items"),
            OsString::from(FIRST_UNSUPPORTED_CIRCUIT_INSTRUCTIONS),
            OsString::from("--evidence-mode"),
            OsString::from("timing"),
            OsString::from("--start-barrier"),
            OsString::from("true"),
        ];
        let program = match implementation {
            Implementation::Stim => self.adapter.path.clone(),
            Implementation::Stab => {
                arguments.insert(0, OsString::from("qualification-worker"));
                self.worker.program()
            }
        };
        let output = run_bounded_process(&ProcessRequest {
            program,
            args: arguments,
            stdin: Vec::new(),
            working_directory: self.root.clone(),
            environment: worker_environment(),
            affinity_cpu: None,
            limits: ProcessLimits {
                stdin_bytes: 0,
                stdout_bytes: PROTOCOL_OUTPUT_LIMIT,
                stderr_bytes: 64 << 10,
                regular_file_bytes: None,
                timeout: CAP_REJECTION_TIMEOUT,
            },
        })?;
        checked_cap_rejection(output, implementation)
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

fn checked_cap_rejection(
    output: ProcessResult,
    implementation: Implementation,
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) = match implementation {
        Implementation::Stim => (
            Some(2),
            "stim qualification adapter: circuit-parse instruction count exceeds the source-owned limit\n",
        ),
        Implementation::Stab => (
            Some(1),
            "[stab-bench] ERROR: performance qualification validation failed:\ncircuit-parse scale has 1000001 instructions, maximum 1000000\n",
        ),
    };
    if output.status != expected_status
        || !output.stdout.is_empty()
        || output.stderr != expected_stderr.as_bytes()
    {
        return Err(InvocationError::CapRejection {
            implementation,
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
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
    #[error(transparent)]
    Group(#[from] super::group::GroupError),
    #[error(transparent)]
    Git(#[from] super::git::GitError),
    #[error(transparent)]
    Toolchain(#[from] super::toolchain::ToolchainError),
    #[error(
        "private worker reproducibility requires a clean checkout before and after both builds"
    )]
    DirtyReproducibilityRepository,
    #[error("private worker reproducibility checkout changed from {before} to {after}")]
    ReproducibilityRepositoryChanged { before: String, after: String },
    #[error(
        "private Stim or Stab worker builds produced different identities: first={first:?}, second={second:?}"
    )]
    NonReproducibleWorkers {
        first: Box<WorkerIdentityEvidence>,
        second: Box<WorkerIdentityEvidence>,
    },
    #[error("qualification runtime group is not implemented by both workers: {0}")]
    UnsupportedGroup(String),
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
    #[error(
        "{implementation} did not reject the first unsupported circuit-parse scale before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    CapRejection {
        implementation: Implementation,
        status: Option<i32>,
        stdout: String,
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
    #[cfg(target_os = "linux")]
    use std::path::Path;

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

    #[test]
    fn reproducibility_requires_one_clean_unchanged_commit() {
        let state = |commit: char, dirty| super::super::git::RepositoryState {
            commit: commit.to_string().repeat(40),
            local_modifications: dirty,
        };
        assert!(matches!(
            require_reproducibility_repository(&state('a', true), &state('a', false)),
            Err(InvocationError::DirtyReproducibilityRepository)
        ));
        assert!(matches!(
            require_reproducibility_repository(&state('a', false), &state('b', false)),
            Err(InvocationError::ReproducibilityRepositoryChanged { before, after })
                if before == "a".repeat(40) && after == "b".repeat(40)
        ));
    }

    #[test]
    fn cap_rejection_requires_the_worker_limit_before_the_start_barrier() {
        let output = |status, stderr: &str| ProcessResult {
            status,
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
            parent_observed_peak_rss_bytes: None,
            wall_elapsed: Duration::from_millis(1),
        };
        checked_cap_rejection(
            output(
                Some(2),
                "stim qualification adapter: circuit-parse instruction count exceeds the source-owned limit\n",
            ),
            Implementation::Stim,
        )
        .expect("adapter cap rejection");
        checked_cap_rejection(
            output(
                Some(1),
                "[stab-bench] ERROR: performance qualification validation failed:\ncircuit-parse scale has 1000001 instructions, maximum 1000000\n",
            ),
            Implementation::Stab,
        )
        .expect("Stab cap rejection");
        assert!(matches!(
            checked_cap_rejection(
                output(
                    Some(2),
                    "stim qualification adapter error: start barrier must contain one newline\n"
                ),
                Implementation::Stim,
            ),
            Err(InvocationError::CapRejection { .. })
        ));
        let signaled = output(
            None,
            "stim qualification adapter: circuit-parse instruction count exceeds the source-owned limit\n",
        );
        assert!(matches!(
            checked_cap_rejection(signaled, Implementation::Stim),
            Err(InvocationError::CapRejection { .. })
        ));
        assert!(matches!(
            checked_cap_rejection(
                output(
                    Some(2),
                    "stim qualification adapter: circuit-parse instruction count exceeds the source-owned limit\nunrelated error\n"
                ),
                Implementation::Stim,
            ),
            Err(InvocationError::CapRejection { .. })
        ));
    }

    #[test]
    #[cfg(target_os = "linux")]
    #[ignore = "builds the pinned Stim adapter and Stab worker twice"]
    fn private_worker_builds_are_byte_reproducible() {
        let root = RepoRoot::resolve(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("repository root");
        verify_private_worker_reproducibility(&root).expect("reproducible private workers");
    }
}
