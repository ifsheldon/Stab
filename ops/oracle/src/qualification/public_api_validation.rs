use std::collections::BTreeSet;

use super::model::{PublicApiKind, QualificationManifest};
use super::validation::{
    ValidationIssues, validate_identifier, validate_relative_path, validate_text,
};

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
}
