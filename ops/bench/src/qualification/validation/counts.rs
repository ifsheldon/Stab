use super::Issues;
use crate::qualification::model::{
    PerformanceDisposition, QualificationSuite, RowClassification, RowDecision,
};

pub(super) fn validate_parent_disposition(
    label: &str,
    id: &str,
    disposition: PerformanceDisposition,
    parents: &[String],
    issues: &mut Issues,
) {
    match disposition {
        PerformanceDisposition::CoveredByParent if parents.is_empty() => {
            issues.push(format!(
                "{label} {id} is covered-by-parent without a parent"
            ));
        }
        PerformanceDisposition::NotPerformanceRelevant if !parents.is_empty() => {
            issues.push(format!("{label} {id} is non-performance but has parents"));
        }
        PerformanceDisposition::FutureCandidate if !parents.is_empty() => {
            issues.push(format!(
                "{label} {id} is a future candidate but has active parents"
            ));
        }
        PerformanceDisposition::Measured | PerformanceDisposition::NoFaithfulStimComparator => {
            issues.push(format!(
                "{label} {id} has invalid PQ0 disposition {disposition:?}"
            ));
        }
        _ => {}
    }
}

pub(super) fn validate_decision_count(
    suite: &QualificationSuite,
    value: RowDecision,
    expected: usize,
    issues: &mut Issues,
) {
    let actual = suite
        .manifest_rows
        .iter()
        .filter(|row| row.decision == value)
        .count();
    if actual != expected {
        issues.push(format!(
            "decision {value:?} has {actual} rows, expected {expected}"
        ));
    }
}

pub(super) fn validate_classification_count(
    suite: &QualificationSuite,
    value: RowClassification,
    expected: usize,
    issues: &mut Issues,
) {
    let actual = suite
        .manifest_rows
        .iter()
        .filter(|row| row.classifications.contains(&value))
        .count();
    if actual != expected {
        issues.push(format!(
            "classification {value:?} has {actual} rows, expected {expected}"
        ));
    }
}
