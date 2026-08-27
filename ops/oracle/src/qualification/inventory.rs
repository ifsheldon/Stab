use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::model::{
    BehavioralSurface, CaseId, Comparator, EvidenceCase, EvidenceProvenance, EvidenceSelector,
    EvidenceState, EvidenceStatus, FeatureId, FeatureRecord, QualificationManifest,
    ResourceContract, SCHEMA_VERSION, SelectorKind, SemanticDigest, StableCaseDomain,
};
use crate::RepoRoot;
use crate::blocker_ledger::selector::CargoTestSelector;

mod case_id;

pub(super) use case_id::stable_id;

const BRIDGE_SCHEMA_VERSION: u32 = 4;
const RUST_TOOLCHAIN: &str = "nightly-2026-06-20";
// Kept in schema 6 until the transitional manifest is retired with the inherited benchmark system.
const HISTORICAL_PYTHON_AST_VERSION: &str = "3.14.6";
const MAX_BRIDGE_BYTES: usize = 16 << 20;
const MAX_BRIDGE_CASES: usize = 128;
const MAX_TEXT_BYTES: usize = 2_048;

#[derive(Debug, Error)]
pub(crate) enum InventoryError {
    #[error("pinned Stim source validation failed: {0}")]
    StimSource(Box<str>),

    #[error("failed to read qualification input {path}: {reason}")]
    Read { path: PathBuf, reason: Box<str> },

    #[error("failed to parse qualification case bridge {path}: {source}")]
    ParseBridge {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("qualification case bridge is invalid: {0}")]
    InvalidBridge(String),

    #[error("failed to serialize qualification semantic payload: {0}")]
    Serialize(serde_json::Error),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BridgeLedger {
    schema_version: u32,
    stim_version: String,
    stim_commit: String,
    cases: Vec<BridgeCase>,
    #[serde(default)]
    existing_parent_mappings: Vec<serde_json::Value>,
    #[serde(default)]
    public_api_aliases: Vec<serde_json::Value>,
    #[serde(default)]
    canonical_owner_exceptions: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BridgeCase {
    id: String,
    feature_id: FeatureId,
    comparator: Comparator,
    primary_selector: EvidenceSelector,
    resource_contract: ResourceContract,
    #[serde(default)]
    negative_axes: Vec<String>,
    standalone: bool,
}

pub(super) fn generate(root: &RepoRoot) -> Result<QualificationManifest, InventoryError> {
    let stim = crate::validate_stim_source(root)
        .map_err(|source| InventoryError::StimSource(source.to_string().into_boxed_str()))?;
    let path = root.qualification_cases();
    let bytes =
        crate::safe_file::read_regular_file_bounded(&path, MAX_BRIDGE_BYTES).map_err(|source| {
            InventoryError::Read {
                path: path.clone(),
                reason: source.to_string().into_boxed_str(),
            }
        })?;
    let ledger = parse_bridge(&path, &bytes)?;
    build_manifest(ledger, &stim.tag, &stim.commit)
}

pub(super) fn semantic_digest(
    manifest: &QualificationManifest,
) -> Result<SemanticDigest, InventoryError> {
    let mut payload = manifest.clone();
    payload.semantic_digest = SemanticDigest::ZERO;
    let bytes = serde_json::to_vec(&payload).map_err(InventoryError::Serialize)?;
    Ok(SemanticDigest::from_bytes(Sha256::digest(bytes).into()))
}

fn parse_bridge(path: &Path, bytes: &[u8]) -> Result<BridgeLedger, InventoryError> {
    serde_json::from_slice(bytes).map_err(|source| InventoryError::ParseBridge {
        path: path.to_path_buf(),
        source,
    })
}

fn build_manifest(
    ledger: BridgeLedger,
    stim_version: &str,
    stim_commit: &str,
) -> Result<QualificationManifest, InventoryError> {
    validate_header(&ledger, stim_version, stim_commit)?;

    let mut source_ids = BTreeSet::new();
    let mut stable_ids = BTreeSet::new();
    let mut evidence_cases = Vec::with_capacity(ledger.cases.len());
    for case in ledger.cases {
        validate_case(&case)?;
        if !source_ids.insert(case.id.clone()) {
            return invalid(format!("case source id {:?} is duplicated", case.id));
        }
        let id = stable_id(StableCaseDomain::EvidenceQualification, &case.id);
        if !stable_ids.insert(id.clone()) {
            return invalid(format!("case id collision for source id {:?}", case.id));
        }
        evidence_cases.push(EvidenceCase {
            id,
            feature_id: case.feature_id,
            behavioral_surface: behavioral_surface(case.feature_id),
            provenance: EvidenceProvenance::QualificationPlan,
            source_id: case.id,
            comparator: case.comparator,
            execution: super::execution_contract::for_status(EvidenceStatus::Implemented),
            statistical_plan: None,
            property_plan: None,
            primary_selector: case.primary_selector,
            supporting_selectors: Vec::new(),
            resource_contract: case.resource_contract,
            negative_axes: case.negative_axes,
            performance_groups: case
                .feature_id
                .performance_groups()
                .iter()
                .map(|group| (*group).to_string())
                .collect(),
            deferred_product: None,
            status: EvidenceStatus::Implemented,
        });
    }
    evidence_cases.sort_by(|left, right| left.id.cmp(&right.id));
    super::execution_contract::assign_pr_tiers(&mut evidence_cases);

    let features = FeatureId::ALL
        .into_iter()
        .map(|id| FeatureRecord {
            id,
            performance_groups: id
                .performance_groups()
                .iter()
                .map(|group| (*group).to_string())
                .collect(),
        })
        .collect();
    let mut manifest = QualificationManifest {
        schema_version: SCHEMA_VERSION,
        stim_version: ledger.stim_version,
        stim_commit: ledger.stim_commit,
        rust_toolchain: RUST_TOOLCHAIN.to_string(),
        python_ast_version: HISTORICAL_PYTHON_AST_VERSION.to_string(),
        semantic_digest: SemanticDigest::ZERO,
        features,
        upstream_cases: Vec::new(),
        public_api_items: Vec::new(),
        public_api_aliases: Vec::new(),
        canonical_owner_exceptions: Vec::new(),
        evidence_cases,
    };
    manifest.semantic_digest = semantic_digest(&manifest)?;
    Ok(manifest)
}

fn validate_header(
    ledger: &BridgeLedger,
    stim_version: &str,
    stim_commit: &str,
) -> Result<(), InventoryError> {
    if ledger.schema_version != BRIDGE_SCHEMA_VERSION {
        return invalid(format!(
            "schema version {} does not match {BRIDGE_SCHEMA_VERSION}",
            ledger.schema_version
        ));
    }
    if ledger.stim_version != stim_version || ledger.stim_commit != stim_commit {
        return invalid(format!(
            "Stim pin {}@{} does not match {stim_version}@{stim_commit}",
            ledger.stim_version, ledger.stim_commit
        ));
    }
    if ledger.cases.is_empty() || ledger.cases.len() > MAX_BRIDGE_CASES {
        return invalid(format!(
            "case count {} is outside 1..={MAX_BRIDGE_CASES}",
            ledger.cases.len()
        ));
    }
    if !ledger.existing_parent_mappings.is_empty()
        || !ledger.public_api_aliases.is_empty()
        || !ledger.canonical_owner_exceptions.is_empty()
    {
        return invalid(
            "retired feature, API, and parent ownership fields must remain empty".to_string(),
        );
    }
    Ok(())
}

fn validate_case(case: &BridgeCase) -> Result<(), InventoryError> {
    CaseId::try_new(case.id.clone())
        .map_err(|reason| InventoryError::InvalidBridge(format!("case {:?}: {reason}", case.id)))?;
    if !case.standalone {
        return invalid(format!(
            "case {:?} must be a standalone benchmark prerequisite",
            case.id
        ));
    }
    if matches!(
        case.comparator,
        Comparator::Statistical | Comparator::Property
    ) {
        return invalid(format!(
            "case {:?} requires a separate plan and cannot enter the compact bridge",
            case.id
        ));
    }
    validate_text("resource contract", &case.resource_contract.detail)?;
    let mut axes = BTreeSet::new();
    for axis in &case.negative_axes {
        validate_text("negative axis", axis)?;
        if !axes.insert(axis) {
            return invalid(format!("case {:?} repeats negative axis {axis:?}", case.id));
        }
    }
    if case.primary_selector.state != EvidenceState::Existing
        || case.primary_selector.kind != SelectorKind::CargoTest
    {
        return invalid(format!(
            "case {:?} must use an existing Cargo test selector",
            case.id
        ));
    }
    let selector = CargoTestSelector::parse(&case.primary_selector.value).map_err(|reason| {
        InventoryError::InvalidBridge(format!("case {:?} Cargo selector {reason}", case.id))
    })?;
    if !selector.is_exact() {
        return invalid(format!("case {:?} Cargo selector is not exact", case.id));
    }
    Ok(())
}

fn validate_text(label: &str, value: &str) -> Result<(), InventoryError> {
    if value.is_empty() || value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return invalid(format!(
            "{label} must be nonempty, control-free, and at most {MAX_TEXT_BYTES} bytes"
        ));
    }
    Ok(())
}

const fn behavioral_surface(feature_id: FeatureId) -> BehavioralSurface {
    match feature_id {
        FeatureId::Cli => BehavioralSurface::Cli,
        FeatureId::StimFormat | FeatureId::DemFormat | FeatureId::ResultFormats => {
            BehavioralSurface::FileFormat
        }
        FeatureId::Resource => BehavioralSurface::ResourceBoundary,
        _ => BehavioralSurface::Engine,
    }
}

fn invalid<T>(message: String) -> Result<T, InventoryError> {
    Err(InventoryError::InvalidBridge(message))
}

#[cfg(test)]
#[path = "inventory/tests.rs"]
mod tests;
