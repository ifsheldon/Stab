use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::super::artifact::{
    DirectQualificationArtifactPath, QualificationOutput, RepositoryBinding,
};
use super::super::host::HostEvidence;
use super::super::protocol::{RAW_WORK_TIMING_BOUNDARY, TimingBoundary};
use super::super::run::RepositoryEvidence;
use super::super::run::sha256_hex;
use super::super::worker;
use super::dem_model::DemAcceptedMaximumMemory;
use super::{ProbeArgs, ProbeError, ProbeEvidenceMode, ProbeGroup};
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::root::RepoRoot;

const MEMORY_RECEIPT_SCHEMA_VERSION: u32 = 2;
pub(in crate::qualification::runtime) const MAX_MEMORY_RECEIPT_BYTES: usize = 4 << 20;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::qualification::runtime) struct AdapterProbeReceipt {
    pub(super) probe_id: String,
    pub(super) runtime_group_id: String,
    pub(super) evidence_mode: String,
    pub(super) iteration_count: u64,
    pub(super) work_items: u64,
    pub(super) work_count: u64,
    pub(super) input_bytes: u64,
    pub(super) input_digest: String,
    pub(super) output_digest: String,
    pub(in crate::qualification::runtime) stim_source_sha256: String,
    pub(in crate::qualification::runtime) stim_build_fingerprint: String,
    pub(in crate::qualification::runtime) stim_binary_sha256: String,
    pub(in crate::qualification::runtime) stab_source_sha256: String,
    pub(in crate::qualification::runtime) stab_build_fingerprint: String,
}

#[derive(Debug)]
pub(super) struct AdapterProbeExecution {
    pub(super) receipt: AdapterProbeReceipt,
    pub(super) dem_accepted_maximum_memory: Vec<DemAcceptedMaximumMemory>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DemAcceptedMaximumMemoryReceipt {
    schema_version: u32,
    output: String,
    repository: RepositoryEvidence,
    runtime_group_id: String,
    timing_boundary: TimingBoundary,
    stim_tag: String,
    stim_commit: String,
    host: HostEvidence,
    probe: AdapterProbeReceipt,
    accepted_maximum_memory: Vec<DemAcceptedMaximumMemory>,
}

#[derive(Debug)]
pub(in crate::qualification::runtime) struct DemMemoryReceiptEvidence {
    pub(in crate::qualification::runtime) path: PathBuf,
    pub(in crate::qualification::runtime) report_sha256: String,
    pub(in crate::qualification::runtime) repository: RepositoryEvidence,
    pub(in crate::qualification::runtime) runtime_group_id: String,
    pub(in crate::qualification::runtime) host: HostEvidence,
    pub(in crate::qualification::runtime) probe: AdapterProbeReceipt,
}

pub(super) fn prepare_output(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    args: &ProbeArgs,
) -> Result<Option<DirectQualificationArtifactPath>, ProbeError> {
    validate_args(args)?;
    let output = args
        .out
        .as_deref()
        .map(DirectQualificationArtifactPath::try_new)
        .transpose()?;
    if let Some(output) = &output {
        QualificationOutput::require_absent_with_repository(root, repository, output)?;
    }
    Ok(output)
}

fn validate_args(args: &ProbeArgs) -> Result<(), ProbeError> {
    if args.out.is_some()
        && (args.evidence_mode != ProbeEvidenceMode::Memory
            || !matches!(
                args.group,
                ProbeGroup::DemParseAdapter | ProbeGroup::DemCanonicalPrintAdapter
            ))
    {
        return Err(ProbeError::Contract(
            "--out is supported only for DEM probes in memory mode".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn require_clean_repository(
    state: &super::super::git::RepositoryState,
) -> Result<(), ProbeError> {
    if state.local_modifications {
        Err(ProbeError::DirtyRepository)
    } else {
        Ok(())
    }
}

pub(super) fn bind_repository(
    before: super::super::git::RepositoryState,
    after: super::super::git::RepositoryState,
) -> Result<RepositoryEvidence, ProbeError> {
    require_clean_repository(&before)?;
    require_clean_repository(&after)?;
    if before.commit != after.commit {
        return Err(ProbeError::RepositoryChanged {
            before: before.commit,
            after: after.commit,
        });
    }
    Ok(RepositoryEvidence {
        commit_before: before.commit,
        commit_after: after.commit,
        local_modifications_before: before.local_modifications,
        local_modifications_after: after.local_modifications,
    })
}

pub(super) fn publish(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    output_path: DirectQualificationArtifactPath,
    repository_evidence: RepositoryEvidence,
    host: HostEvidence,
    execution: AdapterProbeExecution,
) -> Result<(), ProbeError> {
    validate_execution(&execution)?;
    let receipt = DemAcceptedMaximumMemoryReceipt {
        schema_version: MEMORY_RECEIPT_SCHEMA_VERSION,
        output: output_path.as_path().display().to_string(),
        repository: repository_evidence.clone(),
        runtime_group_id: execution.receipt.runtime_group_id.clone(),
        timing_boundary: RAW_WORK_TIMING_BOUNDARY,
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        host,
        probe: execution.receipt,
        accepted_maximum_memory: execution.dem_accepted_maximum_memory,
    };
    let mut bytes = serde_json::to_vec_pretty(&receipt)?;
    bytes.push(b'\n');
    let mut output =
        QualificationOutput::begin_new_with_repository(root, repository, &output_path)?;
    output.write("report.json", &bytes)?;
    output.commit_new_with_source_validation(|binding| {
        super::super::run::require_current_repository(root, &repository_evidence, binding)
    })?;
    println!(
        "[stab-bench] wrote accepted-maximum DEM memory receipt to {}",
        output_path.as_path().display()
    );
    Ok(())
}

pub(in crate::qualification::runtime) fn inspect_memory_receipt(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    path: &DirectQualificationArtifactPath,
) -> Result<DemMemoryReceiptEvidence, ProbeError> {
    let bytes = super::super::artifact::read_artifact_bounded_with_repository(
        root,
        repository,
        path,
        "report.json",
        MAX_MEMORY_RECEIPT_BYTES,
    )?;
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(ProbeError::MemoryReceipt);
    }
    let receipt: DemAcceptedMaximumMemoryReceipt = serde_json::from_slice(&bytes)?;
    let mut canonical = serde_json::to_vec_pretty(&receipt)?;
    canonical.push(b'\n');
    if canonical != bytes
        || receipt.schema_version != MEMORY_RECEIPT_SCHEMA_VERSION
        || Path::new(&receipt.output) != path.as_path()
        || receipt.repository.commit_before != receipt.repository.commit_after
        || receipt.repository.local_modifications_before
        || receipt.repository.local_modifications_after
        || receipt.timing_boundary != RAW_WORK_TIMING_BOUNDARY
        || receipt.stim_tag != STIM_TAG
        || receipt.stim_commit != STIM_COMMIT
        || receipt.probe.runtime_group_id != receipt.runtime_group_id
        || receipt.probe.evidence_mode != ProbeEvidenceMode::Memory.as_str()
        || !valid_probe_identity(&receipt.probe)
        || !valid_accepted_maximum_memory(&receipt.accepted_maximum_memory)
    {
        return Err(ProbeError::MemoryReceipt);
    }
    receipt.host.validate_against_policy(source_root)?;
    Ok(DemMemoryReceiptEvidence {
        path: path.as_path().to_path_buf(),
        report_sha256: sha256_hex(&bytes),
        repository: receipt.repository,
        runtime_group_id: receipt.runtime_group_id,
        host: receipt.host,
        probe: receipt.probe,
    })
}

fn validate_execution(execution: &AdapterProbeExecution) -> Result<(), ProbeError> {
    if execution.receipt.evidence_mode != ProbeEvidenceMode::Memory.as_str()
        || !valid_probe_identity(&execution.receipt)
        || !valid_accepted_maximum_memory(&execution.dem_accepted_maximum_memory)
    {
        return Err(ProbeError::MemoryReceipt);
    }
    Ok(())
}

fn valid_accepted_maximum_memory(values: &[DemAcceptedMaximumMemory]) -> bool {
    let expected = worker::dem_model::DemFamily::ALL;
    values.len() == expected.len()
        && values.iter().zip(expected).all(|(actual, family)| {
            actual.family_id == family.id()
                && actual.work_items == family.maximum_items()
                && actual.input_bytes > 0
                && valid_sha256(&actual.input_digest)
                && valid_sha256(&actual.output_digest)
                && actual.stim_peak_rss_bytes >= actual.stim_setup_rss_bytes
                && actual.stab_peak_rss_bytes >= actual.stab_setup_rss_bytes
        })
}

fn valid_probe_identity(receipt: &AdapterProbeReceipt) -> bool {
    let expected_probe = match receipt.runtime_group_id.as_str() {
        super::DEM_PARSE_RUNTIME_GROUP_ID => super::DEM_PARSE_PROBE_ID,
        super::DEM_CANONICAL_PRINT_RUNTIME_GROUP_ID => super::DEM_CANONICAL_PRINT_PROBE_ID,
        _ => return false,
    };
    receipt.probe_id == expected_probe
        && receipt.iteration_count > 0
        && receipt.work_items > 0
        && receipt.work_count > 0
        && receipt.input_bytes > 0
        && [
            &receipt.input_digest,
            &receipt.output_digest,
            &receipt.stim_source_sha256,
            &receipt.stim_build_fingerprint,
            &receipt.stim_binary_sha256,
            &receipt.stab_source_sha256,
            &receipt.stab_build_fingerprint,
        ]
        .into_iter()
        .all(|digest| valid_sha256(digest))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;

    fn args(group: ProbeGroup, evidence_mode: ProbeEvidenceMode) -> ProbeArgs {
        ProbeArgs {
            group,
            iterations: NonZeroU64::new(1).expect("nonzero iterations"),
            work_items: None,
            evidence_mode,
            out: None,
        }
    }

    fn execution() -> AdapterProbeExecution {
        AdapterProbeExecution {
            receipt: AdapterProbeReceipt {
                probe_id: super::super::DEM_PARSE_PROBE_ID.to_string(),
                runtime_group_id: super::super::DEM_PARSE_RUNTIME_GROUP_ID.to_string(),
                evidence_mode: "memory".to_string(),
                iteration_count: 1,
                work_items: 64,
                work_count: 64,
                input_bytes: 1,
                input_digest: "a".repeat(64),
                output_digest: "b".repeat(64),
                stim_source_sha256: "c".repeat(64),
                stim_build_fingerprint: "d".repeat(64),
                stim_binary_sha256: "e".repeat(64),
                stab_source_sha256: "f".repeat(64),
                stab_build_fingerprint: "0".repeat(64),
            },
            dem_accepted_maximum_memory: worker::dem_model::DemFamily::ALL
                .into_iter()
                .map(|family| DemAcceptedMaximumMemory {
                    family_id: family.id().to_string(),
                    work_items: family.maximum_items(),
                    input_bytes: 1,
                    input_digest: "1".repeat(64),
                    output_digest: "2".repeat(64),
                    stim_setup_rss_bytes: 10,
                    stim_peak_rss_bytes: 20,
                    stim_parent_observed_peak_rss_bytes: Some(30),
                    stab_setup_rss_bytes: 11,
                    stab_peak_rss_bytes: 21,
                    stab_parent_observed_peak_rss_bytes: Some(31),
                })
                .collect(),
        }
    }

    #[test]
    fn publication_is_limited_to_dem_memory_evidence() {
        let mut dem = args(ProbeGroup::DemParseAdapter, ProbeEvidenceMode::Memory);
        dem.out = Some(PathBuf::from(
            "target/benchmarks/qualification/dem-parse-memory",
        ));
        assert!(validate_args(&dem).is_ok());

        let mut timing = dem.clone();
        timing.evidence_mode = ProbeEvidenceMode::Timing;
        assert!(validate_args(&timing).is_err());

        let mut non_dem = args(ProbeGroup::CircuitParseAdapter, ProbeEvidenceMode::Memory);
        non_dem.out = dem.out;
        assert!(validate_args(&non_dem).is_err());
    }

    #[test]
    fn receipt_requires_every_family_maximum_and_valid_rss_lifecycle() {
        let valid = execution();
        assert!(validate_execution(&valid).is_ok());

        let mut missing_family = execution();
        missing_family.dem_accepted_maximum_memory.pop();
        assert!(matches!(
            validate_execution(&missing_family),
            Err(ProbeError::MemoryReceipt)
        ));

        let mut inverted_rss = execution();
        inverted_rss
            .dem_accepted_maximum_memory
            .first_mut()
            .expect("first family")
            .stab_peak_rss_bytes = 10;
        assert!(matches!(
            validate_execution(&inverted_rss),
            Err(ProbeError::MemoryReceipt)
        ));
    }
}
