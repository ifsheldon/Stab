use std::collections::BTreeSet;

use super::super::discovery::PERFORMANCE_FEATURE_IDS;
use super::super::model::{PerformanceDisposition, QualificationSuite};
use super::issues::Issues;
use super::values::{validate_identifier, validate_text};

pub(super) fn validate(suite: &QualificationSuite, issues: &mut Issues) {
    let expected = PERFORMANCE_FEATURE_IDS.into_iter().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let group_ids = suite
        .qualification_groups
        .iter()
        .map(|group| group.id.as_str())
        .collect::<BTreeSet<_>>();
    for feature in &suite.performance_features {
        validate_identifier("performance feature", &feature.id, issues);
        validate_text("feature reason", &feature.reason, issues);
        if !seen.insert(feature.id.as_str()) {
            issues.push(format!("duplicate performance feature {}", feature.id));
        }
        if !expected.contains(feature.id.as_str()) {
            issues.push(format!("unknown performance feature {}", feature.id));
        }
        let mut local_groups = BTreeSet::new();
        for group in &feature.group_ids {
            if !local_groups.insert(group) {
                issues.push(format!("feature {} repeats group {group}", feature.id));
            }
            if !group_ids.contains(group.as_str()) {
                issues.push(format!(
                    "feature {} references unknown group {group}",
                    feature.id
                ));
            }
        }
        if feature.disposition == PerformanceDisposition::Measured && feature.group_ids.is_empty() {
            issues.push(format!(
                "measured feature {} has no measured groups",
                feature.id
            ));
        }
        if feature.disposition == PerformanceDisposition::FutureCandidate
            && !feature.group_ids.is_empty()
        {
            issues.push(format!(
                "future-candidate feature {} has active measured groups",
                feature.id
            ));
        }
    }
    for missing in expected.difference(&seen) {
        issues.push(format!("missing performance feature {missing}"));
    }
}
