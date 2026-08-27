use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use super::inventory;
use super::model::{
    BehavioralSurface, Comparator, EvidenceSelector, EvidenceState, EvidenceStatus, FeatureId,
    QualificationManifest, ResourceKind, SCHEMA_VERSION, SelectorKind,
};
use crate::blocker_ledger::selector::CargoTestSelector;

const STIM_VERSION: &str = "v1.16.0";
const STIM_COMMIT: &str = "e2fc1eca7fd21684d433aa5f10f4504ea4860d07";
const RUST_TOOLCHAIN: &str = "nightly-2026-06-20";
const HISTORICAL_PYTHON_AST_VERSION: &str = "3.14.6";
const MAX_EVIDENCE_CASES: usize = 128;
const MAX_TEXT_BYTES: usize = 2_048;
const MAX_VALIDATION_ISSUES: usize = 256;

#[derive(Default)]
pub(super) struct ValidationIssues {
    messages: Vec<String>,
    omitted: usize,
}

impl ValidationIssues {
    pub(super) fn push(&mut self, message: String) {
        if self.messages.len() < MAX_VALIDATION_ISSUES {
            self.messages.push(message);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }

    fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    fn render(mut self) -> Box<str> {
        if self.omitted != 0 {
            self.messages.push(format!(
                "{} additional validation issues omitted",
                self.omitted
            ));
        }
        self.messages.join("\n").into_boxed_str()
    }
}

#[derive(Debug, Error)]
pub(crate) enum ValidationError {
    #[error("failed to compute qualification semantic digest: {0}")]
    Digest(#[from] inventory::InventoryError),

    #[error("qualification manifest validation failed:\n{0}")]
    Violations(Box<str>),
}

pub(super) fn validate(
    manifest: &QualificationManifest,
    expected_frozen_digest: &str,
) -> Result<(), ValidationError> {
    let mut violations = ValidationIssues::default();
    validate_header(manifest, &mut violations);
    validate_features(manifest, &mut violations);
    validate_bridge(manifest, &mut violations);
    if !violations.is_empty() {
        return Err(ValidationError::Violations(violations.render()));
    }
    validate_digest(manifest, expected_frozen_digest)
}

fn validate_header(manifest: &QualificationManifest, violations: &mut ValidationIssues) {
    for (label, actual, expected) in [
        ("stim_version", manifest.stim_version.as_str(), STIM_VERSION),
        ("stim_commit", manifest.stim_commit.as_str(), STIM_COMMIT),
        (
            "rust_toolchain",
            manifest.rust_toolchain.as_str(),
            RUST_TOOLCHAIN,
        ),
        (
            "python_ast_version",
            manifest.python_ast_version.as_str(),
            HISTORICAL_PYTHON_AST_VERSION,
        ),
    ] {
        if actual != expected {
            violations.push(format!("{label} is {actual:?}, expected {expected:?}"));
        }
    }
    if manifest.schema_version != SCHEMA_VERSION {
        violations.push(format!(
            "schema_version is {}, expected {SCHEMA_VERSION}",
            manifest.schema_version
        ));
    }
    if manifest.evidence_cases.is_empty() || manifest.evidence_cases.len() > MAX_EVIDENCE_CASES {
        violations.push(format!(
            "benchmark correctness bridge has {} cases; expected 1..={MAX_EVIDENCE_CASES}",
            manifest.evidence_cases.len()
        ));
    }
}

fn validate_features(manifest: &QualificationManifest, violations: &mut ValidationIssues) {
    let expected = FeatureId::ALL
        .into_iter()
        .map(|id| {
            (
                id,
                id.performance_groups()
                    .iter()
                    .map(|group| (*group).to_string())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let actual = manifest
        .features
        .iter()
        .map(|feature| (feature.id, feature.performance_groups.clone()))
        .collect::<Vec<_>>();
    if actual != expected {
        violations.push("feature-to-performance-group bridge is incomplete or stale".to_string());
    }
}

fn validate_bridge(manifest: &QualificationManifest, violations: &mut ValidationIssues) {
    if !manifest.upstream_cases.is_empty()
        || !manifest.public_api_items.is_empty()
        || !manifest.public_api_aliases.is_empty()
        || !manifest.canonical_owner_exceptions.is_empty()
    {
        violations.push(
            "benchmark correctness bridge must not contain feature, API, or parent ownership inventories"
                .to_string(),
        );
    }

    let mut ids = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    let mut selectors = BTreeMap::<&EvidenceSelector, &str>::new();
    let mut previous = None;
    for case in &manifest.evidence_cases {
        if !ids.insert(case.id.as_str()) {
            violations.push(format!("duplicate evidence case id {:?}", case.id));
        }
        if !source_ids.insert(case.source_id.as_str()) {
            violations.push(format!(
                "duplicate benchmark prerequisite source id {:?}",
                case.source_id
            ));
        }
        if previous.is_some_and(|previous: &str| previous > case.id.as_str()) {
            violations.push("evidence cases are not sorted by id".to_string());
        }
        previous = Some(case.id.as_str());

        if case.provenance != super::model::EvidenceProvenance::QualificationPlan
            || case.status != EvidenceStatus::Implemented
            || !case.supporting_selectors.is_empty()
            || case.statistical_plan.is_some()
            || case.property_plan.is_some()
            || case.deferred_product.is_some()
        {
            violations.push(format!(
                "benchmark correctness bridge case {:?} contains non-prerequisite qualification state",
                case.id
            ));
        }
        if matches!(
            case.comparator,
            Comparator::Statistical | Comparator::Property
        ) {
            violations.push(format!(
                "benchmark correctness bridge case {:?} needs a separate statistical or property plan",
                case.id
            ));
        }
        validate_text("evidence source id", &case.source_id, violations);
        validate_selector(case.id.as_str(), &case.primary_selector, violations);
        if let Some(previous_case) = selectors.insert(&case.primary_selector, case.id.as_str()) {
            violations.push(format!(
                "evidence cases {previous_case:?} and {:?} share primary selector",
                case.id
            ));
        }
        super::execution_contract::validate(case, violations);

        let expected_surface = behavioral_surface(case.feature_id);
        if case.behavioral_surface != expected_surface {
            violations.push(format!(
                "evidence case {:?} behavioral surface is {:?}, expected {:?}",
                case.id, case.behavioral_surface, expected_surface
            ));
        }
        validate_text(
            "resource contract detail",
            &case.resource_contract.detail,
            violations,
        );
        if case.resource_contract.detail.len() < 20 {
            violations.push(format!(
                "evidence case {:?} resource contract is under-specified",
                case.id
            ));
        }
        let mut axes = BTreeSet::new();
        for axis in &case.negative_axes {
            validate_text("negative axis", axis, violations);
            if !axes.insert(axis) {
                violations.push(format!(
                    "evidence case {:?} repeats negative axis {axis:?}",
                    case.id
                ));
            }
        }
        if case.negative_axes.is_empty()
            && case.resource_contract.kind != ResourceKind::NotApplicable
        {
            violations.push(format!("evidence case {:?} has no negative axes", case.id));
        }
        if !case.negative_axes.is_empty()
            && case.resource_contract.kind == ResourceKind::NotApplicable
        {
            violations.push(format!(
                "evidence case {:?} claims negative axes without a resource contract",
                case.id
            ));
        }
        let expected_groups = case
            .feature_id
            .performance_groups()
            .iter()
            .map(|group| (*group).to_string())
            .collect::<Vec<_>>();
        if case.performance_groups != expected_groups {
            violations.push(format!(
                "evidence case {:?} performance groups are stale",
                case.id
            ));
        }
    }
}

fn validate_selector(
    case_id: &str,
    selector: &EvidenceSelector,
    violations: &mut ValidationIssues,
) {
    if selector.state != EvidenceState::Existing || selector.kind != SelectorKind::CargoTest {
        violations.push(format!(
            "evidence case {case_id:?} must use an existing Cargo test selector"
        ));
        return;
    }
    for token in &selector.value {
        validate_text("selector token", token, violations);
    }
    match CargoTestSelector::parse(&selector.value) {
        Ok(parsed) if !parsed.is_exact() => violations.push(format!(
            "evidence case {case_id:?} Cargo selector is not exact"
        )),
        Ok(_) => {}
        Err(reason) => {
            violations.push(format!("evidence case {case_id:?} Cargo selector {reason}"));
        }
    }
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

fn validate_digest(
    manifest: &QualificationManifest,
    expected_frozen_digest: &str,
) -> Result<(), ValidationError> {
    let computed = inventory::semantic_digest(manifest)?;
    let mut violations = ValidationIssues::default();
    if manifest.semantic_digest != computed {
        violations.push(format!(
            "semantic_digest is {}, computed {}",
            manifest.semantic_digest, computed
        ));
    }
    if expected_frozen_digest != "UNFROZEN" {
        let expected =
            super::model::SemanticDigest::parse(expected_frozen_digest).map_err(|reason| {
                ValidationError::Violations(
                    format!("frozen semantic digest {reason}").into_boxed_str(),
                )
            })?;
        if manifest.semantic_digest != expected {
            violations.push(format!(
                "semantic_digest is {}, expected frozen {}",
                manifest.semantic_digest, expected
            ));
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(ValidationError::Violations(violations.render()))
    }
}

fn validate_text(label: &str, value: &str, violations: &mut ValidationIssues) {
    if value.trim().is_empty() {
        violations.push(format!("{label} must not be empty"));
    }
    if value.len() > MAX_TEXT_BYTES {
        violations.push(format!(
            "{label} is {} bytes; limit is {MAX_TEXT_BYTES}",
            value.len()
        ));
    }
    if value.chars().any(char::is_control) {
        violations.push(format!("{label} contains control characters"));
    }
}

#[cfg(test)]
#[path = "validation/tests.rs"]
mod tests;
