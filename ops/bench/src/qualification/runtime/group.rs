use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::protocol::ProtocolId;
use super::run::ClaimClass;
use crate::root::RepoRoot;

const GROUP_CONTRACT_PATH: &str = "benchmarks/qualification-runtime-groups.json";
const GROUP_CONTRACT_SCHEMA_VERSION: u32 = 1;
const MAX_GROUP_CONTRACT_BYTES: usize = 1 << 20;
const MAX_GROUPS: usize = 256;
const MAX_MEASUREMENTS_PER_GROUP: usize = 64;
const MAX_CORRECTNESS_CASES_PER_GROUP: usize = 4096;

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
    pub(super) correctness_case_ids: Vec<String>,
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

pub(super) fn check(root: &RepoRoot, expected_inventory_sha256: &str) -> Result<(), GroupError> {
    load(root, expected_inventory_sha256).map(|_| ())
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
    Ok((file, super::run::sha256_hex(&bytes)))
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
            || group.correctness_case_ids.len() > MAX_CORRECTNESS_CASES_PER_GROUP
        {
            return Err(GroupError::InvalidGroup(group.id.to_string()));
        }
        let measurement_ids = group.measurement_ids.iter().collect::<BTreeSet<_>>();
        let correctness_case_ids = group.correctness_case_ids.iter().collect::<BTreeSet<_>>();
        if measurement_ids.len() != group.measurement_ids.len()
            || correctness_case_ids.len() != group.correctness_case_ids.len()
            || !group
                .correctness_case_ids
                .windows(2)
                .all(|pair| matches!(pair, [left, right] if left < right))
            || group
                .correctness_case_ids
                .iter()
                .any(|case| !valid_case_id(case))
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
    #[error("runtime group {0} does not match the implemented worker shape")]
    UnsupportedRuntimeShape(String),
    #[error("runtime group contract does not exactly match the executable group registry")]
    ExecutableRegistration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_groups_are_report_only_and_have_no_correctness_cases() {
        let valid = GroupContractFile {
            schema_version: GROUP_CONTRACT_SCHEMA_VERSION,
            performance_inventory_sha256: "a".repeat(64),
            groups: vec![GroupContract {
                id: ProtocolId::try_new(super::super::invocation::PQ1_GROUP_ID).expect("group id"),
                claim_class: ClaimClass::DiagnosticInfrastructure,
                baseline_eligibility: BaselineEligibility::ReportOnly,
                workload_id: ProtocolId::try_new("protocol-smoke").expect("workload id"),
                measurement_ids: vec![ProtocolId::try_new("main").expect("measurement id")],
                correctness_case_ids: Vec::new(),
            }],
        };
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
    fn source_contract_rejects_unregistered_groups() {
        let unsupported = GroupContractFile {
            schema_version: GROUP_CONTRACT_SCHEMA_VERSION,
            performance_inventory_sha256: "a".repeat(64),
            groups: vec![GroupContract {
                id: ProtocolId::try_new("unregistered").expect("group id"),
                claim_class: ClaimClass::DiagnosticInfrastructure,
                baseline_eligibility: BaselineEligibility::ReportOnly,
                workload_id: ProtocolId::try_new("protocol-smoke").expect("workload id"),
                measurement_ids: vec![ProtocolId::try_new("main").expect("measurement id")],
                correctness_case_ids: Vec::new(),
            }],
        };
        assert!(matches!(
            validate(&unsupported, &"a".repeat(64)),
            Err(GroupError::UnsupportedRuntimeShape(group)) if group == "unregistered"
        ));
    }
}
