use std::collections::BTreeSet;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use super::protocol::{
    InputDigest, ProtocolId, RAW_WORK_TIMING_BOUNDARY, Sha256Digest, TimingBoundary,
};
use super::run::ClaimClass;
use crate::qualification::model::{SizeClass, TimingBatchPolicy};
use crate::root::RepoRoot;

mod comparators;
#[cfg(test)]
mod test_contracts;

const GROUP_CONTRACT_PATH: &str = "benchmarks/qualification-runtime-groups.json";
const GROUP_CONTRACT_SCHEMA_VERSION: u32 = 7;
const MAX_GROUP_CONTRACT_BYTES: usize = 1 << 20;
const MAX_GROUPS: usize = 256;
const MAX_RELEASE_GROUPS: usize = 40;
const MAX_DIAGNOSTIC_GROUPS: usize = 60;
const MAX_MEASUREMENTS_PER_GROUP: usize = 64;
const MAX_CORRECTNESS_CASES_PER_GROUP: usize = 4096;
const MAX_SCALES_PER_GROUP: usize = 64;
const MAX_PROFILER_NOTE_PATH_BYTES: usize = 512;
const MAX_COMPARATOR_SOURCE_PATH_BYTES: usize = 512;
const MAX_PROFILER_NOTE_BYTES: usize = 64 << 10;
const MAX_COMPARATOR_SOURCE_BYTES: usize = 1 << 20;
const PROFILER_NOTE_PREFIX: &str = "benchmarks/profiler-notes/qualification/";
const COMPARATOR_SOURCE_PREFIX: &str = "benchmarks/stim_adapter/";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ParityEligibility {
    ReportOnly,
    ThresholdEligible,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GroupContract {
    pub(super) id: ProtocolId,
    pub(super) claim_class: ClaimClass,
    pub(super) parity_eligibility: ParityEligibility,
    pub(super) timing_batch_policy: TimingBatchPolicy,
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
    pub(super) family_id: ProtocolId,
    pub(super) size_class: SizeClass,
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
    timing_boundary: TimingBoundary,
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
                    && contract.family_id.to_string() == inventory.family_id
                    && contract.size_class == inventory.size_class
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
            || group.timing_policy.batch_policy != contract.timing_batch_policy
            || group.status == QualificationStatus::Planned
            || inventory_scale_ids != contract_scale_ids
            || group.memory_policy.scale_ids != contract_scale_ids
            || inventory_comparator_sources != contract_comparator_sources
            || !scales_match
        {
            return Err(GroupError::InventoryContract(contract.id.to_string()));
        }
    }
    for row in &suite.manifest_rows {
        for replacement in &row.replacement_contracts {
            let contract = file
                .groups
                .iter()
                .find(|group| group.id.to_string() == replacement.runtime_group_id)
                .ok_or_else(|| GroupError::ReplacementContract {
                    row: row.id.clone(),
                    group: replacement.runtime_group_id.clone(),
                    measurement: replacement.runtime_measurement_id.clone(),
                })?;
            if !contract
                .measurement_ids
                .iter()
                .any(|measurement| measurement.to_string() == replacement.runtime_measurement_id)
                || replacement.runtime_scale_id.as_ref().is_some_and(|scale| {
                    !contract
                        .scales
                        .iter()
                        .any(|candidate| candidate.id.to_string() == *scale)
                })
            {
                return Err(GroupError::ReplacementContract {
                    row: row.id.clone(),
                    group: replacement.runtime_group_id.clone(),
                    measurement: replacement.runtime_measurement_id.clone(),
                });
            }
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
    if file.timing_boundary != RAW_WORK_TIMING_BOUNDARY {
        return Err(GroupError::TimingBoundary);
    }
    if !valid_sha256(expected_inventory_sha256)
        || file.performance_inventory_sha256 != expected_inventory_sha256
    {
        return Err(GroupError::Inventory {
            actual: file.performance_inventory_sha256.clone(),
            expected: expected_inventory_sha256.to_string(),
        });
    }
    if file.groups.is_empty() || file.groups.len() > MAX_GROUPS {
        return Err(GroupError::GroupCount(file.groups.len()));
    }
    let release_groups = file
        .groups
        .iter()
        .filter(|group| group.claim_class == ClaimClass::PromotablePerformance)
        .count();
    let diagnostic_groups = file
        .groups
        .iter()
        .filter(|group| group.claim_class == ClaimClass::DiagnosticInfrastructure)
        .count();
    if release_groups > MAX_RELEASE_GROUPS || diagnostic_groups > MAX_DIAGNOSTIC_GROUPS {
        return Err(GroupError::MatrixCap {
            release: release_groups,
            release_max: MAX_RELEASE_GROUPS,
            diagnostic: diagnostic_groups,
            diagnostic_max: MAX_DIAGNOSTIC_GROUPS,
        });
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
        let expected_comparator_paths = comparators::expected_paths(group.id.to_string().as_str());
        if measurement_ids.len() != group.measurement_ids.len()
            || scale_ids.len() != group.scales.len()
            || !valid_scale_families(&group.scales)
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
        match (group.claim_class, group.parity_eligibility) {
            (ClaimClass::DiagnosticInfrastructure, ParityEligibility::ReportOnly)
                if group.correctness_case_ids.is_empty() => {}
            (ClaimClass::PromotablePerformance, ParityEligibility::ThresholdEligible)
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

fn valid_scale_families(scales: &[ScaleContract]) -> bool {
    let mut families = std::collections::BTreeMap::<&ProtocolId, Vec<&ScaleContract>>::new();
    for scale in scales {
        families.entry(&scale.family_id).or_default().push(scale);
    }
    families.into_values().all(|family| {
        let mut seen_classes = BTreeSet::new();
        family
            .iter()
            .all(|scale| seen_classes.insert(scale.size_class))
            && family.windows(2).all(|pair| {
                matches!(
                    pair,
                    [left, right]
                        if left.size_class < right.size_class
                            && left.work_items < right.work_items
                )
            })
    })
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
    #[error("runtime group contract does not use the raw-work-v2 timing boundary")]
    TimingBoundary,
    #[error("runtime group contract performance inventory digest is {actual}, expected {expected}")]
    Inventory { actual: String, expected: String },
    #[error("runtime group contract has an invalid group count: {0}")]
    GroupCount(usize),
    #[error(
        "runtime group matrix exceeds its cap: release={release}/{release_max}, diagnostic={diagnostic}/{diagnostic_max}"
    )]
    MatrixCap {
        release: usize,
        release_max: usize,
        diagnostic: usize,
        diagnostic_max: usize,
    },
    #[error("runtime group contract group is invalid: {0}")]
    InvalidGroup(String),
    #[error("runtime group contract does not define group {0}")]
    UnknownGroup(String),
    #[error("runtime group contract group {group} does not define scale {scale}")]
    UnknownScale { group: String, scale: String },
    #[error("runtime group {0} does not match the implemented worker shape")]
    UnsupportedRuntimeShape(String),
    #[error(
        "manifest row {row} replacement target {group}/{measurement} is not an executable runtime measurement"
    )]
    ReplacementContract {
        row: String,
        group: String,
        measurement: String,
    },
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
mod tests;
