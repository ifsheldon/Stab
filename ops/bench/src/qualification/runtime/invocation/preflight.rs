use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::super::contract::{
    PROTOCOL_SMOKE_INPUT_DIGEST, PROTOCOL_SMOKE_ITERATIONS, PROTOCOL_SMOKE_WORK_ITEMS,
    protocol_smoke_output_digest,
};
use super::super::process::ProcessResult;
use super::super::protocol::{
    Implementation, InputDigest, ProtocolId, SemanticDigest, Sha256Digest, WorkerMeasurement,
};
use super::clifford_string::expected_clifford_probes;
use super::pauli::{
    PAULI_EVEN_CASE_ID, PAULI_EVEN_OUTPUT_DIGEST, PAULI_MAX_CASE_ID, PAULI_MAX_INPUT_BYTES,
    PAULI_MAX_INPUT_DIGEST, PAULI_MAX_OUTPUT_DIGEST, PAULI_MAX_WORK_ITEMS, PAULI_ODD_CASE_ID,
    PAULI_ODD_OUTPUT_DIGEST, PAULI_SMALL_INPUT_BYTES, PAULI_SMALL_INPUT_DIGEST,
    PAULI_SMALL_WORK_ITEMS, PauliRejectionClass, pauli_rejection_expectation,
};
use super::pauli_iter::{
    PAULI_ITER_INPUT_BYTES, PAULI_ITER_RANGE_EVEN_CASE_ID, PAULI_ITER_RANGE_EVEN_OUTPUT_DIGEST,
    PAULI_ITER_RANGE_MAX_CASE_ID, PAULI_ITER_RANGE_MAX_INPUT_DIGEST,
    PAULI_ITER_RANGE_MAX_OUTPUT_DIGEST, PAULI_ITER_RANGE_MAX_WORK_ITEMS,
    PAULI_ITER_RANGE_ODD_CASE_ID, PAULI_ITER_RANGE_ODD_OUTPUT_DIGEST,
    PAULI_ITER_RANGE_SMALL_INPUT_DIGEST, PAULI_ITER_RANGE_SMALL_WORK_ITEMS,
    PAULI_ITER_SINGLETON_EVEN_CASE_ID, PAULI_ITER_SINGLETON_EVEN_OUTPUT_DIGEST,
    PAULI_ITER_SINGLETON_MAX_CASE_ID, PAULI_ITER_SINGLETON_MAX_INPUT_DIGEST,
    PAULI_ITER_SINGLETON_MAX_OUTPUT_DIGEST, PAULI_ITER_SINGLETON_MAX_WORK_ITEMS,
    PAULI_ITER_SINGLETON_ODD_CASE_ID, PAULI_ITER_SINGLETON_ODD_OUTPUT_DIGEST,
    PAULI_ITER_SINGLETON_SMALL_INPUT_DIGEST, PAULI_ITER_SINGLETON_SMALL_WORK_ITEMS,
    PauliIterContractKind, PauliIterRejectionClass, pauli_iter_rejection_expectation,
};
use super::sparse_xor::{
    SPARSE_ITEM_BASE_WORK_ITEMS, SPARSE_ITEM_INPUT_BYTES, SPARSE_ITEM_INPUT_DIGEST,
    SPARSE_ITEM_MAX_CASE_ID, SPARSE_ITEM_MAX_OUTPUT_DIGEST, SPARSE_ITEM_MAX_WORK_ITEMS,
    SPARSE_ITEM_SMALL_CASE_ID, SPARSE_ITEM_SMALL_OUTPUT_DIGEST, SPARSE_ROW_BASE_WORK_ITEMS,
    SPARSE_ROW_INPUT_BYTES, SPARSE_ROW_INPUT_DIGEST, SPARSE_ROW_MAX_CASE_ID,
    SPARSE_ROW_MAX_OUTPUT_DIGEST, SPARSE_ROW_MAX_WORK_ITEMS, SPARSE_ROW_SMALL_CASE_ID,
    SPARSE_ROW_SMALL_OUTPUT_DIGEST, SparseXorRejectionClass, sparse_xor_rejection_expectation,
};
use super::transpose::{
    TRANSPOSE_ALLOCATING_EVEN_CASE_ID, TRANSPOSE_ALLOCATING_EVEN_OUTPUT_DIGEST,
    TRANSPOSE_ALLOCATING_MAX_CASE_ID, TRANSPOSE_ALLOCATING_MAX_OUTPUT_DIGEST,
    TRANSPOSE_ALLOCATING_ODD_CASE_ID, TRANSPOSE_ALLOCATING_ODD_OUTPUT_DIGEST,
    TRANSPOSE_IN_PLACE_EVEN_CASE_ID, TRANSPOSE_IN_PLACE_EVEN_OUTPUT_DIGEST,
    TRANSPOSE_IN_PLACE_MAX_CASE_ID, TRANSPOSE_IN_PLACE_MAX_OUTPUT_DIGEST,
    TRANSPOSE_IN_PLACE_ODD_CASE_ID, TRANSPOSE_IN_PLACE_ODD_OUTPUT_DIGEST,
    TRANSPOSE_MAX_INPUT_BYTES, TRANSPOSE_MAX_INPUT_DIGEST, TRANSPOSE_MAX_WORK_ITEMS,
    TRANSPOSE_SMALL_INPUT_BYTES, TRANSPOSE_SMALL_INPUT_DIGEST, TRANSPOSE_SMALL_WORK_ITEMS,
    TransposeRejectionClass, transpose_rejection_expectation,
};
use super::{
    CIRCUIT_CAP_CASE_ID, CONTRACT_PREFLIGHT_SCHEMA_VERSION, DENSE_XOR_ALIGNMENT_CASE_ID,
    DENSE_XOR_CAP_CASE_ID, DENSE_XOR_EVEN_CASE_ID, DENSE_XOR_MAXIMUM_CASE_ID,
    DENSE_XOR_MINIMUM_CASE_ID, DENSE_XOR_ODD_CASE_ID, EVEN_DENSE_XOR_ITERATIONS,
    EVEN_DENSE_XOR_OUTPUT_DIGEST, EVEN_POPCOUNT_ITERATIONS, EVEN_POPCOUNT_OUTPUT_DIGEST,
    GATE_PARTIAL_SWEEP_CASE_ID, InvocationError, MAX_DENSE_XOR_INPUT_DIGEST,
    MAX_DENSE_XOR_OUTPUT_DIGEST, MAX_NOT_ZERO_LATE_INPUT_DIGEST, MAX_NOT_ZERO_LATE_OUTPUT_DIGEST,
    MAX_POPCOUNT_INPUT_DIGEST, MAX_POPCOUNT_OUTPUT_DIGEST, MAX_SUPPORTED_DENSE_XOR_BITS,
    MAX_SUPPORTED_NOT_ZERO_BITS, MAX_SUPPORTED_POPCOUNT_BITS, NOT_ZERO_CAP_CASE_ID,
    NOT_ZERO_EARLY_CASE_ID, NOT_ZERO_ITERATIONS, NOT_ZERO_LATE_CASE_ID, NOT_ZERO_MAXIMUM_CASE_ID,
    NOT_ZERO_MINIMUM_CASE_ID, NOT_ZERO_ZERO_CASE_ID, ODD_DENSE_XOR_ITERATIONS,
    ODD_DENSE_XOR_OUTPUT_DIGEST, ODD_POPCOUNT_ITERATIONS, ODD_POPCOUNT_OUTPUT_DIGEST,
    POPCOUNT_ALIGNMENT_CASE_ID, POPCOUNT_CAP_CASE_ID, POPCOUNT_EVEN_CASE_ID,
    POPCOUNT_MAXIMUM_CASE_ID, POPCOUNT_MINIMUM_CASE_ID, POPCOUNT_ODD_CASE_ID,
    PROTOCOL_SMOKE_CASE_ID, SMALL_DENSE_XOR_BITS, SMALL_DENSE_XOR_INPUT_DIGEST,
    SMALL_NOT_ZERO_BITS, SMALL_NOT_ZERO_EARLY_INPUT_DIGEST, SMALL_NOT_ZERO_EARLY_OUTPUT_DIGEST,
    SMALL_NOT_ZERO_INPUT_BYTES, SMALL_NOT_ZERO_LATE_INPUT_DIGEST,
    SMALL_NOT_ZERO_LATE_OUTPUT_DIGEST, SMALL_NOT_ZERO_ZERO_INPUT_DIGEST,
    SMALL_NOT_ZERO_ZERO_OUTPUT_DIGEST, SMALL_POPCOUNT_BITS, SMALL_POPCOUNT_INPUT_DIGEST,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerContractPreflightEvidence {
    pub(super) schema_version: u32,
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
    worker_identity: &'a WorkerContractIdentityEvidence,
    probes: &'a [WorkerContractProbeEvidence],
}

impl WorkerContractPreflightEvidence {
    pub(super) fn from_actual_probes(
        worker_identity: WorkerContractIdentityEvidence,
        probes: Vec<WorkerContractProbeEvidence>,
    ) -> Result<Self, InvocationError> {
        let evidence = Self {
            schema_version: CONTRACT_PREFLIGHT_SCHEMA_VERSION,
            sha256: worker_contract_preflight_digest(&worker_identity, &probes)?,
            worker_identity,
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
            && worker_contract_preflight_digest(&self.worker_identity, &self.probes)
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
    worker_identity: &WorkerContractIdentityEvidence,
    probes: &[WorkerContractProbeEvidence],
) -> Result<String, InvocationError> {
    let material = serde_json::to_vec(&WorkerContractPreflightDigestMaterial {
        schema_version: CONTRACT_PREFLIGHT_SCHEMA_VERSION,
        worker_identity,
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
    let mut probes = Vec::with_capacity(212);
    let protocol_output_digest = protocol_smoke_output_digest();
    for implementation in [Implementation::Stim, Implementation::Stab] {
        probes.push(expected_accepted_probe(
            PROTOCOL_SMOKE_CASE_ID,
            implementation,
            PROTOCOL_SMOKE_ITERATIONS,
            PROTOCOL_SMOKE_ITERATIONS
                .checked_mul(PROTOCOL_SMOKE_WORK_ITEMS)
                .ok_or(InvocationError::WorkOverflow)?,
            0,
            PROTOCOL_SMOKE_INPUT_DIGEST,
            &protocol_output_digest,
        )?);
    }
    for implementation in [Implementation::Stim, Implementation::Stab] {
        for (case_id, work_items, input_bytes, input_digest, output_digest) in [
            (
                SPARSE_ROW_SMALL_CASE_ID,
                SPARSE_ROW_BASE_WORK_ITEMS,
                SPARSE_ROW_INPUT_BYTES,
                SPARSE_ROW_INPUT_DIGEST,
                SPARSE_ROW_SMALL_OUTPUT_DIGEST,
            ),
            (
                SPARSE_ROW_MAX_CASE_ID,
                SPARSE_ROW_MAX_WORK_ITEMS,
                SPARSE_ROW_INPUT_BYTES,
                SPARSE_ROW_INPUT_DIGEST,
                SPARSE_ROW_MAX_OUTPUT_DIGEST,
            ),
            (
                SPARSE_ITEM_SMALL_CASE_ID,
                SPARSE_ITEM_BASE_WORK_ITEMS,
                SPARSE_ITEM_INPUT_BYTES,
                SPARSE_ITEM_INPUT_DIGEST,
                SPARSE_ITEM_SMALL_OUTPUT_DIGEST,
            ),
            (
                SPARSE_ITEM_MAX_CASE_ID,
                SPARSE_ITEM_MAX_WORK_ITEMS,
                SPARSE_ITEM_INPUT_BYTES,
                SPARSE_ITEM_INPUT_DIGEST,
                SPARSE_ITEM_MAX_OUTPUT_DIGEST,
            ),
        ] {
            probes.push(expected_accepted_probe(
                case_id,
                implementation,
                1,
                work_items,
                input_bytes,
                input_digest,
                output_digest,
            )?);
        }
    }
    for implementation in [Implementation::Stim, Implementation::Stab] {
        for (case_id, iterations, work_items, input_bytes, input_digest, output_digest) in [
            (
                TRANSPOSE_IN_PLACE_ODD_CASE_ID,
                1,
                TRANSPOSE_SMALL_WORK_ITEMS,
                TRANSPOSE_SMALL_INPUT_BYTES,
                TRANSPOSE_SMALL_INPUT_DIGEST,
                TRANSPOSE_IN_PLACE_ODD_OUTPUT_DIGEST,
            ),
            (
                TRANSPOSE_IN_PLACE_EVEN_CASE_ID,
                2,
                TRANSPOSE_SMALL_WORK_ITEMS,
                TRANSPOSE_SMALL_INPUT_BYTES,
                TRANSPOSE_SMALL_INPUT_DIGEST,
                TRANSPOSE_IN_PLACE_EVEN_OUTPUT_DIGEST,
            ),
            (
                TRANSPOSE_IN_PLACE_MAX_CASE_ID,
                1,
                TRANSPOSE_MAX_WORK_ITEMS,
                TRANSPOSE_MAX_INPUT_BYTES,
                TRANSPOSE_MAX_INPUT_DIGEST,
                TRANSPOSE_IN_PLACE_MAX_OUTPUT_DIGEST,
            ),
            (
                TRANSPOSE_ALLOCATING_ODD_CASE_ID,
                1,
                TRANSPOSE_SMALL_WORK_ITEMS,
                TRANSPOSE_SMALL_INPUT_BYTES,
                TRANSPOSE_SMALL_INPUT_DIGEST,
                TRANSPOSE_ALLOCATING_ODD_OUTPUT_DIGEST,
            ),
            (
                TRANSPOSE_ALLOCATING_EVEN_CASE_ID,
                2,
                TRANSPOSE_SMALL_WORK_ITEMS,
                TRANSPOSE_SMALL_INPUT_BYTES,
                TRANSPOSE_SMALL_INPUT_DIGEST,
                TRANSPOSE_ALLOCATING_EVEN_OUTPUT_DIGEST,
            ),
            (
                TRANSPOSE_ALLOCATING_MAX_CASE_ID,
                1,
                TRANSPOSE_MAX_WORK_ITEMS,
                TRANSPOSE_MAX_INPUT_BYTES,
                TRANSPOSE_MAX_INPUT_DIGEST,
                TRANSPOSE_ALLOCATING_MAX_OUTPUT_DIGEST,
            ),
        ] {
            probes.push(expected_accepted_probe(
                case_id,
                implementation,
                iterations,
                iterations
                    .checked_mul(work_items)
                    .ok_or(InvocationError::WorkOverflow)?,
                input_bytes,
                input_digest,
                output_digest,
            )?);
        }
    }
    for implementation in [Implementation::Stim, Implementation::Stab] {
        for (case_id, iterations, work_items, input_digest, output_digest) in [
            (
                PAULI_ITER_RANGE_ODD_CASE_ID,
                1,
                PAULI_ITER_RANGE_SMALL_WORK_ITEMS,
                PAULI_ITER_RANGE_SMALL_INPUT_DIGEST,
                PAULI_ITER_RANGE_ODD_OUTPUT_DIGEST,
            ),
            (
                PAULI_ITER_RANGE_EVEN_CASE_ID,
                2,
                PAULI_ITER_RANGE_SMALL_WORK_ITEMS,
                PAULI_ITER_RANGE_SMALL_INPUT_DIGEST,
                PAULI_ITER_RANGE_EVEN_OUTPUT_DIGEST,
            ),
            (
                PAULI_ITER_RANGE_MAX_CASE_ID,
                1,
                PAULI_ITER_RANGE_MAX_WORK_ITEMS,
                PAULI_ITER_RANGE_MAX_INPUT_DIGEST,
                PAULI_ITER_RANGE_MAX_OUTPUT_DIGEST,
            ),
            (
                PAULI_ITER_SINGLETON_ODD_CASE_ID,
                1,
                PAULI_ITER_SINGLETON_SMALL_WORK_ITEMS,
                PAULI_ITER_SINGLETON_SMALL_INPUT_DIGEST,
                PAULI_ITER_SINGLETON_ODD_OUTPUT_DIGEST,
            ),
            (
                PAULI_ITER_SINGLETON_EVEN_CASE_ID,
                2,
                PAULI_ITER_SINGLETON_SMALL_WORK_ITEMS,
                PAULI_ITER_SINGLETON_SMALL_INPUT_DIGEST,
                PAULI_ITER_SINGLETON_EVEN_OUTPUT_DIGEST,
            ),
            (
                PAULI_ITER_SINGLETON_MAX_CASE_ID,
                1,
                PAULI_ITER_SINGLETON_MAX_WORK_ITEMS,
                PAULI_ITER_SINGLETON_MAX_INPUT_DIGEST,
                PAULI_ITER_SINGLETON_MAX_OUTPUT_DIGEST,
            ),
        ] {
            probes.push(expected_accepted_probe(
                case_id,
                implementation,
                iterations,
                iterations
                    .checked_mul(work_items)
                    .ok_or(InvocationError::WorkOverflow)?,
                PAULI_ITER_INPUT_BYTES,
                input_digest,
                output_digest,
            )?);
        }
    }
    for implementation in [Implementation::Stim, Implementation::Stab] {
        for (case_id, iterations, work_items, input_bytes, input_digest, output_digest) in [
            (
                PAULI_ODD_CASE_ID,
                1,
                PAULI_SMALL_WORK_ITEMS,
                PAULI_SMALL_INPUT_BYTES,
                PAULI_SMALL_INPUT_DIGEST,
                PAULI_ODD_OUTPUT_DIGEST,
            ),
            (
                PAULI_EVEN_CASE_ID,
                2,
                PAULI_SMALL_WORK_ITEMS,
                PAULI_SMALL_INPUT_BYTES,
                PAULI_SMALL_INPUT_DIGEST,
                PAULI_EVEN_OUTPUT_DIGEST,
            ),
            (
                PAULI_MAX_CASE_ID,
                1,
                PAULI_MAX_WORK_ITEMS,
                PAULI_MAX_INPUT_BYTES,
                PAULI_MAX_INPUT_DIGEST,
                PAULI_MAX_OUTPUT_DIGEST,
            ),
        ] {
            probes.push(expected_accepted_probe(
                case_id,
                implementation,
                iterations,
                iterations
                    .checked_mul(work_items)
                    .ok_or(InvocationError::WorkOverflow)?,
                input_bytes,
                input_digest,
                output_digest,
            )?);
        }
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
    for implementation in [Implementation::Stim, Implementation::Stab] {
        probes.push(expected_accepted_probe(
            NOT_ZERO_EARLY_CASE_ID,
            implementation,
            NOT_ZERO_ITERATIONS,
            NOT_ZERO_ITERATIONS * SMALL_NOT_ZERO_BITS,
            SMALL_NOT_ZERO_INPUT_BYTES,
            SMALL_NOT_ZERO_EARLY_INPUT_DIGEST,
            SMALL_NOT_ZERO_EARLY_OUTPUT_DIGEST,
        )?);
        probes.push(expected_accepted_probe(
            NOT_ZERO_ZERO_CASE_ID,
            implementation,
            NOT_ZERO_ITERATIONS,
            NOT_ZERO_ITERATIONS * SMALL_NOT_ZERO_BITS,
            SMALL_NOT_ZERO_INPUT_BYTES,
            SMALL_NOT_ZERO_ZERO_INPUT_DIGEST,
            SMALL_NOT_ZERO_ZERO_OUTPUT_DIGEST,
        )?);
        probes.push(expected_accepted_probe(
            NOT_ZERO_LATE_CASE_ID,
            implementation,
            NOT_ZERO_ITERATIONS,
            NOT_ZERO_ITERATIONS * SMALL_NOT_ZERO_BITS,
            SMALL_NOT_ZERO_INPUT_BYTES,
            SMALL_NOT_ZERO_LATE_INPUT_DIGEST,
            SMALL_NOT_ZERO_LATE_OUTPUT_DIGEST,
        )?);
        probes.push(expected_accepted_probe(
            NOT_ZERO_MAXIMUM_CASE_ID,
            implementation,
            1,
            MAX_SUPPORTED_NOT_ZERO_BITS,
            MAX_SUPPORTED_NOT_ZERO_BITS / 8,
            MAX_NOT_ZERO_LATE_INPUT_DIGEST,
            MAX_NOT_ZERO_LATE_OUTPUT_DIGEST,
        )?);
    }
    for implementation in [Implementation::Stim, Implementation::Stab] {
        probes.push(expected_accepted_probe(
            DENSE_XOR_ODD_CASE_ID,
            implementation,
            ODD_DENSE_XOR_ITERATIONS,
            ODD_DENSE_XOR_ITERATIONS * SMALL_DENSE_XOR_BITS,
            SMALL_DENSE_XOR_BITS / 4,
            SMALL_DENSE_XOR_INPUT_DIGEST,
            ODD_DENSE_XOR_OUTPUT_DIGEST,
        )?);
        probes.push(expected_accepted_probe(
            DENSE_XOR_EVEN_CASE_ID,
            implementation,
            EVEN_DENSE_XOR_ITERATIONS,
            EVEN_DENSE_XOR_ITERATIONS * SMALL_DENSE_XOR_BITS,
            SMALL_DENSE_XOR_BITS / 4,
            SMALL_DENSE_XOR_INPUT_DIGEST,
            EVEN_DENSE_XOR_OUTPUT_DIGEST,
        )?);
        probes.push(expected_accepted_probe(
            DENSE_XOR_MAXIMUM_CASE_ID,
            implementation,
            1,
            MAX_SUPPORTED_DENSE_XOR_BITS,
            MAX_SUPPORTED_DENSE_XOR_BITS / 4,
            MAX_DENSE_XOR_INPUT_DIGEST,
            MAX_DENSE_XOR_OUTPUT_DIGEST,
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
        (
            DENSE_XOR_CAP_CASE_ID,
            super::dense_xor_cap_rejection_expectation as RejectionExpectation,
        ),
        (
            DENSE_XOR_ALIGNMENT_CASE_ID,
            super::dense_xor_alignment_rejection_expectation as RejectionExpectation,
        ),
        (
            DENSE_XOR_MINIMUM_CASE_ID,
            super::dense_xor_minimum_rejection_expectation as RejectionExpectation,
        ),
        (
            NOT_ZERO_CAP_CASE_ID,
            super::not_zero_cap_rejection_expectation as RejectionExpectation,
        ),
        (
            NOT_ZERO_MINIMUM_CASE_ID,
            super::not_zero_minimum_rejection_expectation as RejectionExpectation,
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
    for class in SparseXorRejectionClass::all() {
        for implementation in [Implementation::Stim, Implementation::Stab] {
            let (exit_status, stderr) = sparse_xor_rejection_expectation(implementation, class);
            probes.push(expected_rejected_probe(
                class.case_id(),
                implementation,
                exit_status,
                stderr,
            )?);
        }
    }
    for class in TransposeRejectionClass::all() {
        for implementation in [Implementation::Stim, Implementation::Stab] {
            let (exit_status, stderr) = transpose_rejection_expectation(implementation, class);
            probes.push(expected_rejected_probe(
                class.case_id(),
                implementation,
                exit_status,
                stderr,
            )?);
        }
    }
    for class in PauliRejectionClass::all() {
        for implementation in [Implementation::Stim, Implementation::Stab] {
            let (exit_status, stderr) = pauli_rejection_expectation(implementation, class);
            probes.push(expected_rejected_probe(
                class.case_id(),
                implementation,
                exit_status,
                stderr,
            )?);
        }
    }
    for kind in PauliIterContractKind::all() {
        for class in PauliIterRejectionClass::all() {
            for implementation in [Implementation::Stim, Implementation::Stab] {
                let (exit_status, stderr) =
                    pauli_iter_rejection_expectation(implementation, kind, class);
                probes.push(expected_rejected_probe(
                    class.case_id(kind),
                    implementation,
                    exit_status,
                    stderr,
                )?);
            }
        }
    }
    probes.extend(expected_clifford_probes()?);
    Ok(probes)
}

type RejectionExpectation = fn(Implementation) -> (i32, &'static str);

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
        exit_status: output
            .status
            .ok_or(InvocationError::ContractPreflightDefinition)?,
        stdout_sha256: Sha256Digest::try_new(sha256_hex_bytes(&output.stdout)?)?,
        stderr_sha256: Sha256Digest::try_new(sha256_hex_bytes(&output.stderr)?)?,
    })
}
