use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::super::process::ProcessResult;
use super::super::protocol::{
    Implementation, InputDigest, ProtocolId, SemanticDigest, Sha256Digest, WorkerMeasurement,
};
use super::{
    CIRCUIT_CAP_CASE_ID, CONTRACT_PREFLIGHT_SCHEMA_VERSION, EMPTY_PROTOCOL_INPUT_DIGEST,
    EVEN_POPCOUNT_ITERATIONS, EVEN_POPCOUNT_OUTPUT_DIGEST, GATE_PARTIAL_SWEEP_CASE_ID,
    InvocationError, MAX_POPCOUNT_INPUT_DIGEST, MAX_POPCOUNT_OUTPUT_DIGEST,
    MAX_SUPPORTED_POPCOUNT_BITS, ODD_POPCOUNT_ITERATIONS, ODD_POPCOUNT_OUTPUT_DIGEST,
    POPCOUNT_ALIGNMENT_CASE_ID, POPCOUNT_CAP_CASE_ID, POPCOUNT_EVEN_CASE_ID,
    POPCOUNT_MAXIMUM_CASE_ID, POPCOUNT_MINIMUM_CASE_ID, POPCOUNT_ODD_CASE_ID,
    PROTOCOL_SMOKE_CASE_ID, PROTOCOL_SMOKE_OUTPUT_DIGEST, SMALL_POPCOUNT_BITS,
    SMALL_POPCOUNT_INPUT_DIGEST,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerContractPreflightEvidence {
    pub(super) schema_version: u32,
    pub(super) probes: Vec<WorkerContractProbeEvidence>,
    pub(super) sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum WorkerContractProbeEvidence {
    Accepted {
        case_id: ProtocolId,
        implementation: Implementation,
        iteration_count: u64,
        work_count: u64,
        input_bytes: u64,
        input_digest: InputDigest,
        output_digest: SemanticDigest,
    },
    Rejected {
        case_id: ProtocolId,
        implementation: Implementation,
        exit_status: i32,
        stdout_sha256: Sha256Digest,
        stderr_sha256: Sha256Digest,
    },
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WorkerContractPreflightDigestMaterial<'a> {
    schema_version: u32,
    probes: &'a [WorkerContractProbeEvidence],
}

impl WorkerContractPreflightEvidence {
    pub(super) fn from_actual_probes(
        probes: Vec<WorkerContractProbeEvidence>,
    ) -> Result<Self, InvocationError> {
        let evidence = Self {
            schema_version: CONTRACT_PREFLIGHT_SCHEMA_VERSION,
            sha256: worker_contract_preflight_digest(&probes)?,
            probes,
        };
        if !evidence.validates_source_contract() {
            return Err(InvocationError::ContractPreflightDefinition);
        }
        Ok(evidence)
    }

    pub(crate) fn validates_source_contract(&self) -> bool {
        self.schema_version == CONTRACT_PREFLIGHT_SCHEMA_VERSION
            && expected_contract_preflight_probes().is_ok_and(|expected| self.probes == expected)
            && worker_contract_preflight_digest(&self.probes)
                .is_ok_and(|digest| self.sha256 == digest)
    }

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }

    pub(crate) fn probe_count(&self) -> usize {
        self.probes.len()
    }
}

pub(super) fn worker_contract_preflight_digest(
    probes: &[WorkerContractProbeEvidence],
) -> Result<String, InvocationError> {
    let material = serde_json::to_vec(&WorkerContractPreflightDigestMaterial {
        schema_version: CONTRACT_PREFLIGHT_SCHEMA_VERSION,
        probes,
    })?;
    sha256_hex_bytes(&material)
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

pub(super) fn expected_contract_preflight_probes()
-> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
    let mut probes = Vec::with_capacity(18);
    for implementation in [Implementation::Stim, Implementation::Stab] {
        probes.push(expected_accepted_probe(
            PROTOCOL_SMOKE_CASE_ID,
            implementation,
            1,
            1,
            0,
            EMPTY_PROTOCOL_INPUT_DIGEST,
            PROTOCOL_SMOKE_OUTPUT_DIGEST,
        )?);
    }
    for implementation in [Implementation::Stim, Implementation::Stab] {
        probes.push(expected_accepted_probe(
            POPCOUNT_ODD_CASE_ID,
            implementation,
            ODD_POPCOUNT_ITERATIONS,
            ODD_POPCOUNT_ITERATIONS * SMALL_POPCOUNT_BITS,
            SMALL_POPCOUNT_BITS / 8,
            SMALL_POPCOUNT_INPUT_DIGEST,
            ODD_POPCOUNT_OUTPUT_DIGEST,
        )?);
        probes.push(expected_accepted_probe(
            POPCOUNT_EVEN_CASE_ID,
            implementation,
            EVEN_POPCOUNT_ITERATIONS,
            EVEN_POPCOUNT_ITERATIONS * SMALL_POPCOUNT_BITS,
            SMALL_POPCOUNT_BITS / 8,
            SMALL_POPCOUNT_INPUT_DIGEST,
            EVEN_POPCOUNT_OUTPUT_DIGEST,
        )?);
        probes.push(expected_accepted_probe(
            POPCOUNT_MAXIMUM_CASE_ID,
            implementation,
            1,
            MAX_SUPPORTED_POPCOUNT_BITS,
            MAX_SUPPORTED_POPCOUNT_BITS / 8,
            MAX_POPCOUNT_INPUT_DIGEST,
            MAX_POPCOUNT_OUTPUT_DIGEST,
        )?);
    }
    for (case_id, expectation) in [
        (
            CIRCUIT_CAP_CASE_ID,
            super::cap_rejection_expectation as RejectionExpectation,
        ),
        (
            GATE_PARTIAL_SWEEP_CASE_ID,
            super::gate_partial_sweep_rejection_expectation as RejectionExpectation,
        ),
        (
            POPCOUNT_CAP_CASE_ID,
            super::popcount_cap_rejection_expectation as RejectionExpectation,
        ),
        (
            POPCOUNT_ALIGNMENT_CASE_ID,
            super::popcount_alignment_rejection_expectation as RejectionExpectation,
        ),
        (
            POPCOUNT_MINIMUM_CASE_ID,
            super::popcount_minimum_rejection_expectation as RejectionExpectation,
        ),
    ] {
        for implementation in [Implementation::Stim, Implementation::Stab] {
            let (exit_status, stderr) = expectation(implementation);
            probes.push(expected_rejected_probe(
                case_id,
                implementation,
                exit_status,
                stderr,
            )?);
        }
    }
    Ok(probes)
}

type RejectionExpectation = fn(Implementation) -> (i32, &'static str);

fn expected_accepted_probe(
    case_id: &'static str,
    implementation: Implementation,
    iteration_count: u64,
    work_count: u64,
    input_bytes: u64,
    input_digest: &'static str,
    output_digest: &'static str,
) -> Result<WorkerContractProbeEvidence, InvocationError> {
    Ok(WorkerContractProbeEvidence::Accepted {
        case_id: ProtocolId::try_new(case_id)?,
        implementation,
        iteration_count,
        work_count,
        input_bytes,
        input_digest: InputDigest::try_new(input_digest)?,
        output_digest: SemanticDigest::try_new(output_digest)?,
    })
}

fn expected_rejected_probe(
    case_id: &'static str,
    implementation: Implementation,
    exit_status: i32,
    stderr: &'static str,
) -> Result<WorkerContractProbeEvidence, InvocationError> {
    Ok(WorkerContractProbeEvidence::Rejected {
        case_id: ProtocolId::try_new(case_id)?,
        implementation,
        exit_status,
        stdout_sha256: Sha256Digest::try_new(sha256_hex_bytes(&[])?)?,
        stderr_sha256: Sha256Digest::try_new(sha256_hex_bytes(stderr.as_bytes())?)?,
    })
}

pub(super) fn accepted_probe(
    case_id: &'static str,
    row: &WorkerMeasurement,
) -> Result<WorkerContractProbeEvidence, InvocationError> {
    Ok(WorkerContractProbeEvidence::Accepted {
        case_id: ProtocolId::try_new(case_id)?,
        implementation: row.implementation,
        iteration_count: row.iteration_count,
        work_count: row.work_count,
        input_bytes: row.input_bytes,
        input_digest: row.input_digest.clone(),
        output_digest: row.output_digest.clone(),
    })
}

pub(super) fn rejected_probe(
    case_id: &'static str,
    implementation: Implementation,
    output: &ProcessResult,
) -> Result<WorkerContractProbeEvidence, InvocationError> {
    Ok(WorkerContractProbeEvidence::Rejected {
        case_id: ProtocolId::try_new(case_id)?,
        implementation,
        exit_status: output
            .status
            .ok_or(InvocationError::ContractPreflightDefinition)?,
        stdout_sha256: Sha256Digest::try_new(sha256_hex_bytes(&output.stdout)?)?,
        stderr_sha256: Sha256Digest::try_new(sha256_hex_bytes(&output.stderr)?)?,
    })
}
