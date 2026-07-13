use std::collections::BTreeSet;

use super::model::{
    Comparator, EvidenceCase, EvidenceStatus, ExecutionContract, ExecutionTier, ExpectedSkip,
};
use super::validation::ValidationIssues;

const FAILURE_REASON_ARTIFACT_BYTES: usize = 4 << 10;

pub(super) fn for_status(status: EvidenceStatus) -> ExecutionContract {
    let tiers = match status {
        EvidenceStatus::Implemented | EvidenceStatus::EvidenceClose | EvidenceStatus::Planned => {
            vec![ExecutionTier::Full, ExecutionTier::Soak]
        }
        EvidenceStatus::Deferred => Vec::new(),
    };
    ExecutionContract {
        tiers,
        timeout_ms: 120_000,
        stdout_limit_bytes: crate::process::OUTPUT_LIMIT_BYTES,
        stderr_limit_bytes: crate::process::OUTPUT_LIMIT_BYTES,
        artifact_limit_bytes: 2 * crate::process::OUTPUT_LIMIT_BYTES
            + FAILURE_REASON_ARTIFACT_BYTES,
        expected_skip: ExpectedSkip::Never,
    }
}

pub(super) fn assign_pr_tiers(cases: &mut [EvidenceCase]) {
    let mut represented = BTreeSet::new();
    for case in cases.iter_mut().filter(|case| {
        matches!(
            case.status,
            EvidenceStatus::Implemented | EvidenceStatus::EvidenceClose
        )
    }) {
        let requires_every_case = matches!(
            case.comparator,
            Comparator::ExactBytes
                | Comparator::ExactValue
                | Comparator::ErrorClass
                | Comparator::Statistical
                | Comparator::Property
                | Comparator::Resource
        );
        if (requires_every_case || represented.insert((case.feature_id, case.comparator)))
            && !case.execution.tiers.contains(&ExecutionTier::Pr)
        {
            case.execution.tiers.insert(0, ExecutionTier::Pr);
        }
    }
}

pub(super) fn validate(case: &EvidenceCase, violations: &mut ValidationIssues) {
    let tiers = case
        .execution
        .tiers
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if tiers.len() != case.execution.tiers.len() {
        violations.push(format!(
            "evidence case {:?} repeats an execution tier",
            case.id
        ));
    }
    if !case
        .execution
        .tiers
        .windows(2)
        .all(|pair| matches!(pair, [left, right] if left < right))
    {
        violations.push(format!(
            "evidence case {:?} execution tiers are not in pr/full/soak order",
            case.id
        ));
    }
    let executable = matches!(
        case.status,
        EvidenceStatus::Implemented | EvidenceStatus::EvidenceClose
    );
    if case.status == EvidenceStatus::Deferred && !tiers.is_empty() {
        violations.push(format!(
            "deferred evidence case {:?} has executable tiers",
            case.id
        ));
    }
    if case.status != EvidenceStatus::Deferred
        && (!tiers.contains(&ExecutionTier::Full) || !tiers.contains(&ExecutionTier::Soak))
    {
        violations.push(format!(
            "evidence case {:?} lacks full or soak tier ownership",
            case.id
        ));
    }
    if !executable && tiers.contains(&ExecutionTier::Pr) {
        violations.push(format!(
            "non-executable evidence case {:?} enters the pr tier",
            case.id
        ));
    }
    if !(1..=3_600_000).contains(&case.execution.timeout_ms) {
        violations.push(format!(
            "evidence case {:?} timeout is outside 1..=3600000ms",
            case.id
        ));
    }
    for (label, limit) in [
        ("stdout", case.execution.stdout_limit_bytes),
        ("stderr", case.execution.stderr_limit_bytes),
    ] {
        if !(1..=crate::process::OUTPUT_LIMIT_BYTES).contains(&limit) {
            violations.push(format!(
                "evidence case {:?} {label} limit {limit} is outside the qualification bound",
                case.id
            ));
        }
    }
    if !(1..=2 * crate::process::OUTPUT_LIMIT_BYTES + FAILURE_REASON_ARTIFACT_BYTES)
        .contains(&case.execution.artifact_limit_bytes)
    {
        violations.push(format!(
            "evidence case {:?} artifact limit {} is outside the qualification bound",
            case.id, case.execution.artifact_limit_bytes
        ));
    }
    let minimum_artifact = case
        .execution
        .stdout_limit_bytes
        .checked_add(case.execution.stderr_limit_bytes)
        .and_then(|minimum| minimum.checked_add(FAILURE_REASON_ARTIFACT_BYTES));
    if minimum_artifact.is_none_or(|minimum| case.execution.artifact_limit_bytes < minimum) {
        violations.push(format!(
            "evidence case {:?} artifact limit cannot retain bounded stdout and stderr",
            case.id
        ));
    }
}
