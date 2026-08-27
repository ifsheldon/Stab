use std::collections::BTreeSet;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use super::protocol::{
    InputDigest, ProtocolId, RAW_WORK_TIMING_BOUNDARY, SemanticDigest, Sha256Digest, TimingBoundary,
};
use super::run::ClaimClass;
use crate::qualification::model::{RowOrigin, SizeClass, TimingBatchPolicy};
use crate::root::RepoRoot;

mod comparators;
#[cfg(test)]
mod test_contracts;

const GROUP_CONTRACT_PATH: &str = "benchmarks/qualification-runtime-groups.json";
pub(in crate::qualification) const GROUP_CONTRACT_SCHEMA_VERSION: u32 = 11;
const MAX_GROUP_CONTRACT_BYTES: usize = 1 << 20;
const MAX_GROUPS: usize = 256;
const MAX_RELEASE_GROUPS: usize = 40;
const MAX_DIAGNOSTIC_GROUPS: usize = 60;
const MAX_MEASUREMENTS_PER_GROUP: usize = 64;
const MAX_CORRECTNESS_CASES_PER_GROUP: usize = 4096;
const MAX_PUBLIC_API_ITEMS_PER_GROUP: usize = 4096;
const MAX_CHECKLIST_ITEMS_PER_GROUP: usize = 512;
const MAX_CHECKLIST_CHILDREN_PER_GROUP: usize = 4096;
const MAX_SCALES_PER_GROUP: usize = 64;
const MAX_PROFILER_NOTE_PATH_BYTES: usize = 512;
const MAX_COMPARATOR_SOURCE_PATH_BYTES: usize = 512;
const MAX_PROFILER_NOTE_BYTES: usize = 64 << 10;
const MAX_COMPARATOR_SOURCE_BYTES: usize = 1 << 20;
const MAX_PRODUCT_DIAGNOSTIC_SUITE_TIMEOUT_SECONDS: u64 = 3_600;
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
    pub(super) feature_id: ProtocolId,
    pub(super) origin: RowOrigin,
    pub(super) claim_class: ClaimClass,
    pub(super) parity_eligibility: ParityEligibility,
    pub(super) timing_batch_policy: TimingBatchPolicy,
    pub(super) workload_id: ProtocolId,
    pub(super) measurement_ids: Vec<ProtocolId>,
    pub(super) scales: Vec<ScaleContract>,
    pub(super) correctness_case_ids: Vec<String>,
    #[serde(default)]
    pub(super) public_api_item_ids: Vec<String>,
    #[serde(default)]
    pub(super) checklist_item_ids: Vec<String>,
    #[serde(default)]
    pub(super) checklist_child_ids: Vec<String>,
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ProductDiagnosticBatchPolicy {
    CalibratedRepeat,
    SinglePass,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProductDiagnosticScalePolicy {
    pub(super) scale_id: ProtocolId,
    pub(super) batch_policy: ProductDiagnosticBatchPolicy,
    pub(super) witness_case_id: String,
    pub(super) expected_output_digest: SemanticDigest,
    pub(super) max_worker_peak_rss_bytes: Option<NonZeroU64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProductDiagnosticPolicy {
    pub(super) group_id: ProtocolId,
    pub(super) scales: Vec<ProductDiagnosticScalePolicy>,
}

impl ProductDiagnosticPolicy {
    pub(super) fn scale(
        &self,
        scale_id: &ProtocolId,
    ) -> Result<&ProductDiagnosticScalePolicy, GroupError> {
        self.scales
            .iter()
            .find(|scale| scale.scale_id == *scale_id)
            .ok_or_else(|| GroupError::UnknownDiagnosticPolicyScale {
                group: self.group_id.to_string(),
                scale: scale_id.to_string(),
            })
    }
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
    pub(super) product_diagnostic_suite_timeout_seconds: NonZeroU64,
    pub(super) product_diagnostic_policy: Option<ProductDiagnosticPolicy>,
    pub(super) contract: GroupContract,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupContractFile {
    schema_version: u32,
    timing_boundary: TimingBoundary,
    performance_inventory_sha256: String,
    product_diagnostic_suite_timeout_seconds: NonZeroU64,
    product_diagnostic_policies: Vec<ProductDiagnosticPolicy>,
    groups: Vec<GroupContract>,
}

pub(super) fn load_group(
    root: &RepoRoot,
    expected_inventory_sha256: &str,
    group_id: &str,
) -> Result<ResolvedGroupContract, GroupError> {
    let (file, source_sha256) = load(root, expected_inventory_sha256)?;
    let product_diagnostic_policy = file
        .product_diagnostic_policies
        .iter()
        .find(|policy| policy.group_id.to_string() == group_id)
        .cloned();
    let contract = file
        .groups
        .into_iter()
        .find(|group| group.id.to_string() == group_id)
        .ok_or_else(|| GroupError::UnknownGroup(group_id.to_string()))?;
    Ok(ResolvedGroupContract {
        source_sha256,
        product_diagnostic_suite_timeout_seconds: file.product_diagnostic_suite_timeout_seconds,
        product_diagnostic_policy,
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
    references: &super::super::discovery::SourceReferences,
) -> Result<(), GroupError> {
    let (file, _) = load(root, expected_inventory_sha256)?;
    validate_inventory_contracts(&file, suite, references)
}

fn validate_inventory_contracts(
    file: &GroupContractFile,
    suite: &super::super::model::QualificationSuite,
    references: &super::super::discovery::SourceReferences,
) -> Result<(), GroupError> {
    let feature_ids = suite
        .performance_features
        .iter()
        .map(|feature| feature.id.as_str())
        .collect::<BTreeSet<_>>();
    let runtime_group_ids = file
        .groups
        .iter()
        .map(|group| group.id.to_string())
        .collect::<BTreeSet<_>>();
    let runtime_correctness_cases = file
        .groups
        .iter()
        .flat_map(|group| group.correctness_case_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    if runtime_correctness_cases != references.correctness_cases {
        return Err(GroupError::CorrectnessPrerequisiteCoverage {
            bridge_only: references
                .correctness_cases
                .difference(&runtime_correctness_cases)
                .cloned()
                .collect(),
            runtime_only: runtime_correctness_cases
                .difference(&references.correctness_cases)
                .cloned()
                .collect(),
        });
    }
    let mut inherited_links = BTreeSet::new();
    let mut public_api_owners = BTreeSet::new();
    let mut checklist_item_owners = BTreeSet::new();
    let mut checklist_child_owners = BTreeSet::new();
    for contract in &file.groups {
        let group_id = contract.id.to_string();
        let feature_id = contract.feature_id.to_string();
        if !feature_ids.contains(feature_id.as_str()) {
            return Err(GroupError::UnknownFeature {
                group: group_id,
                feature: feature_id,
            });
        }
        let linked_row = suite
            .manifest_rows
            .iter()
            .find(|row| row.runtime_group_id.as_deref() == Some(group_id.as_str()));
        match (contract.origin, linked_row) {
            (RowOrigin::Inherited, Some(row)) if row.performance_feature == feature_id => {
                inherited_links.insert(group_id.clone());
            }
            (RowOrigin::Planned, None) => {}
            _ => return Err(GroupError::InvalidOrigin(group_id)),
        }
        validate_public_api_ownership(contract, references, &mut public_api_owners)?;
        validate_checklist_ownership(
            contract,
            references,
            &mut checklist_item_owners,
            &mut checklist_child_owners,
        )?;
    }
    let expected_inherited_links = suite
        .manifest_rows
        .iter()
        .filter_map(|row| row.runtime_group_id.clone())
        .collect::<BTreeSet<_>>();
    if inherited_links != expected_inherited_links
        || expected_inherited_links
            .iter()
            .any(|group| !runtime_group_ids.contains(group))
    {
        return Err(GroupError::InheritedCoverage);
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
            if contract.claim_class != ClaimClass::PromotablePerformance
                || contract.feature_id.to_string() != row.performance_feature
                || !contract.measurement_ids.iter().any(|measurement| {
                    measurement.to_string() == replacement.runtime_measurement_id
                })
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

fn validate_public_api_ownership(
    contract: &GroupContract,
    references: &super::super::discovery::SourceReferences,
    owners: &mut BTreeSet<String>,
) -> Result<(), GroupError> {
    let group_id = contract.id.to_string();
    let feature_id = contract.feature_id.to_string();
    if !contract
        .public_api_item_ids
        .windows(2)
        .all(|pair| matches!(pair, [left, right] if left < right))
    {
        return Err(GroupError::InvalidPublicApiOwnership(group_id));
    }
    for item_id in &contract.public_api_item_ids {
        let Some(item) = references.public_api.get(item_id) else {
            return Err(GroupError::UnknownPublicApi {
                group: group_id,
                item: item_id.clone(),
            });
        };
        if !owners.insert(item_id.clone())
            || !item.performance_groups.contains(&feature_id)
            || !contract.correctness_case_ids.contains(&item.owner_case_id)
        {
            return Err(GroupError::InvalidPublicApiOwnership(group_id));
        }
    }
    Ok(())
}

fn validate_checklist_ownership(
    contract: &GroupContract,
    references: &super::super::discovery::SourceReferences,
    item_owners: &mut BTreeSet<String>,
    child_owners: &mut BTreeSet<String>,
) -> Result<(), GroupError> {
    let group_id = contract.id.to_string();
    let feature_id = contract.feature_id.to_string();
    if !contract
        .checklist_item_ids
        .windows(2)
        .all(|pair| matches!(pair, [left, right] if left < right))
        || !contract
            .checklist_child_ids
            .windows(2)
            .all(|pair| matches!(pair, [left, right] if left < right))
    {
        return Err(GroupError::InvalidChecklistOwnership(group_id));
    }
    for item_id in &contract.checklist_item_ids {
        let Some(item) = references.checklist_items.get(item_id) else {
            return Err(GroupError::UnknownChecklistItem {
                group: group_id,
                item: item_id.clone(),
            });
        };
        if !item_owners.insert(item_id.clone()) || !item.performance_features.contains(&feature_id)
        {
            return Err(GroupError::InvalidChecklistOwnership(group_id));
        }
    }
    for child_id in &contract.checklist_child_ids {
        let Some(child) = references.checklist_children.get(child_id) else {
            return Err(GroupError::UnknownChecklistChild {
                group: group_id,
                child: child_id.clone(),
            });
        };
        let item_owns_child = references
            .checklist_items
            .get(&child.item_id)
            .is_some_and(|item| item.selected_child_ids.contains(child_id));
        if !child_owners.insert(child_id.clone())
            || !contract.checklist_item_ids.contains(&child.item_id)
            || !item_owns_child
            || !child.performance_features.contains(&feature_id)
        {
            return Err(GroupError::InvalidChecklistOwnership(group_id));
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
    if !Sha256Digest::is_valid_str(expected_inventory_sha256)
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
    if file.product_diagnostic_suite_timeout_seconds.get()
        > MAX_PRODUCT_DIAGNOSTIC_SUITE_TIMEOUT_SECONDS
    {
        return Err(GroupError::InvalidProductDiagnosticSuiteTimeout(
            file.product_diagnostic_suite_timeout_seconds.get(),
        ));
    }
    let release_groups = file
        .groups
        .iter()
        .filter(|group| group.claim_class == ClaimClass::PromotablePerformance)
        .count();
    let diagnostic_groups = file
        .groups
        .iter()
        .filter(|group| group.claim_class != ClaimClass::PromotablePerformance)
        .count();
    if release_groups > MAX_RELEASE_GROUPS || diagnostic_groups > MAX_DIAGNOSTIC_GROUPS {
        return Err(GroupError::MatrixCap {
            release: release_groups,
            release_max: MAX_RELEASE_GROUPS,
            diagnostic: diagnostic_groups,
            diagnostic_max: MAX_DIAGNOSTIC_GROUPS,
        });
    }
    let mut policy_group_ids = BTreeSet::new();
    for policy in &file.product_diagnostic_policies {
        let Some(group) = file.groups.iter().find(|group| group.id == policy.group_id) else {
            return Err(GroupError::InvalidProductDiagnosticPolicy(
                policy.group_id.to_string(),
            ));
        };
        let group_scale_ids = group
            .scales
            .iter()
            .map(|scale| &scale.id)
            .collect::<Vec<_>>();
        let policy_scale_ids = policy
            .scales
            .iter()
            .map(|scale| &scale.scale_id)
            .collect::<Vec<_>>();
        let capped_scale_count = policy
            .scales
            .iter()
            .filter(|scale| scale.max_worker_peak_rss_bytes.is_some())
            .count();
        if !policy_group_ids.insert(&policy.group_id)
            || group.claim_class != ClaimClass::ProductDiagnostic
            || policy.scales.is_empty()
            || group_scale_ids != policy_scale_ids
            || capped_scale_count > 1
            || policy.scales.iter().any(|scale| {
                !valid_case_id(&scale.witness_case_id)
                    || !group.correctness_case_ids.contains(&scale.witness_case_id)
            })
        {
            return Err(GroupError::InvalidProductDiagnosticPolicy(
                policy.group_id.to_string(),
            ));
        }
    }
    let diagnostic_group_ids = file
        .groups
        .iter()
        .filter(|group| group.claim_class == ClaimClass::ProductDiagnostic)
        .map(|group| &group.id)
        .collect::<BTreeSet<_>>();
    if policy_group_ids != diagnostic_group_ids {
        return Err(GroupError::ProductDiagnosticPolicyCoverage);
    }
    let mut group_ids = BTreeSet::new();
    for group in &file.groups {
        if !group_ids.insert(group.id.clone())
            || group.measurement_ids.is_empty()
            || group.measurement_ids.len() > MAX_MEASUREMENTS_PER_GROUP
            || group.scales.is_empty()
            || group.scales.len() > MAX_SCALES_PER_GROUP
            || group.correctness_case_ids.len() > MAX_CORRECTNESS_CASES_PER_GROUP
            || group.public_api_item_ids.len() > MAX_PUBLIC_API_ITEMS_PER_GROUP
            || group.checklist_item_ids.len() > MAX_CHECKLIST_ITEMS_PER_GROUP
            || group.checklist_child_ids.len() > MAX_CHECKLIST_CHILDREN_PER_GROUP
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
            (ClaimClass::ProductDiagnostic, ParityEligibility::ReportOnly)
                if !group.correctness_case_ids.is_empty()
                    && group.comparator_sources.is_empty()
                    && group.profiler_note.is_none() => {}
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
        "runtime group contract product-diagnostic suite timeout {0} seconds exceeds the 3600-second safety maximum"
    )]
    InvalidProductDiagnosticSuiteTimeout(u64),
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
    #[error("runtime product-diagnostic policy is invalid for group {0}")]
    InvalidProductDiagnosticPolicy(String),
    #[error("runtime product-diagnostic policies do not exactly cover product diagnostic groups")]
    ProductDiagnosticPolicyCoverage,
    #[error("runtime group contract does not define group {0}")]
    UnknownGroup(String),
    #[error("runtime group contract group {group} does not define scale {scale}")]
    UnknownScale { group: String, scale: String },
    #[error("runtime product-diagnostic policy for group {group} does not define scale {scale}")]
    UnknownDiagnosticPolicyScale { group: String, scale: String },
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
    #[error("runtime group {group} references unknown performance feature {feature}")]
    UnknownFeature { group: String, feature: String },
    #[error("runtime group {0} has an invalid inherited/planned origin link")]
    InvalidOrigin(String),
    #[error("runtime inherited groups do not exactly cover compact manifest parent links")]
    InheritedCoverage,
    #[error(
        "runtime prerequisites do not exactly match the correctness bridge: bridge-only={bridge_only:?}, runtime-only={runtime_only:?}"
    )]
    CorrectnessPrerequisiteCoverage {
        bridge_only: Vec<String>,
        runtime_only: Vec<String>,
    },
    #[error("runtime group {group} references unknown public API item {item}")]
    UnknownPublicApi { group: String, item: String },
    #[error("runtime group {0} has invalid or duplicate public API ownership")]
    InvalidPublicApiOwnership(String),
    #[error("runtime group {group} references unknown checklist item {item}")]
    UnknownChecklistItem { group: String, item: String },
    #[error("runtime group {group} references unknown checklist child {child}")]
    UnknownChecklistChild { group: String, child: String },
    #[error("runtime group {0} has invalid or duplicate checklist ownership")]
    InvalidChecklistOwnership(String),
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
