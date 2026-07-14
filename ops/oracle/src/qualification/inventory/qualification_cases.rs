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
    UpstreamCase,
};

const LEDGER_SCHEMA_VERSION: u32 = 1;
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
struct PublicApiOwnerSpec {
    crate_name: String,
    owner_path: ApiPath,
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
        for owner in &spec.upstream_owners {
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
                &spec,
                &old_owner,
                EvidenceProvenance::UpstreamSemanticCase,
                evidence_cases,
                &mut claimed_evidence,
            )?;
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
                &spec,
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
                &spec,
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

fn claim_planned_evidence(
    spec: &QualificationCaseSpec,
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
                spec.id, evidence_id
            ))
        })?;
    if case.status != EvidenceStatus::Planned
        || case.provenance != provenance
        || case.feature_id != spec.feature_id
        || case.comparator != spec.comparator
    {
        return invalid(format!(
            "qualification case {:?} cannot claim {} with {:?}/{:?}/{:?}/{:?}",
            spec.id, evidence_id, case.status, case.provenance, case.feature_id, case.comparator
        ));
    }
    if !claimed.insert(evidence_id.clone()) {
        return invalid(format!(
            "qualification case {:?} repeats or steals evidence {}",
            spec.id, evidence_id
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
    fn claiming_evidence_rejects_wrong_feature_comparator_and_duplicate_owner() {
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
            &spec,
            &id,
            EvidenceProvenance::UpstreamSemanticCase,
            &evidence,
            &mut claimed,
        )
        .expect("first claim");
        assert!(
            claim_planned_evidence(
                &spec,
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
                &wrong,
                &id,
                EvidenceProvenance::UpstreamSemanticCase,
                &evidence,
                &mut BTreeSet::new(),
            )
            .is_err()
        );
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
            public_api_owners: Vec::new(),
            oracle_fixture_owners: Vec::new(),
            static_property_plan: None,
            standalone: true,
        }
    }
}
