use std::collections::BTreeSet;

use super::model::{EvidenceCase, EvidenceStatus, ExecutionTier, FeatureId};

pub(super) type QualificationTier = ExecutionTier;

pub(super) struct CaseSelection<'a> {
    pub(super) selected: Vec<&'a EvidenceCase>,
    pub(super) planned: Vec<&'a EvidenceCase>,
    pub(super) deferred: Vec<&'a EvidenceCase>,
    pub(super) out_of_tier: Vec<&'a EvidenceCase>,
}

pub(super) fn select_cases<'a>(
    cases: &'a [EvidenceCase],
    tier: QualificationTier,
    features: &BTreeSet<FeatureId>,
    case_ids: &BTreeSet<&str>,
) -> CaseSelection<'a> {
    let matches_filter = |case: &&EvidenceCase| {
        (features.is_empty() || features.contains(&case.feature_id))
            && (case_ids.is_empty() || case_ids.contains(case.id.as_str()))
    };
    let in_scope = cases.iter().filter(matches_filter).collect::<Vec<_>>();
    let planned = in_scope
        .iter()
        .copied()
        .filter(|case| case.status == EvidenceStatus::Planned)
        .collect();
    let deferred = in_scope
        .iter()
        .copied()
        .filter(|case| case.status == EvidenceStatus::Deferred)
        .collect();
    let executable = in_scope
        .into_iter()
        .filter(|case| {
            matches!(
                case.status,
                EvidenceStatus::Implemented | EvidenceStatus::EvidenceClose
            )
        })
        .collect::<Vec<_>>();
    let (selected, out_of_tier) = executable
        .into_iter()
        .partition(|case| case.execution.tiers.contains(&tier));
    CaseSelection {
        selected,
        planned,
        deferred,
        out_of_tier,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{QualificationTier, select_cases};
    use crate::qualification::model::{
        BehavioralSurface, CaseId, Comparator, EvidenceCase, EvidenceProvenance, EvidenceSelector,
        EvidenceState, EvidenceStatus, ExecutionContract, ExecutionTier, ExpectedSkip, FeatureId,
        ResourceContract, ResourceKind, SelectorKind,
    };

    fn case(id: &str, comparator: Comparator, status: EvidenceStatus) -> EvidenceCase {
        EvidenceCase {
            id: CaseId::try_new(id.to_string()).expect("case id"),
            feature_id: FeatureId::StimFormat,
            behavioral_surface: BehavioralSurface::Engine,
            provenance: EvidenceProvenance::QualificationPlan,
            source_id: id.to_string(),
            comparator,
            execution: ExecutionContract {
                tiers: vec![ExecutionTier::Pr, ExecutionTier::Full, ExecutionTier::Soak],
                timeout_ms: 1_000,
                stdout_limit_bytes: 1_024,
                stderr_limit_bytes: 1_024,
                artifact_limit_bytes: 2_048,
                expected_skip: ExpectedSkip::Never,
            },
            statistical_plan: None,
            property_plan: None,
            primary_selector: EvidenceSelector {
                state: EvidenceState::Existing,
                kind: SelectorKind::CargoTest,
                value: vec!["selector".to_string()],
            },
            supporting_selectors: Vec::new(),
            resource_contract: ResourceContract {
                kind: ResourceKind::NotApplicable,
                detail: "not applicable".to_string(),
            },
            negative_axes: Vec::new(),
            performance_groups: Vec::new(),
            deferred_product: None,
            status,
        }
    }

    #[test]
    fn pr_uses_source_owned_tier_membership() {
        let cases = vec![
            case(
                "exact-a",
                Comparator::ExactBytes,
                EvidenceStatus::Implemented,
            ),
            case(
                "exact-b",
                Comparator::ExactBytes,
                EvidenceStatus::Implemented,
            ),
            case(
                "structural-a",
                Comparator::Structural,
                EvidenceStatus::Implemented,
            ),
            case(
                "structural-b",
                Comparator::Structural,
                EvidenceStatus::Implemented,
            ),
        ];

        let selection = select_cases(
            &cases,
            QualificationTier::Pr,
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        let ids = selection
            .selected
            .iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec!["exact-a", "exact-b", "structural-a", "structural-b"]
        );
    }

    #[test]
    fn full_separates_planned_and_deferred_from_executable_cases() {
        let cases = vec![
            case(
                "implemented",
                Comparator::Structural,
                EvidenceStatus::Implemented,
            ),
            case("planned", Comparator::Structural, EvidenceStatus::Planned),
            case("deferred", Comparator::Structural, EvidenceStatus::Deferred),
        ];

        let selection = select_cases(
            &cases,
            QualificationTier::Full,
            &BTreeSet::new(),
            &BTreeSet::new(),
        );

        assert_eq!(selection.selected.len(), 1);
        assert_eq!(selection.planned.len(), 1);
        assert_eq!(selection.deferred.len(), 1);
        assert!(selection.out_of_tier.is_empty());
    }

    #[test]
    fn selection_retains_executable_cases_outside_the_requested_tier() {
        let mut full_only = case(
            "full-only",
            Comparator::Structural,
            EvidenceStatus::Implemented,
        );
        full_only.execution.tiers = vec![ExecutionTier::Full, ExecutionTier::Soak];

        let cases = [full_only];
        let selection = select_cases(
            &cases,
            QualificationTier::Pr,
            &BTreeSet::new(),
            &BTreeSet::from(["full-only"]),
        );

        assert!(selection.selected.is_empty());
        assert_eq!(selection.out_of_tier.len(), 1);
    }
}
