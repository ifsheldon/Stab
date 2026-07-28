use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{ExtractedPublicApiItem, PublicApiError, RustdocInventory, validate_api_path};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ExternalReexport {
    pub(super) canonical_crate_name: String,
    pub(super) canonical_path: String,
    pub(super) alias_path: String,
    pub(super) source_path: PathBuf,
    pub(super) source_line: u32,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::qualification) struct ResolvedExternalReexport {
    pub(in crate::qualification) alias_crate_name: String,
    pub(in crate::qualification) alias_path: String,
    pub(in crate::qualification) canonical_crate_name: String,
    pub(in crate::qualification) canonical_path: String,
}

pub(in crate::qualification) fn resolve_external_reexports(
    inventory: &mut RustdocInventory,
    dependencies: &[RustdocInventory],
) -> Result<Vec<ResolvedExternalReexport>, PublicApiError> {
    let mut items = inventory
        .items
        .drain(..)
        .map(|item| ((item.path.clone(), item.kind), item))
        .collect::<BTreeMap<_, _>>();
    let mut resolved = Vec::with_capacity(inventory.external_reexports.len());

    for reexport in &inventory.external_reexports {
        let dependency = dependencies
            .iter()
            .find(|dependency| dependency.crate_name == reexport.canonical_crate_name)
            .ok_or_else(|| unresolved(reexport))?;
        let canonical_prefix = dependency
            .visible_reexports
            .get(&reexport.canonical_path)
            .map_or(reexport.canonical_path.as_str(), String::as_str);
        let mut matched = false;
        for canonical in &dependency.items {
            let Some(path) =
                rebase_api_path(&canonical.path, canonical_prefix, &reexport.alias_path)
            else {
                continue;
            };
            let Some(owner_path) = rebase_api_path(
                &canonical.owner_path,
                canonical_prefix,
                &reexport.alias_path,
            ) else {
                continue;
            };
            matched = true;
            validate_api_path(&path)?;
            validate_api_path(&owner_path)?;
            let alias = ExtractedPublicApiItem {
                crate_name: inventory.crate_name.clone(),
                path: path.clone(),
                kind: canonical.kind,
                source_path: reexport.source_path.clone(),
                source_line: reexport.source_line,
                owner_path,
                evidence_crate_name: canonical.evidence_crate_name.clone(),
                evidence_owner_path: canonical.evidence_owner_path.clone(),
            };
            match items.entry((path.clone(), canonical.kind)) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(alias);
                }
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err(PublicApiError::DuplicateIdentity {
                        path,
                        kind: canonical.kind,
                    });
                }
            }
        }
        if !matched {
            return Err(unresolved(reexport));
        }
        resolved.push(ResolvedExternalReexport {
            alias_crate_name: inventory.crate_name.clone(),
            alias_path: reexport.alias_path.clone(),
            canonical_crate_name: dependency.crate_name.clone(),
            canonical_path: canonical_prefix.to_string(),
        });
    }

    inventory.items = items.into_values().collect();
    inventory.external_reexports.clear();
    resolved.sort();
    resolved.dedup();
    Ok(resolved)
}

fn unresolved(reexport: &ExternalReexport) -> PublicApiError {
    PublicApiError::UnresolvedExternalReexport {
        alias_path: reexport.alias_path.clone(),
        canonical_path: reexport.canonical_path.clone(),
    }
}

fn rebase_api_path(path: &str, canonical_prefix: &str, alias_prefix: &str) -> Option<String> {
    if path == canonical_prefix {
        return Some(alias_prefix.to_string());
    }
    let suffix = path.strip_prefix(canonical_prefix)?;
    if !suffix.starts_with("::") && !suffix.starts_with(" as ") {
        return None;
    }
    Some(format!("{alias_prefix}{suffix}"))
}
