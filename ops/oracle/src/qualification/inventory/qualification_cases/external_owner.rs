use std::collections::BTreeSet;

use super::{exact_public_api_owner, invalid};
use crate::qualification::inventory::InventoryError;
use crate::qualification::model::{
    ApiPath, CaseId, EvidenceCase, EvidenceProvenance, FeatureId, PublicApiItem,
};
use crate::qualification::public_api::ResolvedExternalReexport;

#[derive(Clone, Copy)]
pub(super) struct ExternalAliasPolicy<'a> {
    aliases: &'a [ResolvedExternalReexport],
    explicit_public_api_owners: &'a BTreeSet<(String, String)>,
}

impl<'a> ExternalAliasPolicy<'a> {
    pub(super) fn new(
        aliases: &'a [ResolvedExternalReexport],
        explicit_public_api_owners: &'a BTreeSet<(String, String)>,
    ) -> Self {
        Self {
            aliases,
            explicit_public_api_owners,
        }
    }

    pub(super) fn bind<'b>(
        self,
        items: &'b [PublicApiItem],
        evidence: &'b [EvidenceCase],
    ) -> PublicApiResolution<'a, 'b> {
        PublicApiResolution {
            items,
            evidence,
            external_alias_policy: self,
        }
    }
}

pub(super) struct PublicApiResolution<'a, 'b> {
    items: &'b [PublicApiItem],
    evidence: &'b [EvidenceCase],
    external_alias_policy: ExternalAliasPolicy<'a>,
}

pub(super) fn resolve_direct_public_api_owner(
    role: &str,
    crate_name: &str,
    owner_path: &ApiPath,
    expected_feature: FeatureId,
    target_owner: &CaseId,
    resolution: PublicApiResolution<'_, '_>,
) -> Result<Option<CaseId>, InventoryError> {
    let (feature_id, owner_case_id) =
        exact_public_api_owner(role, crate_name, owner_path, resolution.items)?;
    if feature_id != expected_feature {
        return invalid(format!(
            "{role} public API owner {crate_name}::{owner_path} has feature {}, expected {}",
            feature_id.as_str(),
            expected_feature.as_str()
        ));
    }
    if owner_case_id == *target_owner {
        return Ok(None);
    }
    let matches = resolution
        .evidence
        .iter()
        .filter(|case| case.id == owner_case_id)
        .collect::<Vec<_>>();
    let [evidence] = matches.as_slice() else {
        return invalid(format!(
            "{role} public API owner {crate_name}::{owner_path} resolved {} evidence records",
            matches.len()
        ));
    };
    if evidence.provenance != EvidenceProvenance::PublicRustApi
        || evidence.feature_id != expected_feature
    {
        return invalid(format!(
            "{role} public API owner {crate_name}::{owner_path} resolved incompatible evidence {}",
            evidence.id
        ));
    }
    if evidence.source_id != owner_path.as_str()
        && !resolution
            .external_alias_policy
            .aliases
            .iter()
            .any(|alias| {
                external_alias_maps_source(
                    alias,
                    crate_name,
                    owner_path.as_str(),
                    &evidence.source_id,
                    resolution.external_alias_policy.explicit_public_api_owners,
                )
            })
    {
        return Ok(None);
    }
    Ok(Some(owner_case_id))
}

fn external_alias_maps_source(
    alias: &ResolvedExternalReexport,
    crate_name: &str,
    owner_path: &str,
    evidence_source: &str,
    explicit_public_api_owners: &BTreeSet<(String, String)>,
) -> bool {
    if alias.alias_crate_name != crate_name || !api_path_is_owned_by(&alias.alias_path, owner_path)
    {
        return false;
    }
    let Some(suffix) = owner_path.strip_prefix(&alias.alias_path) else {
        return false;
    };
    let canonical_path = format!("{}{}", alias.canonical_path, suffix);
    canonical_path == evidence_source
        && !explicit_public_api_owners
            .contains(&(alias.canonical_crate_name.clone(), canonical_path))
}

fn api_path_is_owned_by(owner: &str, item: &str) -> bool {
    item == owner
        || item
            .strip_prefix(owner)
            .is_some_and(|suffix| suffix.starts_with("::") || suffix.starts_with(" as "))
}
