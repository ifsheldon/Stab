use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use super::{ProbeError, ProbeGroup, checked_process, probe_environment};
use crate::config::STIM_COMMIT;
use crate::root::RepoRoot;

use super::super::adapter::AdapterExecutable;
use super::super::clifford_vectors::{CliffordRequestResult, checked_file};
use super::super::invocation::clifford_string::{checked_clifford_rejection, clifford_arguments};
use super::super::process::{ProcessLimits, ProcessRequest, run_bounded_process};
use super::super::protocol::{
    EvidenceMode, GitCommit, Implementation, InputDigest, ProtocolExpectation, ProtocolId,
    SemanticDigest, parse_worker_json_lines,
};
use super::super::worker::WorkerIdentity;
use super::super::worker::clifford_string::{
    CLIFFORD_DESCRIPTOR_BYTES, CLIFFORD_PUBLIC_CAP, CliffordDescriptor, CliffordWorkloadKind,
};

pub(super) const IDENTITY_RUNTIME_GROUP_ID: &str = "PERFQ-M6-CLIFFORD-STRING";
pub(super) const NON_IDENTITY_RUNTIME_GROUP_ID: &str = "PERFQ-M6-CLIFFORD-STRING-NON-IDENTITY";
const IDENTITY_PROBE_ID: &str = "pq2-clifford-string-identity-adapter-smoke";
const NON_IDENTITY_PROBE_ID: &str = "pq2-clifford-string-non-identity-adapter-smoke";
const PROTOCOL_OUTPUT_LIMIT: usize = 1 << 20;

pub(super) fn probe_contract(
    group: ProbeGroup,
) -> Option<(&'static str, &'static str, &'static str)> {
    let kind = contract_kind(group)?;
    let probe_id = match kind {
        CliffordWorkloadKind::Identity => IDENTITY_PROBE_ID,
        CliffordWorkloadKind::NonIdentity => NON_IDENTITY_PROBE_ID,
    };
    Some((probe_id, kind.workload(), kind.measurement()))
}

pub(super) fn append_descriptor_arguments(
    group: ProbeGroup,
    work_items: u64,
    arguments: &mut Vec<OsString>,
) -> Result<(), ProbeError> {
    let Some(kind) = contract_kind(group) else {
        return Ok(());
    };
    let descriptor = CliffordDescriptor::canonical(kind, work_items);
    arguments.extend([
        OsString::from("--input-descriptor-hex"),
        OsString::from(descriptor.to_string()),
    ]);
    Ok(())
}

pub(super) fn validate_work_items(group: ProbeGroup, work_items: u64) -> Result<(), ProbeError> {
    if contract_kind(group).is_some() && !(1..=CLIFFORD_PUBLIC_CAP).contains(&work_items) {
        return Err(ProbeError::Contract(format!(
            "Clifford-string probe width {work_items} is outside 1..={CLIFFORD_PUBLIC_CAP} qubits"
        )));
    }
    Ok(())
}

pub(super) fn validate_boundaries(
    root: &RepoRoot,
    group: ProbeGroup,
    adapter: &AdapterExecutable,
    worker_program: &Path,
    worker_identity: &WorkerIdentity,
) -> Result<(), ProbeError> {
    let Some(kind) = contract_kind(group) else {
        return Ok(());
    };
    let file = checked_file().map_err(ProbeError::Contract)?;
    for vector in file
        .requests
        .iter()
        .filter(|vector| vector.workload == kind.workload())
    {
        for implementation in [Implementation::Stim, Implementation::Stab] {
            match vector.result {
                CliffordRequestResult::Accepted => validate_acceptance(
                    root,
                    implementation,
                    adapter,
                    worker_program,
                    worker_identity,
                    vector,
                )?,
                CliffordRequestResult::Rejected => {
                    let request =
                        request(root, implementation, adapter, worker_program, vector, false);
                    let output = run_bounded_process(&request)?;
                    checked_clifford_rejection(&output, implementation, vector)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_acceptance(
    root: &RepoRoot,
    implementation: Implementation,
    adapter: &AdapterExecutable,
    worker_program: &Path,
    worker_identity: &WorkerIdentity,
    vector: &super::super::clifford_vectors::CliffordRequestVector,
) -> Result<(), ProbeError> {
    let request = request(root, implementation, adapter, worker_program, vector, true);
    let label = match implementation {
        Implementation::Stim => "Stim Clifford boundary probe",
        Implementation::Stab => "Stab Clifford boundary probe",
    };
    let output = checked_process(run_bounded_process(&request)?, label)?;
    let rows = parse_worker_json_lines(&output.stdout)?;
    let (source_digest, build_fingerprint) = match implementation {
        Implementation::Stim => (
            adapter.source_digest.clone(),
            adapter.build_fingerprint.clone(),
        ),
        Implementation::Stab => (
            worker_identity.source_digest.clone(),
            worker_identity.build_fingerprint.clone(),
        ),
    };
    ProtocolExpectation {
        implementation,
        evidence_mode: EvidenceMode::Timing,
        workload_id: ProtocolId::try_new(vector.workload.clone())?,
        measurement_ids: BTreeSet::from([ProtocolId::try_new(vector.measurement_id.clone())?]),
        iteration_count: vector.iterations,
        expected_work_count: vector
            .iterations
            .checked_mul(vector.work_items)
            .ok_or(ProbeError::WorkOverflow)?,
        expected_input_bytes: CLIFFORD_DESCRIPTOR_BYTES,
        expected_input_digest: InputDigest::try_new(vector.input_sha256.clone())?,
        expected_output_digest: Some(SemanticDigest::try_new(
            vector.output_sha256.clone().ok_or_else(|| {
                ProbeError::Contract(format!(
                    "accepted Clifford vector {} lacks output_sha256",
                    vector.id
                ))
            })?,
        )?),
        affinity_cpu: None,
        stim_commit: GitCommit::try_new(STIM_COMMIT)?,
        source_digest,
        build_fingerprint,
    }
    .validate(&rows)?;
    Ok(())
}

fn request(
    root: &RepoRoot,
    implementation: Implementation,
    adapter: &AdapterExecutable,
    worker_program: &Path,
    vector: &super::super::clifford_vectors::CliffordRequestVector,
    release_barrier: bool,
) -> ProcessRequest {
    let mut arguments = clifford_arguments(vector);
    if implementation == Implementation::Stab {
        arguments.insert(0, OsString::from("qualification-worker"));
    }
    ProcessRequest {
        program: match implementation {
            Implementation::Stim => adapter.path.clone(),
            Implementation::Stab => worker_program.to_path_buf(),
        },
        args: arguments,
        stdin: if release_barrier {
            vec![b'\n']
        } else {
            Vec::new()
        },
        working_directory: root.path.clone(),
        environment: probe_environment().into(),
        affinity_cpu: None,
        limits: ProcessLimits {
            stdin_bytes: usize::from(release_barrier),
            stdout: (PROTOCOL_OUTPUT_LIMIT).into(),
            stderr: (64 << 10).into(),
            regular_file_bytes: None,
            timeout: Duration::from_secs(30),
        },
    }
}

const fn contract_kind(group: ProbeGroup) -> Option<CliffordWorkloadKind> {
    match group {
        ProbeGroup::CliffordStringIdentityAdapter => Some(CliffordWorkloadKind::Identity),
        ProbeGroup::CliffordStringNonIdentityAdapter => Some(CliffordWorkloadKind::NonIdentity),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::DEFAULT_CLIFFORD_WORK_ITEMS;
    use super::*;

    #[test]
    fn probe_groups_bind_distinct_runtime_contracts_and_frozen_defaults() {
        for (group, expected_id, kind) in [
            (
                ProbeGroup::CliffordStringIdentityAdapter,
                IDENTITY_RUNTIME_GROUP_ID,
                CliffordWorkloadKind::Identity,
            ),
            (
                ProbeGroup::CliffordStringNonIdentityAdapter,
                NON_IDENTITY_RUNTIME_GROUP_ID,
                CliffordWorkloadKind::NonIdentity,
            ),
        ] {
            assert_eq!(group.runtime_group_id(), Some(expected_id));
            let mut arguments = Vec::new();
            append_descriptor_arguments(group, DEFAULT_CLIFFORD_WORK_ITEMS, &mut arguments)
                .expect("descriptor arguments");
            assert_eq!(
                arguments.get(1).and_then(|value| value.to_str()),
                Some(
                    CliffordDescriptor::canonical(kind, DEFAULT_CLIFFORD_WORK_ITEMS)
                        .to_string()
                        .as_str()
                )
            );
        }
    }

    #[test]
    fn probe_widths_enforce_the_public_resource_contract() {
        for group in [
            ProbeGroup::CliffordStringIdentityAdapter,
            ProbeGroup::CliffordStringNonIdentityAdapter,
        ] {
            assert!(validate_work_items(group, 1).is_ok());
            assert!(validate_work_items(group, CLIFFORD_PUBLIC_CAP).is_ok());
            assert!(validate_work_items(group, 0).is_err());
            assert!(validate_work_items(group, CLIFFORD_PUBLIC_CAP + 1).is_err());
        }
    }
}
