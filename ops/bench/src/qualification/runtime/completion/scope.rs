use super::{CompletionError, QualificationTier, rollup_key};
use crate::qualification::runtime::run::ClaimClass;
use crate::root::RepoRoot;

pub(super) const DEM_SCOPE_ID: &str = "dem-r6";
pub(super) const RELEASE_SCOPE_ID: &str = "a9-release";
pub(super) const DEM_PARSE_GROUP: &str = "PERFQ-M10-DEM-PARSE-CONTRACT";
pub(super) const DEM_PRINT_GROUP: &str = "PERFQ-M10-DEM-PRINT-CONTRACT";
pub(super) const MAX_ROLLUPS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompletionScope {
    pub(super) id: String,
    pub(super) group_ids: Vec<String>,
    pub(super) expected_source_reports: usize,
}

pub(super) fn load(
    source_root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
    scope_id: &str,
) -> Result<CompletionScope, CompletionError> {
    let groups =
        super::super::group::load_groups(source_root, expected_performance_inventory_sha256)?;
    let group_ids = match scope_id {
        DEM_SCOPE_ID => vec![DEM_PARSE_GROUP.to_string(), DEM_PRINT_GROUP.to_string()],
        RELEASE_SCOPE_ID => groups
            .iter()
            .filter(|group| group.claim_class == ClaimClass::PromotablePerformance)
            .map(|group| group.id.to_string())
            .collect(),
        _ => return Err(CompletionError::UnknownScope(scope_id.to_string())),
    };
    if group_ids.is_empty() {
        return Err(CompletionError::EmptyScope(scope_id.to_string()));
    }

    let mut expected_source_reports = 0usize;
    for group_id in &group_ids {
        let group = groups
            .iter()
            .find(|group| group.id.to_string() == *group_id)
            .ok_or_else(|| CompletionError::MissingScopeGroup(group_id.clone()))?;
        if group.claim_class != ClaimClass::PromotablePerformance {
            return Err(CompletionError::NonPromotableScopeGroup(group_id.clone()));
        }
        let group_reports = group
            .scales
            .len()
            .checked_mul(2)
            .ok_or(CompletionError::ScopeSizeOverflow)?;
        expected_source_reports = expected_source_reports
            .checked_add(group_reports)
            .ok_or(CompletionError::ScopeSizeOverflow)?;
    }

    let scope = CompletionScope {
        id: scope_id.to_string(),
        group_ids,
        expected_source_reports,
    };
    if expected_rollup_keys(&scope).len() > MAX_ROLLUPS {
        return Err(CompletionError::RollupCount(
            expected_rollup_keys(&scope).len(),
        ));
    }
    Ok(scope)
}

pub(super) fn expected_rollup_keys(scope: &CompletionScope) -> Vec<String> {
    scope
        .group_ids
        .iter()
        .flat_map(|group| {
            [QualificationTier::Full, QualificationTier::Soak]
                .into_iter()
                .map(move |tier| rollup_key(group, tier))
        })
        .collect()
}
