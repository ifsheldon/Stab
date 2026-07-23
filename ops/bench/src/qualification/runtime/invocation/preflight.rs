use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::super::group::{GroupContract, ScaleContract};
use super::super::process::ProcessResult;
use super::super::protocol::{
    EvidenceMode, Implementation, InputDigest, ProtocolId, RAW_WORK_TIMING_BOUNDARY,
    SemanticDigest, Sha256Digest, TimingBoundary, WorkerMeasurement,
};
use super::{
    CIRCUIT_CAP_CASE_ID, CONTRACT_PREFLIGHT_SCHEMA_VERSION, GATE_PARTIAL_SWEEP_CASE_ID,
    InvocationError, POPCOUNT_CAP_CASE_ID, cap_rejection_expectation,
    gate_partial_sweep_rejection_expectation, popcount_cap_rejection_expectation,
};

const MAX_PREFLIGHT_RECEIPTS: usize = 128;
const SHARED_REJECTION_CLASSES: usize = 3;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerContractPreflightEvidence {
    pub(super) schema_version: u32,
    pub(super) performance_inventory_sha256: Sha256Digest,
    pub(super) worker_identity: WorkerContractIdentityEvidence,
    pub(super) probes: Vec<WorkerContractProbeEvidence>,
    pub(super) sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerContractIdentityEvidence {
    pub(super) stim_source_sha256: Sha256Digest,
    pub(super) stim_build_fingerprint: Sha256Digest,
    pub(super) stim_binary_sha256: Sha256Digest,
    pub(super) stab_source_sha256: Sha256Digest,
    pub(super) stab_build_fingerprint: Sha256Digest,
    pub(super) stab_binary_sha256: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum WorkerContractProbeEvidence {
    Accepted {
        case_id: ProtocolId,
        implementation: Implementation,
        evidence_mode: EvidenceMode,
        timing_boundary: TimingBoundary,
        iteration_count: u64,
        work_count: u64,
        input_bytes: u64,
        input_digest: InputDigest,
        output_digest: SemanticDigest,
    },
    Rejected {
        case_id: ProtocolId,
        implementation: Implementation,
        evidence_mode: EvidenceMode,
        exit_status: i32,
        stdout_sha256: Sha256Digest,
        stderr_sha256: Sha256Digest,
    },
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WorkerContractPreflightDigestMaterial<'a> {
    schema_version: u32,
    performance_inventory_sha256: &'a Sha256Digest,
    worker_identity: &'a WorkerContractIdentityEvidence,
    probes: &'a [WorkerContractProbeEvidence],
}

impl WorkerContractPreflightEvidence {
    pub(super) fn from_actual_probes(
        worker_identity: WorkerContractIdentityEvidence,
        performance_inventory_sha256: &str,
        contracts: &[GroupContract],
        probes: Vec<WorkerContractProbeEvidence>,
    ) -> Result<Self, InvocationError> {
        let performance_inventory_sha256 =
            Sha256Digest::try_new(performance_inventory_sha256.to_string())?;
        let evidence = Self {
            schema_version: CONTRACT_PREFLIGHT_SCHEMA_VERSION,
            sha256: worker_contract_preflight_digest(
                &performance_inventory_sha256,
                &worker_identity,
                &probes,
            )?,
            performance_inventory_sha256,
            worker_identity,
            probes,
        };
        if !evidence
            .validates_source_contract(evidence.performance_inventory_sha256.as_str(), contracts)
        {
            return Err(InvocationError::ContractPreflightDefinition);
        }
        Ok(evidence)
    }

    pub(crate) fn validates_source_contract(
        &self,
        performance_inventory_sha256: &str,
        contracts: &[GroupContract],
    ) -> bool {
        self.schema_version == CONTRACT_PREFLIGHT_SCHEMA_VERSION
            && self.performance_inventory_sha256.as_str() == performance_inventory_sha256
            && validate_probes(contracts, &self.probes).is_ok()
            && worker_contract_preflight_digest(
                &self.performance_inventory_sha256,
                &self.worker_identity,
                &self.probes,
            )
            .is_ok_and(|digest| self.sha256 == digest)
    }

    pub(crate) fn validates_worker_identity(
        &self,
        identity: &super::WorkerIdentityEvidence,
    ) -> bool {
        self.worker_identity.stim_source_sha256.as_str() == identity.stim_source_sha256
            && self.worker_identity.stim_build_fingerprint.as_str()
                == identity.stim_build_fingerprint
            && self.worker_identity.stim_binary_sha256.as_str() == identity.stim_binary_sha256
            && self.worker_identity.stab_source_sha256.as_str() == identity.stab_source_sha256
            && self.worker_identity.stab_build_fingerprint.as_str()
                == identity.stab_build_fingerprint
            && self.worker_identity.stab_binary_sha256.as_str() == identity.stab_binary_sha256
    }

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }

    pub(crate) fn probe_count(&self) -> usize {
        self.probes.len()
    }
}

pub(super) fn worker_contract_preflight_digest(
    performance_inventory_sha256: &Sha256Digest,
    worker_identity: &WorkerContractIdentityEvidence,
    probes: &[WorkerContractProbeEvidence],
) -> Result<String, InvocationError> {
    let material = serde_json::to_vec(&WorkerContractPreflightDigestMaterial {
        schema_version: CONTRACT_PREFLIGHT_SCHEMA_VERSION,
        performance_inventory_sha256,
        worker_identity,
        probes,
    })?;
    sha256_hex_bytes(&material)
}

pub(super) fn accepted_case_id(
    group: &GroupContract,
    scale: &ScaleContract,
) -> Result<String, InvocationError> {
    let value = format!("{}:{}", group.id, scale.id);
    ProtocolId::try_new(value.clone())?;
    Ok(value)
}

fn validate_probes(
    contracts: &[GroupContract],
    probes: &[WorkerContractProbeEvidence],
) -> Result<(), InvocationError> {
    let expected_count = contracts
        .len()
        .checked_mul(2)
        .and_then(|count| count.checked_add(SHARED_REJECTION_CLASSES * 2))
        .ok_or(InvocationError::WorkOverflow)?;
    if expected_count > MAX_PREFLIGHT_RECEIPTS || probes.len() != expected_count {
        return Err(InvocationError::ContractPreflightDefinition);
    }
    let mut index = 0;
    for group in contracts {
        let scale = group
            .scales
            .first()
            .ok_or(InvocationError::ContractPreflightDefinition)?;
        let case_id = accepted_case_id(group, scale)?;
        let stim = probes
            .get(index)
            .ok_or(InvocationError::ContractPreflightDefinition)?;
        index += 1;
        let stab = probes
            .get(index)
            .ok_or(InvocationError::ContractPreflightDefinition)?;
        index += 1;
        validate_accepted_probe(stim, &case_id, Implementation::Stim, scale)?;
        validate_accepted_probe(stab, &case_id, Implementation::Stab, scale)?;
        if accepted_output_digest(stim)? != accepted_output_digest(stab)? {
            return Err(InvocationError::ContractPreflightDefinition);
        }
    }
    for (case_id, expectation) in [
        (
            CIRCUIT_CAP_CASE_ID,
            cap_rejection_expectation as fn(Implementation) -> (i32, &'static str),
        ),
        (
            GATE_PARTIAL_SWEEP_CASE_ID,
            gate_partial_sweep_rejection_expectation,
        ),
        (POPCOUNT_CAP_CASE_ID, popcount_cap_rejection_expectation),
    ] {
        for implementation in [Implementation::Stim, Implementation::Stab] {
            let actual = probes
                .get(index)
                .ok_or(InvocationError::ContractPreflightDefinition)?;
            index += 1;
            let (status, stderr) = expectation(implementation);
            let expected = expected_rejected_probe(case_id, implementation, status, stderr)?;
            if *actual != expected {
                return Err(InvocationError::ContractPreflightDefinition);
            }
        }
    }
    if index != probes.len() {
        return Err(InvocationError::ContractPreflightDefinition);
    }
    Ok(())
}

fn validate_accepted_probe(
    probe: &WorkerContractProbeEvidence,
    expected_case_id: &str,
    expected_implementation: Implementation,
    scale: &ScaleContract,
) -> Result<(), InvocationError> {
    match probe {
        WorkerContractProbeEvidence::Accepted {
            case_id,
            implementation,
            evidence_mode,
            timing_boundary,
            iteration_count,
            work_count,
            input_bytes,
            input_digest,
            ..
        } if case_id.to_string() == expected_case_id
            && *implementation == expected_implementation
            && *evidence_mode == EvidenceMode::Contract
            && *timing_boundary == RAW_WORK_TIMING_BOUNDARY
            && *iteration_count == 1
            && *work_count == scale.work_items.get()
            && *input_bytes == scale.input_bytes
            && input_digest == &scale.input_digest =>
        {
            Ok(())
        }
        _ => Err(InvocationError::ContractPreflightDefinition),
    }
}

fn accepted_output_digest(
    probe: &WorkerContractProbeEvidence,
) -> Result<&SemanticDigest, InvocationError> {
    match probe {
        WorkerContractProbeEvidence::Accepted { output_digest, .. } => Ok(output_digest),
        WorkerContractProbeEvidence::Rejected { .. } => {
            Err(InvocationError::ContractPreflightDefinition)
        }
    }
}

fn sha256_hex_bytes(bytes: &[u8]) -> Result<String, InvocationError> {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut output, "{byte:02x}")
            .map_err(|_| InvocationError::ContractPreflightDefinition)?;
    }
    Ok(output)
}

#[cfg(test)]
pub(super) fn expected_accepted_probe(
    case_id: &str,
    implementation: Implementation,
    iteration_count: u64,
    work_count: u64,
    input_bytes: u64,
    input_digest: &str,
    output_digest: &str,
) -> Result<WorkerContractProbeEvidence, InvocationError> {
    Ok(WorkerContractProbeEvidence::Accepted {
        case_id: ProtocolId::try_new(case_id)?,
        implementation,
        evidence_mode: EvidenceMode::Contract,
        timing_boundary: RAW_WORK_TIMING_BOUNDARY,
        iteration_count,
        work_count,
        input_bytes,
        input_digest: InputDigest::try_new(input_digest)?,
        output_digest: SemanticDigest::try_new(output_digest)?,
    })
}

pub(super) fn expected_rejected_probe(
    case_id: &str,
    implementation: Implementation,
    exit_status: i32,
    stderr: &str,
) -> Result<WorkerContractProbeEvidence, InvocationError> {
    Ok(WorkerContractProbeEvidence::Rejected {
        case_id: ProtocolId::try_new(case_id)?,
        implementation,
        evidence_mode: EvidenceMode::Contract,
        exit_status,
        stdout_sha256: Sha256Digest::try_new(sha256_hex_bytes(&[])?)?,
        stderr_sha256: Sha256Digest::try_new(sha256_hex_bytes(stderr.as_bytes())?)?,
    })
}

pub(super) fn accepted_probe(
    case_id: &str,
    row: &WorkerMeasurement,
) -> Result<WorkerContractProbeEvidence, InvocationError> {
    Ok(WorkerContractProbeEvidence::Accepted {
        case_id: ProtocolId::try_new(case_id)?,
        implementation: row.implementation,
        evidence_mode: row.evidence_mode,
        timing_boundary: row.timing_boundary,
        iteration_count: row.iteration_count,
        work_count: row.work_count,
        input_bytes: row.input_bytes,
        input_digest: row.input_digest.clone(),
        output_digest: row.output_digest.clone(),
    })
}

pub(super) fn rejected_probe(
    case_id: &str,
    implementation: Implementation,
    output: &ProcessResult,
) -> Result<WorkerContractProbeEvidence, InvocationError> {
    Ok(WorkerContractProbeEvidence::Rejected {
        case_id: ProtocolId::try_new(case_id)?,
        implementation,
        evidence_mode: EvidenceMode::Contract,
        exit_status: output
            .status
            .ok_or(InvocationError::ContractPreflightDefinition)?,
        stdout_sha256: Sha256Digest::try_new(sha256_hex_bytes(&output.stdout)?)?,
        stderr_sha256: Sha256Digest::try_new(sha256_hex_bytes(&output.stderr)?)?,
    })
}
