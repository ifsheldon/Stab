use std::collections::BTreeSet;

use serde::Deserialize;

use crate::ReleaseError;

pub(super) const ID: u64 = 20_419_793;

const NAME: &str = "Protect Stab v0.2.0 release tag";
const REPOSITORY: &str = "ifsheldon/Stab";
const RELEASE_TAG_REF: &str = "refs/tags/v0.2.0";

#[derive(Clone, Debug, Deserialize)]
pub(super) struct RemoteRuleset {
    id: u64,
    name: String,
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
struct RemoteRulesetConditions {
    ref_name: RemoteRefNameCondition,
}

#[derive(Clone, Debug, Deserialize)]
struct RemoteRefNameCondition {
    include: Vec<String>,
    exclude: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct RemoteRulesetRule {
    #[serde(rename = "type")]
    kind: String,
}

pub(super) fn validate(ruleset: &RemoteRuleset) -> Result<(), ReleaseError> {
    let rule_types = ruleset
        .rules
        .iter()
        .map(|rule| rule.kind.as_str())
        .collect::<BTreeSet<_>>();
    let expected_rule_types = ["deletion", "update"].into_iter().collect::<BTreeSet<_>>();
    if ruleset.id != ID
        || ruleset.name != NAME
        || ruleset.target != "tag"
        || ruleset.source_type != "Repository"
        || ruleset.source != REPOSITORY
        || ruleset.enforcement != "active"
        || ruleset.conditions.ref_name.include != [RELEASE_TAG_REF]
        || !ruleset.conditions.ref_name.exclude.is_empty()
        || rule_types != expected_rule_types
        || ruleset.rules.len() != expected_rule_types.len()
        || ruleset
            .bypass_actors
            .as_ref()
            .is_some_and(|actors| !actors.is_empty())
        || ruleset.current_user_can_bypass.as_deref() != Some("never")
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

    fn exact_ruleset() -> RemoteRuleset {
        RemoteRuleset {
            id: ID,
            name: NAME.to_string(),
            target: "tag".to_string(),
            source_type: "Repository".to_string(),
            source: REPOSITORY.to_string(),
            enforcement: "active".to_string(),
            conditions: RemoteRulesetConditions {
                ref_name: RemoteRefNameCondition {
                    include: vec![RELEASE_TAG_REF.to_string()],
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
        validate(&exact_ruleset()).expect("exact release-tag ruleset");
    }

    #[test]
    fn changed_or_bypassable_rulesets_are_rejected() {
        let mut inactive = exact_ruleset();
        inactive.enforcement = "disabled".to_string();
        assert!(validate(&inactive).is_err());

        let mut bypassable = exact_ruleset();
        bypassable.bypass_actors = Some(vec![serde_json::json!({"actor_type": "User"})]);
        assert!(validate(&bypassable).is_err());

        let mut current_user_bypass = exact_ruleset();
        current_user_bypass.current_user_can_bypass = Some("always".to_string());
        assert!(validate(&current_user_bypass).is_err());

        let mut missing_bypass_identity = exact_ruleset();
        missing_bypass_identity.current_user_can_bypass = None;
        assert!(validate(&missing_bypass_identity).is_err());

        let mut missing_update = exact_ruleset();
        missing_update.rules.pop();
        assert!(validate(&missing_update).is_err());
    }
}
