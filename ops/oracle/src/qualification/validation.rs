use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use thiserror::Error;

use super::inventory;
use super::model::{
    BehavioralSurface, EvidenceCase, EvidenceProvenance, EvidenceSelector, EvidenceState,
    EvidenceStatus, FeatureId, Parameterization, PublicApiKind, QualificationManifest,
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
    validate_public_api_items(manifest, &mut violations);
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
        validate_selector(case.id.as_str(), &case.primary_selector, violations);
        if let Some(previous_case) = selectors.insert(&case.primary_selector, case.id.as_str()) {
            violations.push(format!(
                "evidence cases {:?} and {:?} share primary selector",
                previous_case, case.id
            ));
        }
        for selector in &case.supporting_selectors {
            validate_selector(case.id.as_str(), selector, violations);
        }
        super::statistical_validation::validate(case, violations);
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

fn validate_public_api_items(manifest: &QualificationManifest, violations: &mut ValidationIssues) {
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut previous = None;
    for item in &manifest.public_api_items {
        validate_identifier("public API item", item.id.as_str(), violations);
        if !ids.insert(item.id.as_str()) {
            violations.push(format!("duplicate public API item id {:?}", item.id));
        }
        let key = (&item.crate_name, &item.path, item.kind);
        if previous.is_some_and(|previous| previous > key) {
            violations.push("public API items are not in deterministic path order".to_string());
        }
        previous = Some(key);
        if !paths.insert((item.crate_name.as_str(), item.path.as_str(), item.kind)) {
            violations.push(format!("duplicate public API path {:?}", item.path));
        }
        if item.kind == PublicApiKind::Module {
            violations.push(format!(
                "public API module {:?} is a namespace and must map through behavioral items",
                item.path
            ));
        }
        if item
            .path
            .as_str()
            .split("::")
            .any(|component| component.starts_with("__"))
        {
            violations.push(format!(
                "public API item {:?} leaks an evidence-only export",
                item.path
            ));
        }
        validate_text("public API crate", &item.crate_name, violations);
        validate_text("public API path", item.path.as_str(), violations);
        validate_relative_path(
            "public API source path",
            item.source_path.as_path(),
            violations,
        );
        if item.source_line == 0 {
            violations.push(format!("public API item {:?} has line zero", item.id));
        }
        if !item
            .path
            .as_str()
            .starts_with(&format!("{}::", item.crate_name))
        {
            violations.push(format!(
                "public API path {:?} is not rooted at crate {:?}",
                item.path, item.crate_name
            ));
        }
        let expected_groups = item
            .feature_id
            .performance_groups()
            .iter()
            .map(|group| (*group).to_string())
            .collect::<Vec<_>>();
        if item.performance_groups != expected_groups {
            violations.push(format!(
                "public API item {:?} performance groups are stale",
                item.id
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
                if case.primary_selector.kind != SelectorKind::OracleFixture
                    || case.primary_selector.value.as_slice() != [case.source_id.as_str()]
                {
                    violations.push(format!(
                        "oracle evidence case {:?} selector does not name source fixture {:?}",
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
                if case.status != EvidenceStatus::Planned
                    || case.primary_selector.kind != SelectorKind::PropertyTarget
                {
                    violations.push(format!(
                        "qualification plan case {:?} is not a planned property target",
                        case.id
                    ));
                }
            }
            EvidenceProvenance::BlockerLedger => {
                if case.primary_selector.kind != SelectorKind::OpsCheck
                    || case.primary_selector.value.as_slice()
                        != ["blocker-ledger", case.source_id.as_str()]
                {
                    violations.push(format!(
                        "blocker evidence case {:?} selector does not name source case {:?}",
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
            Ok(parsed) if !parsed.is_exact() => violations.push(format!(
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
        SelectorKind::PropertyTarget => {}
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

fn validate_identifier(label: &str, value: &str, violations: &mut ValidationIssues) {
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

fn validate_relative_path(label: &str, path: &Path, violations: &mut ValidationIssues) {
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
mod tests {
    use std::sync::OnceLock;

    use serde_json::{Value, json};

    use super::*;
    use crate::qualification::model::{ApiPath, CaseId, Comparator, SemanticDigest};

    static REPOSITORY_MANIFEST: OnceLock<QualificationManifest> = OnceLock::new();

    #[test]
    fn repository_manifest_passes_structural_validation() {
        let manifest = repository_manifest();
        validate(&manifest, super::super::EXPECTED_FROZEN_DIGEST)
            .expect("repository manifest must validate");
    }

    #[test]
    fn validation_rejects_shared_primary_selectors() {
        let mut manifest = repository_manifest();
        let selector = manifest
            .evidence_cases
            .first()
            .expect("first evidence case")
            .primary_selector
            .clone();
        manifest
            .evidence_cases
            .get_mut(1)
            .expect("second evidence case")
            .primary_selector = selector;
        refresh_digest(&mut manifest);
        let error = validate(&manifest, "UNFROZEN").expect_err("shared selector must fail");
        assert!(error.to_string().contains("share primary selector"));
    }

    #[test]
    fn validation_rejects_duplicate_ids() {
        let mut manifest = repository_manifest();
        let id = manifest
            .evidence_cases
            .first()
            .expect("first evidence case")
            .id
            .clone();
        manifest.evidence_cases.get_mut(1).expect("second case").id = id;
        refresh_digest(&mut manifest);
        let error = validate(&manifest, "UNFROZEN").expect_err("duplicate id must fail");
        assert!(error.to_string().contains("duplicate evidence case id"));
    }

    #[test]
    fn validation_rejects_duplicate_upstream_anchors() {
        let mut manifest = repository_manifest();
        let first = manifest
            .upstream_cases
            .first()
            .expect("first upstream case")
            .clone();
        let second = manifest
            .upstream_cases
            .get_mut(1)
            .expect("second upstream case");
        second.path = first.path;
        second.symbol = first.symbol;
        second.subcase = first.subcase;
        refresh_digest(&mut manifest);
        let error = validate(&manifest, "UNFROZEN")
            .expect_err("duplicate upstream source anchor must fail");
        assert!(error.to_string().contains("duplicate upstream source case"));
    }

    #[test]
    fn validation_rejects_unsafe_upstream_path() {
        let mut value = serde_json::to_value(repository_manifest()).expect("serialize manifest");
        let path = value
            .get_mut("upstream_cases")
            .and_then(Value::as_array_mut)
            .and_then(|cases| cases.first_mut())
            .and_then(Value::as_object_mut)
            .and_then(|case| case.get_mut("path"))
            .expect("upstream path");
        *path = json!("../escape.test.cc");
        let error = serde_json::from_value::<QualificationManifest>(value)
            .expect_err("unsafe path must fail during deserialization");
        assert!(error.to_string().contains("source path must be"));
    }

    #[test]
    fn validation_rejects_stale_public_api_owner() {
        let mut manifest = repository_manifest();
        manifest
            .public_api_items
            .first_mut()
            .expect("public API item")
            .owner_case_id = CaseId::try_new("missing-owner".to_string()).expect("valid test id");
        refresh_digest(&mut manifest);
        let error = validate(&manifest, "UNFROZEN").expect_err("stale owner must fail");
        assert!(error.to_string().contains("references missing owner"));
    }

    #[test]
    fn public_api_ownership_is_component_delimited() {
        assert!(api_path_is_owned_by(
            "stab_core::Foo",
            "stab_core::Foo::new"
        ));
        assert!(api_path_is_owned_by(
            "stab_core::Foo",
            "stab_core::Foo as Clone for@0123456789ab"
        ));
        assert!(!api_path_is_owned_by("stab_core::Foo", "stab_core::Foobar"));
    }

    #[test]
    fn validation_rejects_evidence_only_public_api_leak() {
        let mut manifest = repository_manifest();
        let item = manifest
            .public_api_items
            .first_mut()
            .expect("public API item");
        item.path = ApiPath::try_new(format!("{}::__ops_contract", item.path))
            .expect("valid test API path");
        refresh_digest(&mut manifest);
        let error = validate(&manifest, "UNFROZEN").expect_err("API leak must fail");
        assert!(error.to_string().contains("evidence-only export"));
    }

    #[test]
    fn manifest_schema_denies_unknown_fields() {
        let mut value = serde_json::to_value(repository_manifest()).expect("serialize manifest");
        value
            .as_object_mut()
            .expect("manifest object")
            .insert("unexpected".to_string(), json!(true));
        let error = serde_json::from_value::<QualificationManifest>(value)
            .expect_err("unknown field must fail");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn manifest_schema_rejects_missing_required_fields() {
        let mut value = serde_json::to_value(repository_manifest()).expect("serialize manifest");
        value
            .as_object_mut()
            .expect("manifest object")
            .remove("upstream_cases");
        let error = serde_json::from_value::<QualificationManifest>(value)
            .expect_err("missing required field must fail");
        assert!(error.to_string().contains("missing field `upstream_cases`"));
    }

    #[test]
    fn manifest_schema_rejects_unknown_upstream_disposition() {
        let mut value = serde_json::to_value(repository_manifest()).expect("serialize manifest");
        let disposition = value
            .get_mut("upstream_cases")
            .and_then(Value::as_array_mut)
            .and_then(|cases| cases.first_mut())
            .and_then(Value::as_object_mut)
            .and_then(|case| case.get_mut("disposition"))
            .expect("upstream disposition");
        *disposition = json!("invented-disposition");
        let error = serde_json::from_value::<QualificationManifest>(value)
            .expect_err("unknown disposition must fail");
        assert!(error.to_string().contains("unknown variant"));
    }

    #[test]
    fn validation_rejects_deferred_case_without_named_product() {
        let mut manifest = repository_manifest();
        let case = manifest
            .upstream_cases
            .iter_mut()
            .find(|case| case.disposition == UpstreamDisposition::DeferredProduct)
            .expect("deferred upstream case");
        case.deferred_product = None;
        refresh_digest(&mut manifest);
        let error =
            validate(&manifest, "UNFROZEN").expect_err("deferred case without product must fail");
        assert!(error.to_string().contains("does not name its product"));
    }

    #[test]
    fn manifest_schema_rejects_invalid_typed_ids_and_api_paths() {
        let mut invalid_id =
            serde_json::to_value(repository_manifest()).expect("serialize manifest");
        *invalid_id
            .get_mut("evidence_cases")
            .and_then(Value::as_array_mut)
            .and_then(|cases| cases.first_mut())
            .and_then(Value::as_object_mut)
            .and_then(|case| case.get_mut("id"))
            .expect("evidence id") = json!("Not Valid");
        let error = serde_json::from_value::<QualificationManifest>(invalid_id)
            .expect_err("invalid typed case id must fail");
        assert!(error.to_string().contains("lowercase kebab-case"));

        let mut invalid_path =
            serde_json::to_value(repository_manifest()).expect("serialize manifest");
        *invalid_path
            .get_mut("public_api_items")
            .and_then(Value::as_array_mut)
            .and_then(|items| items.first_mut())
            .and_then(Value::as_object_mut)
            .and_then(|item| item.get_mut("path"))
            .expect("public API path") = json!("");
        let error = serde_json::from_value::<QualificationManifest>(invalid_path)
            .expect_err("invalid typed API path must fail");
        assert!(error.to_string().contains("API path must be nonempty"));
    }

    #[test]
    fn manifest_schema_rejects_oversized_sequences_and_invalid_digest_shape() {
        let mut oversized =
            serde_json::to_value(repository_manifest()).expect("serialize manifest");
        let features = oversized
            .get_mut("features")
            .and_then(Value::as_array_mut)
            .expect("feature rows");
        let first_feature = features.first().expect("feature row").clone();
        features.push(first_feature);
        let error = serde_json::from_value::<QualificationManifest>(oversized)
            .expect_err("oversized bounded sequence must fail during deserialization");
        assert!(error.to_string().contains("more than 16 entries"));

        let mut bad_digest =
            serde_json::to_value(repository_manifest()).expect("serialize manifest");
        *bad_digest
            .get_mut("semantic_digest")
            .expect("semantic digest") = json!("A".repeat(64));
        let error = serde_json::from_value::<QualificationManifest>(bad_digest)
            .expect_err("uppercase digest must fail during deserialization");
        assert!(error.to_string().contains("lowercase hexadecimal"));
    }

    #[test]
    fn manifest_schema_rejects_oversized_text() {
        let mut value = serde_json::to_value(repository_manifest()).expect("serialize manifest");
        *value
            .get_mut("upstream_cases")
            .and_then(Value::as_array_mut)
            .and_then(|cases| cases.first_mut())
            .and_then(Value::as_object_mut)
            .and_then(|case| case.get_mut("reason"))
            .expect("upstream reason") = json!("x".repeat(2_049));
        let error = serde_json::from_value::<QualificationManifest>(value)
            .expect_err("oversized bounded text must fail during deserialization");
        assert!(error.to_string().contains("2048-byte"));
    }

    #[test]
    fn validation_issue_rendering_is_bounded() {
        let mut issues = ValidationIssues::default();
        for index in 0..300 {
            issues.push(format!("issue {index}"));
        }
        let rendered = issues.render();
        assert_eq!(rendered.lines().count(), MAX_VALIDATION_ISSUES + 1);
        assert!(rendered.ends_with("44 additional validation issues omitted"));
    }

    #[test]
    fn validation_requires_owned_statistical_plans() {
        let mut manifest = repository_manifest();
        let case = manifest
            .evidence_cases
            .iter_mut()
            .find(|case| case.comparator == Comparator::Statistical)
            .expect("statistical evidence case");
        case.statistical_plan = None;
        refresh_digest(&mut manifest);
        let error =
            validate(&manifest, "UNFROZEN").expect_err("statistical case without plan must fail");
        assert!(error.to_string().contains("has no statistical plan owner"));
    }

    #[test]
    fn validation_requires_the_complete_resource_case_inventory() {
        let mut manifest = repository_manifest();
        let index = manifest
            .evidence_cases
            .iter()
            .position(|case| case.source_id == "resource-streaming-writer-failure")
            .expect("planned resource case");
        manifest.evidence_cases.remove(index);
        refresh_digest(&mut manifest);
        let error = validate(&manifest, "UNFROZEN")
            .expect_err("missing resource boundary family must fail");
        assert!(
            error
                .to_string()
                .contains("CQ-RESOURCE source-owned case inventory")
        );
    }

    #[test]
    fn validation_rejects_behavioral_surface_and_resource_overclaims() {
        let mut manifest = repository_manifest();
        let case = manifest
            .evidence_cases
            .iter_mut()
            .find(|case| case.provenance == EvidenceProvenance::PublicRustApi)
            .expect("public API evidence case");
        case.behavioral_surface = BehavioralSurface::Engine;
        case.negative_axes.push("unowned-overflow".to_string());
        refresh_digest(&mut manifest);
        let error =
            validate(&manifest, "UNFROZEN").expect_err("surface and resource overclaims must fail");
        let message = error.to_string();
        assert!(message.contains("behavioral surface"));
        assert!(message.contains("negative axes without a resource contract"));
    }

    #[test]
    fn manifest_schema_rejects_unknown_feature_id() {
        let mut value = serde_json::to_value(repository_manifest()).expect("serialize manifest");
        let feature_id = value
            .get_mut("features")
            .and_then(Value::as_array_mut)
            .and_then(|features| features.first_mut())
            .and_then(Value::as_object_mut)
            .and_then(|feature| feature.get_mut("id"))
            .expect("feature id");
        *feature_id = json!("CQ-UNKNOWN");
        let error = serde_json::from_value::<QualificationManifest>(value)
            .expect_err("unknown feature must fail");
        assert!(error.to_string().contains("unknown variant"));
    }

    #[test]
    fn validation_rejects_duplicate_public_api_paths() {
        let mut manifest = repository_manifest();
        let first = manifest
            .public_api_items
            .first()
            .expect("first public API item")
            .clone();
        let second = manifest
            .public_api_items
            .get_mut(1)
            .expect("second public API item");
        second.crate_name = first.crate_name;
        second.path = first.path;
        second.kind = first.kind;
        refresh_digest(&mut manifest);
        let error = validate(&manifest, "UNFROZEN").expect_err("duplicate API path must fail");
        assert!(error.to_string().contains("duplicate public API path"));
    }

    #[test]
    fn validation_rejects_stale_upstream_owner() {
        let mut manifest = repository_manifest();
        let upstream = manifest
            .upstream_cases
            .iter_mut()
            .find(|case| !case.ownerships.is_empty())
            .expect("owned upstream case");
        upstream
            .ownerships
            .first_mut()
            .expect("upstream ownership")
            .owner_case_id = CaseId::try_new("missing-owner".to_string()).expect("valid test id");
        refresh_digest(&mut manifest);
        let error = validate(&manifest, "UNFROZEN").expect_err("stale owner must fail");
        assert!(error.to_string().contains("references missing owner"));
    }

    #[test]
    fn validation_rejects_feature_without_closed_primary_case() {
        let mut manifest = repository_manifest();
        for case in &mut manifest.evidence_cases {
            if case.feature_id == FeatureId::StimFormat
                && matches!(
                    case.status,
                    EvidenceStatus::Implemented | EvidenceStatus::EvidenceClose
                )
            {
                case.status = EvidenceStatus::Planned;
                case.primary_selector.state = EvidenceState::Planned;
            }
        }
        refresh_digest(&mut manifest);
        let error = validate(&manifest, "UNFROZEN")
            .expect_err("feature without closed primary evidence must fail");
        assert!(
            error
                .to_string()
                .contains("CQ-STIM-FORMAT has no implemented or evidence-close primary case")
        );
    }

    #[test]
    fn validation_rejects_semantic_digest_drift() {
        let mut manifest = repository_manifest();
        manifest.semantic_digest = SemanticDigest::ZERO;
        let error = validate(&manifest, "UNFROZEN").expect_err("digest drift must fail");
        assert!(error.to_string().contains("computed"));
    }

    fn repository_manifest() -> QualificationManifest {
        REPOSITORY_MANIFEST
            .get_or_init(|| {
                let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("..")
                    .join("oracle")
                    .join("qualification-manifest.json");
                let bytes = std::fs::read(path).expect("read repository qualification manifest");
                serde_json::from_slice(&bytes).expect("parse repository qualification manifest")
            })
            .clone()
    }

    fn refresh_digest(manifest: &mut QualificationManifest) {
        manifest.semantic_digest = inventory::semantic_digest(manifest).expect("semantic digest");
    }
}
