use std::num::NonZeroU64;

use super::{
    BaselineEligibility, ComparatorSourceContract, ComparatorSourcePath, GroupContract,
    ScaleContract, comparators,
};
use crate::qualification::runtime::protocol::{InputDigest, ProtocolId, Sha256Digest};
use crate::qualification::runtime::run::ClaimClass;

pub(super) fn pauli_contract() -> GroupContract {
    GroupContract {
        id: ProtocolId::try_new(super::super::invocation::PAULI_STRING_MULTIPLY_GROUP_ID)
            .expect("group id"),
        claim_class: ClaimClass::PromotablePerformance,
        baseline_eligibility: BaselineEligibility::ThresholdEligible,
        timing_batch_policy: crate::qualification::model::TimingBatchPolicy::CommonIterations,
        workload_id: ProtocolId::try_new("pauli-string-right-multiply").expect("workload id"),
        measurement_ids: vec![
            ProtocolId::try_new("right-multiply-in-place").expect("measurement id"),
        ],
        scales: [("small", 10_000), ("medium", 100_000), ("large", 1_000_000)]
            .into_iter()
            .map(|(id, work_items)| ScaleContract {
                id: ProtocolId::try_new(id).expect("scale id"),
                work_items: NonZeroU64::new(work_items).expect("positive work"),
                input_bytes: 8,
                input_digest: InputDigest::try_new("d".repeat(64)).expect("input digest"),
            })
            .collect(),
        correctness_case_ids: vec![
            "cq-evidence-qualification-3bab0f51237445f6".to_string(),
            "cq-evidence-qualification-489e6445120743c2".to_string(),
        ],
        owner: ProtocolId::try_new("stab-core/stabilizers").expect("owner"),
        profiler_note: None,
        comparator_sources: comparators::PAULI_STRING_MULTIPLY
            .iter()
            .map(|path| ComparatorSourceContract {
                path: ComparatorSourcePath::try_new((*path).to_string()).expect("comparator path"),
                sha256: Sha256Digest::try_new("d".repeat(64)).expect("comparator digest"),
            })
            .collect(),
    }
}

pub(super) fn pauli_iter_contract(
    group_id: &str,
    workload_id: &str,
    work_items: u64,
) -> GroupContract {
    GroupContract {
        id: ProtocolId::try_new(group_id).expect("group id"),
        claim_class: ClaimClass::PromotablePerformance,
        baseline_eligibility: BaselineEligibility::ThresholdEligible,
        timing_batch_policy: crate::qualification::model::TimingBatchPolicy::CommonIterations,
        workload_id: ProtocolId::try_new(workload_id).expect("workload id"),
        measurement_ids: vec![
            ProtocolId::try_new("construct-and-iterate-borrowed").expect("measurement id"),
        ],
        scales: vec![ScaleContract {
            id: ProtocolId::try_new("small").expect("scale id"),
            work_items: NonZeroU64::new(work_items).expect("positive work"),
            input_bytes: 64,
            input_digest: InputDigest::try_new("e".repeat(64)).expect("input digest"),
        }],
        correctness_case_ids: vec!["cq-evidence-pauli-iterator".to_string()],
        owner: ProtocolId::try_new("stab-core/stabilizers").expect("owner"),
        profiler_note: None,
        comparator_sources: comparators::PAULI_STRING_ITER
            .iter()
            .map(|path| ComparatorSourceContract {
                path: ComparatorSourcePath::try_new((*path).to_string()).expect("comparator path"),
                sha256: Sha256Digest::try_new("e".repeat(64)).expect("comparator digest"),
            })
            .collect(),
    }
}

pub(super) fn clifford_contract(
    group_id: &str,
    workload_id: &str,
    measurement_id: &str,
) -> GroupContract {
    GroupContract {
        id: ProtocolId::try_new(group_id).expect("group id"),
        claim_class: ClaimClass::PromotablePerformance,
        baseline_eligibility: BaselineEligibility::ThresholdEligible,
        timing_batch_policy: if group_id == super::super::invocation::CLIFFORD_IDENTITY_GROUP_ID {
            crate::qualification::model::TimingBatchPolicy::IndependentThroughput
        } else {
            crate::qualification::model::TimingBatchPolicy::CommonIterations
        },
        workload_id: ProtocolId::try_new(workload_id).expect("workload id"),
        measurement_ids: vec![ProtocolId::try_new(measurement_id).expect("measurement id")],
        scales: vec![ScaleContract {
            id: ProtocolId::try_new("small").expect("scale id"),
            work_items: NonZeroU64::new(10_000).expect("positive work"),
            input_bytes: 64,
            input_digest: InputDigest::try_new("f".repeat(64)).expect("input digest"),
        }],
        correctness_case_ids: vec![
            "cq-evidence-qualification-40e5ad2f2f4c4fd4".to_string(),
            "cq-evidence-qualification-510e746ec36e7d1c".to_string(),
            "cq-evidence-qualification-ae9390dd6a207cb6".to_string(),
        ],
        owner: ProtocolId::try_new("stab-core/stabilizers").expect("owner"),
        profiler_note: None,
        comparator_sources: comparators::CLIFFORD_STRING
            .iter()
            .map(|path| ComparatorSourceContract {
                path: ComparatorSourcePath::try_new((*path).to_string()).expect("comparator path"),
                sha256: Sha256Digest::try_new("f".repeat(64)).expect("comparator digest"),
            })
            .collect(),
    }
}

pub(super) fn dem_contracts() -> [GroupContract; 2] {
    [
        dem_contract(
            super::super::invocation::DEM_PARSE_GROUP_ID,
            "dem-parse",
            "parse",
        ),
        dem_contract(
            super::super::invocation::DEM_CANONICAL_PRINT_GROUP_ID,
            "dem-canonical-print",
            "serialize",
        ),
    ]
}

fn dem_contract(group_id: &str, workload_id: &str, measurement_id: &str) -> GroupContract {
    GroupContract {
        id: ProtocolId::try_new(group_id).expect("group id"),
        claim_class: ClaimClass::PromotablePerformance,
        baseline_eligibility: BaselineEligibility::ThresholdEligible,
        timing_batch_policy: crate::qualification::model::TimingBatchPolicy::CommonIterations,
        workload_id: ProtocolId::try_new(workload_id).expect("workload id"),
        measurement_ids: vec![ProtocolId::try_new(measurement_id).expect("measurement id")],
        scales: vec![ScaleContract {
            id: ProtocolId::try_new("small").expect("scale id"),
            work_items: NonZeroU64::new(64).expect("positive work"),
            input_bytes: 1_776,
            input_digest: InputDigest::try_new("a".repeat(64)).expect("input digest"),
        }],
        correctness_case_ids: vec!["cq-evidence-qualification-0908c21b917526e3".to_string()],
        owner: ProtocolId::try_new("stab-core/dem-model").expect("owner"),
        profiler_note: None,
        comparator_sources: comparators::DEM_MODEL
            .iter()
            .map(|path| ComparatorSourceContract {
                path: ComparatorSourcePath::try_new((*path).to_string()).expect("comparator path"),
                sha256: Sha256Digest::try_new("a".repeat(64)).expect("comparator digest"),
            })
            .collect(),
    }
}

#[test]
fn runtime_contract_requires_an_explicit_timing_policy() {
    let value = serde_json::json!({
        "id": "group",
        "claim_class": "promotable-performance",
        "baseline_eligibility": "threshold-eligible",
        "workload_id": "workload",
        "measurement_ids": ["main"],
        "scales": [{
            "id": "small",
            "work_items": 1,
            "input_bytes": 64,
            "input_digest": "a".repeat(64)
        }],
        "correctness_case_ids": ["case"],
        "owner": "owner",
        "profiler_note": null,
        "comparator_sources": []
    });
    assert!(serde_json::from_value::<GroupContract>(value).is_err());
}

#[test]
fn runtime_contract_binds_clifford_identity_timing_policy() {
    use crate::qualification::model::TimingBatchPolicy;

    let root = crate::root::RepoRoot::resolve(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
    )
    .expect("repository root");
    let manifest = crate::manifest::BenchmarkManifest::read(&root).expect("manifest");
    let mut suite = super::super::super::discovery::generate(&root, &manifest)
        .expect("generated performance inventory");
    let (mut file, _) = super::load(&root, &suite.semantic_digest).expect("runtime contract");
    super::validate_inventory_contracts(&file, &suite).expect("matching ledgers");

    let identity_group = suite
        .qualification_groups
        .iter_mut()
        .find(|group| group.id == super::super::invocation::CLIFFORD_IDENTITY_GROUP_ID)
        .expect("Clifford identity group");
    assert_eq!(
        identity_group.timing_policy.batch_policy,
        TimingBatchPolicy::IndependentThroughput
    );
    identity_group.timing_policy.batch_policy = TimingBatchPolicy::CommonIterations;
    assert!(matches!(
        super::validate_inventory_contracts(&file, &suite),
        Err(super::GroupError::InventoryContract(group))
            if group == super::super::invocation::CLIFFORD_IDENTITY_GROUP_ID
    ));

    let suite = super::super::super::discovery::generate(&root, &manifest)
        .expect("generated performance inventory");
    file.groups
        .iter_mut()
        .find(|group| group.id.to_string() == super::super::invocation::CLIFFORD_IDENTITY_GROUP_ID)
        .expect("runtime Clifford identity group")
        .timing_batch_policy = TimingBatchPolicy::CommonIterations;
    assert!(matches!(
        super::validate_inventory_contracts(&file, &suite),
        Err(super::GroupError::InventoryContract(group))
            if group == super::super::invocation::CLIFFORD_IDENTITY_GROUP_ID
    ));
}

#[test]
fn runtime_contract_enforces_release_and_diagnostic_caps() {
    let root = crate::root::RepoRoot::resolve(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
    )
    .expect("repository root");
    let bytes =
        std::fs::read(root.path.join(super::GROUP_CONTRACT_PATH)).expect("runtime group contract");
    let source: super::GroupContractFile =
        serde_json::from_slice(&bytes).expect("parse runtime group contract");

    for (claim_class, count) in [
        (
            ClaimClass::PromotablePerformance,
            super::MAX_RELEASE_GROUPS + 1,
        ),
        (
            ClaimClass::DiagnosticInfrastructure,
            super::MAX_DIAGNOSTIC_GROUPS + 1,
        ),
    ] {
        let mut file: super::GroupContractFile =
            serde_json::from_slice(&bytes).expect("fresh runtime group contract");
        let template = source
            .groups
            .iter()
            .find(|group| group.claim_class == claim_class)
            .expect("claim-class template");
        file.groups.retain(|group| group.claim_class != claim_class);
        for index in 0..count {
            let mut group = template.clone();
            group.id = ProtocolId::try_new(
                format!("cap-probe-{claim_class:?}-{index}").to_ascii_lowercase(),
            )
            .expect("unique group id");
            file.groups.push(group);
        }

        assert!(matches!(
            super::validate(&file, &file.performance_inventory_sha256),
            Err(super::GroupError::MatrixCap { .. })
        ));
    }
}
