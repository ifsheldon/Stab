use super::model::{
    Comparator, EvidenceCase, EvidenceProvenance, EvidenceState, EvidenceStatus,
    StatisticalPlanSource,
};
use super::validation::{ValidationIssues, validate_text};

pub(super) fn validate(case: &EvidenceCase, violations: &mut ValidationIssues) {
    match (case.comparator, &case.statistical_plan) {
        (Comparator::Statistical, Some(plan)) => {
            validate_text("statistical plan id", &plan.id, violations);
            let expected_state = if case.status == EvidenceStatus::Planned {
                EvidenceState::Planned
            } else {
                EvidenceState::Existing
            };
            if plan.state != expected_state {
                violations.push(format!(
                    "statistical evidence case {:?} has plan state {:?}, expected {:?}",
                    case.id, plan.state, expected_state
                ));
            }
            match (case.provenance, plan.source, plan.state) {
                (
                    EvidenceProvenance::OracleFixture,
                    StatisticalPlanSource::OracleFixture,
                    EvidenceState::Existing,
                )
                | (
                    EvidenceProvenance::BlockerLedger,
                    StatisticalPlanSource::BlockerLedger,
                    EvidenceState::Existing,
                ) if plan.id == case.source_id => {}
                (_, StatisticalPlanSource::QualificationCase, EvidenceState::Planned)
                    if plan.id == case.id.as_str() => {}
                _ => violations.push(format!(
                    "statistical evidence case {:?} plan source or id does not match its provenance",
                    case.id
                )),
            }
        }
        (Comparator::Statistical, None) => violations.push(format!(
            "statistical evidence case {:?} has no statistical plan owner",
            case.id
        )),
        (_, Some(_)) => violations.push(format!(
            "non-statistical evidence case {:?} has a statistical plan",
            case.id
        )),
        (_, None) => {}
    }
}
