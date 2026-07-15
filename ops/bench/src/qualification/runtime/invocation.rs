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

mod preflight;

pub(crate) use preflight::WorkerContractPreflightEvidence;
use preflight::{WorkerContractProbeEvidence, accepted_probe, rejected_probe};
#[cfg(test)]
use preflight::{expected_contract_preflight_probes, worker_contract_preflight_digest};

const PROTOCOL_OUTPUT_LIMIT: usize = 1 << 20;
const IDENTITY_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const CAP_REJECTION_TIMEOUT: Duration = Duration::from_secs(5);
const FIRST_UNSUPPORTED_CIRCUIT_INSTRUCTIONS: &str = "1000001";
const FIRST_PARTIAL_GATE_SWEEP_WORK_ITEMS: &str = "83";
const FIRST_UNSUPPORTED_POPCOUNT_BITS: &str = "268435712";
const FIRST_UNALIGNED_POPCOUNT_BITS: &str = "513";
const FIRST_BELOW_MINIMUM_POPCOUNT_BITS: &str = "256";
const MAX_SUPPORTED_POPCOUNT_BITS: u64 = 268_435_456;
const SMALL_POPCOUNT_BITS: u64 = 4_096;
const ODD_POPCOUNT_ITERATIONS: u64 = 1;
const EVEN_POPCOUNT_ITERATIONS: u64 = 2;
const SMALL_POPCOUNT_INPUT_DIGEST: &str =
    "101e05fc22ce0676c277e9b16363a38750079d12e0b93f3c687ed95457b79d1c";
const ODD_POPCOUNT_OUTPUT_DIGEST: &str =
    "b7c42176f3f0246013376d1d65756b9b6092f0aed397cb2afefd29eba663acf9";
const EVEN_POPCOUNT_OUTPUT_DIGEST: &str =
    "b29b34efb75f68c6c751edd91d96fecacef5d5032644a76bb36973ca427ea649";
const MAX_POPCOUNT_INPUT_DIGEST: &str =
    "cf5061f39d456d884fbdbcebfc53e04c47c29c872830a6a424f55d2e1e3d8ab4";
const MAX_POPCOUNT_OUTPUT_DIGEST: &str =
    "72b158a2870c2bca123553e5aca970f39107a3c7448bdbdda1512a9bcdfa33aa";
const EMPTY_PROTOCOL_INPUT_DIGEST: &str =
    "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1";
const PROTOCOL_SMOKE_OUTPUT_DIGEST: &str =
    "656c7d8a03ff449d0c248bdef4c3140b02252abffcd761d668e9bc4c63e0059d";
const CONTRACT_PREFLIGHT_SCHEMA_VERSION: u32 = 2;
const PROTOCOL_SMOKE_CASE_ID: &str = "protocol-smoke";
const POPCOUNT_ODD_CASE_ID: &str = "simd-word-popcount-odd";
const POPCOUNT_EVEN_CASE_ID: &str = "simd-word-popcount-even";
const POPCOUNT_MAXIMUM_CASE_ID: &str = "simd-word-popcount-maximum";
const CIRCUIT_CAP_CASE_ID: &str = "circuit-parse-over-cap";
const GATE_PARTIAL_SWEEP_CASE_ID: &str = "gate-name-hash-partial-sweep";
const POPCOUNT_CAP_CASE_ID: &str = "simd-word-popcount-over-cap";
const POPCOUNT_ALIGNMENT_CASE_ID: &str = "simd-word-popcount-unaligned";
const POPCOUNT_MINIMUM_CASE_ID: &str = "simd-word-popcount-below-minimum";
pub(super) const PQ1_GROUP_ID: &str = "pq1-adapter-protocol-smoke";
pub(super) const CIRCUIT_PARSE_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-PARSE";
pub(super) const CIRCUIT_CANONICAL_PRINT_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-CANONICAL-PRINT";
pub(super) const GATE_NAME_HASH_GROUP_ID: &str = "PERFQ-M4-GATE-LOOKUP";
pub(super) const SIMD_WORD_POPCOUNT_GROUP_ID: &str = "PERFQ-M5-SIMD-WORD";

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
                || (group == GATE_NAME_HASH_GROUP_ID
                    && workload == "gate-name-hash"
                    && measurement == "hash-all-names")
                || (group == SIMD_WORD_POPCOUNT_GROUP_ID
                    && workload == "simd-word-popcount"
                    && measurement == "toggle-popcount")
    )
}

pub(super) const fn registered_group_count() -> usize {
    5
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
    pub(super) contract_preflight_sha256: String,
}

#[derive(Debug)]
pub(crate) struct PreparedWorkers {
    root: PathBuf,
    adapter: AdapterExecutable,
    worker: super::stab_build::StabWorkerExecutable,
    repository_commit: String,
    toolchain: super::toolchain::ToolchainEvidence,
    cpu: Option<usize>,
    contract_preflight: Option<WorkerContractPreflightEvidence>,
}

pub(super) fn verify_private_worker_reproducibility(
    root: &RepoRoot,
) -> Result<WorkerIdentityEvidence, InvocationError> {
    let repository_before = super::git::repository_state(root)?;
    require_reproducibility_repository(&repository_before, &repository_before)?;
    let toolchain = super::toolchain::collect(root)?;
    let first = PreparedWorkers::prepare(root, &repository_before.commit, &toolchain)?;
    let first_identity = first.identity_evidence()?;
    drop(first);
    let second = PreparedWorkers::prepare(root, &repository_before.commit, &toolchain)?;
    let second_identity = second.identity_evidence()?;
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
        let mut workers = Self {
            root: root.path.clone(),
            adapter,
            worker,
            repository_commit: repository_commit.to_string(),
            toolchain: toolchain.clone(),
            cpu: None,
            contract_preflight: None,
        };
        workers.verify_executables()?;
        workers.contract_preflight = Some(workers.verify_identity_handshake()?);
        workers.verify()?;
        Ok(workers)
    }

    pub(crate) fn pin_to_cpu(&mut self, cpu: usize) {
        self.cpu = Some(cpu);
    }

    pub(crate) fn identity_evidence(&self) -> Result<WorkerIdentityEvidence, InvocationError> {
        let contract_preflight = self
            .contract_preflight
            .as_ref()
            .ok_or(InvocationError::MissingContractPreflight)?;
        Ok(WorkerIdentityEvidence {
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
            contract_preflight_sha256: contract_preflight.sha256.clone(),
        })
    }

    pub(crate) fn contract_preflight_evidence(
        &self,
    ) -> Result<&WorkerContractPreflightEvidence, InvocationError> {
        self.contract_preflight
            .as_ref()
            .ok_or(InvocationError::MissingContractPreflight)
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
        if !self
            .adapter
            .receipt
            .validates_comparator_sources(&group.comparator_sources)
        {
            return Err(InvocationError::ComparatorSourceContract(
                group.id.to_string(),
            ));
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

    fn verify_identity_handshake(
        &self,
    ) -> Result<WorkerContractPreflightEvidence, InvocationError> {
        let mut probes = Vec::with_capacity(18);
        let protocol_output = SemanticDigest::try_new(PROTOCOL_SMOKE_OUTPUT_DIGEST)?;
        probes.push(self.invoke_identity_probe(Implementation::Stim, &protocol_output)?);
        probes.push(self.invoke_identity_probe(Implementation::Stab, &protocol_output)?);
        let small_input = InputDigest::try_new(SMALL_POPCOUNT_INPUT_DIGEST)?;
        let odd_output = SemanticDigest::try_new(ODD_POPCOUNT_OUTPUT_DIGEST)?;
        let even_output = SemanticDigest::try_new(EVEN_POPCOUNT_OUTPUT_DIGEST)?;
        let maximum_input = InputDigest::try_new(MAX_POPCOUNT_INPUT_DIGEST)?;
        let maximum_output = SemanticDigest::try_new(MAX_POPCOUNT_OUTPUT_DIGEST)?;
        for implementation in [Implementation::Stim, Implementation::Stab] {
            probes.push(self.invoke_popcount_acceptance(
                POPCOUNT_ODD_CASE_ID,
                implementation,
                ODD_POPCOUNT_ITERATIONS,
                SMALL_POPCOUNT_BITS,
                &small_input,
                &odd_output,
            )?);
            probes.push(self.invoke_popcount_acceptance(
                POPCOUNT_EVEN_CASE_ID,
                implementation,
                EVEN_POPCOUNT_ITERATIONS,
                SMALL_POPCOUNT_BITS,
                &small_input,
                &even_output,
            )?);
            probes.push(self.invoke_popcount_acceptance(
                POPCOUNT_MAXIMUM_CASE_ID,
                implementation,
                1,
                MAX_SUPPORTED_POPCOUNT_BITS,
                &maximum_input,
                &maximum_output,
            )?);
        }
        for implementation in [Implementation::Stim, Implementation::Stab] {
            probes.push(self.invoke_cap_rejection(implementation)?);
        }
        for implementation in [Implementation::Stim, Implementation::Stab] {
            probes.push(self.invoke_gate_partial_sweep_rejection(implementation)?);
        }
        for implementation in [Implementation::Stim, Implementation::Stab] {
            probes.push(self.invoke_popcount_cap_rejection(implementation)?);
        }
        for implementation in [Implementation::Stim, Implementation::Stab] {
            probes.push(self.invoke_popcount_alignment_rejection(implementation)?);
        }
        for implementation in [Implementation::Stim, Implementation::Stab] {
            probes.push(self.invoke_popcount_minimum_rejection(implementation)?);
        }
        WorkerContractPreflightEvidence::from_actual_probes(probes)
    }

    fn invoke_identity_probe(
        &self,
        implementation: Implementation,
        expected_output_digest: &SemanticDigest,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
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
            expected_output_digest: Some(expected_output_digest.clone()),
            affinity_cpu: None,
            stim_commit: GitCommit::try_new(STIM_COMMIT)?,
            source_digest,
            build_fingerprint,
        }
        .validate(&rows)?;
        let row = rows
            .into_iter()
            .next()
            .ok_or(InvocationError::MissingMeasurement)?;
        accepted_probe(PROTOCOL_SMOKE_CASE_ID, &row)
    }

    fn invoke_popcount_acceptance(
        &self,
        case_id: &'static str,
        implementation: Implementation,
        iterations: u64,
        work_items: u64,
        expected_input_digest: &InputDigest,
        expected_output_digest: &SemanticDigest,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let mut arguments = vec![
            OsString::from("--workload"),
            OsString::from("simd-word-popcount"),
            OsString::from("--measurement-id"),
            OsString::from("toggle-popcount"),
            OsString::from("--iterations"),
            OsString::from(iterations.to_string()),
            OsString::from("--work-items"),
            OsString::from(work_items.to_string()),
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
            workload_id: ProtocolId::try_new("simd-word-popcount")?,
            measurement_ids: BTreeSet::from([ProtocolId::try_new("toggle-popcount")?]),
            iteration_count: iterations,
            expected_work_count: iterations
                .checked_mul(work_items)
                .ok_or(InvocationError::WorkOverflow)?,
            expected_input_bytes: work_items / 8,
            expected_input_digest: expected_input_digest.clone(),
            expected_output_digest: Some(expected_output_digest.clone()),
            affinity_cpu: None,
            stim_commit: GitCommit::try_new(STIM_COMMIT)?,
            source_digest,
            build_fingerprint,
        }
        .validate(&rows)?;
        let row = rows.first().ok_or(InvocationError::MissingMeasurement)?;
        accepted_probe(case_id, row)
    }

    fn invoke_cap_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
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
        checked_cap_rejection(&output, implementation)?;
        rejected_probe(CIRCUIT_CAP_CASE_ID, implementation, &output)
    }

    fn invoke_gate_partial_sweep_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let mut arguments = vec![
            OsString::from("--workload"),
            OsString::from("gate-name-hash"),
            OsString::from("--measurement-id"),
            OsString::from("hash-all-names"),
            OsString::from("--iterations"),
            OsString::from("1"),
            OsString::from("--work-items"),
            OsString::from(FIRST_PARTIAL_GATE_SWEEP_WORK_ITEMS),
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
        checked_gate_partial_sweep_rejection(&output, implementation)?;
        rejected_probe(GATE_PARTIAL_SWEEP_CASE_ID, implementation, &output)
    }

    fn invoke_popcount_cap_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let output =
            self.invoke_invalid_popcount_width(implementation, FIRST_UNSUPPORTED_POPCOUNT_BITS)?;
        checked_popcount_cap_rejection(&output, implementation)?;
        rejected_probe(POPCOUNT_CAP_CASE_ID, implementation, &output)
    }

    fn invoke_popcount_alignment_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let output =
            self.invoke_invalid_popcount_width(implementation, FIRST_UNALIGNED_POPCOUNT_BITS)?;
        checked_popcount_alignment_rejection(&output, implementation)?;
        rejected_probe(POPCOUNT_ALIGNMENT_CASE_ID, implementation, &output)
    }

    fn invoke_popcount_minimum_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let output =
            self.invoke_invalid_popcount_width(implementation, FIRST_BELOW_MINIMUM_POPCOUNT_BITS)?;
        checked_popcount_minimum_rejection(&output, implementation)?;
        rejected_probe(POPCOUNT_MINIMUM_CASE_ID, implementation, &output)
    }

    fn invoke_invalid_popcount_width(
        &self,
        implementation: Implementation,
        work_items: &'static str,
    ) -> Result<ProcessResult, InvocationError> {
        let mut arguments = vec![
            OsString::from("--workload"),
            OsString::from("simd-word-popcount"),
            OsString::from("--measurement-id"),
            OsString::from("toggle-popcount"),
            OsString::from("--iterations"),
            OsString::from("1"),
            OsString::from("--work-items"),
            OsString::from(work_items),
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
        run_bounded_process(&ProcessRequest {
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
        })
        .map_err(InvocationError::Process)
    }

    pub(crate) fn verify(&self) -> Result<(), InvocationError> {
        self.verify_executables()?;
        if self
            .contract_preflight
            .as_ref()
            .is_none_or(|evidence| !evidence.validates_source_contract())
        {
            return Err(InvocationError::MissingContractPreflight);
        }
        Ok(())
    }

    fn verify_executables(&self) -> Result<(), InvocationError> {
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
    output: &ProcessResult,
    implementation: Implementation,
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) = cap_rejection_expectation(implementation);
    if output.status != Some(expected_status)
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

fn cap_rejection_expectation(implementation: Implementation) -> (i32, &'static str) {
    match implementation {
        Implementation::Stim => (
            2,
            "stim qualification adapter: circuit-parse instruction count exceeds the source-owned limit\n",
        ),
        Implementation::Stab => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\ncircuit-parse scale has 1000001 instructions, maximum 1000000\n",
        ),
    }
}

fn checked_gate_partial_sweep_rejection(
    output: &ProcessResult,
    implementation: Implementation,
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) =
        gate_partial_sweep_rejection_expectation(implementation);
    if output.status != Some(expected_status)
        || !output.stdout.is_empty()
        || output.stderr != expected_stderr.as_bytes()
    {
        return Err(InvocationError::GatePartialSweepRejection {
            implementation,
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

fn gate_partial_sweep_rejection_expectation(implementation: Implementation) -> (i32, &'static str) {
    match implementation {
        Implementation::Stim => (
            2,
            "stim qualification adapter: gate-name-hash work count is not a complete gate-table sweep\n",
        ),
        Implementation::Stab => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\ngate-name-hash work count 83 is not a complete sweep of 82 names\n",
        ),
    }
}

fn checked_popcount_cap_rejection(
    output: &ProcessResult,
    implementation: Implementation,
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) = popcount_cap_rejection_expectation(implementation);
    if output.status != Some(expected_status)
        || !output.stdout.is_empty()
        || output.stderr != expected_stderr.as_bytes()
    {
        return Err(InvocationError::PopcountCapRejection {
            implementation,
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

fn popcount_cap_rejection_expectation(implementation: Implementation) -> (i32, &'static str) {
    match implementation {
        Implementation::Stim => (
            2,
            "stim qualification adapter: simd-word-popcount bit width exceeds the source-owned limit\n",
        ),
        Implementation::Stab => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nsimd-word-popcount width 268435712 bits exceeds the maximum 268435456\n",
        ),
    }
}

fn checked_popcount_alignment_rejection(
    output: &ProcessResult,
    implementation: Implementation,
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) =
        popcount_alignment_rejection_expectation(implementation);
    if output.status != Some(expected_status)
        || !output.stdout.is_empty()
        || output.stderr != expected_stderr.as_bytes()
    {
        return Err(InvocationError::PopcountAlignmentRejection {
            implementation,
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

fn popcount_alignment_rejection_expectation(implementation: Implementation) -> (i32, &'static str) {
    match implementation {
        Implementation::Stim => (
            2,
            "stim qualification adapter: simd-word-popcount bit width is not a multiple of 256\n",
        ),
        Implementation::Stab => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nsimd-word-popcount width 513 bits is not a multiple of 256\n",
        ),
    }
}

fn checked_popcount_minimum_rejection(
    output: &ProcessResult,
    implementation: Implementation,
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) = popcount_minimum_rejection_expectation(implementation);
    if output.status != Some(expected_status)
        || !output.stdout.is_empty()
        || output.stderr != expected_stderr.as_bytes()
    {
        return Err(InvocationError::PopcountMinimumRejection {
            implementation,
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

fn popcount_minimum_rejection_expectation(implementation: Implementation) -> (i32, &'static str) {
    match implementation {
        Implementation::Stim => (
            2,
            "stim qualification adapter: simd-word-popcount bit width is below the source-owned minimum\n",
        ),
        Implementation::Stab => (
            1,
            "[stab-bench] ERROR: performance qualification validation failed:\nsimd-word-popcount width 256 bits is below the minimum 512\n",
        ),
    }
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
    Json(#[from] serde_json::Error),
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
    #[error("qualification runtime group {0} does not match the materialized comparator sources")]
    ComparatorSourceContract(String),
    #[error("qualification CPU {0} exceeds the shared worker protocol")]
    CpuRange(usize),
    #[error("qualification workers were invoked before selecting a host-policy CPU")]
    MissingCpu,
    #[error("qualification workers lack the mandatory canonical contract preflight")]
    MissingContractPreflight,
    #[error("the source-owned worker contract preflight digest is stale")]
    ContractPreflightDefinition,
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
    #[error(
        "{implementation} did not reject a partial gate-name-hash sweep before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    GatePartialSweepRejection {
        implementation: Implementation,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject the first unsupported simd-word-popcount width before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    PopcountCapRejection {
        implementation: Implementation,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject an unaligned simd-word-popcount width before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    PopcountAlignmentRejection {
        implementation: Implementation,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject a below-minimum simd-word-popcount width before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    PopcountMinimumRejection {
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
#[path = "invocation_tests.rs"]
mod tests;
