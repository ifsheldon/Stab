//! Repository-local `uses: ./path` step actions.
//!
//! A local action runs repository code with the job's credentials, so its
//! `action.yml` or `action.yaml` metadata is held to the same rules as a
//! workflow: pinned remote actions, no persisted checkout credentials, and no
//! release-credential expressions. Composite steps may use further local
//! actions; each action directory is inspected once.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use yaml_rust2::{Yaml, YamlLoader};

use super::{
    WorkflowActionReport, immutable_docker_reference, inspect_step_action, mapping_value,
    read_bounded_regular_file, release_secret_violation, yaml_contains_release_token_expression,
    yaml_scalar,
};
use crate::{CheckError, Violation};

const METADATA_FILE_NAMES: [&str; 2] = ["action.yml", "action.yaml"];
/// Bounds composite-action fan-out from hostile metadata.
const MAX_LOCAL_ACTIONS: usize = 256;

#[derive(Clone, Debug)]
pub(super) struct LocalActionUse {
    source: PathBuf,
    location: String,
    reference: String,
}

impl LocalActionUse {
    pub(super) fn new(source: &Path, location: &str, reference: &str) -> Self {
        Self {
            source: source.to_path_buf(),
            location: location.to_owned(),
            reference: reference.to_owned(),
        }
    }

    fn violation(&self, code: &'static str, detail: impl std::fmt::Display) -> Violation {
        Violation::new(
            code,
            format!(
                "{} action at {} uses local action {:?}: {detail}",
                self.source.display(),
                self.location,
                self.reference
            ),
        )
    }
}

pub(super) fn inspect_local_actions(
    root: &Path,
    report: &mut WorkflowActionReport,
) -> Result<(), CheckError> {
    let mut inspected = BTreeSet::new();
    while let Some(action) = report.local_actions.pop() {
        let Some(directory) = action_directory(&action, report) else {
            continue;
        };
        if !inspected.insert(directory.clone()) {
            continue;
        }
        if inspected.len() > MAX_LOCAL_ACTIONS {
            report.violations.push(action.violation(
                "workflow-local-action-limit",
                format!("more than {MAX_LOCAL_ACTIONS} local actions are referenced"),
            ));
            report.local_actions.clear();
            break;
        }
        if has_symlink_component(root, &directory, &action, report)? {
            continue;
        }
        let Some(metadata_path) = METADATA_FILE_NAMES
            .iter()
            .map(|name| directory.join(name))
            .find(|path| root.join(path).symlink_metadata().is_ok())
        else {
            report.violations.push(action.violation(
                "workflow-local-action-missing",
                format!(
                    "{} has no action.yml or action.yaml metadata",
                    directory.display()
                ),
            ));
            continue;
        };
        let Some(source) = read_bounded_regular_file(
            root,
            &root.join(&metadata_path),
            "action metadata",
            report,
            || {},
        )?
        else {
            continue;
        };
        inspect_action_source(&metadata_path, &source, report);
    }
    Ok(())
}

/// Resolves `./dir` against the repository root, rejecting anything that is not
/// a plain relative path below it.
fn action_directory(action: &LocalActionUse, report: &mut WorkflowActionReport) -> Option<PathBuf> {
    let mut directory = PathBuf::new();
    for component in Path::new(&action.reference).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(name) => directory.push(name),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                report.violations.push(action.violation(
                    "workflow-local-action-invalid-path",
                    "local action paths must stay below the repository root",
                ));
                return None;
            }
        }
    }
    if directory.as_os_str().is_empty() {
        report.violations.push(action.violation(
            "workflow-local-action-invalid-path",
            "the repository root is not an action directory",
        ));
        return None;
    }
    Some(directory)
}

fn has_symlink_component(
    root: &Path,
    directory: &Path,
    action: &LocalActionUse,
    report: &mut WorkflowActionReport,
) -> Result<bool, CheckError> {
    let mut current = root.to_path_buf();
    for component in directory.components() {
        current.push(component);
        match current.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                report.violations.push(action.violation(
                    "workflow-local-action-symlink",
                    format!("{} is a symlink", current.trim_prefix(root).display()),
                ));
                return Ok(true);
            }
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) | Err(_) => {
                report.violations.push(action.violation(
                    "workflow-local-action-missing",
                    format!("{} is not a directory", current.trim_prefix(root).display()),
                ));
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn inspect_action_source(path: &Path, source: &str, report: &mut WorkflowActionReport) {
    let documents = match YamlLoader::load_from_str(source) {
        Ok(documents) => documents,
        Err(error) => {
            report.violations.push(Violation::new(
                "workflow-invalid-yaml",
                format!(
                    "action metadata {} is invalid YAML: {error:?}",
                    path.display()
                ),
            ));
            return;
        }
    };
    let [document] = documents.as_slice() else {
        report.violations.push(Violation::new(
            "workflow-document-count",
            format!(
                "action metadata {} must contain exactly one YAML document",
                path.display()
            ),
        ));
        return;
    };
    let Some(root) = document.as_hash() else {
        report.violations.push(Violation::new(
            "workflow-invalid-root",
            format!(
                "action metadata {} must have a mapping root",
                path.display()
            ),
        ));
        return;
    };
    // Inputs defaults, step env, and `with` values are all evaluated with the
    // caller's contexts, so any credential expression anywhere is a leak.
    if yaml_contains_release_token_expression(document) {
        release_secret_violation(path, "action metadata", report);
    }
    let Some(runs) = mapping_value(root, "runs").and_then(Yaml::as_hash) else {
        report.violations.push(Violation::new(
            "workflow-local-action-invalid-runs",
            format!(
                "action metadata {} must declare a runs mapping",
                path.display()
            ),
        ));
        return;
    };
    if let Some(image) = mapping_value(runs, "image").and_then(yaml_scalar)
        && let Some(image) = image.strip_prefix("docker://")
        && !immutable_docker_reference(image)
    {
        report.violations.push(Violation::new(
            "workflow-action-mutable-ref",
            format!(
                "action metadata {} uses mutable Docker image {image:?}; pin it with @sha256:<64-hex-digest>",
                path.display()
            ),
        ));
    }
    let Some(steps) = mapping_value(runs, "steps").and_then(Yaml::as_vec) else {
        return;
    };
    for (index, step) in steps.iter().enumerate() {
        if let Some(step) = step.as_hash() {
            inspect_step_action(path, &format!("runs.steps[{index}]"), step, report);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::super::scan_workflow_actions;

    fn codes(root: &std::path::Path) -> BTreeSet<&'static str> {
        scan_workflow_actions(root)
            .expect("scan fixture workflows")
            .violations
            .iter()
            .map(|violation| violation.code)
            // The fixture has no release workflow; that rule is tested elsewhere.
            .filter(|code| *code != "workflow-release-missing")
            .collect()
    }

    #[test]
    fn local_composite_actions_are_held_to_workflow_rules() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/local-action-repository");

        let report = scan_workflow_actions(&root).expect("scan fixture workflows");
        let messages = report
            .violations
            .iter()
            .map(|violation| format!("{}: {}", violation.code, violation.message))
            .collect::<Vec<_>>();

        assert_eq!(
            codes(&root),
            BTreeSet::from([
                "workflow-action-mutable-ref",
                "workflow-checkout-persists-credentials",
                "workflow-local-action-invalid-path",
                "workflow-local-action-missing",
                "workflow-release-secret-scope",
            ]),
            "{messages:#?}"
        );
        // The nested local action is reached through the composite action.
        assert!(messages.iter().any(|message| {
            message.starts_with("workflow-action-mutable-ref")
                && message.contains("actions/nested/action.yaml")
        }));
        assert!(messages.iter().any(|message| {
            message.starts_with("workflow-checkout-persists-credentials")
                && message.contains("actions/build/action.yml")
        }));
        // Workflow steps plus composite and nested action steps.
        assert_eq!(report.action_use_count, 7, "{messages:#?}");
    }
}
