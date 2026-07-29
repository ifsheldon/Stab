use super::super::model::QualificationSuite;
use super::issues::Issues;

const CANONICAL_GROUP_OWNERS: [(&str, &str); 1] = [(
    "PERFQ-A2-CIRCUIT-MODEL-FINGERPRINT",
    "stab-model/model-fingerprint",
)];

pub(super) fn validate(suite: &QualificationSuite, issues: &mut Issues) {
    for (group_id, expected_owner) in CANONICAL_GROUP_OWNERS {
        let Some(group) = suite
            .qualification_groups
            .iter()
            .find(|group| group.id == group_id)
        else {
            continue;
        };
        if group.owner != expected_owner {
            issues.push(format!(
                "qualification group {group_id} has owner {}, expected canonical owner {expected_owner}",
                group.owner
            ));
        }
    }
}
