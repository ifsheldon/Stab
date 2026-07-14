use std::collections::BTreeSet;

use super::model::{
    Comparator, EvidenceCase, EvidenceProvenance, EvidenceState, EvidenceStatus,
    PropertyExecutionMode, PropertyPersistencePolicy, PropertyPlanSource, SelectorKind,
};
use super::validation::ValidationIssues;

pub(super) fn validate(case: &EvidenceCase, violations: &mut ValidationIssues) {
    let Some(reference) = &case.property_plan else {
        if case.comparator == Comparator::Property {
            violations.push(format!("property case {:?} has no property plan", case.id));
        }
        return;
    };
    if case.comparator != Comparator::Property {
        violations.push(format!(
            "non-property case {:?} unexpectedly has a property plan",
            case.id
        ));
    }
    let expected_state = match case.status {
        EvidenceStatus::Planned => EvidenceState::Planned,
        EvidenceStatus::Implemented | EvidenceStatus::EvidenceClose => EvidenceState::Existing,
        EvidenceStatus::Deferred => EvidenceState::NotApplicable,
    };
    if reference.state != expected_state {
        violations.push(format!(
            "property case {:?} plan state disagrees with case status",
            case.id
        ));
    }
    if reference.id.is_empty() || reference.id.len() > 128 {
        violations.push(format!(
            "property case {:?} has an invalid plan id",
            case.id
        ));
    }
    match (reference.state, reference.source, case.provenance) {
        (EvidenceState::Planned, PropertyPlanSource::QualificationCase, _)
            if reference.id == case.id.as_str() => {}
        (
            EvidenceState::Existing,
            PropertyPlanSource::OracleFixture,
            EvidenceProvenance::OracleFixture,
        ) if reference.id == case.source_id => {}
        (
            EvidenceState::Existing,
            PropertyPlanSource::QualificationCase,
            EvidenceProvenance::QualificationPlan,
        ) if reference.id == case.source_id
            && matches!(
                case.primary_selector.kind,
                SelectorKind::PropertyTarget | SelectorKind::CargoTest
            ) => {}
        _ => violations.push(format!(
            "property case {:?} plan source or id is stale",
            case.id
        )),
    }
    let Some(plan) = &reference.plan else {
        if reference.state == EvidenceState::Existing {
            violations.push(format!(
                "implemented property case {:?} has no executable plan data",
                case.id
            ));
        }
        return;
    };
    if reference.state != EvidenceState::Existing {
        violations.push(format!(
            "non-executable property case {:?} contains executable plan data",
            case.id
        ));
    }
    if plan.generator_domain.is_empty() || plan.generator_domain.len() > 2_048 {
        violations.push(format!(
            "property case {:?} has an invalid generator domain",
            case.id
        ));
    }
    if plan.case_count == 0 || plan.case_count > 1_000_000 {
        violations.push(format!(
            "property case {:?} case count is outside 1..=1000000",
            case.id
        ));
    }
    if plan.maximum_generated_bytes > super::property::MAX_GENERATED_CASE_BYTES {
        violations.push(format!(
            "property case {:?} generated-byte limit exceeds the CQ1 bound",
            case.id
        ));
    }
    if plan.seeds.iter().collect::<BTreeSet<_>>().len() != plan.seeds.len() {
        violations.push(format!("property case {:?} repeats a seed", case.id));
    }
    let static_corpus = plan.seeds.is_empty();
    if static_corpus
        && (plan.maximum_generated_bytes != 0
            || plan.corpus_path.is_none()
            || plan.corpus_sha256.is_none()
            || plan.persistence_policy != PropertyPersistencePolicy::ExistingFocusedRegression
            || plan.execution_mode != PropertyExecutionMode::CargoSubprocess)
    {
        violations.push(format!(
            "property case {:?} static corpus plan is incomplete",
            case.id
        ));
    }
    if !static_corpus
        && (plan.maximum_generated_bytes == 0
            || plan.persistence_policy != PropertyPersistencePolicy::PersistMinimizedRegression
            || plan.execution_mode != PropertyExecutionMode::QualificationWorkerSubprocess)
    {
        violations.push(format!(
            "property case {:?} generated plan lacks worker or persistence ownership",
            case.id
        ));
    }
    if case.primary_selector.kind == SelectorKind::PropertyTarget
        && plan.execution_mode != PropertyExecutionMode::QualificationWorkerSubprocess
    {
        violations.push(format!(
            "property target {:?} is not assigned to a killable worker",
            case.id
        ));
    }
    if case.primary_selector.kind == SelectorKind::PropertyTarget
        && (!super::property::is_registered_target(&reference.id)
            || !super::property::registered_execution_plan_matches(&reference.id, plan))
    {
        violations.push(format!(
            "property target {:?} manifest plan disagrees with its registered worker contract",
            case.id
        ));
    }
}
