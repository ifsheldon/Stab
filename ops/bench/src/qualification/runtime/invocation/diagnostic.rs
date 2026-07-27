use std::collections::BTreeSet;
use std::ffi::OsString;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::super::process::{ProcessLimits, ProcessRequest, run_bounded_process};
use super::super::protocol::{
    EvidenceMode, GitCommit, Implementation, ProtocolExpectation, SemanticDigest,
    parse_worker_json_lines,
};
use super::{
    InvocationError, InvocationRecord, PROTOCOL_OUTPUT_LIMIT, checked_process, checked_work_count,
    supports_group, worker_environment,
};
use crate::config::STIM_COMMIT;
use crate::root::RepoRoot;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::qualification::runtime) struct DiagnosticWorkerIdentityEvidence {
    pub(in crate::qualification::runtime) stab_source_sha256: String,
    pub(in crate::qualification::runtime) stab_build_fingerprint: String,
    pub(in crate::qualification::runtime) stab_binary_sha256: String,
}

pub(in crate::qualification::runtime) struct PreparedDiagnosticWorker {
    root: PathBuf,
    worker: super::super::stab_build::StabWorkerExecutable,
    repository_commit: String,
    toolchain: super::super::toolchain::ToolchainEvidence,
    cpu: Option<usize>,
}

pub(in crate::qualification::runtime) struct DiagnosticInvocationRequest<'a> {
    pub(in crate::qualification::runtime) group: &'a super::super::group::GroupContract,
    pub(in crate::qualification::runtime) evidence_mode: EvidenceMode,
    pub(in crate::qualification::runtime) iterations: NonZeroU64,
    pub(in crate::qualification::runtime) scale: &'a super::super::group::ScaleContract,
    pub(in crate::qualification::runtime) expected_output_digest: Option<&'a SemanticDigest>,
    pub(in crate::qualification::runtime) timeout: Duration,
}

impl PreparedDiagnosticWorker {
    pub(in crate::qualification::runtime) fn prepare(
        root: &RepoRoot,
        repository_commit: &str,
        toolchain: &super::super::toolchain::ToolchainEvidence,
    ) -> Result<Self, InvocationError> {
        let worker = super::super::stab_build::StabWorkerExecutable::prepare(
            root,
            repository_commit,
            toolchain,
        )?;
        let prepared = Self {
            root: root.path.clone(),
            worker,
            repository_commit: repository_commit.to_string(),
            toolchain: toolchain.clone(),
            cpu: None,
        };
        prepared.verify()?;
        Ok(prepared)
    }

    pub(in crate::qualification::runtime) fn pin_to_cpu(&mut self, cpu: usize) {
        self.cpu = Some(cpu);
    }

    pub(in crate::qualification::runtime) fn invoke(
        &self,
        request: DiagnosticInvocationRequest<'_>,
    ) -> Result<InvocationRecord, InvocationError> {
        let cpu = self.cpu.ok_or(InvocationError::MissingCpu)?;
        let DiagnosticInvocationRequest {
            group,
            evidence_mode,
            iterations,
            scale,
            expected_output_digest,
            timeout,
        } = request;
        if !supports_group(group)
            || group.claim_class != super::super::run::ClaimClass::ProductDiagnostic
        {
            return Err(InvocationError::UnsupportedGroup(group.id.to_string()));
        }
        let measurement_id = group.single_measurement()?;
        let expected_cpu = u32::try_from(cpu).map_err(|_| InvocationError::CpuRange(cpu))?;
        let expected_work_count = checked_work_count(iterations, scale.work_items)?;
        let arguments = vec![
            OsString::from("qualification-worker"),
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
                EvidenceMode::Contract => "contract",
                EvidenceMode::Timing => "timing",
                EvidenceMode::Memory => "memory",
            }),
            OsString::from("--start-barrier"),
            OsString::from("true"),
            OsString::from("--expected-cpu"),
            OsString::from(expected_cpu.to_string()),
        ];
        let process = run_bounded_process(&ProcessRequest {
            program: self.worker.program(),
            args: arguments,
            stdin: vec![b'\n'],
            working_directory: self.root.clone(),
            environment: worker_environment().into(),
            affinity_cpu: Some(cpu),
            limits: ProcessLimits {
                stdin_bytes: 1,
                stdout: (PROTOCOL_OUTPUT_LIMIT).into(),
                stderr: (64 << 10).into(),
                regular_file_bytes: None,
                timeout,
            },
        })?;
        let process = checked_process(process, Implementation::Stab)?;
        let rows = parse_worker_json_lines(&process.stdout)?;
        ProtocolExpectation {
            implementation: Implementation::Stab,
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
            source_digest: self.worker.identity().source_digest.clone(),
            build_fingerprint: self.worker.identity().build_fingerprint.clone(),
        }
        .validate(&rows)?;
        Ok(InvocationRecord {
            implementation: Implementation::Stab,
            evidence_mode,
            process_wall_seconds: process.wall_elapsed.as_secs_f64(),
            parent_observed_peak_rss_bytes: process.parent_observed_peak_rss_bytes,
            rows,
        })
    }

    pub(in crate::qualification::runtime) fn identity_evidence(
        &self,
    ) -> DiagnosticWorkerIdentityEvidence {
        DiagnosticWorkerIdentityEvidence {
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

    pub(in crate::qualification::runtime) fn build_receipt(
        &self,
    ) -> &super::super::stab_build::StabBuildReceipt {
        self.worker.receipt()
    }

    pub(in crate::qualification::runtime) fn verify(&self) -> Result<(), InvocationError> {
        self.worker
            .verify(&self.toolchain, &self.repository_commit)
            .map_err(InvocationError::StabBuild)
    }
}
