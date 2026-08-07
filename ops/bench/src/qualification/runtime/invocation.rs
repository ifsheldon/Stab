use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::Duration;

use super::adapter::{AdapterExecutable, prepare_adapter};
use super::process::{ProcessResult, run_bounded_process};
use super::protocol::{
    EvidenceMode, GitCommit, Implementation, ProtocolExpectation, SemanticDigest, Sha256Digest,
    WorkerMeasurement, parse_worker_json_lines,
};
use crate::config::STIM_COMMIT;
use crate::root::RepoRoot;
use serde::{Deserialize, Serialize};

pub(in crate::qualification::runtime) mod clifford_string;
mod diagnostic;
mod error;
pub(super) mod pauli_iter;
mod preflight;
pub(in crate::qualification::runtime) mod request;

pub(super) use diagnostic::{
    DiagnosticInvocationRequest, DiagnosticWorkerIdentityEvidence, PreparedDiagnosticWorker,
};
pub(crate) use error::InvocationError;
#[cfg(test)]
use preflight::worker_contract_preflight_digest;
pub(crate) use preflight::{WorkerContractIdentityEvidence, WorkerContractPreflightEvidence};
use preflight::{WorkerContractProbeEvidence, accepted_probe, rejected_probe};
use request::{WorkerRequestSpec, matches_rejection};

const IDENTITY_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const CAP_REJECTION_TIMEOUT: Duration = Duration::from_secs(5);
const FIRST_UNSUPPORTED_CIRCUIT_INSTRUCTIONS: &str = "1000001";
const FIRST_PARTIAL_GATE_SWEEP_WORK_ITEMS: &str = "83";
const FIRST_UNSUPPORTED_POPCOUNT_BITS: &str = "268435712";
const CONTRACT_PREFLIGHT_SCHEMA_VERSION: u32 = 16;
const CIRCUIT_CAP_CASE_ID: &str = "circuit-parse-over-cap";
const GATE_PARTIAL_SWEEP_CASE_ID: &str = "gate-name-hash-partial-sweep";
const POPCOUNT_CAP_CASE_ID: &str = "simd-word-popcount-over-cap";
pub(super) const PQ1_GROUP_ID: &str = "pq1-adapter-protocol-smoke";
pub(super) const CIRCUIT_PARSE_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-PARSE";
pub(super) const CIRCUIT_CANONICAL_PRINT_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-CANONICAL-PRINT";
pub(super) const A2_CIRCUIT_MODEL_FINGERPRINT_GROUP_ID: &str = "PERFQ-A2-CIRCUIT-MODEL-FINGERPRINT";
pub(super) const A2_SAMPLING_REQUEST_FINGERPRINT_GROUP_ID: &str =
    "PERFQ-A2-SAMPLING-REQUEST-FINGERPRINT";
pub(super) const A2_SAMPLING_REQUEST_ESTIMATE_GROUP_ID: &str = "PERFQ-A2-SAMPLING-REQUEST-ESTIMATE";
pub(super) const A2_SAMPLER_COMPILE_GROUP_ID: &str = "PERFQ-A2-SAMPLER-COMPILE";
pub(super) const A7_EXACT_ML_COMPILE_GROUP_ID: &str = "PERFQ-A7-EXACT-ML-COMPILE";
pub(super) const A7_EXACT_ML_REUSED_DECODE_GROUP_ID: &str = "PERFQ-A7-EXACT-ML-REUSED-DECODE";
pub(super) const A7_PIPELINE_GROUP_ID: &str = "PERFQ-A7-SAMPLE-DETECT-DECODE-PIPELINE";
pub(super) const A8_EXTERNAL_NOISE_PASS_GROUP_ID: &str = "PERFQ-A8-EXTERNAL-NOISE-PASS";
pub(super) const GATE_NAME_HASH_GROUP_ID: &str = "PERFQ-M4-GATE-LOOKUP";
pub(super) const SIMD_WORD_POPCOUNT_GROUP_ID: &str = "PERFQ-M5-SIMD-WORD";
pub(super) const SIMD_BITS_XOR_GROUP_ID: &str = "PERFQ-M5-SIMD-BITS";
pub(super) const SIMD_BITS_NOT_ZERO_EARLY_GROUP_ID: &str = "PERFQ-M5-SIMD-BITS-NOT-ZERO-EARLY";
pub(super) const SIMD_BITS_NOT_ZERO_ALL_ZERO_GROUP_ID: &str =
    "PERFQ-M5-SIMD-BITS-NOT-ZERO-ALL-ZERO";
pub(super) const SIMD_BITS_NOT_ZERO_LATE_GROUP_ID: &str = "PERFQ-M5-SIMD-BITS-NOT-ZERO-LATE";
pub(super) const SPARSE_XOR_ROW_GROUP_ID: &str = "PERFQ-M5-SPARSE-XOR";
pub(super) const SPARSE_XOR_ITEM_GROUP_ID: &str = "PERFQ-M5-SPARSE-XOR-ITEM";
pub(super) const BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID: &str =
    "PERFQ-M5-BIT-MATRIX-TRANSPOSE-IN-PLACE";
pub(super) const BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID: &str =
    "PERFQ-M5-BIT-MATRIX-TRANSPOSE-ALLOCATING";
pub(super) const PAULI_STRING_MULTIPLY_GROUP_ID: &str = "PERFQ-M6-PAULI-STRING";
pub(super) const PAULI_STRING_ITER_RANGE_GROUP_ID: &str = "PERFQ-M6-PAULI-ITER";
pub(super) const PAULI_STRING_ITER_SINGLETON_GROUP_ID: &str = "PERFQ-M6-PAULI-ITER-SINGLETON";
pub(super) const DEM_PARSE_GROUP_ID: &str = "PERFQ-M10-DEM-PARSE-CONTRACT";
pub(super) const DEM_CANONICAL_PRINT_GROUP_ID: &str = "PERFQ-M10-DEM-PRINT-CONTRACT";
pub(super) use clifford_string::{CLIFFORD_IDENTITY_GROUP_ID, CLIFFORD_NON_IDENTITY_GROUP_ID};

/// One registered runtime group: the exact `(group, workload, measurement)`
/// identity this parent can invoke. `registered_group_count` is derived from
/// this table, so adding a row here is the single registration step.
struct RegisteredGroup {
    group_id: &'static str,
    workload_id: &'static str,
    measurement_id: &'static str,
}

const REGISTERED_GROUPS: [RegisteredGroup; 28] = [
    RegisteredGroup {
        group_id: PQ1_GROUP_ID,
        workload_id: "protocol-smoke",
        measurement_id: "main",
    },
    RegisteredGroup {
        group_id: CIRCUIT_PARSE_GROUP_ID,
        workload_id: "circuit-parse",
        measurement_id: "parse",
    },
    RegisteredGroup {
        group_id: CIRCUIT_CANONICAL_PRINT_GROUP_ID,
        workload_id: "circuit-canonical-print",
        measurement_id: "serialize",
    },
    RegisteredGroup {
        group_id: A2_CIRCUIT_MODEL_FINGERPRINT_GROUP_ID,
        workload_id: "circuit-model-fingerprint",
        measurement_id: "fingerprint",
    },
    RegisteredGroup {
        group_id: A2_SAMPLING_REQUEST_FINGERPRINT_GROUP_ID,
        workload_id: "sampling-request-fingerprint",
        measurement_id: "fingerprint-inclusive",
    },
    RegisteredGroup {
        group_id: A2_SAMPLING_REQUEST_ESTIMATE_GROUP_ID,
        workload_id: "sampling-request-estimate",
        measurement_id: "estimate",
    },
    RegisteredGroup {
        group_id: A2_SAMPLER_COMPILE_GROUP_ID,
        workload_id: "sampler-compile",
        measurement_id: "compile-and-release",
    },
    RegisteredGroup {
        group_id: A7_EXACT_ML_COMPILE_GROUP_ID,
        workload_id: "exact-ml-compile",
        measurement_id: "compile-and-release",
    },
    RegisteredGroup {
        group_id: A7_EXACT_ML_REUSED_DECODE_GROUP_ID,
        workload_id: "exact-ml-reused-decode",
        measurement_id: "decode-batch",
    },
    RegisteredGroup {
        group_id: A7_PIPELINE_GROUP_ID,
        workload_id: "sample-detect-decode-pipeline",
        measurement_id: "sample-detect-decode",
    },
    RegisteredGroup {
        group_id: A8_EXTERNAL_NOISE_PASS_GROUP_ID,
        workload_id: "external-noise-pass",
        measurement_id: "run-and-release",
    },
    RegisteredGroup {
        group_id: GATE_NAME_HASH_GROUP_ID,
        workload_id: "gate-name-hash",
        measurement_id: "hash-all-names",
    },
    RegisteredGroup {
        group_id: SIMD_WORD_POPCOUNT_GROUP_ID,
        workload_id: "simd-word-popcount",
        measurement_id: "toggle-popcount",
    },
    RegisteredGroup {
        group_id: SIMD_BITS_XOR_GROUP_ID,
        workload_id: "simd-bits-xor",
        measurement_id: "xor-complete-vector",
    },
    RegisteredGroup {
        group_id: SIMD_BITS_NOT_ZERO_EARLY_GROUP_ID,
        workload_id: "simd-bits-not-zero-early",
        measurement_id: "not-zero",
    },
    RegisteredGroup {
        group_id: SIMD_BITS_NOT_ZERO_ALL_ZERO_GROUP_ID,
        workload_id: "simd-bits-not-zero-zero",
        measurement_id: "not-zero",
    },
    RegisteredGroup {
        group_id: SIMD_BITS_NOT_ZERO_LATE_GROUP_ID,
        workload_id: "simd-bits-not-zero-late",
        measurement_id: "not-zero",
    },
    RegisteredGroup {
        group_id: SPARSE_XOR_ROW_GROUP_ID,
        workload_id: "sparse-xor-row",
        measurement_id: "row-xor",
    },
    RegisteredGroup {
        group_id: SPARSE_XOR_ITEM_GROUP_ID,
        workload_id: "sparse-xor-item",
        measurement_id: "xor-item",
    },
    RegisteredGroup {
        group_id: BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID,
        workload_id: "bit-matrix-transpose-in-place",
        measurement_id: "in-place-transpose",
    },
    RegisteredGroup {
        group_id: BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID,
        workload_id: "bit-matrix-transpose-allocating",
        measurement_id: "allocating-transpose",
    },
    RegisteredGroup {
        group_id: PAULI_STRING_MULTIPLY_GROUP_ID,
        workload_id: "pauli-string-right-multiply",
        measurement_id: "right-multiply-in-place",
    },
    RegisteredGroup {
        group_id: PAULI_STRING_ITER_RANGE_GROUP_ID,
        workload_id: "pauli-string-iter-range",
        measurement_id: "construct-and-iterate-borrowed",
    },
    RegisteredGroup {
        group_id: PAULI_STRING_ITER_SINGLETON_GROUP_ID,
        workload_id: "pauli-string-iter-singleton",
        measurement_id: "construct-and-iterate-borrowed",
    },
    RegisteredGroup {
        group_id: CLIFFORD_IDENTITY_GROUP_ID,
        workload_id: "clifford-string-right-multiply-identity",
        measurement_id: "right-multiply-identity",
    },
    RegisteredGroup {
        group_id: CLIFFORD_NON_IDENTITY_GROUP_ID,
        workload_id: "clifford-string-right-multiply-non-identity",
        measurement_id: "right-multiply-non-identity",
    },
    RegisteredGroup {
        group_id: DEM_PARSE_GROUP_ID,
        workload_id: "dem-parse",
        measurement_id: "parse",
    },
    RegisteredGroup {
        group_id: DEM_CANONICAL_PRINT_GROUP_ID,
        workload_id: "dem-canonical-print",
        measurement_id: "serialize",
    },
];

pub(super) fn supports_group(contract: &super::group::GroupContract) -> bool {
    if contract.measurement_ids.len() != 1 {
        return false;
    }
    let Some(measurement) = contract.measurement_ids.first() else {
        return false;
    };
    let group = contract.id.to_string();
    let workload = contract.workload_id.to_string();
    let measurement = measurement.to_string();
    REGISTERED_GROUPS.iter().any(|registered| {
        group == registered.group_id
            && workload == registered.workload_id
            && measurement == registered.measurement_id
    })
}

pub(super) const fn registered_group_count() -> usize {
    REGISTERED_GROUPS.len()
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
    source_root: RepoRoot,
    root: PathBuf,
    adapter: AdapterExecutable,
    worker: super::stab_build::StabWorkerExecutable,
    repository_commit: String,
    toolchain: super::toolchain::ToolchainEvidence,
    cpu: Option<usize>,
    performance_inventory_sha256: String,
    contract_preflight: Option<WorkerContractPreflightEvidence>,
}

pub(super) fn verify_private_worker_reproducibility(
    root: &RepoRoot,
    performance_inventory_sha256: &str,
) -> Result<WorkerIdentityEvidence, InvocationError> {
    let repository_before = super::git::repository_state(root)?;
    require_reproducibility_repository(&repository_before, &repository_before)?;
    let toolchain = super::toolchain::collect(root)?;
    let first = PreparedWorkers::prepare(
        root,
        &repository_before.commit,
        &toolchain,
        performance_inventory_sha256,
    )?;
    let first_identity = first.identity_evidence()?;
    drop(first);
    let second = PreparedWorkers::prepare(
        root,
        &repository_before.commit,
        &toolchain,
        performance_inventory_sha256,
    )?;
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

fn request_spec(
    group: &super::group::GroupContract,
    measurement_id: &super::protocol::ProtocolId,
    evidence_mode: EvidenceMode,
    iterations: NonZeroU64,
    scale: &super::group::ScaleContract,
    expected_cpu: Option<u32>,
    timeout: Duration,
) -> Result<WorkerRequestSpec, InvocationError> {
    let mut spec = WorkerRequestSpec::new(
        group.workload_id.to_string(),
        measurement_id.to_string(),
        iterations.get(),
        scale.work_items.get(),
        evidence_mode,
        timeout,
    )
    .start_barrier(true)
    .release_barrier();
    if let Some(expected_cpu) = expected_cpu {
        spec = spec.expected_cpu(expected_cpu);
    }
    if let Some(descriptor) = clifford_string::runtime_descriptor(
        &group.id.to_string(),
        &group.workload_id.to_string(),
        scale.work_items.get(),
    )? {
        spec = spec.input_descriptor_hex(descriptor);
    }
    if matches!(
        group.id.to_string().as_str(),
        DEM_PARSE_GROUP_ID | DEM_CANONICAL_PRINT_GROUP_ID
    ) {
        spec = spec.input_family(scale.family_id.to_string());
    }
    Ok(spec)
}

impl PreparedWorkers {
    pub(crate) fn prepare(
        root: &RepoRoot,
        repository_commit: &str,
        toolchain: &super::toolchain::ToolchainEvidence,
        performance_inventory_sha256: &str,
    ) -> Result<Self, InvocationError> {
        let contracts = pairable_contracts(super::group::load_groups(
            root,
            performance_inventory_sha256,
        )?);
        let adapter = prepare_adapter(root, repository_commit)?;
        let worker =
            super::stab_build::StabWorkerExecutable::prepare(root, repository_commit, toolchain)?;
        let mut workers = Self {
            source_root: root.clone(),
            root: root.path.clone(),
            adapter,
            worker,
            repository_commit: repository_commit.to_string(),
            toolchain: toolchain.clone(),
            cpu: None,
            performance_inventory_sha256: performance_inventory_sha256.to_string(),
            contract_preflight: None,
        };
        workers.verify_executables()?;
        workers.contract_preflight =
            Some(workers.verify_identity_handshake(performance_inventory_sha256, &contracts)?);
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

    fn contract_identity_evidence(
        &self,
    ) -> Result<WorkerContractIdentityEvidence, InvocationError> {
        Ok(WorkerContractIdentityEvidence {
            stim_source_sha256: self.adapter.source_digest.clone(),
            stim_build_fingerprint: self.adapter.build_fingerprint.clone(),
            stim_binary_sha256: self.adapter.binary_digest.clone(),
            stab_source_sha256: self.worker.identity().source_digest.clone(),
            stab_build_fingerprint: self.worker.identity().build_fingerprint.clone(),
            stab_binary_sha256: Sha256Digest::try_new(self.worker.binary_sha256().to_string())?,
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
        let cpu = self.cpu.ok_or(InvocationError::MissingCpu)?;
        self.invoke_with_affinity(request, Some(cpu))
    }

    fn invoke_with_affinity(
        &self,
        request: InvocationRequest<'_>,
        affinity_cpu: Option<usize>,
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
        if group.claim_class == super::run::ClaimClass::ProductDiagnostic
            && implementation == Implementation::Stim
        {
            return Err(InvocationError::UnsupportedImplementation {
                group: group.id.to_string(),
                implementation,
            });
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
        let expected_cpu = affinity_cpu
            .map(|cpu| u32::try_from(cpu).map_err(|_| InvocationError::CpuRange(cpu)))
            .transpose()?;
        let expected_work_count = checked_work_count(iterations, scale.work_items)?;
        let spec = request_spec(
            group,
            measurement_id,
            evidence_mode,
            iterations,
            scale,
            expected_cpu,
            timeout,
        )?;
        let (program, source_digest, build_fingerprint) = match implementation {
            Implementation::Stim => (
                self.adapter.path.clone(),
                self.adapter.source_digest.clone(),
                self.adapter.build_fingerprint.clone(),
            ),
            Implementation::Stab => (
                self.worker.program(),
                self.worker.identity().source_digest.clone(),
                self.worker.identity().build_fingerprint.clone(),
            ),
        };
        let process = run_bounded_process(&spec.process_request(
            implementation,
            program,
            self.root.clone(),
            affinity_cpu,
        ))?;
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
            affinity_cpu: expected_cpu,
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
        performance_inventory_sha256: &str,
        contracts: &[super::group::GroupContract],
    ) -> Result<WorkerContractPreflightEvidence, InvocationError> {
        let mut probes = Vec::with_capacity(
            contracts
                .len()
                .checked_mul(2)
                .and_then(|count| count.checked_add(6))
                .ok_or(InvocationError::WorkOverflow)?,
        );
        for group in contracts {
            let scale = group
                .scales
                .first()
                .ok_or(InvocationError::ContractPreflightDefinition)?;
            let case_id = preflight::accepted_case_id(group, scale)?;
            let stim = self.invoke_with_affinity(
                InvocationRequest {
                    group,
                    implementation: Implementation::Stim,
                    evidence_mode: EvidenceMode::Contract,
                    iterations: NonZeroU64::MIN,
                    scale,
                    expected_output_digest: None,
                    timeout: IDENTITY_PROBE_TIMEOUT,
                },
                None,
            )?;
            let stim_row = stim
                .rows
                .first()
                .ok_or(InvocationError::MissingMeasurement)?;
            let expected_output = stim_row.output_digest.clone();
            probes.push(accepted_probe(&case_id, stim_row)?);
            let stab = self.invoke_with_affinity(
                InvocationRequest {
                    group,
                    implementation: Implementation::Stab,
                    evidence_mode: EvidenceMode::Contract,
                    iterations: NonZeroU64::MIN,
                    scale,
                    expected_output_digest: Some(&expected_output),
                    timeout: IDENTITY_PROBE_TIMEOUT,
                },
                None,
            )?;
            let stab_row = stab
                .rows
                .first()
                .ok_or(InvocationError::MissingMeasurement)?;
            probes.push(accepted_probe(&case_id, stab_row)?);
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
        WorkerContractPreflightEvidence::from_actual_probes(
            self.contract_identity_evidence()?,
            performance_inventory_sha256,
            contracts,
            probes,
        )
    }

    fn invoke_cap_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let output = self.invoke_invalid_work(
            implementation,
            "circuit-parse",
            "parse",
            "1",
            FIRST_UNSUPPORTED_CIRCUIT_INSTRUCTIONS,
        )?;
        checked_cap_rejection(&output, implementation)?;
        rejected_probe(CIRCUIT_CAP_CASE_ID, implementation, &output)
    }

    fn invoke_gate_partial_sweep_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let output = self.invoke_invalid_work(
            implementation,
            "gate-name-hash",
            "hash-all-names",
            "1",
            FIRST_PARTIAL_GATE_SWEEP_WORK_ITEMS,
        )?;
        checked_gate_partial_sweep_rejection(&output, implementation)?;
        rejected_probe(GATE_PARTIAL_SWEEP_CASE_ID, implementation, &output)
    }

    fn invoke_popcount_cap_rejection(
        &self,
        implementation: Implementation,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let output = self.invoke_invalid_bit_width(
            implementation,
            "simd-word-popcount",
            "toggle-popcount",
            FIRST_UNSUPPORTED_POPCOUNT_BITS,
        )?;
        checked_popcount_cap_rejection(&output, implementation)?;
        rejected_probe(POPCOUNT_CAP_CASE_ID, implementation, &output)
    }

    fn invoke_invalid_bit_width(
        &self,
        implementation: Implementation,
        workload: &'static str,
        measurement: &'static str,
        work_items: &'static str,
    ) -> Result<ProcessResult, InvocationError> {
        self.invoke_invalid_work(implementation, workload, measurement, "1", work_items)
    }

    fn invoke_invalid_work(
        &self,
        implementation: Implementation,
        workload: &'static str,
        measurement: &'static str,
        iterations: &'static str,
        work_items: &'static str,
    ) -> Result<ProcessResult, InvocationError> {
        let spec = WorkerRequestSpec::new(
            workload,
            measurement,
            iterations,
            work_items,
            EvidenceMode::Contract,
            CAP_REJECTION_TIMEOUT,
        )
        .start_barrier(true);
        let program = match implementation {
            Implementation::Stim => self.adapter.path.clone(),
            Implementation::Stab => self.worker.program(),
        };
        run_bounded_process(&spec.process_request(implementation, program, self.root.clone(), None))
            .map_err(InvocationError::Process)
    }

    pub(crate) fn verify(&self) -> Result<(), InvocationError> {
        self.verify_executables()?;
        let contracts = pairable_contracts(super::group::load_groups(
            &self.source_root,
            &self.performance_inventory_sha256,
        )?);
        if self.contract_preflight.as_ref().is_none_or(|evidence| {
            !evidence.validates_source_contract(&self.performance_inventory_sha256, &contracts)
        }) {
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

fn pairable_contracts(
    contracts: Vec<super::group::GroupContract>,
) -> Vec<super::group::GroupContract> {
    contracts
        .into_iter()
        .filter(|contract| contract.claim_class.uses_paired_workers())
        .collect()
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
    if !matches_rejection(output, expected_status, expected_stderr) {
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
    if !matches_rejection(output, expected_status, expected_stderr) {
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
    if !matches_rejection(output, expected_status, expected_stderr) {
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

fn checked_work_count(
    iterations: NonZeroU64,
    work_items: NonZeroU64,
) -> Result<u64, InvocationError> {
    iterations
        .get()
        .checked_mul(work_items.get())
        .ok_or(InvocationError::WorkOverflow)
}

#[cfg(test)]
#[path = "invocation_tests.rs"]
mod tests;
