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
