use std::collections::BTreeSet;

use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use super::evidence::behavioral_surface_for_feature;
use super::{InventoryError, MAX_QUALIFICATION_CASES_BYTES, MAX_SOURCE_BYTES, stable_id};
use crate::RepoRoot;
use crate::blocker_ledger::selector::CargoTestSelector;
use crate::qualification::model::{
    ApiPath, CaseId, Comparator, EvidenceCase, EvidenceProvenance, EvidenceSelector, EvidenceState,
    EvidenceStatus, FeatureId, PropertyExecutionMode, PropertyExecutionPlan,
    PropertyPersistencePolicy, PropertyPlanRef, PropertyPlanSource, PublicApiItem,
    RelativeSourcePath, ResourceContract, SelectorKind, SemanticDigest, StableCaseDomain,
    UpstreamCase, UpstreamDisposition,
};

const LEDGER_SCHEMA_VERSION: u32 = 2;
const MAX_LEDGER_CASES: usize = 4_096;
const MAX_OWNERS_PER_CASE: usize = 2_048;
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
) -> Result<(), InventoryError> {
    let ledger = load(root)?;
    validate_header(&ledger, stim_version, stim_commit)?;

    let mut source_ids = BTreeSet::new();
    let mut qualification_ids = BTreeSet::new();
    let mut claimed_evidence = BTreeSet::new();
    let mut qualification_cases = Vec::with_capacity(ledger.cases.len());

    for spec in ledger.cases {
        validate_case_shape(&spec)?;
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
        let upstream_owners = expand_upstream_owners(
            &spec.id,
            &spec.upstream_owners,
            &spec.upstream_word_size_families,
        )?;
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
            let matches = evidence_cases
                .iter()
                .filter(|case| {
                    case.provenance == EvidenceProvenance::PublicRustApi
                        && case.feature_id == spec.feature_id
                        && case.source_id == owner.owner_path.as_str()
                })
                .map(|case| case.id.clone())
                .collect::<Vec<_>>();
            let [old_owner] = matches.as_slice() else {
                return invalid(format!(
                    "qualification case {:?} public API owner {}::{} resolved {} evidence records",
                    spec.id,
                    owner.crate_name,
                    owner.owner_path,
                    matches.len()
                ));
            };
            claim_planned_evidence(
                &spec.id,
                spec.feature_id,
                old_owner,
                EvidenceProvenance::PublicRustApi,
                evidence_cases,
                &mut claimed_evidence,
            )?;
            let mut mapped_items = 0usize;
            for item in public_api_items.iter_mut().filter(|item| {
                item.crate_name == owner.crate_name && item.owner_case_id == *old_owner
            }) {
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
            claim_planned_evidence(
                &spec.id,
                spec.feature_id,
                old_owner,
                EvidenceProvenance::OracleFixture,
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
        validate_existing_parent_mapping_shape(&mapping)?;
        if !source_ids.insert(mapping.id.clone()) {
            return invalid(format!(
                "qualification mapping source id {:?} is duplicated",
                mapping.id
            ));
        }
        apply_existing_parent_mapping(
            &mapping,
            upstream_cases,
            public_api_items,
            evidence_cases,
            &mut claimed_evidence,
        )?;
    }

    evidence_cases.retain(|case| !claimed_evidence.contains(&case.id));
    evidence_cases.extend(qualification_cases);
    Ok(())
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
    Ok(())
}

fn validate_case_shape(spec: &QualificationCaseSpec) -> Result<(), InventoryError> {
    CaseId::try_new(spec.id.clone()).map_err(|reason| {
        InventoryError::InvalidQualificationCases(format!(
            "qualification case source id {:?} is invalid: {reason}",
            spec.id
        ))
    })?;
    let owner_count = spec
        .upstream_owners
        .len()
        .saturating_add(spec.public_api_owners.len())
        .saturating_add(spec.oracle_fixture_owners.len());
    if owner_count > MAX_OWNERS_PER_CASE {
        return invalid(format!(
            "qualification case {:?} has {} owners; limit is {}",
            spec.id, owner_count, MAX_OWNERS_PER_CASE
        ));
    }
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
    Ok(())
}

fn validate_existing_parent_mapping_shape(
    mapping: &ExistingParentMappingSpec,
) -> Result<(), InventoryError> {
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
        &mapping.id,
        &mapping.upstream_owners,
        &mapping.upstream_word_size_families,
    )?;
    let owner_count = upstream_owners
        .len()
        .saturating_add(mapping.public_api_owners.len())
        .saturating_add(mapping.oracle_fixture_owners.len());
    if owner_count == 0 || owner_count > MAX_OWNERS_PER_CASE {
        return invalid(format!(
            "qualification mapping {:?} has {} owners; expected 1..={}",
            mapping.id, owner_count, MAX_OWNERS_PER_CASE
        ));
    }
    Ok(())
}

fn apply_existing_parent_mapping(
    mapping: &ExistingParentMappingSpec,
    upstream_cases: &mut [UpstreamCase],
    public_api_items: &mut [PublicApiItem],
    evidence_cases: &mut [EvidenceCase],
    claimed_evidence: &mut BTreeSet<CaseId>,
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

    let upstream_owners = expand_upstream_owners(
        &mapping.id,
        &mapping.upstream_owners,
        &mapping.upstream_word_size_families,
    )?;
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
        let matches = evidence_cases
            .iter()
            .filter(|case| {
                case.provenance == EvidenceProvenance::PublicRustApi
                    && case.feature_id == mapping.feature_id
                    && case.source_id == owner.owner_path.as_str()
            })
            .map(|case| case.id.clone())
            .collect::<Vec<_>>();
        let [old_owner] = matches.as_slice() else {
            return invalid(format!(
                "qualification mapping {:?} public API owner {}::{} resolved {} evidence records",
                mapping.id,
                owner.crate_name,
                owner.owner_path,
                matches.len()
            ));
        };
        claim_planned_evidence(
            &mapping.id,
            mapping.feature_id,
            old_owner,
            EvidenceProvenance::PublicRustApi,
            evidence_cases,
            claimed_evidence,
        )?;
        let mut mapped_items = 0usize;
        for item in public_api_items
            .iter_mut()
            .filter(|item| item.crate_name == owner.crate_name && item.owner_case_id == *old_owner)
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

fn expand_upstream_owners(
    mapping_id: &str,
    owners: &[UpstreamOwnerSpec],
    families: &[UpstreamWordSizeFamilySpec],
) -> Result<Vec<UpstreamOwnerSpec>, InventoryError> {
    let family_owner_count = families
        .iter()
        .map(|family| family.word_sizes.len())
        .sum::<usize>();
    let mut expanded = Vec::with_capacity(owners.len().saturating_add(family_owner_count));
    for owner in owners {
        expanded.push(UpstreamOwnerSpec {
            path: owner.path.clone(),
            symbol: owner.symbol.clone(),
            subcase: owner.subcase.clone(),
        });
    }
    for family in families {
        validate_text("upstream word-size symbol base", &family.symbol_base)?;
        if family.word_sizes.is_empty() {
            return invalid(format!(
                "qualification case {:?} has an empty upstream word-size family for {}:{}",
                mapping_id, family.path, family.symbol_base
            ));
        }
        let mut seen_sizes = BTreeSet::new();
        for word_size in &family.word_sizes {
            if !matches!(word_size, 64 | 128 | 256) || !seen_sizes.insert(*word_size) {
                return invalid(format!(
                    "qualification case {:?} has invalid or duplicate Stim word size {} for {}:{}",
                    mapping_id, word_size, family.path, family.symbol_base
                ));
            }
            expanded.push(UpstreamOwnerSpec {
                path: family.path.clone(),
                symbol: format!("{}_{}", family.symbol_base, word_size),
                subcase: Some(format!("W={word_size}")),
            });
        }
    }
    Ok(expanded)
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
mod tests {
    use super::*;

    #[test]
    fn case_shape_rejects_non_exact_and_cross_mode_selectors() {
        let mut spec = test_spec();
        spec.primary_selector.value.pop();
        assert!(validate_case_shape(&spec).is_err());

        let mut spec = test_spec();
        spec.primary_selector.kind = SelectorKind::OracleFixture;
        assert!(validate_case_shape(&spec).is_err());

        let mut spec = test_spec();
        spec.comparator = Comparator::Statistical;
        assert!(validate_case_shape(&spec).is_err());
    }

    #[test]
    fn claiming_evidence_allows_exact_comparator_refinement_but_rejects_wrong_feature_and_duplicate_owner()
     {
        let spec = test_spec();
        let id = CaseId::try_new("cq-evidence-upstream-test".to_string()).expect("case id");
        let evidence = vec![EvidenceCase {
            id: id.clone(),
            feature_id: FeatureId::StimFormat,
            behavioral_surface: crate::qualification::model::BehavioralSurface::FileFormat,
            provenance: EvidenceProvenance::UpstreamSemanticCase,
            source_id: "source".to_string(),
            comparator: Comparator::Canonical,
            execution: super::super::super::execution_contract::for_status(EvidenceStatus::Planned),
            statistical_plan: None,
            property_plan: None,
            primary_selector: EvidenceSelector {
                state: EvidenceState::Planned,
                kind: SelectorKind::CargoTest,
                value: vec![
                    "cargo".to_string(),
                    "test".to_string(),
                    "-p".to_string(),
                    "stab-core".to_string(),
                    "planned".to_string(),
                    "--quiet".to_string(),
                    "--exact".to_string(),
                ],
            },
            supporting_selectors: Vec::new(),
            resource_contract: super::super::evidence::semantic_only_resource_contract(),
            negative_axes: Vec::new(),
            performance_groups: vec!["PERF-CIRCUIT-MODEL".to_string()],
            deferred_product: None,
            status: EvidenceStatus::Planned,
        }];
        let mut claimed = BTreeSet::new();
        claim_planned_evidence(
            &spec.id,
            spec.feature_id,
            &id,
            EvidenceProvenance::UpstreamSemanticCase,
            &evidence,
            &mut claimed,
        )
        .expect("first claim");
        assert!(
            claim_planned_evidence(
                &spec.id,
                spec.feature_id,
                &id,
                EvidenceProvenance::UpstreamSemanticCase,
                &evidence,
                &mut claimed,
            )
            .is_err()
        );

        let mut wrong = test_spec();
        wrong.feature_id = FeatureId::DemFormat;
        assert!(
            claim_planned_evidence(
                &wrong.id,
                wrong.feature_id,
                &id,
                EvidenceProvenance::UpstreamSemanticCase,
                &evidence,
                &mut BTreeSet::new(),
            )
            .is_err()
        );
    }

    #[test]
    fn existing_parent_mapping_shape_requires_owned_supported_parent_kind() {
        let mapping = ExistingParentMappingSpec {
            id: "cq2-existing-parent-map".to_string(),
            feature_id: FeatureId::GateContract,
            parent: ExistingParentSpec {
                provenance: EvidenceProvenance::BlockerLedger,
                source_id: "pfm3-contract-fixed-tableau".to_string(),
            },
            upstream_owners: vec![UpstreamOwnerSpec {
                path: RelativeSourcePath::try_new(
                    "src/stim/simulators/tableau_simulator.test.cc".into(),
                )
                .expect("path"),
                symbol: "TableauSimulator.unitary_gates_consistent_with_tableau_data_64"
                    .to_string(),
                subcase: None,
            }],
            upstream_word_size_families: Vec::new(),
            public_api_owners: Vec::new(),
            oracle_fixture_owners: Vec::new(),
        };
        validate_existing_parent_mapping_shape(&mapping).expect("valid mapping");

        let mut empty = mapping;
        empty.upstream_owners.clear();
        assert!(validate_existing_parent_mapping_shape(&empty).is_err());
        empty.upstream_owners.push(UpstreamOwnerSpec {
            path: RelativeSourcePath::try_new("src/stim/gates/gates.test.cc".into()).expect("path"),
            symbol: "gate_data.lookup".to_string(),
            subcase: None,
        });
        empty.parent.provenance = EvidenceProvenance::QualificationPlan;
        assert!(validate_existing_parent_mapping_shape(&empty).is_err());
    }

    #[test]
    fn upstream_word_size_families_expand_to_exact_parameterized_owners() {
        let path =
            RelativeSourcePath::try_new("src/stim/simulators/frame_simulator.test.cc".into())
                .expect("path");
        let families = vec![UpstreamWordSizeFamilySpec {
            path: path.clone(),
            symbol_base: "FrameSimulator.noisy_measurement_x".to_string(),
            word_sizes: vec![64, 128, 256],
        }];
        let expanded =
            expand_upstream_owners("cq2-word-size-family", &[], &families).expect("expand");
        assert_eq!(expanded.len(), 3);
        let first = expanded.first().expect("first expanded owner");
        let third = expanded.last().expect("last expanded owner");
        assert_eq!(first.path, path);
        assert_eq!(first.symbol, "FrameSimulator.noisy_measurement_x_64");
        assert_eq!(first.subcase.as_deref(), Some("W=64"));
        assert_eq!(third.symbol, "FrameSimulator.noisy_measurement_x_256");
        assert_eq!(third.subcase.as_deref(), Some("W=256"));

        let duplicate = vec![UpstreamWordSizeFamilySpec {
            path: first.path.clone(),
            symbol_base: "FrameSimulator.noisy_measurement_x".to_string(),
            word_sizes: vec![64, 64],
        }];
        assert!(expand_upstream_owners("cq2-word-size-family", &[], &duplicate).is_err());
    }

    fn test_spec() -> QualificationCaseSpec {
        QualificationCaseSpec {
            id: "cq2-test-case".to_string(),
            feature_id: FeatureId::StimFormat,
            comparator: Comparator::Canonical,
            primary_selector: EvidenceSelector {
                state: EvidenceState::Existing,
                kind: SelectorKind::CargoTest,
                value: vec![
                    "cargo".to_string(),
                    "test".to_string(),
                    "-p".to_string(),
                    "stab-core".to_string(),
                    "--test".to_string(),
                    "stim_format".to_string(),
                    "parses_and_prints_basic_m4_fixture".to_string(),
                    "--quiet".to_string(),
                    "--exact".to_string(),
                ],
            },
            resource_contract: super::super::evidence::semantic_only_resource_contract(),
            negative_axes: Vec::new(),
            upstream_owners: Vec::new(),
            upstream_word_size_families: Vec::new(),
            public_api_owners: Vec::new(),
            oracle_fixture_owners: Vec::new(),
            static_property_plan: None,
            standalone: true,
        }
    }
}
