use std::collections::{BTreeMap, BTreeSet};

use super::model::{EvidenceStatus, PublicApiKind, QualificationManifest, SelectorKind};
use super::validation::{
    ValidationIssues, validate_identifier, validate_relative_path, validate_text,
};
use crate::blocker_ledger::selector::CargoTestSelector;

const CANONICAL_OWNER_PACKAGES: [(&str, &str); 6] = [
    ("stab_bits", "stab-bits"),
    ("stab_records", "stab-records"),
    ("stab_algebra", "stab-algebra"),
    ("stab_model", "stab-model"),
    ("stab_analysis", "stab-analysis"),
    ("stab_engine", "stab-engine"),
];

pub(super) fn validate(manifest: &QualificationManifest, violations: &mut ValidationIssues) {
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
    validate_canonical_owner_packages(manifest, violations);
}

fn validate_canonical_owner_packages(
    manifest: &QualificationManifest,
    violations: &mut ValidationIssues,
) {
    let evidence = manifest
        .evidence_cases
        .iter()
        .map(|case| (case.id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let mut exceptions = BTreeMap::new();
    let mut previous = None;
    for exception in &manifest.canonical_owner_exceptions {
        let key = (
            exception.crate_name.as_str(),
            exception.owner_source_id.as_str(),
        );
        if previous.is_some_and(|previous| previous > key) {
            violations
                .push("canonical owner exceptions are not in deterministic order".to_string());
        }
        previous = Some(key);
        validate_text(
            "canonical owner exception crate",
            &exception.crate_name,
            violations,
        );
        validate_identifier(
            "canonical owner exception source",
            &exception.owner_source_id,
            violations,
        );
        validate_text(
            "canonical owner exception package",
            &exception.evidence_package,
            violations,
        );
        validate_text(
            "canonical owner exception reason",
            &exception.reason,
            violations,
        );
        if exception.reason.trim().len() < 40 {
            violations.push(format!(
                "canonical owner exception {:?}/{:?} has an under-specified reason",
                exception.crate_name, exception.owner_source_id
            ));
        }
        let Some((_, expected_package)) = CANONICAL_OWNER_PACKAGES
            .iter()
            .find(|(crate_name, _)| *crate_name == exception.crate_name)
        else {
            violations.push(format!(
                "canonical owner exception {:?}/{:?} names a crate without direct-package enforcement",
                exception.crate_name, exception.owner_source_id
            ));
            continue;
        };
        if exception.evidence_package == *expected_package {
            violations.push(format!(
                "canonical owner exception {:?}/{:?} is stale because it names the canonical package",
                exception.crate_name, exception.owner_source_id
            ));
        }
        if exceptions.insert(key, exception).is_some() {
            violations.push(format!(
                "duplicate canonical owner exception {:?}/{:?}",
                exception.crate_name, exception.owner_source_id
            ));
        }
    }

    let mut owner_pairs = BTreeSet::new();
    let mut used_exceptions = BTreeSet::new();
    for item in &manifest.public_api_items {
        let Some((_, expected_package)) = CANONICAL_OWNER_PACKAGES
            .iter()
            .find(|(crate_name, _)| *crate_name == item.crate_name)
        else {
            continue;
        };
        if !owner_pairs.insert((item.crate_name.as_str(), item.owner_case_id.as_str())) {
            continue;
        }
        let Some(owner) = evidence.get(item.owner_case_id.as_str()) else {
            continue;
        };
        if owner.status != EvidenceStatus::Implemented {
            continue;
        }
        if owner.primary_selector.kind != SelectorKind::CargoTest {
            violations.push(format!(
                "implemented canonical owner {:?}/{:?} uses a non-Cargo primary selector",
                item.crate_name, owner.source_id
            ));
            continue;
        }
        let Ok(selector) = CargoTestSelector::parse(&owner.primary_selector.value) else {
            continue;
        };
        if selector.package() == *expected_package {
            continue;
        }

        let key = (item.crate_name.as_str(), owner.source_id.as_str());
        match exceptions.get(&key) {
            Some(exception) if exception.evidence_package == selector.package() => {
                used_exceptions.insert(key);
            }
            Some(exception) => violations.push(format!(
                "canonical owner exception {:?}/{:?} allows package {:?}, but selector executes {:?}",
                exception.crate_name,
                exception.owner_source_id,
                exception.evidence_package,
                selector.package()
            )),
            None => violations.push(format!(
                "implemented canonical owner {:?}/{:?} executes package {:?} instead of {:?}",
                item.crate_name,
                owner.source_id,
                selector.package(),
                expected_package
            )),
        }
    }

    for key in exceptions.keys() {
        if !used_exceptions.contains(key) {
            violations.push(format!(
                "canonical owner exception {:?}/{:?} is stale or owns no implemented API case",
                key.0, key.1
            ));
        }
    }
}
