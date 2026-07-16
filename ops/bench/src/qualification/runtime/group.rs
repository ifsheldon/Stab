use std::collections::BTreeSet;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use super::protocol::{InputDigest, ProtocolId, Sha256Digest};
use super::run::ClaimClass;
use crate::root::RepoRoot;

const GROUP_CONTRACT_PATH: &str = "benchmarks/qualification-runtime-groups.json";
const GROUP_CONTRACT_SCHEMA_VERSION: u32 = 4;
const MAX_GROUP_CONTRACT_BYTES: usize = 1 << 20;
const MAX_GROUPS: usize = 256;
const MAX_MEASUREMENTS_PER_GROUP: usize = 64;
const MAX_CORRECTNESS_CASES_PER_GROUP: usize = 4096;
const MAX_SCALES_PER_GROUP: usize = 64;
const MAX_PROFILER_NOTE_PATH_BYTES: usize = 512;
const MAX_COMPARATOR_SOURCE_PATH_BYTES: usize = 512;
const MAX_PROFILER_NOTE_BYTES: usize = 64 << 10;
const MAX_COMPARATOR_SOURCE_BYTES: usize = 1 << 20;
const PROFILER_NOTE_PREFIX: &str = "benchmarks/profiler-notes/qualification/";
const COMPARATOR_SOURCE_PREFIX: &str = "benchmarks/stim_adapter/";
const SIMD_WORD_POPCOUNT_GROUP_ID: &str = "PERFQ-M5-SIMD-WORD";
const SIMD_WORD_POPCOUNT_COMPARATOR_PATHS: [&str; 2] = [
    "benchmarks/stim_adapter/main.cc",
    "benchmarks/stim_adapter/simd_word_popcount_contract.h",
];
const SIMD_BITS_XOR_GROUP_ID: &str = "PERFQ-M5-SIMD-BITS";
const SIMD_BITS_XOR_COMPARATOR_PATHS: [&str; 2] = [
    "benchmarks/stim_adapter/main.cc",
    "benchmarks/stim_adapter/simd_bits_xor_contract.h",
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum BaselineEligibility {
    ReportOnly,
    ThresholdEligible,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GroupContract {
    pub(super) id: ProtocolId,
    pub(super) claim_class: ClaimClass,
    pub(super) baseline_eligibility: BaselineEligibility,
    pub(super) workload_id: ProtocolId,
    pub(super) measurement_ids: Vec<ProtocolId>,
    pub(super) scales: Vec<ScaleContract>,
    pub(super) correctness_case_ids: Vec<String>,
    pub(super) owner: ProtocolId,
    pub(super) profiler_note: Option<ProfilerNoteContract>,
    #[serde(default)]
    pub(super) comparator_sources: Vec<ComparatorSourceContract>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ComparatorSourceContract {
    pub(super) path: ComparatorSourcePath,
    pub(super) sha256: Sha256Digest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(super) struct ComparatorSourcePath(Box<str>);

impl ComparatorSourcePath {
    fn try_new(value: String) -> Result<Self, GroupError> {
        if value.is_empty()
            || value.len() > MAX_COMPARATOR_SOURCE_PATH_BYTES
            || !value.starts_with(COMPARATOR_SOURCE_PREFIX)
            || value
                .split('/')
                .any(|component| component.is_empty() || component == "." || component == "..")
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/')
            })
        {
            return Err(GroupError::ComparatorSourcePath(value));
        }
        Ok(Self(value.into_boxed_str()))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ComparatorSourcePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProfilerNoteContract {
    pub(super) path: ProfilerNotePath,
    pub(super) sha256: Sha256Digest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(super) struct ProfilerNotePath(Box<str>);

impl ProfilerNotePath {
    fn try_new(value: String) -> Result<Self, GroupError> {
        if value.is_empty()
            || value.len() > MAX_PROFILER_NOTE_PATH_BYTES
            || !value.starts_with(PROFILER_NOTE_PREFIX)
            || !value.ends_with(".md")
            || value
                .split('/')
                .any(|component| component.is_empty() || component == "." || component == "..")
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/')
            })
        {
            return Err(GroupError::ProfilerNotePath(value));
        }
        Ok(Self(value.into_boxed_str()))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProfilerNotePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScaleContract {
    pub(super) id: ProtocolId,
    pub(super) work_items: NonZeroU64,
    pub(super) input_bytes: u64,
    pub(super) input_digest: InputDigest,
}

impl GroupContract {
    pub(super) fn single_measurement(&self) -> Result<&ProtocolId, GroupError> {
        let [measurement] = self.measurement_ids.as_slice() else {
            return Err(GroupError::UnsupportedRuntimeShape(self.id.to_string()));
        };
        Ok(measurement)
    }

    pub(super) fn validate_worker_shape(
        &self,
        workload_id: &ProtocolId,
        measurement_id: &ProtocolId,
    ) -> Result<(), GroupError> {
        if self.workload_id != *workload_id || self.single_measurement()? != measurement_id {
            return Err(GroupError::UnsupportedRuntimeShape(self.id.to_string()));
        }
        Ok(())
    }

    pub(super) fn scale(&self, scale_id: &str) -> Result<&ScaleContract, GroupError> {
        self.scales
            .iter()
            .find(|scale| scale.id.to_string() == scale_id)
            .ok_or_else(|| GroupError::UnknownScale {
                group: self.id.to_string(),
                scale: scale_id.to_string(),
            })
    }
}

#[derive(Clone, Debug)]
pub(super) struct ResolvedGroupContract {
    pub(super) source_sha256: String,
    pub(super) contract: GroupContract,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupContractFile {
    schema_version: u32,
    performance_inventory_sha256: String,
    groups: Vec<GroupContract>,
}

pub(super) fn load_group(
    root: &RepoRoot,
    expected_inventory_sha256: &str,
    group_id: &str,
) -> Result<ResolvedGroupContract, GroupError> {
    let (file, source_sha256) = load(root, expected_inventory_sha256)?;
    let contract = file
        .groups
        .into_iter()
        .find(|group| group.id.to_string() == group_id)
        .ok_or_else(|| GroupError::UnknownGroup(group_id.to_string()))?;
    Ok(ResolvedGroupContract {
        source_sha256,
        contract,
    })
}

pub(super) fn load_groups(
    root: &RepoRoot,
    expected_inventory_sha256: &str,
) -> Result<Vec<GroupContract>, GroupError> {
    load(root, expected_inventory_sha256).map(|(file, _)| file.groups)
}

pub(super) fn check(
    root: &RepoRoot,
    expected_inventory_sha256: &str,
    suite: &super::super::model::QualificationSuite,
) -> Result<(), GroupError> {
    let (file, _) = load(root, expected_inventory_sha256)?;
    validate_inventory_contracts(&file, suite)
}

fn validate_inventory_contracts(
    file: &GroupContractFile,
    suite: &super::super::model::QualificationSuite,
) -> Result<(), GroupError> {
    use super::super::model::{
        CorrectnessBinding, EvidenceState, InputByteCount, PerformanceDisposition,
        QualificationStatus, RunnerFidelity, ThresholdPolicy,
    };

    let runtime_group_ids = file
        .groups
        .iter()
        .filter(|group| group.claim_class == ClaimClass::PromotablePerformance)
        .map(|group| group.id.to_string())
        .collect::<BTreeSet<_>>();
    let inventory_group_ids = suite
        .qualification_groups
        .iter()
        .filter(|group| {
            group.status != QualificationStatus::Planned
                && group.threshold_policy == ThresholdPolicy::Primary1_25
        })
        .map(|group| group.id.clone())
        .collect::<BTreeSet<_>>();
    if runtime_group_ids != inventory_group_ids {
        return Err(GroupError::InventoryCoverage {
            runtime_only: runtime_group_ids
                .difference(&inventory_group_ids)
                .cloned()
                .collect(),
            inventory_only: inventory_group_ids
                .difference(&runtime_group_ids)
                .cloned()
                .collect(),
        });
    }

    for contract in file
        .groups
        .iter()
        .filter(|group| group.claim_class == ClaimClass::PromotablePerformance)
    {
        let group = suite
            .qualification_groups
            .iter()
            .find(|candidate| candidate.id == contract.id.to_string())
            .ok_or_else(|| GroupError::InventoryContract(contract.id.to_string()))?;
        let contract_scale_ids = contract
            .scales
            .iter()
            .map(|scale| scale.id.to_string())
            .collect::<Vec<_>>();
        let inventory_scale_ids = group
            .workload_family
            .scales
            .iter()
            .map(|scale| scale.id.clone())
            .collect::<Vec<_>>();
        let scales_match = contract
            .scales
            .iter()
            .zip(&group.workload_family.scales)
            .all(|(contract, inventory)| {
                contract.id.to_string() == inventory.id
                    && inventory.semantic_work == Some(contract.work_items.get())
                    && inventory.input_bytes
                        == InputByteCount::Exact {
                            bytes: contract.input_bytes,
                        }
                    && inventory.input_digest.as_deref() == Some(contract.input_digest.as_str())
            });
        let contract_comparator_sources = contract
            .comparator_sources
            .iter()
            .map(|source| (source.path.as_str(), source.sha256.as_str()))
            .collect::<Vec<_>>();
        let inventory_comparator_sources = group
            .output_contract
            .comparator_sources
            .iter()
            .map(|source| (source.path.as_str(), source.sha256.as_str()))
            .collect::<Vec<_>>();
        if group.disposition != PerformanceDisposition::Measured
            || group.runner_fidelity != RunnerFidelity::AdapterLibrary
            || group.correctness_binding != CorrectnessBinding::ExactCases
            || group.correctness_cases != contract.correctness_case_ids
            || group.owner != contract.owner.to_string()
            || group.planned_correctness_case_id.is_some()
            || group.output_contract.digest_state != EvidenceState::Existing
            || group.threshold_policy != ThresholdPolicy::Primary1_25
            || group.status == QualificationStatus::Planned
            || inventory_scale_ids != contract_scale_ids
            || group.memory_policy.scale_ids != contract_scale_ids
            || inventory_comparator_sources != contract_comparator_sources
            || !scales_match
        {
            return Err(GroupError::InventoryContract(contract.id.to_string()));
        }
    }
    Ok(())
}

fn load(
    root: &RepoRoot,
    expected_inventory_sha256: &str,
) -> Result<(GroupContractFile, String), GroupError> {
    let path = root.path.join(GROUP_CONTRACT_PATH);
    let bytes =
        crate::source_file::read_repo_regular_file_bounded(root, &path, MAX_GROUP_CONTRACT_BYTES)
            .map_err(|error| GroupError::Read(error.to_string()))?;
    let file: GroupContractFile = serde_json::from_slice(&bytes).map_err(GroupError::Json)?;
    validate(&file, expected_inventory_sha256)?;
    validate_profiler_notes(root, &file)?;
    validate_comparator_sources(root, &file)?;
    Ok((file, super::run::sha256_hex(&bytes)))
}

fn validate_profiler_notes(root: &RepoRoot, file: &GroupContractFile) -> Result<(), GroupError> {
    for group in &file.groups {
        let Some(note) = &group.profiler_note else {
            continue;
        };
        let path = root.path.join(note.path.as_str());
        let bytes = crate::source_file::read_repo_regular_file_bounded(
            root,
            &path,
            MAX_PROFILER_NOTE_BYTES,
        )
        .map_err(|error| GroupError::ProfilerNote(error.to_string()))?;
        if super::run::sha256_hex(&bytes) != note.sha256.as_str() {
            return Err(GroupError::ProfilerNoteDigest(group.id.to_string()));
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|error| GroupError::ProfilerNote(error.to_string()))?;
        if !text.contains("Dominant cost:") || !text.contains("Next owner action:") {
            return Err(GroupError::ProfilerNoteContent(group.id.to_string()));
        }
    }
    Ok(())
}

fn validate_comparator_sources(
    root: &RepoRoot,
    file: &GroupContractFile,
) -> Result<(), GroupError> {
    for group in &file.groups {
        for source in &group.comparator_sources {
            let path = root.path.join(source.path.as_str());
            let bytes = crate::source_file::read_repo_regular_file_bounded(
                root,
                &path,
                MAX_COMPARATOR_SOURCE_BYTES,
            )
            .map_err(|error| GroupError::ComparatorSource(error.to_string()))?;
            if super::run::sha256_hex(&bytes) != source.sha256.as_str() {
                return Err(GroupError::ComparatorSourceDigest(group.id.to_string()));
            }
        }
    }
    Ok(())
}

fn validate(file: &GroupContractFile, expected_inventory_sha256: &str) -> Result<(), GroupError> {
    if file.schema_version != GROUP_CONTRACT_SCHEMA_VERSION {
        return Err(GroupError::SchemaVersion {
            actual: file.schema_version,
            expected: GROUP_CONTRACT_SCHEMA_VERSION,
        });
    }
    if !valid_sha256(expected_inventory_sha256)
        || file.performance_inventory_sha256 != expected_inventory_sha256
    {
        return Err(GroupError::Inventory);
    }
    if file.groups.is_empty() || file.groups.len() > MAX_GROUPS {
        return Err(GroupError::GroupCount(file.groups.len()));
    }
    let mut group_ids = BTreeSet::new();
    for group in &file.groups {
        if !group_ids.insert(group.id.clone())
            || group.measurement_ids.is_empty()
            || group.measurement_ids.len() > MAX_MEASUREMENTS_PER_GROUP
            || group.scales.is_empty()
            || group.scales.len() > MAX_SCALES_PER_GROUP
            || group.correctness_case_ids.len() > MAX_CORRECTNESS_CASES_PER_GROUP
        {
            return Err(GroupError::InvalidGroup(group.id.to_string()));
        }
        let measurement_ids = group.measurement_ids.iter().collect::<BTreeSet<_>>();
        let scale_ids = group
            .scales
            .iter()
            .map(|scale| &scale.id)
            .collect::<BTreeSet<_>>();
        let correctness_case_ids = group.correctness_case_ids.iter().collect::<BTreeSet<_>>();
        let comparator_paths = group
            .comparator_sources
            .iter()
            .map(|source| source.path.as_str())
            .collect::<Vec<_>>();
        let expected_comparator_paths = match group.id.to_string().as_str() {
            SIMD_WORD_POPCOUNT_GROUP_ID => SIMD_WORD_POPCOUNT_COMPARATOR_PATHS.as_slice(),
            SIMD_BITS_XOR_GROUP_ID => SIMD_BITS_XOR_COMPARATOR_PATHS.as_slice(),
            _ => &[],
        };
        if measurement_ids.len() != group.measurement_ids.len()
            || scale_ids.len() != group.scales.len()
            || !group
                .scales
                .windows(2)
                .all(|pair| matches!(pair, [left, right] if left.work_items < right.work_items))
            || correctness_case_ids.len() != group.correctness_case_ids.len()
            || !group
                .correctness_case_ids
                .windows(2)
                .all(|pair| matches!(pair, [left, right] if left < right))
            || group
                .correctness_case_ids
                .iter()
                .any(|case| !valid_case_id(case))
            || comparator_paths != expected_comparator_paths
        {
            return Err(GroupError::InvalidGroup(group.id.to_string()));
        }
        match (group.claim_class, group.baseline_eligibility) {
            (ClaimClass::DiagnosticInfrastructure, BaselineEligibility::ReportOnly)
                if group.correctness_case_ids.is_empty() => {}
            (ClaimClass::PromotablePerformance, BaselineEligibility::ThresholdEligible)
                if !group.correctness_case_ids.is_empty() => {}
            _ => return Err(GroupError::InvalidGroup(group.id.to_string())),
        }
    }
    if file.groups.len() != super::invocation::registered_group_count() {
        return Err(GroupError::ExecutableRegistration);
    }
    if let Some(group) = file
        .groups
        .iter()
        .find(|group| !super::invocation::supports_group(group))
    {
        return Err(GroupError::UnsupportedRuntimeShape(group.id.to_string()));
    }
    Ok(())
}

fn valid_case_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
        })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Error)]
pub(super) enum GroupError {
    #[error("failed to read the source-owned runtime group contract: {0}")]
    Read(String),
    #[error("runtime group contract JSON is invalid: {0}")]
    Json(serde_json::Error),
    #[error("runtime group contract schema is {actual}, expected {expected}")]
    SchemaVersion { actual: u32, expected: u32 },
    #[error("runtime group contract has a stale performance inventory digest")]
    Inventory,
    #[error("runtime group contract has an invalid group count: {0}")]
    GroupCount(usize),
    #[error("runtime group contract group is invalid: {0}")]
    InvalidGroup(String),
    #[error("runtime group contract does not define group {0}")]
    UnknownGroup(String),
    #[error("runtime group contract group {group} does not define scale {scale}")]
    UnknownScale { group: String, scale: String },
    #[error("runtime group {0} does not match the implemented worker shape")]
    UnsupportedRuntimeShape(String),
    #[error("runtime group contract does not exactly match the executable group registry")]
    ExecutableRegistration,
    #[error("runtime group contract does not match performance inventory group {0}")]
    InventoryContract(String),
    #[error(
        "runtime and implemented threshold-eligible inventory groups differ: runtime-only={runtime_only:?}, inventory-only={inventory_only:?}"
    )]
    InventoryCoverage {
        runtime_only: Vec<String>,
        inventory_only: Vec<String>,
    },
    #[error("invalid source-owned profiler-note path {0:?}")]
    ProfilerNotePath(String),
    #[error("failed to read source-owned profiler note: {0}")]
    ProfilerNote(String),
    #[error("runtime group {0} profiler-note digest is stale")]
    ProfilerNoteDigest(String),
    #[error("runtime group {0} profiler note lacks required cost and owner-action fields")]
    ProfilerNoteContent(String),
    #[error("invalid source-owned comparator path {0:?}")]
    ComparatorSourcePath(String),
    #[error("failed to read source-owned comparator source: {0}")]
    ComparatorSource(String),
    #[error("runtime group {0} comparator-source digest is stale")]
    ComparatorSourceDigest(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_contract_file() -> GroupContractFile {
        GroupContractFile {
            schema_version: GROUP_CONTRACT_SCHEMA_VERSION,
            performance_inventory_sha256: "a".repeat(64),
            groups: vec![
                GroupContract {
                    id: ProtocolId::try_new(super::super::invocation::PQ1_GROUP_ID)
                        .expect("group id"),
                    claim_class: ClaimClass::DiagnosticInfrastructure,
                    baseline_eligibility: BaselineEligibility::ReportOnly,
                    workload_id: ProtocolId::try_new("protocol-smoke").expect("workload id"),
                    measurement_ids: vec![ProtocolId::try_new("main").expect("measurement id")],
                    scales: vec![ScaleContract {
                        id: ProtocolId::try_new("default").expect("scale id"),
                        work_items: NonZeroU64::new(4096).expect("positive work"),
                        input_bytes: 0,
                        input_digest: InputDigest::try_new(
                            "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1",
                        )
                        .expect("empty input digest"),
                    }],
                    correctness_case_ids: Vec::new(),
                    owner: ProtocolId::try_new("ops/bench").expect("owner"),
                    profiler_note: None,
                    comparator_sources: Vec::new(),
                },
                GroupContract {
                    id: ProtocolId::try_new(
                        super::super::invocation::CIRCUIT_CANONICAL_PRINT_GROUP_ID,
                    )
                    .expect("group id"),
                    claim_class: ClaimClass::PromotablePerformance,
                    baseline_eligibility: BaselineEligibility::ThresholdEligible,
                    workload_id: ProtocolId::try_new("circuit-canonical-print")
                        .expect("workload id"),
                    measurement_ids: vec![
                        ProtocolId::try_new("serialize").expect("measurement id"),
                    ],
                    scales: vec![ScaleContract {
                        id: ProtocolId::try_new("small").expect("scale id"),
                        work_items: NonZeroU64::new(64).expect("positive work"),
                        input_bytes: 64,
                        input_digest: InputDigest::try_new("b".repeat(64)).expect("input digest"),
                    }],
                    correctness_case_ids: vec!["cq-evidence-canonical-print".to_string()],
                    owner: ProtocolId::try_new("stab-core/circuit-printer").expect("owner"),
                    profiler_note: None,
                    comparator_sources: Vec::new(),
                },
                GroupContract {
                    id: ProtocolId::try_new(super::super::invocation::CIRCUIT_PARSE_GROUP_ID)
                        .expect("group id"),
                    claim_class: ClaimClass::PromotablePerformance,
                    baseline_eligibility: BaselineEligibility::ThresholdEligible,
                    workload_id: ProtocolId::try_new("circuit-parse").expect("workload id"),
                    measurement_ids: vec![ProtocolId::try_new("parse").expect("measurement id")],
                    scales: vec![ScaleContract {
                        id: ProtocolId::try_new("small").expect("scale id"),
                        work_items: NonZeroU64::new(64).expect("positive work"),
                        input_bytes: 64,
                        input_digest: InputDigest::try_new("a".repeat(64)).expect("input digest"),
                    }],
                    correctness_case_ids: vec!["cq-evidence-example".to_string()],
                    owner: ProtocolId::try_new("stab-core/circuit-parser").expect("owner"),
                    profiler_note: Some(ProfilerNoteContract {
                        path: ProfilerNotePath::try_new(
                            "benchmarks/profiler-notes/qualification/example.md".to_string(),
                        )
                        .expect("note path"),
                        sha256: Sha256Digest::try_new("d".repeat(64)).expect("note digest"),
                    }),
                    comparator_sources: Vec::new(),
                },
                GroupContract {
                    id: ProtocolId::try_new(super::super::invocation::GATE_NAME_HASH_GROUP_ID)
                        .expect("group id"),
                    claim_class: ClaimClass::PromotablePerformance,
                    baseline_eligibility: BaselineEligibility::ThresholdEligible,
                    workload_id: ProtocolId::try_new("gate-name-hash").expect("workload id"),
                    measurement_ids: vec![
                        ProtocolId::try_new("hash-all-names").expect("measurement id"),
                    ],
                    scales: vec![ScaleContract {
                        id: ProtocolId::try_new("small").expect("scale id"),
                        work_items: NonZeroU64::new(82).expect("positive work"),
                        input_bytes: 0,
                        input_digest: InputDigest::try_new(
                            "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1",
                        )
                        .expect("empty input digest"),
                    }],
                    correctness_case_ids: vec!["cq-evidence-gate-name-hash".to_string()],
                    owner: ProtocolId::try_new("stab-core/gates").expect("owner"),
                    profiler_note: None,
                    comparator_sources: Vec::new(),
                },
                GroupContract {
                    id: ProtocolId::try_new(super::super::invocation::SIMD_BITS_XOR_GROUP_ID)
                        .expect("group id"),
                    claim_class: ClaimClass::PromotablePerformance,
                    baseline_eligibility: BaselineEligibility::ThresholdEligible,
                    workload_id: ProtocolId::try_new("simd-bits-xor").expect("workload id"),
                    measurement_ids: vec![
                        ProtocolId::try_new("xor-complete-vector").expect("measurement id"),
                    ],
                    scales: vec![ScaleContract {
                        id: ProtocolId::try_new("small").expect("scale id"),
                        work_items: NonZeroU64::new(4_096).expect("positive work"),
                        input_bytes: 1_024,
                        input_digest: InputDigest::try_new("d".repeat(64)).expect("input digest"),
                    }],
                    correctness_case_ids: vec!["cq-evidence-simd-bits-xor".to_string()],
                    owner: ProtocolId::try_new("stab-core/bits").expect("owner"),
                    profiler_note: None,
                    comparator_sources: SIMD_BITS_XOR_COMPARATOR_PATHS
                        .iter()
                        .map(|path| ComparatorSourceContract {
                            path: ComparatorSourcePath::try_new((*path).to_string())
                                .expect("comparator path"),
                            sha256: Sha256Digest::try_new("e".repeat(64))
                                .expect("comparator digest"),
                        })
                        .collect(),
                },
                GroupContract {
                    id: ProtocolId::try_new(super::super::invocation::SIMD_WORD_POPCOUNT_GROUP_ID)
                        .expect("group id"),
                    claim_class: ClaimClass::PromotablePerformance,
                    baseline_eligibility: BaselineEligibility::ThresholdEligible,
                    workload_id: ProtocolId::try_new("simd-word-popcount").expect("workload id"),
                    measurement_ids: vec![
                        ProtocolId::try_new("toggle-popcount").expect("measurement id"),
                    ],
                    scales: vec![ScaleContract {
                        id: ProtocolId::try_new("small").expect("scale id"),
                        work_items: NonZeroU64::new(4_096).expect("positive work"),
                        input_bytes: 512,
                        input_digest: InputDigest::try_new("e".repeat(64)).expect("input digest"),
                    }],
                    correctness_case_ids: vec!["cq-evidence-simd-word-popcount".to_string()],
                    owner: ProtocolId::try_new("stab-core/bits").expect("owner"),
                    profiler_note: None,
                    comparator_sources: SIMD_WORD_POPCOUNT_COMPARATOR_PATHS
                        .iter()
                        .map(|path| ComparatorSourceContract {
                            path: ComparatorSourcePath::try_new((*path).to_string())
                                .expect("comparator path"),
                            sha256: Sha256Digest::try_new("f".repeat(64))
                                .expect("comparator digest"),
                        })
                        .collect(),
                },
            ],
        }
    }

    #[test]
    fn diagnostic_groups_are_report_only_and_have_no_correctness_cases() {
        let valid = valid_contract_file();
        validate(&valid, &"a".repeat(64)).expect("valid diagnostic contract");

        let mut thresholded = valid;
        thresholded
            .groups
            .first_mut()
            .expect("one group")
            .baseline_eligibility = BaselineEligibility::ThresholdEligible;
        assert!(matches!(
            validate(&thresholded, &"a".repeat(64)),
            Err(GroupError::InvalidGroup(_))
        ));
    }

    #[test]
    fn product_contract_allows_profiler_note_to_follow_a_failure() {
        let mut file = valid_contract_file();
        file.groups
            .iter_mut()
            .find(|group| group.claim_class == ClaimClass::PromotablePerformance)
            .expect("product group")
            .profiler_note = None;
        validate(&file, &"a".repeat(64)).expect("product contract without a preemptive note");
    }

    #[test]
    fn source_contract_rejects_unregistered_groups() {
        let mut unsupported = valid_contract_file();
        unsupported.groups.first_mut().expect("diagnostic group").id =
            ProtocolId::try_new("unregistered").expect("group id");
        assert!(matches!(
            validate(&unsupported, &"a".repeat(64)),
            Err(GroupError::UnsupportedRuntimeShape(group)) if group == "unregistered"
        ));
    }

    #[test]
    fn source_contract_rejects_duplicate_and_zero_scales() {
        let mut duplicate = valid_contract_file();
        duplicate
            .groups
            .first_mut()
            .expect("diagnostic group")
            .scales = vec![
            ScaleContract {
                id: ProtocolId::try_new("same").expect("scale id"),
                work_items: NonZeroU64::new(1).expect("positive work"),
                input_bytes: 1,
                input_digest: InputDigest::try_new("a".repeat(64)).expect("input digest"),
            },
            ScaleContract {
                id: ProtocolId::try_new("same").expect("scale id"),
                work_items: NonZeroU64::new(2).expect("positive work"),
                input_bytes: 2,
                input_digest: InputDigest::try_new("b".repeat(64)).expect("input digest"),
            },
        ];
        assert!(matches!(
            validate(&duplicate, &"a".repeat(64)),
            Err(GroupError::InvalidGroup(_))
        ));

        let zero = serde_json::json!({
            "schema_version": GROUP_CONTRACT_SCHEMA_VERSION,
            "performance_inventory_sha256": "a".repeat(64),
            "groups": [{
                "id": "group",
                "claim_class": "diagnostic-infrastructure",
                "baseline_eligibility": "report-only",
                "workload_id": "protocol-smoke",
                "measurement_ids": ["main"],
                "scales": [{
                    "id": "zero",
                    "work_items": 0,
                    "input_bytes": 0,
                    "input_digest": "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1"
                }],
                "correctness_case_ids": [],
                "owner": "ops/bench",
                "profiler_note": null
            }]
        });
        assert!(serde_json::from_value::<GroupContractFile>(zero).is_err());

        let mut nonmonotonic = valid_contract_file();
        nonmonotonic
            .groups
            .first_mut()
            .expect("diagnostic group")
            .scales = vec![
            ScaleContract {
                id: ProtocolId::try_new("small").expect("scale id"),
                work_items: NonZeroU64::new(2).expect("positive work"),
                input_bytes: 2,
                input_digest: InputDigest::try_new("a".repeat(64)).expect("input digest"),
            },
            ScaleContract {
                id: ProtocolId::try_new("large").expect("scale id"),
                work_items: NonZeroU64::new(1).expect("positive work"),
                input_bytes: 1,
                input_digest: InputDigest::try_new("b".repeat(64)).expect("input digest"),
            },
        ];
        assert!(matches!(
            validate(&nonmonotonic, &"a".repeat(64)),
            Err(GroupError::InvalidGroup(_))
        ));
    }

    #[test]
    fn scale_lookup_is_exact_and_fail_closed() {
        let file = valid_contract_file();
        let group = file.groups.first().expect("diagnostic group");
        assert_eq!(
            group.scale("default").expect("default scale").work_items,
            NonZeroU64::new(4096).expect("positive work")
        );
        assert!(matches!(
            group.scale("Default"),
            Err(GroupError::UnknownScale { group, scale })
                if group == super::super::invocation::PQ1_GROUP_ID && scale == "Default"
        ));
    }

    #[test]
    fn runtime_contract_rejects_inventory_scale_drift() {
        let root =
            RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .expect("repository root");
        let manifest = crate::manifest::BenchmarkManifest::read(&root).expect("manifest");
        let mut suite = super::super::super::discovery::generate(&root, &manifest)
            .expect("generated performance inventory");
        let (file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");
        validate_inventory_contracts(&file, &suite).expect("matching ledgers");

        let scale = suite
            .qualification_groups
            .iter_mut()
            .find(|group| group.id == super::super::invocation::CIRCUIT_PARSE_GROUP_ID)
            .and_then(|group| group.workload_family.scales.first_mut())
            .expect("circuit parse scale");
        scale.semantic_work = scale.semantic_work.and_then(|work| work.checked_add(1));

        assert!(matches!(
            validate_inventory_contracts(&file, &suite),
            Err(GroupError::InventoryContract(group))
                if group == super::super::invocation::CIRCUIT_PARSE_GROUP_ID
        ));

        let mut suite = super::super::super::discovery::generate(&root, &manifest)
            .expect("generated performance inventory");
        let scale = suite
            .qualification_groups
            .iter_mut()
            .find(|group| group.id == super::super::invocation::CIRCUIT_PARSE_GROUP_ID)
            .and_then(|group| group.workload_family.scales.first_mut())
            .expect("circuit parse scale");
        scale.input_digest = Some("e".repeat(64));
        assert!(matches!(
            validate_inventory_contracts(&file, &suite),
            Err(GroupError::InventoryContract(group))
                if group == super::super::invocation::CIRCUIT_PARSE_GROUP_ID
        ));
    }

    #[test]
    fn runtime_contract_rejects_inventory_groups_without_runtime_owners() {
        let root =
            RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .expect("repository root");
        let manifest = crate::manifest::BenchmarkManifest::read(&root).expect("manifest");
        let mut suite = super::super::super::discovery::generate(&root, &manifest)
            .expect("generated performance inventory");
        let (file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");
        let mut orphan = suite
            .qualification_groups
            .iter()
            .find(|group| group.id == super::super::invocation::CIRCUIT_PARSE_GROUP_ID)
            .expect("implemented threshold group")
            .clone();
        orphan.id = "PERFQ-ORPHAN".to_string();
        suite.qualification_groups.push(orphan);

        assert!(matches!(
            validate_inventory_contracts(&file, &suite),
            Err(GroupError::InventoryCoverage {
                runtime_only,
                inventory_only,
            }) if runtime_only.is_empty() && inventory_only == ["PERFQ-ORPHAN"]
        ));

        suite.qualification_groups.retain(|group| {
            group.id != "PERFQ-ORPHAN"
                && group.id != super::super::invocation::CIRCUIT_PARSE_GROUP_ID
        });
        assert!(matches!(
            validate_inventory_contracts(&file, &suite),
            Err(GroupError::InventoryCoverage {
                runtime_only,
                inventory_only,
            }) if runtime_only == [super::super::invocation::CIRCUIT_PARSE_GROUP_ID]
                && inventory_only.is_empty()
        ));
    }

    #[test]
    fn runtime_contract_rejects_stale_profiler_note_digest() {
        let root =
            RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .expect("repository root");
        let manifest = crate::manifest::BenchmarkManifest::read(&root).expect("manifest");
        let suite = super::super::super::discovery::generate(&root, &manifest)
            .expect("generated performance inventory");
        let (mut file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");
        file.groups
            .iter_mut()
            .find(|group| group.id.to_string() == super::super::invocation::CIRCUIT_PARSE_GROUP_ID)
            .and_then(|group| group.profiler_note.as_mut())
            .expect("profiler note")
            .sha256 = Sha256Digest::try_new("e".repeat(64)).expect("different digest");

        assert!(matches!(
            validate_profiler_notes(&root, &file),
            Err(GroupError::ProfilerNoteDigest(group))
                if group == super::super::invocation::CIRCUIT_PARSE_GROUP_ID
        ));
    }

    #[test]
    fn runtime_contract_rejects_stale_comparator_source_digest() {
        let root =
            RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .expect("repository root");
        let manifest = crate::manifest::BenchmarkManifest::read(&root).expect("manifest");
        let suite = super::super::super::discovery::generate(&root, &manifest)
            .expect("generated performance inventory");
        let (mut file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");
        file.groups
            .iter_mut()
            .find(|group| {
                group.id.to_string() == super::super::invocation::SIMD_WORD_POPCOUNT_GROUP_ID
            })
            .and_then(|group| group.comparator_sources.first_mut())
            .expect("comparator source")
            .sha256 = Sha256Digest::try_new("e".repeat(64)).expect("different digest");

        assert!(matches!(
            validate_comparator_sources(&root, &file),
            Err(GroupError::ComparatorSourceDigest(group))
                if group == super::super::invocation::SIMD_WORD_POPCOUNT_GROUP_ID
        ));
    }
}
