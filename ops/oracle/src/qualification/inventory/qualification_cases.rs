use std::collections::BTreeSet;

use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use super::evidence::behavioral_surface_for_feature;
use super::{InventoryError, MAX_QUALIFICATION_CASES_BYTES, MAX_SOURCE_BYTES, stable_id};
use crate::RepoRoot;
use crate::blocker_ledger::selector::CargoTestSelector;
use crate::qualification::model::{
    ApiPath, CanonicalOwnerException, CaseId, Comparator, EvidenceCase, EvidenceProvenance,
    EvidenceSelector, EvidenceState, EvidenceStatus, FeatureId, PropertyExecutionMode,
    PropertyExecutionPlan, PropertyPersistencePolicy, PropertyPlanRef, PropertyPlanSource,
    PublicApiAlias, PublicApiItem, RelativeSourcePath, ResourceContract, SelectorKind,
    SemanticDigest, StableCaseDomain, UpstreamCase, UpstreamDisposition,
};
use crate::qualification::public_api::ResolvedExternalReexport;

mod external_owner;
mod owner_expansion;

use external_owner::{ExternalAliasPolicy, resolve_direct_public_api_owner};
use owner_expansion::{MAX_OWNERS_PER_CASE, OwnerEntryKind, expand_upstream_owners};

const LEDGER_SCHEMA_VERSION: u32 = 4;
const MAX_LEDGER_CASES: usize = 4_096;
const MAX_PUBLIC_API_ALIASES: usize = 512;
const MAX_LEDGER_TEXT_BYTES: usize = 2_048;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QualificationCaseLedger {
    schema_version: u32,
    stim_version: String,
    stim_commit: String,
    cases: Vec<QualificationCaseSpec>,
    #[serde(default)]
    existing_parent_mappings: Vec<ExistingParentMappingSpec>,
    #[serde(default)]
    public_api_aliases: Vec<PublicApiAliasSpec>,
    #[serde(default)]
    canonical_owner_exceptions: Vec<CanonicalOwnerException>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QualificationCaseSpec {
    id: String,
    feature_id: FeatureId,
    comparator: Comparator,
    primary_selector: EvidenceSelector,
    resource_contract: ResourceContract,
    #[serde(default)]
    negative_axes: Vec<String>,
    #[serde(default)]
    upstream_owners: Vec<UpstreamOwnerSpec>,
    #[serde(default)]
    upstream_word_size_families: Vec<UpstreamWordSizeFamilySpec>,
    #[serde(default)]
    public_api_owners: Vec<PublicApiOwnerSpec>,
    #[serde(default)]
    oracle_fixture_owners: Vec<String>,
    #[serde(default)]
    static_property_plan: Option<StaticPropertyPlanSpec>,
    #[serde(default)]
    standalone: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpstreamOwnerSpec {
    path: RelativeSourcePath,
    symbol: String,
    #[serde(default)]
    subcase: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpstreamWordSizeFamilySpec {
    path: RelativeSourcePath,
    symbol_base: String,
    word_sizes: Vec<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicApiOwnerSpec {
    crate_name: String,
    owner_path: ApiPath,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicApiAliasSpec {
    crate_name: String,
    alias_owner_path: ApiPath,
    canonical_owner_path: ApiPath,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExistingParentMappingSpec {
    id: String,
    feature_id: FeatureId,
    parent: ExistingParentSpec,
    #[serde(default)]
    upstream_owners: Vec<UpstreamOwnerSpec>,
    #[serde(default)]
    upstream_word_size_families: Vec<UpstreamWordSizeFamilySpec>,
    #[serde(default)]
    public_api_owners: Vec<PublicApiOwnerSpec>,
    #[serde(default)]
    oracle_fixture_owners: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExistingParentSpec {
    provenance: EvidenceProvenance,
    source_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StaticPropertyPlanSpec {
    generator_domain: String,
    case_count: u32,
    corpus_path: RelativeSourcePath,
}

pub(super) fn apply(
    root: &RepoRoot,
    stim_version: &str,
    stim_commit: &str,
    upstream_cases: &mut [UpstreamCase],
    public_api_items: &mut [PublicApiItem],
    evidence_cases: &mut Vec<EvidenceCase>,
    external_aliases: &[ResolvedExternalReexport],
) -> Result<(Vec<PublicApiAlias>, Vec<CanonicalOwnerException>), InventoryError> {
    let ledger = load(root)?;
    validate_header(&ledger, stim_version, stim_commit)?;
    let explicit_public_api_owners = ledger
        .cases
        .iter()
        .flat_map(|case| case.public_api_owners.iter())
        .chain(
            ledger
                .existing_parent_mappings
                .iter()
                .flat_map(|mapping| mapping.public_api_owners.iter()),
        )
        .map(|owner| (owner.crate_name.clone(), owner.owner_path.to_string()))
        .collect::<BTreeSet<_>>();
    let external_alias_policy =
        ExternalAliasPolicy::new(external_aliases, &explicit_public_api_owners);

    let mut source_ids = BTreeSet::new();
    let mut qualification_ids = BTreeSet::new();
    let mut claimed_evidence = BTreeSet::new();
    let mut qualification_cases = Vec::with_capacity(ledger.cases.len());

    for spec in ledger.cases {
        let upstream_owners = validate_case_shape(&spec)?;
        if !source_ids.insert(spec.id.clone()) {
            return invalid(format!(
                "qualification case source id {:?} is duplicated",
                spec.id
            ));
        }
        let qualification_id = stable_id(StableCaseDomain::EvidenceQualification, spec.id.as_str());
        if !qualification_ids.insert(qualification_id.clone()) {
            return invalid(format!("qualification case id collision for {:?}", spec.id));
        }

        let mut owner_count = 0usize;
        for owner in &upstream_owners {
            validate_text("upstream symbol", &owner.symbol)?;
            if let Some(subcase) = &owner.subcase {
                validate_text("upstream subcase", subcase)?;
            }
            let matches = upstream_cases
                .iter()
                .enumerate()
                .filter(|(_, case)| {
                    case.path == owner.path
                        && case.symbol == owner.symbol
                        && case.subcase == owner.subcase
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [case_index] = matches.as_slice() else {
                return invalid(format!(
                    "qualification case {:?} upstream owner {}:{}:{:?} resolved {} records",
                    spec.id,
                    owner.path,
                    owner.symbol,
                    owner.subcase,
                    matches.len()
                ));
            };
            let upstream_case = upstream_cases.get_mut(*case_index).ok_or_else(|| {
                InventoryError::InvalidQualificationCases(format!(
                    "qualification case {:?} resolved an invalid upstream owner index",
                    spec.id
                ))
            })?;
            let ownership = upstream_case
                .ownerships
                .iter_mut()
                .find(|ownership| ownership.feature_id == spec.feature_id)
                .ok_or_else(|| {
                    InventoryError::InvalidQualificationCases(format!(
                        "qualification case {:?} upstream owner {}:{}:{:?} has no {} ownership",
                        spec.id,
                        owner.path,
                        owner.symbol,
                        owner.subcase,
                        spec.feature_id.as_str()
                    ))
                })?;
            let old_owner = ownership.owner_case_id.clone();
            claim_planned_evidence(
                &spec.id,
                spec.feature_id,
                &old_owner,
                EvidenceProvenance::UpstreamSemanticCase,
                evidence_cases,
                &mut claimed_evidence,
            )?;
            ownership.comparator = spec.comparator;
            ownership.owner_case_id = qualification_id.clone();
            owner_count = owner_count.saturating_add(1);
        }

        for owner in &spec.public_api_owners {
            validate_text("public API crate", &owner.crate_name)?;
            let Some(old_owner) = resolve_direct_public_api_owner(
                "qualification case",
                &owner.crate_name,
                &owner.owner_path,
                spec.feature_id,
                &qualification_id,
                external_alias_policy.bind(public_api_items, evidence_cases),
            )?
            else {
                owner_count = owner_count.saturating_add(1);
                continue;
            };
            claim_planned_evidence(
                &spec.id,
                spec.feature_id,
                &old_owner,
                EvidenceProvenance::PublicRustApi,
                evidence_cases,
                &mut claimed_evidence,
            )?;
            let mut mapped_items = 0usize;
            for item in public_api_items
                .iter_mut()
                .filter(|item| item.owner_case_id == old_owner)
            {
                item.owner_case_id = qualification_id.clone();
                mapped_items = mapped_items.saturating_add(1);
            }
            if mapped_items == 0 {
                return invalid(format!(
                    "qualification case {:?} public API owner {}::{} owns no API items",
                    spec.id, owner.crate_name, owner.owner_path
                ));
            }
            owner_count = owner_count.saturating_add(1);
        }

        let mut supporting_selectors = Vec::new();
        for fixture_id in &spec.oracle_fixture_owners {
            validate_identifier("oracle fixture", fixture_id)?;
            let matches = evidence_cases
                .iter()
                .filter(|case| {
                    case.provenance == EvidenceProvenance::OracleFixture
                        && case.feature_id == spec.feature_id
                        && case.source_id == *fixture_id
                })
                .map(|case| case.id.clone())
                .collect::<Vec<_>>();
            let [old_owner] = matches.as_slice() else {
                return invalid(format!(
                    "qualification case {:?} oracle fixture owner {:?} resolved {} evidence records",
                    spec.id,
                    fixture_id,
                    matches.len()
                ));
            };
            claim_oracle_fixture_evidence(
                &spec.id,
                spec.feature_id,
                old_owner,
                &spec.primary_selector,
                evidence_cases,
                &mut claimed_evidence,
            )?;
            supporting_selectors.push(EvidenceSelector {
                state: EvidenceState::Existing,
                kind: SelectorKind::OracleFixture,
                value: vec![fixture_id.clone()],
            });
            owner_count = owner_count.saturating_add(1);
        }

        if owner_count == 0 && !spec.standalone {
            return invalid(format!(
                "qualification case {:?} has no exact source owner and is not standalone",
                spec.id
            ));
        }
        supporting_selectors.sort();
        supporting_selectors.dedup();
        let property_plan = property_plan(root, &spec)?;
        qualification_cases.push(EvidenceCase {
            id: qualification_id,
            feature_id: spec.feature_id,
            behavioral_surface: behavioral_surface_for_feature(
                spec.feature_id,
                EvidenceProvenance::QualificationPlan,
            ),
            provenance: EvidenceProvenance::QualificationPlan,
            source_id: spec.id,
            comparator: spec.comparator,
            execution: super::super::execution_contract::for_status(EvidenceStatus::Implemented),
            statistical_plan: None,
            property_plan,
            primary_selector: spec.primary_selector,
            supporting_selectors,
            resource_contract: spec.resource_contract,
            negative_axes: spec.negative_axes,
            performance_groups: spec
                .feature_id
                .performance_groups()
                .iter()
                .map(|group| (*group).to_string())
                .collect(),
            deferred_product: None,
            status: EvidenceStatus::Implemented,
        });
    }

    for mapping in ledger.existing_parent_mappings {
        let upstream_owners = validate_existing_parent_mapping_shape(&mapping)?;
        if !source_ids.insert(mapping.id.clone()) {
            return invalid(format!(
                "qualification mapping source id {:?} is duplicated",
                mapping.id
            ));
        }
        apply_existing_parent_mapping(
            &mapping,
            &upstream_owners,
            upstream_cases,
            public_api_items,
            evidence_cases,
            &mut claimed_evidence,
            external_alias_policy,
        )?;
    }

    apply_public_api_aliases(
        &ledger.public_api_aliases,
        public_api_items,
        evidence_cases,
        &qualification_cases,
        &mut claimed_evidence,
    )?;

    evidence_cases.retain(|case| !claimed_evidence.contains(&case.id));
    evidence_cases.extend(qualification_cases);
    let aliases = ledger
        .public_api_aliases
        .into_iter()
        .map(|alias| PublicApiAlias {
            crate_name: alias.crate_name,
            alias_path: alias.alias_owner_path,
            canonical_crate_name: None,
            canonical_path: alias.canonical_owner_path,
        })
        .collect();
    let mut canonical_owner_exceptions = ledger.canonical_owner_exceptions;
    canonical_owner_exceptions.sort();
    Ok((aliases, canonical_owner_exceptions))
}

fn load(root: &RepoRoot) -> Result<QualificationCaseLedger, InventoryError> {
    let path = root.qualification_cases();
    let bytes = crate::safe_file::read_regular_file_bounded(&path, MAX_QUALIFICATION_CASES_BYTES)
        .map_err(|source| InventoryError::Read {
        path: path.clone(),
        reason: source.to_string().into_boxed_str(),
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|source| InventoryError::ParseQualificationCases { path, source })
}

fn validate_header(
    ledger: &QualificationCaseLedger,
    stim_version: &str,
    stim_commit: &str,
) -> Result<(), InventoryError> {
    if ledger.schema_version != LEDGER_SCHEMA_VERSION {
        return invalid(format!(
            "schema version {} does not match {}",
            ledger.schema_version, LEDGER_SCHEMA_VERSION
        ));
    }
    if ledger.stim_version != stim_version || ledger.stim_commit != stim_commit {
        return invalid(format!(
            "Stim pin {}@{} does not match {}@{}",
            ledger.stim_version, ledger.stim_commit, stim_version, stim_commit
        ));
    }
    if ledger.cases.len() > MAX_LEDGER_CASES {
        return invalid(format!(
            "case count {} exceeds {}",
            ledger.cases.len(),
            MAX_LEDGER_CASES
        ));
    }
    if ledger.public_api_aliases.len() > MAX_PUBLIC_API_ALIASES {
        return invalid(format!(
            "public API alias count {} exceeds {}",
            ledger.public_api_aliases.len(),
            MAX_PUBLIC_API_ALIASES
        ));
    }
    Ok(())
}

fn validate_case_shape(
    spec: &QualificationCaseSpec,
) -> Result<Vec<UpstreamOwnerSpec>, InventoryError> {
    CaseId::try_new(spec.id.clone()).map_err(|reason| {
        InventoryError::InvalidQualificationCases(format!(
            "qualification case source id {:?} is invalid: {reason}",
            spec.id
        ))
    })?;
    let upstream_owners = expand_upstream_owners(
        OwnerEntryKind::Case,
        &spec.id,
        &spec.upstream_owners,
        &spec.upstream_word_size_families,
        spec.public_api_owners
            .len()
            .checked_add(spec.oracle_fixture_owners.len())
            .ok_or_else(|| {
                InventoryError::InvalidQualificationCases(format!(
                    "qualification case {:?} owner count overflowed",
                    spec.id
                ))
            })?,
    )?;
    validate_text("resource contract", &spec.resource_contract.detail)?;
    for axis in &spec.negative_axes {
        validate_text("negative axis", axis)?;
    }
    if spec.primary_selector.state != EvidenceState::Existing {
        return invalid(format!(
            "qualification case {:?} primary selector is not existing",
            spec.id
        ));
    }
    match spec.primary_selector.kind {
        SelectorKind::CargoTest => {
            let parsed =
                CargoTestSelector::parse(&spec.primary_selector.value).map_err(|reason| {
                    InventoryError::InvalidQualificationCases(format!(
                        "qualification case {:?} Cargo selector {reason}",
                        spec.id
                    ))
                })?;
            if !parsed.is_exact() {
                return invalid(format!(
                    "qualification case {:?} Cargo selector is not exact",
                    spec.id
                ));
            }
        }
        SelectorKind::PropertyTarget if spec.comparator == Comparator::Property => {
            let [target] = spec.primary_selector.value.as_slice() else {
                return invalid(format!(
                    "qualification case {:?} property selector must contain one target",
                    spec.id
                ));
            };
            if target != &spec.id {
                return invalid(format!(
                    "qualification case {:?} property target must equal its source id",
                    spec.id
                ));
            }
        }
        _ => {
            return invalid(format!(
                "qualification case {:?} primary selector kind is unsupported for {:?}",
                spec.id, spec.comparator
            ));
        }
    }
    if spec.comparator == Comparator::Statistical {
        return invalid(format!(
            "qualification case {:?} needs a source-owned statistical plan before promotion",
            spec.id
        ));
    }
    if spec.comparator != Comparator::Property && spec.static_property_plan.is_some() {
        return invalid(format!(
            "non-property qualification case {:?} declares a property plan",
            spec.id
        ));
    }
    Ok(upstream_owners)
}

fn validate_existing_parent_mapping_shape(
    mapping: &ExistingParentMappingSpec,
) -> Result<Vec<UpstreamOwnerSpec>, InventoryError> {
    CaseId::try_new(mapping.id.clone()).map_err(|reason| {
        InventoryError::InvalidQualificationCases(format!(
            "qualification mapping source id {:?} is invalid: {reason}",
            mapping.id
        ))
    })?;
    validate_text("existing parent source id", &mapping.parent.source_id)?;
    if !matches!(
        mapping.parent.provenance,
        EvidenceProvenance::BlockerLedger
            | EvidenceProvenance::OracleFixture
            | EvidenceProvenance::RustRegression
    ) {
        return invalid(format!(
            "qualification mapping {:?} uses unsupported existing parent provenance {:?}",
            mapping.id, mapping.parent.provenance
        ));
    }
    let upstream_owners = expand_upstream_owners(
        OwnerEntryKind::Mapping,
        &mapping.id,
        &mapping.upstream_owners,
        &mapping.upstream_word_size_families,
        mapping
            .public_api_owners
            .len()
            .checked_add(mapping.oracle_fixture_owners.len())
            .ok_or_else(|| {
                InventoryError::InvalidQualificationCases(format!(
                    "qualification mapping {:?} owner count overflowed",
                    mapping.id
                ))
            })?,
    )?;
    let owner_count = upstream_owners
        .len()
        .checked_add(mapping.public_api_owners.len())
        .and_then(|count| count.checked_add(mapping.oracle_fixture_owners.len()))
        .ok_or_else(|| {
            InventoryError::InvalidQualificationCases(format!(
                "qualification mapping {:?} owner count overflowed",
                mapping.id
            ))
        })?;
    if owner_count == 0 {
        return invalid(format!(
            "qualification mapping {:?} has {} owners; expected 1..={}",
            mapping.id, owner_count, MAX_OWNERS_PER_CASE
        ));
    }
    Ok(upstream_owners)
}

fn apply_existing_parent_mapping(
    mapping: &ExistingParentMappingSpec,
    upstream_owners: &[UpstreamOwnerSpec],
    upstream_cases: &mut [UpstreamCase],
    public_api_items: &mut [PublicApiItem],
    evidence_cases: &mut [EvidenceCase],
    claimed_evidence: &mut BTreeSet<CaseId>,
    external_alias_policy: ExternalAliasPolicy<'_>,
) -> Result<(), InventoryError> {
    let parent_matches = evidence_cases
        .iter()
        .enumerate()
        .filter(|(_, case)| {
            case.feature_id == mapping.feature_id
                && case.provenance == mapping.parent.provenance
                && case.source_id == mapping.parent.source_id
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let [parent_index] = parent_matches.as_slice() else {
        return invalid(format!(
            "qualification mapping {:?} existing parent {:?}/{:?} resolved {} evidence records",
            mapping.id,
            mapping.parent.provenance,
            mapping.parent.source_id,
            parent_matches.len()
        ));
    };
    let parent = evidence_cases.get(*parent_index).ok_or_else(|| {
        InventoryError::InvalidQualificationCases(format!(
            "qualification mapping {:?} resolved an invalid existing parent index",
            mapping.id
        ))
    })?;
    if !matches!(
        parent.status,
        EvidenceStatus::Implemented | EvidenceStatus::EvidenceClose
    ) || parent.primary_selector.state != EvidenceState::Existing
    {
        return invalid(format!(
            "qualification mapping {:?} parent {} is not executable existing evidence",
            mapping.id, parent.id
        ));
    }
    let parent_id = parent.id.clone();
    let parent_comparator = parent.comparator;

    for owner in upstream_owners {
        validate_text("upstream symbol", &owner.symbol)?;
        if let Some(subcase) = &owner.subcase {
            validate_text("upstream subcase", subcase)?;
        }
        let matches = upstream_cases
            .iter()
            .enumerate()
            .filter(|(_, case)| {
                case.path == owner.path
                    && case.symbol == owner.symbol
                    && case.subcase == owner.subcase
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [case_index] = matches.as_slice() else {
            return invalid(format!(
                "qualification mapping {:?} upstream owner {}:{}:{:?} resolved {} records",
                mapping.id,
                owner.path,
                owner.symbol,
                owner.subcase,
                matches.len()
            ));
        };
        let upstream_case = upstream_cases.get_mut(*case_index).ok_or_else(|| {
            InventoryError::InvalidQualificationCases(format!(
                "qualification mapping {:?} resolved an invalid upstream owner index",
                mapping.id
            ))
        })?;
        let ownership = upstream_case
            .ownerships
            .iter_mut()
            .find(|ownership| ownership.feature_id == mapping.feature_id)
            .ok_or_else(|| {
                InventoryError::InvalidQualificationCases(format!(
                    "qualification mapping {:?} upstream owner {}:{}:{:?} has no {} ownership",
                    mapping.id,
                    owner.path,
                    owner.symbol,
                    owner.subcase,
                    mapping.feature_id.as_str()
                ))
            })?;
        let old_owner = ownership.owner_case_id.clone();
        claim_planned_evidence(
            &mapping.id,
            mapping.feature_id,
            &old_owner,
            EvidenceProvenance::UpstreamSemanticCase,
            evidence_cases,
            claimed_evidence,
        )?;
        ownership.comparator = parent_comparator;
        ownership.owner_case_id = parent_id.clone();
        upstream_case.disposition = UpstreamDisposition::PortedRust;
        upstream_case.deferred_product = None;
        upstream_case.reason = format!(
            "Qualification mapping {} binds this exact upstream owner to canonical existing Rust evidence.",
            mapping.id
        );
    }

    for owner in &mapping.public_api_owners {
        validate_text("public API crate", &owner.crate_name)?;
        let Some(old_owner) = resolve_direct_public_api_owner(
            "qualification mapping",
            &owner.crate_name,
            &owner.owner_path,
            mapping.feature_id,
            &parent_id,
            external_alias_policy.bind(public_api_items, evidence_cases),
        )?
        else {
            continue;
        };
        claim_planned_evidence(
            &mapping.id,
            mapping.feature_id,
            &old_owner,
            EvidenceProvenance::PublicRustApi,
            evidence_cases,
            claimed_evidence,
        )?;
        let mut mapped_items = 0usize;
        for item in public_api_items
            .iter_mut()
            .filter(|item| item.owner_case_id == old_owner)
        {
            item.owner_case_id = parent_id.clone();
            mapped_items = mapped_items.saturating_add(1);
        }
        if mapped_items == 0 {
            return invalid(format!(
                "qualification mapping {:?} public API owner {}::{} owns no API items",
                mapping.id, owner.crate_name, owner.owner_path
            ));
        }
    }

    let mut supporting_selectors = Vec::new();
    for fixture_id in &mapping.oracle_fixture_owners {
        validate_identifier("oracle fixture", fixture_id)?;
        let matches = evidence_cases
            .iter()
            .filter(|case| {
                case.provenance == EvidenceProvenance::OracleFixture
                    && case.feature_id == mapping.feature_id
                    && case.source_id == *fixture_id
            })
            .map(|case| case.id.clone())
            .collect::<Vec<_>>();
        let [old_owner] = matches.as_slice() else {
            return invalid(format!(
                "qualification mapping {:?} oracle fixture owner {:?} resolved {} evidence records",
                mapping.id,
                fixture_id,
                matches.len()
            ));
        };
        claim_planned_evidence(
            &mapping.id,
            mapping.feature_id,
            old_owner,
            EvidenceProvenance::OracleFixture,
            evidence_cases,
            claimed_evidence,
        )?;
        supporting_selectors.push(EvidenceSelector {
            state: EvidenceState::Existing,
            kind: SelectorKind::OracleFixture,
            value: vec![fixture_id.clone()],
        });
    }
    let parent = evidence_cases.get_mut(*parent_index).ok_or_else(|| {
        InventoryError::InvalidQualificationCases(format!(
            "qualification mapping {:?} lost its existing parent",
            mapping.id
        ))
    })?;
    parent.supporting_selectors.extend(supporting_selectors);
    parent.supporting_selectors.sort();
    parent.supporting_selectors.dedup();
    Ok(())
}

fn apply_public_api_aliases(
    aliases: &[PublicApiAliasSpec],
    public_api_items: &mut [PublicApiItem],
    evidence_cases: &[EvidenceCase],
    qualification_cases: &[EvidenceCase],
    claimed_evidence: &mut BTreeSet<CaseId>,
) -> Result<(), InventoryError> {
    let mut alias_paths = BTreeSet::new();
    for alias in aliases {
        validate_public_api_alias_shape(alias)?;
        if !alias_paths.insert((alias.crate_name.as_str(), alias.alias_owner_path.as_str())) {
            return invalid(format!(
                "public API alias {}::{} is duplicated",
                alias.crate_name, alias.alias_owner_path
            ));
        }
    }

    for alias in aliases {
        if alias_paths.contains(&(
            alias.crate_name.as_str(),
            alias.canonical_owner_path.as_str(),
        )) {
            return invalid(format!(
                "public API alias {}::{} uses another alias as its canonical owner",
                alias.crate_name, alias.alias_owner_path
            ));
        }

        let (alias_feature, alias_owner) = exact_public_api_owner(
            "alias",
            &alias.crate_name,
            &alias.alias_owner_path,
            public_api_items,
        )?;
        let (canonical_feature, canonical_owner) = exact_public_api_owner(
            "canonical",
            &alias.crate_name,
            &alias.canonical_owner_path,
            public_api_items,
        )?;
        if alias_feature != canonical_feature {
            return invalid(format!(
                "public API alias {}::{} has feature {}, but canonical owner {} has feature {}",
                alias.crate_name,
                alias.alias_owner_path,
                alias_feature.as_str(),
                alias.canonical_owner_path,
                canonical_feature.as_str()
            ));
        }
        if alias_owner == canonical_owner {
            continue;
        }

        let alias_evidence = evidence_cases
            .iter()
            .filter(|case| {
                case.provenance == EvidenceProvenance::PublicRustApi
                    && case.source_id == alias.alias_owner_path.as_str()
            })
            .collect::<Vec<_>>();
        let [alias_evidence] = alias_evidence.as_slice() else {
            return invalid(format!(
                "public API alias {}::{} resolved {} API evidence records",
                alias.crate_name,
                alias.alias_owner_path,
                alias_evidence.len()
            ));
        };
        if alias_evidence.id != alias_owner {
            return invalid(format!(
                "public API alias {}::{} no longer owns its planned API evidence",
                alias.crate_name, alias.alias_owner_path
            ));
        }
        claim_planned_evidence(
            alias.alias_owner_path.as_str(),
            alias_feature,
            &alias_owner,
            EvidenceProvenance::PublicRustApi,
            evidence_cases,
            claimed_evidence,
        )?;

        let canonical_parents = evidence_cases
            .iter()
            .chain(qualification_cases)
            .filter(|case| case.id == canonical_owner)
            .collect::<Vec<_>>();
        let [canonical_parent] = canonical_parents.as_slice() else {
            return invalid(format!(
                "public API alias {}::{} canonical owner {} resolved {} parent records",
                alias.crate_name,
                alias.alias_owner_path,
                alias.canonical_owner_path,
                canonical_parents.len()
            ));
        };
        if claimed_evidence.contains(&canonical_owner)
            || canonical_parent.feature_id != canonical_feature
            || !matches!(
                canonical_parent.status,
                EvidenceStatus::Planned
                    | EvidenceStatus::Implemented
                    | EvidenceStatus::EvidenceClose
            )
        {
            return invalid(format!(
                "public API alias {}::{} canonical parent {} is stale or incompatible",
                alias.crate_name, alias.alias_owner_path, canonical_parent.id
            ));
        }

        for item in public_api_items
            .iter_mut()
            .filter(|item| item.crate_name == alias.crate_name && item.owner_case_id == alias_owner)
        {
            item.owner_case_id = canonical_owner.clone();
        }
    }
    Ok(())
}

fn validate_public_api_alias_shape(alias: &PublicApiAliasSpec) -> Result<(), InventoryError> {
    validate_text("public API alias crate", &alias.crate_name)?;
    if alias.alias_owner_path == alias.canonical_owner_path {
        return invalid(format!(
            "public API alias {}::{} is self-referential",
            alias.crate_name, alias.alias_owner_path
        ));
    }
    let core_namespace_alias = alias.crate_name == "stab_core"
        && (alias
            .alias_owner_path
            .as_str()
            .starts_with("stab_core::analysis::")
            || alias
                .alias_owner_path
                .as_str()
                .starts_with("stab_core::execution::"))
        && alias
            .canonical_owner_path
            .as_str()
            .starts_with("stab_core::")
        && !alias
            .canonical_owner_path
            .as_str()
            .starts_with("stab_core::analysis::")
        && !alias
            .canonical_owner_path
            .as_str()
            .starts_with("stab_core::execution::");
    let analysis_root_alias = ["circuit", "gate"].iter().any(|module| {
        alias.crate_name == "stab_analysis"
            && alias
                .canonical_owner_path
                .as_str()
                .strip_prefix("stab_analysis::")
                .zip(
                    alias
                        .alias_owner_path
                        .as_str()
                        .strip_prefix(&format!("stab_analysis::{module}::")),
                )
                .is_some_and(|(canonical_suffix, alias_suffix)| canonical_suffix == alias_suffix)
    });
    let engine_root_alias = ["fingerprint", "probability"].iter().any(|module| {
        alias.crate_name == "stab_engine"
            && alias
                .canonical_owner_path
                .as_str()
                .strip_prefix("stab_engine::")
                .zip(
                    alias
                        .alias_owner_path
                        .as_str()
                        .strip_prefix(&format!("stab_engine::{module}::")),
                )
                .is_some_and(|(canonical_suffix, alias_suffix)| canonical_suffix == alias_suffix)
    });
    if !core_namespace_alias && !analysis_root_alias && !engine_root_alias {
        return invalid(format!(
            "public API alias {}::{} -> {} is outside the namespace/root contract",
            alias.crate_name, alias.alias_owner_path, alias.canonical_owner_path
        ));
    }
    Ok(())
}

fn exact_public_api_owner(
    role: &str,
    crate_name: &str,
    owner_path: &ApiPath,
    public_api_items: &[PublicApiItem],
) -> Result<(FeatureId, CaseId), InventoryError> {
    let matches = public_api_items
        .iter()
        .filter(|item| item.crate_name == crate_name && item.path == *owner_path)
        .collect::<Vec<_>>();
    let [item] = matches.as_slice() else {
        return invalid(format!(
            "public API alias {role} {crate_name}::{owner_path} resolved {} API items",
            matches.len()
        ));
    };
    Ok((item.feature_id, item.owner_case_id.clone()))
}

fn claim_planned_evidence(
    mapping_id: &str,
    feature_id: FeatureId,
    evidence_id: &CaseId,
    provenance: EvidenceProvenance,
    evidence_cases: &[EvidenceCase],
    claimed: &mut BTreeSet<CaseId>,
) -> Result<(), InventoryError> {
    let case = evidence_cases
        .iter()
        .find(|case| case.id == *evidence_id)
        .ok_or_else(|| {
            InventoryError::InvalidQualificationCases(format!(
                "qualification case {:?} references missing evidence {}",
                mapping_id, evidence_id
            ))
        })?;
    if case.status != EvidenceStatus::Planned
        || case.provenance != provenance
        || case.feature_id != feature_id
    {
        return invalid(format!(
            "qualification case {:?} cannot claim {} with {:?}/{:?}/{:?}/{:?}",
            mapping_id, evidence_id, case.status, case.provenance, case.feature_id, case.comparator
        ));
    }
    if !claimed.insert(evidence_id.clone()) {
        return invalid(format!(
            "qualification case {:?} repeats or steals evidence {}",
            mapping_id, evidence_id
        ));
    }
    Ok(())
}

fn claim_oracle_fixture_evidence(
    mapping_id: &str,
    feature_id: FeatureId,
    evidence_id: &CaseId,
    primary_selector: &EvidenceSelector,
    evidence_cases: &[EvidenceCase],
    claimed: &mut BTreeSet<CaseId>,
) -> Result<(), InventoryError> {
    let case = evidence_cases
        .iter()
        .find(|case| case.id == *evidence_id)
        .ok_or_else(|| {
            InventoryError::InvalidQualificationCases(format!(
                "qualification case {:?} references missing oracle evidence {}",
                mapping_id, evidence_id
            ))
        })?;
    if case.status == EvidenceStatus::Planned {
        return claim_planned_evidence(
            mapping_id,
            feature_id,
            evidence_id,
            EvidenceProvenance::OracleFixture,
            evidence_cases,
            claimed,
        );
    }
    if case.status != EvidenceStatus::Implemented
        || case.provenance != EvidenceProvenance::OracleFixture
        || case.feature_id != feature_id
        || case.primary_selector != *primary_selector
    {
        return invalid(format!(
            "qualification case {:?} cannot absorb exact oracle evidence {} with {:?}/{:?}/{:?}/{:?}",
            mapping_id,
            evidence_id,
            case.status,
            case.provenance,
            case.feature_id,
            case.primary_selector
        ));
    }
    if !claimed.insert(evidence_id.clone()) {
        return invalid(format!(
            "qualification case {:?} repeats or steals oracle evidence {}",
            mapping_id, evidence_id
        ));
    }
    Ok(())
}

fn property_plan(
    root: &RepoRoot,
    spec: &QualificationCaseSpec,
) -> Result<Option<PropertyPlanRef>, InventoryError> {
    if spec.comparator != Comparator::Property {
        return Ok(None);
    }
    let plan = match spec.primary_selector.kind {
        SelectorKind::PropertyTarget => {
            if spec.static_property_plan.is_some() {
                return invalid(format!(
                    "qualification case {:?} mixes a worker target with a static corpus",
                    spec.id
                ));
            }
            crate::qualification::property::registered_execution_plan(&spec.id).ok_or_else(
                || {
                    InventoryError::InvalidQualificationCases(format!(
                        "qualification case {:?} property target is not registered",
                        spec.id
                    ))
                },
            )?
        }
        SelectorKind::CargoTest => {
            let static_plan = spec.static_property_plan.as_ref().ok_or_else(|| {
                InventoryError::InvalidQualificationCases(format!(
                    "qualification case {:?} Cargo property has no static corpus",
                    spec.id
                ))
            })?;
            validate_text("property generator domain", &static_plan.generator_domain)?;
            if static_plan.case_count == 0 {
                return invalid(format!(
                    "qualification case {:?} static property case count is zero",
                    spec.id
                ));
            }
            let path = root.path.join(static_plan.corpus_path.as_path());
            let bytes = crate::safe_file::read_regular_file_bounded(&path, MAX_SOURCE_BYTES)
                .map_err(|source| InventoryError::Read {
                    path,
                    reason: source.to_string().into_boxed_str(),
                })?;
            PropertyExecutionPlan {
                generator_domain: static_plan.generator_domain.clone(),
                maximum_generated_bytes: 0,
                seeds: Vec::new(),
                case_count: static_plan.case_count,
                corpus_path: Some(static_plan.corpus_path.clone()),
                corpus_sha256: Some(SemanticDigest::from_bytes(Sha256::digest(bytes).into())),
                persistence_policy: PropertyPersistencePolicy::ExistingFocusedRegression,
                execution_mode: PropertyExecutionMode::CargoSubprocess,
            }
        }
        _ => unreachable!("validated qualification selector kind"),
    };
    Ok(Some(PropertyPlanRef {
        state: EvidenceState::Existing,
        source: PropertyPlanSource::QualificationCase,
        id: spec.id.clone(),
        plan: Some(plan),
    }))
}

fn validate_identifier(label: &str, value: &str) -> Result<(), InventoryError> {
    CaseId::try_new(value.to_string())
        .map(|_| ())
        .map_err(|reason| {
            InventoryError::InvalidQualificationCases(format!(
                "{label} {value:?} is invalid: {reason}"
            ))
        })
}

fn validate_text(label: &str, value: &str) -> Result<(), InventoryError> {
    if value.is_empty()
        || value.len() > MAX_LEDGER_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        invalid(format!(
            "{label} must be nonempty, control-free, and at most {MAX_LEDGER_TEXT_BYTES} bytes"
        ))
    } else {
        Ok(())
    }
}

fn invalid<T>(message: String) -> Result<T, InventoryError> {
    Err(InventoryError::InvalidQualificationCases(message))
}

#[cfg(test)]
mod tests;
