use std::collections::BTreeSet;

use serde::Deserialize;

use crate::ReleaseError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Contract {
    pub(super) id: u64,
    pub(super) name: &'static str,
    pub(super) repository: &'static str,
    pub(super) ref_include: &'static str,
    pub(super) node_id: &'static str,
    pub(super) created_at: &'static str,
    pub(super) updated_at: &'static str,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct RemoteRuleset {
    id: u64,
    name: String,
    node_id: String,
    created_at: String,
    updated_at: String,
    target: String,
    source_type: String,
    source: String,
    enforcement: String,
    conditions: RemoteRulesetConditions,
    rules: Vec<RemoteRulesetRule>,
    #[serde(default)]
    bypass_actors: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    current_user_can_bypass: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteRulesetConditions {
    ref_name: RemoteRefNameCondition,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteRefNameCondition {
    include: Vec<String>,
    exclude: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteRulesetRule {
    #[serde(rename = "type")]
    kind: String,
}

pub(super) fn validate(ruleset: &RemoteRuleset, contract: Contract) -> Result<(), ReleaseError> {
    let rule_types = ruleset
        .rules
        .iter()
        .map(|rule| rule.kind.as_str())
        .collect::<BTreeSet<_>>();
    let expected_rule_types = ["deletion", "update"].into_iter().collect::<BTreeSet<_>>();
    let bypass_state_is_verified = matches!(
        (
            ruleset.bypass_actors.as_deref(),
            ruleset.current_user_can_bypass.as_deref(),
        ),
        (Some([]), Some("never")) | (None, None)
    );
    if ruleset.id != contract.id
        || ruleset.name != contract.name
        || ruleset.node_id != contract.node_id
        || ruleset.created_at != contract.created_at
        || ruleset.updated_at != contract.updated_at
        || ruleset.target != "tag"
        || ruleset.source_type != "Repository"
        || ruleset.source != contract.repository
        || ruleset.enforcement != "active"
        || ruleset.conditions.ref_name.include != [contract.ref_include]
        || !ruleset.conditions.ref_name.exclude.is_empty()
        || rule_types != expected_rule_types
        || ruleset.rules.len() != expected_rule_types.len()
        || !bypass_state_is_verified
    {
        return Err(ReleaseError::GitHubRelease(
            "GitHub release-tag ruleset is missing, inactive, bypassable, or changed".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRACT: Contract = Contract {
        id: 20_419_793,
        name: "Protect Stab v0.2.0 release tag",
        repository: "ifsheldon/Stab",
        ref_include: "refs/tags/v0.2.0",
        node_id: "RRS_lACqUmVwb3NpdG9yec5MX4zLzgE3lNE",
        created_at: "2026-08-05T00:07:23.349Z",
        updated_at: "2026-08-05T00:07:23.370Z",
    };

    fn exact_ruleset() -> RemoteRuleset {
        RemoteRuleset {
            id: CONTRACT.id,
            name: CONTRACT.name.to_string(),
            node_id: CONTRACT.node_id.to_string(),
            created_at: CONTRACT.created_at.to_string(),
            updated_at: CONTRACT.updated_at.to_string(),
            target: "tag".to_string(),
            source_type: "Repository".to_string(),
            source: CONTRACT.repository.to_string(),
            enforcement: "active".to_string(),
            conditions: RemoteRulesetConditions {
                ref_name: RemoteRefNameCondition {
                    include: vec![CONTRACT.ref_include.to_string()],
                    exclude: Vec::new(),
                },
            },
            rules: vec![
                RemoteRulesetRule {
                    kind: "update".to_string(),
                },
                RemoteRulesetRule {
                    kind: "deletion".to_string(),
                },
            ],
            bypass_actors: Some(Vec::new()),
            current_user_can_bypass: Some("never".to_string()),
        }
    }

    #[test]
    fn exact_ruleset_prevents_updates_and_deletions_without_bypass() {
        validate(&exact_ruleset(), CONTRACT).expect("exact release-tag ruleset");
    }

    #[test]
    fn changed_or_bypassable_rulesets_are_rejected() {
        let mut inactive = exact_ruleset();
        inactive.enforcement = "disabled".to_string();
        assert!(validate(&inactive, CONTRACT).is_err());

        let mut bypassable = exact_ruleset();
        bypassable.bypass_actors = Some(vec![serde_json::json!({"actor_type": "User"})]);
        assert!(validate(&bypassable, CONTRACT).is_err());

        let mut public_projection = exact_ruleset();
        public_projection.bypass_actors = None;
        public_projection.current_user_can_bypass = None;
        validate(&public_projection, CONTRACT).expect("pinned public ruleset projection");

        let mut incomplete_bypass = exact_ruleset();
        incomplete_bypass.bypass_actors = None;
        assert!(validate(&incomplete_bypass, CONTRACT).is_err());

        let mut current_user_bypass = exact_ruleset();
        current_user_bypass.current_user_can_bypass = Some("always".to_string());
        assert!(validate(&current_user_bypass, CONTRACT).is_err());

        let mut missing_bypass_identity = exact_ruleset();
        missing_bypass_identity.current_user_can_bypass = None;
        assert!(validate(&missing_bypass_identity, CONTRACT).is_err());

        let mut missing_update = exact_ruleset();
        missing_update.rules.pop();
        assert!(validate(&missing_update, CONTRACT).is_err());

        let mut changed_fingerprint = exact_ruleset();
        changed_fingerprint.updated_at = "2026-08-05T00:07:24.000Z".to_string();
        assert!(validate(&changed_fingerprint, CONTRACT).is_err());
    }

    #[test]
    fn unknown_applicability_conditions_fail_closed() {
        let value = serde_json::json!({
            "id": CONTRACT.id,
            "name": CONTRACT.name,
            "node_id": CONTRACT.node_id,
            "created_at": CONTRACT.created_at,
            "updated_at": CONTRACT.updated_at,
            "target": "tag",
            "source_type": "Repository",
            "source": CONTRACT.repository,
            "enforcement": "active",
            "conditions": {
                "ref_name": {
                    "include": [CONTRACT.ref_include],
                    "exclude": []
                },
                "repository_name": {"include": ["other"]}
            },
            "rules": [{"type": "update"}, {"type": "deletion"}],
            "bypass_actors": [],
            "current_user_can_bypass": "never"
        });
        assert!(serde_json::from_value::<RemoteRuleset>(value).is_err());
    }
}
