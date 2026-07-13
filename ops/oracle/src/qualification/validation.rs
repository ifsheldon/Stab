use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use thiserror::Error;

use super::inventory;
use super::model::{
    BehavioralSurface, Comparator, EvidenceCase, EvidenceProvenance, EvidenceSelector,
    EvidenceState, EvidenceStatus, FeatureId, Parameterization, QualificationManifest,
    SCHEMA_VERSION, SelectorKind, UpstreamDisposition,
};
use crate::blocker_ledger::selector::CargoTestSelector;

const STIM_VERSION: &str = "v1.16.0";
const STIM_COMMIT: &str = "e2fc1eca7fd21684d433aa5f10f4504ea4860d07";
const RUST_TOOLCHAIN: &str = "nightly-2026-06-20";
const PYTHON_AST_VERSION: &str = "3.14.6";
const MAX_UPSTREAM_CASES: usize = 8_192;
const MAX_PUBLIC_API_ITEMS: usize = 8_192;
const MAX_EVIDENCE_CASES: usize = 8_192;
const MAX_TEXT_BYTES: usize = 2_048;
const MAX_IDENTIFIER_BYTES: usize = 128;
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
    validate_upstream_cases(manifest, &mut violations);
    validate_evidence_cases(manifest, &mut violations);
    super::public_api_validation::validate(manifest, &mut violations);
    validate_cross_references(manifest, &mut violations);
    super::resource::validate_inventory(manifest, &mut violations);
    if !violations.is_empty() {
        return Err(ValidationError::Violations(violations.render()));
    }
    validate_digest(manifest, expected_frozen_digest)
}

fn validate_header(manifest: &QualificationManifest, violations: &mut ValidationIssues) {
    if manifest.schema_version != SCHEMA_VERSION {
        violations.push(format!(
            "schema_version is {}, expected {SCHEMA_VERSION}",
            manifest.schema_version
        ));
    }
    if manifest.stim_version != STIM_VERSION {
        violations.push(format!(
            "stim_version is {:?}, expected {STIM_VERSION:?}",
            manifest.stim_version
        ));
    }
    if manifest.stim_commit != STIM_COMMIT {
        violations.push(format!(
            "stim_commit is {:?}, expected {STIM_COMMIT:?}",
            manifest.stim_commit
        ));
    }
    if manifest.rust_toolchain != RUST_TOOLCHAIN {
        violations.push(format!(
            "rust_toolchain is {:?}, expected {RUST_TOOLCHAIN:?}",
            manifest.rust_toolchain
        ));
    }
    if manifest.python_ast_version != PYTHON_AST_VERSION {
        violations.push(format!(
            "python_ast_version is {:?}, expected {PYTHON_AST_VERSION:?}",
            manifest.python_ast_version
        ));
    }
    validate_limit(
        "upstream cases",
        manifest.upstream_cases.len(),
        MAX_UPSTREAM_CASES,
        violations,
    );
    validate_limit(
        "public API items",
        manifest.public_api_items.len(),
        MAX_PUBLIC_API_ITEMS,
        violations,
    );
    validate_limit(
        "evidence cases",
        manifest.evidence_cases.len(),
        MAX_EVIDENCE_CASES,
        violations,
    );
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

fn validate_features(manifest: &QualificationManifest, violations: &mut ValidationIssues) {
    if manifest.features.len() != FeatureId::ALL.len() {
        violations.push(format!(
            "feature inventory has {} rows, expected {}",
            manifest.features.len(),
            FeatureId::ALL.len()
        ));
    }
    let mut seen = BTreeSet::new();
    for feature in &manifest.features {
        if !seen.insert(feature.id) {
            violations.push(format!("duplicate feature {}", feature.id.as_str()));
        }
        let expected = feature
            .id
            .performance_groups()
            .iter()
            .map(|group| (*group).to_string())
            .collect::<Vec<_>>();
        if feature.performance_groups != expected {
            violations.push(format!(
                "feature {} performance groups are stale",
                feature.id.as_str()
            ));
        }
    }
    for feature in FeatureId::ALL {
        if !seen.contains(&feature) {
            violations.push(format!("missing feature {}", feature.as_str()));
        }
    }
}

fn validate_upstream_cases(manifest: &QualificationManifest, violations: &mut ValidationIssues) {
    let mut ids = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    let mut previous = None;
    for case in &manifest.upstream_cases {
        validate_identifier("upstream case", case.id.as_str(), violations);
        if !ids.insert(case.id.as_str()) {
            violations.push(format!("duplicate upstream case id {:?}", case.id));
        }
        let key = (
            &case.path,
            case.line,
            &case.symbol,
            case.subcase.as_deref().unwrap_or(""),
        );
        if previous.is_some_and(|previous| previous > key) {
            violations.push("upstream cases are not in deterministic source order".to_string());
        }
        previous = Some(key);
        if !symbols.insert((
            case.path.as_path(),
            case.symbol.as_str(),
            case.subcase.as_deref(),
        )) {
            violations.push(format!(
                "duplicate upstream source case {} {}",
                case.path, case.symbol
            ));
        }
        validate_relative_path("upstream path", case.path.as_path(), violations);
        validate_text("upstream symbol", &case.symbol, violations);
        validate_text("upstream disposition reason", &case.reason, violations);
        if case.reason.trim().len() < 20 {
            violations.push(format!(
                "upstream case {:?} reason does not explain its disposition",
                case.id
            ));
        }
        if case.line == 0 {
            violations.push(format!("upstream case {:?} has line zero", case.id));
        }
        match (case.parameterization, case.subcase.is_some()) {
            (Parameterization::None, false)
            | (Parameterization::StaticSubcase | Parameterization::DynamicFamily, true) => {}
            _ => violations.push(format!(
                "upstream case {:?} parameterization and subcase disagree",
                case.id
            )),
        }
        let selected = case.disposition.is_executable_scope();
        if selected && case.parameterization == Parameterization::DynamicFamily {
            violations.push(format!(
                "selected upstream case {:?} has an unexpanded dynamic parameter family",
                case.id
            ));
        }
        let mut previous_domain = None;
        let mut domain_ids = BTreeSet::new();
        for domain_id in &case.domain_ids {
            if previous_domain.is_some_and(|previous| previous >= *domain_id) {
                violations.push(format!(
                    "upstream case {:?} domain ids are not strictly sorted",
                    case.id
                ));
            }
            previous_domain = Some(*domain_id);
            domain_ids.insert(*domain_id);
        }
        if selected && case.ownerships.is_empty() {
            violations.push(format!(
                "selected upstream case {:?} lacks domain ownership",
                case.id
            ));
        }
        if !selected && !case.ownerships.is_empty() {
            violations.push(format!(
                "non-executable upstream case {:?} claims executable ownership",
                case.id
            ));
        }
        if case.disposition == UpstreamDisposition::DeferredProduct
            && case.deferred_product.is_none()
        {
            violations.push(format!(
                "deferred upstream case {:?} does not name its product",
                case.id
            ));
        }
        if case.disposition != UpstreamDisposition::DeferredProduct
            && case.deferred_product.is_some()
        {
            violations.push(format!(
                "non-deferred upstream case {:?} names a deferred product",
                case.id
            ));
        }
        let mut features = BTreeSet::new();
        let mut owners = BTreeSet::new();
        for ownership in &case.ownerships {
            if !features.insert(ownership.feature_id) {
                violations.push(format!(
                    "upstream case {:?} repeats feature {}",
                    case.id,
                    ownership.feature_id.as_str()
                ));
            }
            if !owners.insert(ownership.owner_case_id.as_str()) {
                violations.push(format!(
                    "upstream case {:?} repeats owner {:?}",
                    case.id, ownership.owner_case_id
                ));
            }
            validate_identifier(
                "upstream owner case",
                ownership.owner_case_id.as_str(),
                violations,
            );
        }
        if selected && features != domain_ids {
            violations.push(format!(
                "selected upstream case {:?} ownership features do not match its domain ids",
                case.id
            ));
        }
    }
}

fn validate_evidence_cases(manifest: &QualificationManifest, violations: &mut ValidationIssues) {
    let mut ids = BTreeSet::new();
    let mut selectors = BTreeMap::<&EvidenceSelector, &str>::new();
    let mut previous = None;
    let mut feature_counts = BTreeMap::<FeatureId, usize>::new();
    let mut closed_feature_counts = BTreeMap::<FeatureId, usize>::new();
    for case in &manifest.evidence_cases {
        validate_identifier("evidence case", case.id.as_str(), violations);
        if !ids.insert(case.id.as_str()) {
            violations.push(format!("duplicate evidence case id {:?}", case.id));
        }
        if previous.is_some_and(|previous: &str| previous > case.id.as_str()) {
            violations.push("evidence cases are not sorted by id".to_string());
        }
        previous = Some(case.id.as_str());
        *feature_counts.entry(case.feature_id).or_default() += 1;
        if matches!(
            case.status,
            EvidenceStatus::Implemented | EvidenceStatus::EvidenceClose
        ) {
            *closed_feature_counts.entry(case.feature_id).or_default() += 1;
        }
        validate_text("evidence provenance", &case.provenance_label(), violations);
        validate_text("evidence source id", &case.source_id, violations);
        validate_selector(case.id.as_str(), &case.primary_selector, true, violations);
        if let Some(previous_case) = selectors.insert(&case.primary_selector, case.id.as_str()) {
            violations.push(format!(
                "evidence cases {:?} and {:?} share primary selector",
                previous_case, case.id
            ));
        }
        for selector in &case.supporting_selectors {
            validate_selector(case.id.as_str(), selector, false, violations);
        }
        super::statistical_validation::validate(case, violations);
        super::property_validation::validate(case, violations);
        super::execution_contract::validate(case, violations);
        let expected_behavioral_surface = expected_behavioral_surface(case);
        if case.behavioral_surface != expected_behavioral_surface {
            violations.push(format!(
                "evidence case {:?} behavioral surface is {:?}, expected {:?}",
                case.id, case.behavioral_surface, expected_behavioral_surface
            ));
        }
        validate_text(
            "resource contract detail",
            &case.resource_contract.detail,
            violations,
        );
        if case.resource_contract.detail.trim().len() < 20 {
            violations.push(format!(
                "evidence case {:?} resource contract is under-specified",
                case.id
            ));
        }
        if case.negative_axes.is_empty()
            && case.resource_contract.kind != super::model::ResourceKind::NotApplicable
        {
            violations.push(format!("evidence case {:?} has no negative axes", case.id));
        }
        if !case.negative_axes.is_empty()
            && case.resource_contract.kind == super::model::ResourceKind::NotApplicable
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
        if case.status == EvidenceStatus::Deferred && case.deferred_product.is_none() {
            violations.push(format!(
                "deferred evidence case {:?} does not name its deferred product",
                case.id
            ));
        }
        if case.status != EvidenceStatus::Deferred && case.deferred_product.is_some() {
            violations.push(format!(
                "non-deferred evidence case {:?} names a deferred product",
                case.id
            ));
        }
        match (case.status, case.primary_selector.state) {
            (EvidenceStatus::Planned, EvidenceState::Planned)
            | (EvidenceStatus::Implemented, EvidenceState::Existing)
            | (EvidenceStatus::EvidenceClose, EvidenceState::Existing)
            | (EvidenceStatus::Deferred, EvidenceState::NotApplicable) => {}
            _ => violations.push(format!(
                "evidence case {:?} status and primary selector state disagree",
                case.id
            )),
        }
    }
    for feature in FeatureId::ALL {
        if feature_counts.get(&feature).copied().unwrap_or(0) == 0 {
            violations.push(format!(
                "feature {} has no owned evidence case",
                feature.as_str()
            ));
        }
        if closed_feature_counts.get(&feature).copied().unwrap_or(0) == 0 {
            violations.push(format!(
                "feature {} has no implemented or evidence-close primary case",
                feature.as_str()
            ));
        }
    }
}

fn validate_cross_references(manifest: &QualificationManifest, violations: &mut ValidationIssues) {
    let evidence = manifest
        .evidence_cases
        .iter()
        .map(|case| (case.id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    for case in &manifest.upstream_cases {
        for ownership in &case.ownerships {
            let owner_id = ownership.owner_case_id.as_str();
            match evidence.get(owner_id) {
                Some(owner)
                    if owner.feature_id == ownership.feature_id
                        && owner.comparator == ownership.comparator
                        && ((owner.provenance == EvidenceProvenance::UpstreamSemanticCase
                            && owner.source_id == case.id.as_str())
                            || (owner.provenance == EvidenceProvenance::BlockerLedger
                                && case.disposition == UpstreamDisposition::PortedRust)) => {}
                Some(_) => violations.push(format!(
                    "upstream case {:?} owner {:?} has mismatched surface, feature, comparator, or source",
                    case.id, owner_id
                )),
                None => violations.push(format!(
                    "upstream case {:?} references missing owner {:?}",
                    case.id, owner_id
                )),
            }
        }
    }
    for item in &manifest.public_api_items {
        match evidence.get(item.owner_case_id.as_str()) {
            Some(owner)
                if owner.provenance == EvidenceProvenance::PublicRustApi
                    && owner.feature_id == item.feature_id
                    && api_path_is_owned_by(&owner.source_id, item.path.as_str()) => {}
            Some(owner) => violations.push(format!(
                "public API item {:?} path {:?} feature {} owner {:?} has surface {:?}, feature {}, and source {:?}",
                item.id,
                item.path,
                item.feature_id.as_str(),
                item.owner_case_id,
                owner.provenance,
                owner.feature_id.as_str(),
                owner.source_id
            )),
            None => violations.push(format!(
                "public API item {:?} references missing owner {:?}",
                item.id, item.owner_case_id
            )),
        }
    }
    let referenced = manifest
        .upstream_cases
        .iter()
        .flat_map(|case| {
            case.ownerships
                .iter()
                .map(|ownership| ownership.owner_case_id.as_str())
        })
        .chain(
            manifest
                .public_api_items
                .iter()
                .map(|item| item.owner_case_id.as_str()),
        )
        .collect::<BTreeSet<_>>();
    for case in &manifest.evidence_cases {
        if matches!(
            case.provenance,
            EvidenceProvenance::UpstreamSemanticCase | EvidenceProvenance::PublicRustApi
        ) && !referenced.contains(case.id.as_str())
        {
            violations.push(format!(
                "evidence case {:?} is not referenced by its source inventory",
                case.id
            ));
        }
        match case.provenance {
            EvidenceProvenance::OracleFixture => {
                let source_marker = std::iter::once(&case.primary_selector)
                    .chain(case.supporting_selectors.iter())
                    .any(|selector| {
                        selector.kind == SelectorKind::OracleFixture
                            && selector.value.as_slice() == [case.source_id.as_str()]
                    });
                if !source_marker
                    || !matches!(
                        case.primary_selector.kind,
                        SelectorKind::OracleFixture | SelectorKind::CargoTest
                    )
                {
                    violations.push(format!(
                        "oracle evidence case {:?} selectors do not bind source fixture {:?} to an executable terminal selector",
                        case.id, case.source_id
                    ));
                }
            }
            EvidenceProvenance::RustRegression => {
                if case.primary_selector.kind != SelectorKind::CargoTest {
                    violations.push(format!(
                        "Rust regression case {:?} does not use a Cargo test selector",
                        case.id
                    ));
                }
            }
            EvidenceProvenance::QualificationPlan => {
                let valid_status = match case.status {
                    EvidenceStatus::Planned => {
                        case.primary_selector.state == EvidenceState::Planned
                    }
                    EvidenceStatus::Implemented | EvidenceStatus::EvidenceClose => {
                        case.primary_selector.state == EvidenceState::Existing
                            && case.comparator == Comparator::Property
                    }
                    EvidenceStatus::Deferred => false,
                };
                if !valid_status || case.primary_selector.kind != SelectorKind::PropertyTarget {
                    violations.push(format!(
                        "qualification plan case {:?} is not a status-consistent property target",
                        case.id
                    ));
                }
            }
            EvidenceProvenance::BlockerLedger => {
                let source_marker = case.supporting_selectors.iter().any(|selector| {
                    selector.kind == SelectorKind::OpsCheck
                        && selector.value.as_slice() == ["blocker-ledger", case.source_id.as_str()]
                });
                if case.primary_selector.kind != SelectorKind::CargoTest || !source_marker {
                    violations.push(format!(
                        "blocker evidence case {:?} selectors do not bind source case {:?} to its terminal Cargo selector",
                        case.id, case.source_id
                    ));
                }
            }
            EvidenceProvenance::UpstreamSemanticCase | EvidenceProvenance::PublicRustApi => {}
        }
    }
}

fn expected_behavioral_surface(case: &EvidenceCase) -> BehavioralSurface {
    match case.provenance {
        EvidenceProvenance::PublicRustApi => BehavioralSurface::RustApi,
        EvidenceProvenance::RustRegression => BehavioralSurface::ResourceBoundary,
        EvidenceProvenance::QualificationPlan => BehavioralSurface::ResourceBoundary,
        EvidenceProvenance::OracleFixture => case.behavioral_surface,
        EvidenceProvenance::UpstreamSemanticCase | EvidenceProvenance::BlockerLedger => {
            match case.feature_id {
                FeatureId::Cli => BehavioralSurface::Cli,
                FeatureId::StimFormat | FeatureId::DemFormat | FeatureId::ResultFormats => {
                    BehavioralSurface::FileFormat
                }
                FeatureId::Resource => BehavioralSurface::ResourceBoundary,
                _ => BehavioralSurface::Engine,
            }
        }
    }
}

fn validate_selector(
    case_id: &str,
    selector: &EvidenceSelector,
    primary: bool,
    violations: &mut ValidationIssues,
) {
    if selector.value.is_empty() {
        violations.push(format!("evidence case {case_id:?} has empty selector"));
        return;
    }
    for token in &selector.value {
        validate_text("selector token", token, violations);
    }
    match selector.kind {
        SelectorKind::CargoTest => match CargoTestSelector::parse(&selector.value) {
            Ok(parsed) if primary && !parsed.is_exact() => violations.push(format!(
                "evidence case {case_id:?} Cargo selector is not exact"
            )),
            Ok(_) => {}
            Err(reason) => {
                violations.push(format!("evidence case {case_id:?} Cargo selector {reason}"))
            }
        },
        SelectorKind::OracleFixture => {
            if selector.value.len() != 1 || !selector.value.iter().all(|value| is_identifier(value))
            {
                violations.push(format!(
                    "evidence case {case_id:?} oracle selector must contain one fixture id"
                ));
            }
        }
        SelectorKind::OpsCheck => {
            if selector.value.len() != 2
                || selector.value.first().map(String::as_str) != Some("blocker-ledger")
                || !selector
                    .value
                    .get(1)
                    .is_some_and(|value| is_identifier(value))
            {
                violations.push(format!(
                    "evidence case {case_id:?} ops selector is not an exact blocker-ledger reference"
                ));
            }
        }
        SelectorKind::PropertyTarget => {
            let [target] = selector.value.as_slice() else {
                violations.push(format!(
                    "evidence case {case_id:?} property selector must contain one target id"
                ));
                return;
            };
            if !is_identifier(target) {
                violations.push(format!(
                    "evidence case {case_id:?} property selector must contain one target id"
                ));
            } else if selector.state == EvidenceState::Existing
                && !super::property::is_registered_target(target)
            {
                violations.push(format!(
                    "evidence case {case_id:?} property target {:?} is not registered",
                    target
                ));
            }
        }
    }
    if selector.state == EvidenceState::NotApplicable && selector.value != ["not-applicable"] {
        violations.push(format!(
            "evidence case {case_id:?} not-applicable selector has executable tokens"
        ));
    }
}

fn api_path_is_owned_by(owner: &str, item: &str) -> bool {
    item == owner
        || item
            .strip_prefix(owner)
            .is_some_and(|suffix| suffix.starts_with("::") || suffix.starts_with(" as "))
}

pub(super) fn validate_identifier(label: &str, value: &str, violations: &mut ValidationIssues) {
    let valid = !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if !valid {
        violations.push(format!("{label} id {value:?} is not lowercase kebab-case"));
    }
}

pub(super) fn validate_text(label: &str, value: &str, violations: &mut ValidationIssues) {
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

pub(super) fn validate_relative_path(label: &str, path: &Path, violations: &mut ValidationIssues) {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        violations.push(format!("{label} {path:?} is not a safe relative path"));
    }
}

fn validate_limit(label: &str, actual: usize, limit: usize, violations: &mut ValidationIssues) {
    if actual > limit {
        violations.push(format!("{label} has {actual} rows; limit is {limit}"));
    }
}

impl EvidenceCase {
    fn provenance_label(&self) -> String {
        match self.provenance {
            EvidenceProvenance::UpstreamSemanticCase => "upstream semantic case".to_string(),
            EvidenceProvenance::PublicRustApi => "public Rust API".to_string(),
            EvidenceProvenance::OracleFixture => "oracle fixture".to_string(),
            EvidenceProvenance::RustRegression => "Rust regression".to_string(),
            EvidenceProvenance::BlockerLedger => "blocker ledger".to_string(),
            EvidenceProvenance::QualificationPlan => "qualification plan".to_string(),
        }
    }
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[cfg(test)]
#[path = "validation/tests.rs"]
mod tests;
