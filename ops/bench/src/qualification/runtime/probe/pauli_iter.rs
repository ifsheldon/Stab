use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;

use super::{ProbeError, ProbeGroup, checked_process, probe_environment, probe_limits};
use crate::root::RepoRoot;

use super::super::adapter::AdapterExecutable;
use super::super::invocation::pauli_iter::{
    PAULI_ITER_INPUT_BYTES, PauliIterContractKind, PauliIterRejectionClass,
    checked_pauli_iter_rejection,
};
use super::super::process::{ProcessRequest, run_bounded_process};
use super::super::protocol::{
    EvidenceMode, GitCommit, Implementation, InputDigest, ProtocolExpectation, ProtocolId,
    SemanticDigest, parse_worker_json_lines,
};
use super::super::worker::WorkerIdentity;
use crate::config::STIM_COMMIT;

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
    validate_accepted_maximum(root, kind, adapter, worker_program, worker_identity)?;
    validate_rejections(root, kind, adapter, worker_program)?;
    Ok(())
}

fn validate_accepted_maximum(
    root: &RepoRoot,
    kind: PauliIterContractKind,
    adapter: &AdapterExecutable,
    worker_program: &Path,
    worker_identity: &WorkerIdentity,
) -> Result<(), ProbeError> {
    let workload_id = ProtocolId::try_new(kind.workload())?;
    let measurement_id = ProtocolId::try_new(kind.measurement())?;
    let measurement_ids = BTreeSet::from([measurement_id]);
    let input_digest = InputDigest::try_new(kind.maximum_input_digest())?;
    let output_digest = SemanticDigest::try_new(kind.maximum_output_digest())?;
    let stim_commit = GitCommit::try_new(STIM_COMMIT)?;
    for implementation in [Implementation::Stim, Implementation::Stab] {
        let request = request(
            root,
            implementation,
            adapter,
            worker_program,
            kind.workload(),
            kind.measurement(),
            "1",
            &kind.maximum_work_items().to_string(),
        );
        let name = match implementation {
            Implementation::Stim => "Stim Pauli iterator maximum probe",
            Implementation::Stab => "Stab Pauli iterator maximum probe",
        };
        let output = checked_process(run_bounded_process(&request)?, name)?;
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
            workload_id: workload_id.clone(),
            measurement_ids: measurement_ids.clone(),
            iteration_count: 1,
            expected_work_count: kind.maximum_work_items(),
            expected_input_bytes: PAULI_ITER_INPUT_BYTES,
            expected_input_digest: input_digest.clone(),
            expected_output_digest: Some(output_digest.clone()),
            affinity_cpu: None,
            stim_commit: stim_commit.clone(),
            source_digest,
            build_fingerprint,
        }
        .validate(&rows)?;
    }
    Ok(())
}

fn validate_rejections(
    root: &RepoRoot,
    kind: PauliIterContractKind,
    adapter: &AdapterExecutable,
    worker_program: &Path,
) -> Result<(), ProbeError> {
    for class in PauliIterRejectionClass::all() {
        for implementation in [Implementation::Stim, Implementation::Stab] {
            let request = request(
                root,
                implementation,
                adapter,
                worker_program,
                kind.workload(),
                class.measurement(),
                class.iterations(kind),
                class.work_items(kind),
            );
            let output = run_bounded_process(&request)?;
            checked_pauli_iter_rejection(&output, implementation, kind, class)?;
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "the worker protocol shape is explicit"
)]
fn request(
    root: &RepoRoot,
    implementation: Implementation,
    adapter: &AdapterExecutable,
    worker_program: &Path,
    workload: &str,
    measurement: &str,
    iterations: &str,
    work_items: &str,
) -> ProcessRequest {
    let mut args = Vec::with_capacity(11);
    if implementation == Implementation::Stab {
        args.push(OsString::from("qualification-worker"));
    }
    args.extend([
        OsString::from("--workload"),
        OsString::from(workload),
        OsString::from("--measurement-id"),
        OsString::from(measurement),
        OsString::from("--iterations"),
        OsString::from(iterations),
        OsString::from("--work-items"),
        OsString::from(work_items),
        OsString::from("--evidence-mode"),
        OsString::from("timing"),
    ]);
    ProcessRequest {
        program: match implementation {
            Implementation::Stim => adapter.path.clone(),
            Implementation::Stab => worker_program.to_path_buf(),
        },
        args,
        stdin: Vec::new(),
        working_directory: root.path.clone(),
        environment: probe_environment(),
        affinity_cpu: None,
        limits: probe_limits(),
    }
}

const fn contract_kind(group: ProbeGroup) -> Option<PauliIterContractKind> {
    match group {
        ProbeGroup::PauliStringIterRangeAdapter => Some(PauliIterContractKind::Range),
        ProbeGroup::PauliStringIterSingletonAdapter => Some(PauliIterContractKind::Singleton),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterator_probe_groups_map_to_exact_boundary_contracts() {
        assert_eq!(
            contract_kind(ProbeGroup::PauliStringIterRangeAdapter),
            Some(PauliIterContractKind::Range)
        );
        assert_eq!(
            contract_kind(ProbeGroup::PauliStringIterSingletonAdapter),
            Some(PauliIterContractKind::Singleton)
        );
        assert_eq!(contract_kind(ProbeGroup::PauliStringMultiplyAdapter), None);
    }

    #[test]
    fn each_iterator_probe_runs_one_maximum_and_twelve_rejections() {
        assert_eq!(PauliIterRejectionClass::all().len() * 2 + 2, 14);
    }
}
