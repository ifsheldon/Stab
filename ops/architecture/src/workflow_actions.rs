use std::fs::File;
use std::io::Read;
use std::path::Path;

use yaml_rust2::{Yaml, YamlLoader};

use crate::{CheckError, Violation};

mod release;

const WORKFLOW_DIRECTORY: &str = ".github/workflows";
const MAX_WORKFLOW_BYTES: usize = 1 << 20;

pub(super) struct WorkflowActionReport {
    pub action_use_count: usize,
    pub violations: Vec<Violation>,
}

pub(super) fn scan_workflow_actions(root: &Path) -> Result<WorkflowActionReport, CheckError> {
    let directory = root.join(WORKFLOW_DIRECTORY);
    let entries =
        std::fs::read_dir(&directory).map_err(|source| CheckError::InspectWorkflowDirectory {
            path: directory.clone(),
            source,
        })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| CheckError::InspectWorkflowDirectory {
            path: directory.clone(),
            source,
        })?;
        let path = entry.path();
        if is_workflow_path(&path) {
            paths.push(path);
        }
    }
    paths.sort();

    let mut report = WorkflowActionReport {
        action_use_count: 0,
        violations: Vec::new(),
    };
    for path in paths {
        inspect_workflow(root, &path, &mut report)?;
    }
    Ok(report)
}

fn is_workflow_path(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| matches!(extension, "yml" | "yaml"))
}

fn inspect_workflow(
    root: &Path,
    path: &Path,
    report: &mut WorkflowActionReport,
) -> Result<(), CheckError> {
    inspect_workflow_with_post_open_hook(root, path, report, || {})
}

fn inspect_workflow_with_post_open_hook(
    root: &Path,
    path: &Path,
    report: &mut WorkflowActionReport,
    post_open_hook: impl FnOnce(),
) -> Result<(), CheckError> {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let file = match open_workflow_descriptor(path) {
        Ok(file) => file,
        Err(WorkflowOpenError::Symlink) => {
            report.violations.push(Violation::new(
                "workflow-not-regular-file",
                format!("workflow {} must be a regular file", relative.display()),
            ));
            return Ok(());
        }
        Err(WorkflowOpenError::Io(source)) => {
            return Err(CheckError::ReadWorkflow {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let metadata = file.metadata().map_err(|source| CheckError::ReadWorkflow {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.file_type().is_file() {
        report.violations.push(Violation::new(
            "workflow-not-regular-file",
            format!("workflow {} must be a regular file", relative.display()),
        ));
        return Ok(());
    }

    post_open_hook();
    let mut bytes = Vec::new();
    file.take((MAX_WORKFLOW_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|source| CheckError::ReadWorkflow {
            path: path.to_path_buf(),
            source,
        })?;
    if bytes.len() > MAX_WORKFLOW_BYTES {
        report.violations.push(Violation::new(
            "workflow-too-large",
            format!(
                "workflow {} exceeds the {}-byte architecture-check limit",
                relative.display(),
                MAX_WORKFLOW_BYTES
            ),
        ));
        return Ok(());
    }
    let Ok(source) = std::str::from_utf8(&bytes) else {
        report.violations.push(Violation::new(
            "workflow-invalid-utf8",
            format!("workflow {} is not valid UTF-8", relative.display()),
        ));
        return Ok(());
    };
    inspect_workflow_source(relative, source, report);
    Ok(())
}

enum WorkflowOpenError {
    Symlink,
    Io(std::io::Error),
}

fn open_workflow_descriptor(path: &Path) -> Result<File, WorkflowOpenError> {
    #[cfg(unix)]
    {
        use rustix::fs::{Mode, OFlags};

        let descriptor = rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|source| match source {
            rustix::io::Errno::LOOP => WorkflowOpenError::Symlink,
            source => WorkflowOpenError::Io(std::io::Error::from(source)),
        })?;
        Ok(File::from(descriptor))
    }

    #[cfg(not(unix))]
    {
        Err(WorkflowOpenError::Io(std::io::Error::other(
            "workflow action scanning requires a Unix no-follow open",
        )))
    }
}

fn inspect_workflow_source(path: &Path, source: &str, report: &mut WorkflowActionReport) {
    let documents = match YamlLoader::load_from_str(source) {
        Ok(documents) => documents,
        Err(error) => {
            report.violations.push(Violation::new(
                "workflow-invalid-yaml",
                format!("workflow {} is invalid YAML: {error:?}", path.display()),
            ));
            return;
        }
    };
    let [document] = documents.as_slice() else {
        report.violations.push(Violation::new(
            "workflow-document-count",
            format!(
                "workflow {} must contain exactly one YAML document",
                path.display()
            ),
        ));
        return;
    };
    let Some(root) = document.as_hash() else {
        report.violations.push(Violation::new(
            "workflow-invalid-root",
            format!("workflow {} must have a mapping root", path.display()),
        ));
        return;
    };
    release::inspect(path, root, report);
    let workflow_environment = mapping_value(root, "env");
    if workflow_environment.is_some_and(release_scope_declared) {
        release_secret_violation(path, "workflow env", report);
    }
    if mapping_contains_release_token_expression_except(root, &["env", "jobs"]) {
        release_secret_violation(path, "workflow metadata", report);
    }
    let Some(jobs) = mapping_value(root, "jobs") else {
        return;
    };
    let Some(jobs) = jobs.as_hash() else {
        report.violations.push(Violation::new(
            "workflow-invalid-jobs",
            format!("workflow {} jobs must be a mapping", path.display()),
        ));
        return;
    };

    for (job_id, job) in jobs {
        let job_name = yaml_scalar(job_id).unwrap_or("<non-string-job>");
        let Some(job) = job.as_hash() else {
            continue;
        };
        if mapping_value(job, "env").is_some_and(release_scope_declared) {
            release_secret_violation(path, &format!("job {job_name} env"), report);
        }
        if mapping_contains_release_token_expression_except(job, &["env", "steps"]) {
            release_secret_violation(path, &format!("job {job_name} metadata"), report);
        }
        if let Some(reference) = mapping_value(job, "uses") {
            inspect_action_reference(path, job_name, reference, report);
        }
        let Some(steps) = mapping_value(job, "steps") else {
            continue;
        };
        let Some(steps) = steps.as_vec() else {
            continue;
        };
        for (index, step) in steps.iter().enumerate() {
            let Some(step) = step.as_hash() else {
                continue;
            };
            if let Some(reference) = mapping_value(step, "uses") {
                inspect_action_reference(
                    path,
                    &format!("{job_name}.steps[{index}]"),
                    reference,
                    report,
                );
            }
            let location = format!("{job_name}.steps[{index}]");
            let step_secrets = mapping_value(step, "env")
                .map(release_secrets)
                .unwrap_or_default();
            if step_secrets.any() || mapping_contains_release_token_expression_except(step, &[]) {
                inspect_secret_bearing_step(
                    path,
                    &location,
                    step,
                    step_secrets,
                    release::is_final_draft_step(path, job_name, index, steps.len()),
                    report,
                );
            }
        }
    }
}

#[derive(Clone, Copy, Default)]
struct ReleaseSecrets {
    github_token: bool,
    unexpected: bool,
}

impl ReleaseSecrets {
    fn any(self) -> bool {
        self.github_token || self.unexpected
    }
}

fn release_secrets(value: &Yaml) -> ReleaseSecrets {
    let Some(environment) = value.as_hash() else {
        return ReleaseSecrets::default();
    };
    ReleaseSecrets {
        github_token: mapping_value(environment, "GITHUB_TOKEN").is_some(),
        unexpected: [
            "CARGO_REGISTRY_TOKEN",
            "CARGO_REGISTRIES_CRATES_IO_TOKEN",
            "GH_TOKEN",
        ]
        .iter()
        .any(|key| mapping_value(environment, key).is_some()),
    }
}

fn release_scope_declared(value: &Yaml) -> bool {
    release_secrets(value).any() || yaml_contains_release_token_expression(value)
}

fn mapping_contains_release_token_expression_except(
    mapping: &yaml_rust2::yaml::Hash,
    excluded: &[&str],
) -> bool {
    mapping.iter().any(|(key, value)| {
        let excluded = yaml_scalar(key).is_some_and(|key| excluded.contains(&key));
        !excluded && yaml_contains_release_token_expression(value)
    })
}

fn yaml_contains_release_token_expression(value: &Yaml) -> bool {
    match value {
        Yaml::String(value) => scalar_contains_release_token_expression(value),
        Yaml::Array(values) => values.iter().any(yaml_contains_release_token_expression),
        Yaml::Hash(mapping) => mapping.iter().any(|(key, value)| {
            yaml_contains_release_token_expression(key)
                || yaml_contains_release_token_expression(value)
        }),
        _ => false,
    }
}

fn scalar_contains_release_token_expression(value: &str) -> bool {
    let compact = value
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    [
        "secrets.github_token",
        "secrets['github_token']",
        "secrets[\"github_token\"]",
        "secrets.gh_token",
        "secrets['gh_token']",
        "secrets[\"gh_token\"]",
        "secrets.cargo_registry_token",
        "secrets['cargo_registry_token']",
        "secrets[\"cargo_registry_token\"]",
        "secrets.cargo_registries_crates_io_token",
        "secrets['cargo_registries_crates_io_token']",
        "secrets[\"cargo_registries_crates_io_token\"]",
        "github.token",
    ]
    .iter()
    .any(|needle| compact.contains(needle))
}

fn inspect_secret_bearing_step(
    path: &Path,
    location: &str,
    step: &yaml_rust2::yaml::Hash,
    step_secrets: ReleaseSecrets,
    is_final_draft_step: bool,
    report: &mut WorkflowActionReport,
) {
    let exact_operator = is_final_draft_step
        && step_secrets.github_token
        && !step_secrets.unexpected
        && exact_release_operator_invocation_mapping(step);
    if !exact_operator {
        release_secret_violation(path, &format!("step {location}"), report);
    }
}

fn exact_release_operator_invocation_mapping(step: &yaml_rust2::yaml::Hash) -> bool {
    release::is_exact_operator_invocation(step)
}

fn release_secret_violation(path: &Path, location: &str, report: &mut WorkflowActionReport) {
    report.violations.push(Violation::new(
        "workflow-release-secret-scope",
        format!(
            "workflow {} {location} exposes a release credential outside the exact final draft operator",
            path.display()
        ),
    ));
}

fn mapping_value<'a>(mapping: &'a yaml_rust2::yaml::Hash, key: &str) -> Option<&'a Yaml> {
    mapping.get(&Yaml::String(key.to_owned()))
}

fn yaml_scalar(value: &Yaml) -> Option<&str> {
    match value {
        Yaml::String(value) => Some(value),
        _ => None,
    }
}

fn inspect_action_reference(
    path: &Path,
    location: &str,
    reference: &Yaml,
    report: &mut WorkflowActionReport,
) {
    report.action_use_count += 1;
    let Some(reference) = yaml_scalar(reference) else {
        report.violations.push(Violation::new(
            "workflow-action-invalid-reference",
            format!(
                "workflow {} action at {location} must use a string reference",
                path.display()
            ),
        ));
        return;
    };
    if reference.starts_with("./") {
        return;
    }
    if let Some(image) = reference.strip_prefix("docker://") {
        if immutable_docker_reference(image) {
            return;
        }
        report.violations.push(Violation::new(
            "workflow-action-mutable-ref",
            format!(
                "workflow {} action at {location} uses mutable Docker reference {reference:?}; pin it with @sha256:<64-hex-digest>",
                path.display()
            ),
        ));
        return;
    }
    if immutable_github_action_reference(reference) {
        return;
    }
    report.violations.push(Violation::new(
        "workflow-action-mutable-ref",
        format!(
            "workflow {} action at {location} uses mutable reference {reference:?}; pin remote actions and reusable workflows to a full 40-character commit SHA",
            path.display()
        ),
    ));
}

fn immutable_github_action_reference(reference: &str) -> bool {
    let Some((action, revision)) = reference.rsplit_once('@') else {
        return false;
    };
    let mut components = action.split('/');
    let valid_action = components.next().is_some_and(|part| !part.is_empty())
        && components.next().is_some_and(|part| !part.is_empty())
        && components.all(|part| !part.is_empty());
    valid_action && revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn immutable_docker_reference(reference: &str) -> bool {
    let Some((image, digest)) = reference.rsplit_once('@') else {
        return false;
    };
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return false;
    };
    !image.is_empty() && hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inspect(source: &str) -> WorkflowActionReport {
        inspect_at(Path::new(".github/workflows/test.yml"), source)
    }

    fn inspect_at(path: &Path, source: &str) -> WorkflowActionReport {
        let mut report = WorkflowActionReport {
            action_use_count: 0,
            violations: Vec::new(),
        };
        inspect_workflow_source(path, source, &mut report);
        report
    }

    #[test]
    fn full_commit_refs_are_accepted_for_steps_and_reusable_jobs() {
        let report = inspect(
            r#"
jobs:
  build:
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
  delegated:
    uses: owner/repository/.github/workflows/reuse.yml@ABCDEF0123456789abcdef0123456789ABCDEF01
"#,
        );
        assert_eq!(report.action_use_count, 2);
        assert!(report.violations.is_empty());
    }

    #[test]
    fn mutable_remote_refs_are_rejected() {
        let report = inspect(
            r#"
jobs:
  build:
    steps:
      - uses: actions/checkout@v7
      - uses: owner/action@main
      - uses: owner/action@${{ github.sha }}
      - uses: owner/action@012345678901234567890123456789012345678
"#,
        );
        assert_eq!(report.action_use_count, 4);
        assert_eq!(report.violations.len(), 4);
        assert!(
            report
                .violations
                .iter()
                .all(|violation| violation.code == "workflow-action-mutable-ref")
        );
    }

    #[test]
    fn repository_local_and_digest_pinned_docker_actions_are_accepted() {
        let report = inspect(
            r#"
jobs:
  build:
    steps:
      - uses: ./actions/build
      - uses: docker://example.invalid/tool@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
"#,
        );
        assert_eq!(report.action_use_count, 2);
        assert!(report.violations.is_empty());
    }

    #[test]
    fn mutable_docker_tags_and_non_string_refs_are_rejected() {
        let report = inspect(
            r#"
jobs:
  build:
    steps:
      - uses: docker://alpine:latest
      - uses: 42
"#,
        );
        assert_eq!(report.action_use_count, 2);
        assert_eq!(
            report
                .violations
                .iter()
                .map(|violation| violation.code)
                .collect::<Vec<_>>(),
            [
                "workflow-action-mutable-ref",
                "workflow-action-invalid-reference"
            ]
        );
    }

    #[test]
    fn similarly_named_action_inputs_are_not_treated_as_action_refs() {
        let report = inspect(
            r#"
jobs:
  build:
    steps:
      - uses: owner/action@0123456789abcdef0123456789abcdef01234567
        with:
          uses: mutable-but-not-an-action-reference
"#,
        );
        assert_eq!(report.action_use_count, 1);
        assert!(report.violations.is_empty());
    }

    #[test]
    fn release_credentials_are_rejected_from_cargo_and_action_steps() {
        let report = inspect(
            r#"
jobs:
  draft:
    steps:
      - env:
          GITHUB_TOKEN: secret
        run: cargo run -p stab-release -- create-draft
      - env:
          CARGO_REGISTRY_TOKEN: secret
        uses: owner/action@0123456789abcdef0123456789abcdef01234567
"#,
        );
        assert_eq!(
            report
                .violations
                .iter()
                .filter(|violation| violation.code == "workflow-release-secret-scope")
                .count(),
            2
        );
    }

    #[test]
    fn inherited_release_credentials_are_rejected() {
        for source in [
            r#"
env:
  GITHUB_TOKEN: secret
jobs:
  draft:
    steps:
      - run: ./target/debug/stab-release create-draft --assets target/releases/assets --tag "$RELEASE_TAG" --confirm-version 0.2.0
"#,
            r#"
jobs:
  draft:
    env:
      GITHUB_TOKEN: secret
    steps:
      - run: ./target/debug/stab-release create-draft --assets target/releases/assets --tag "$RELEASE_TAG" --confirm-version 0.2.0
"#,
        ] {
            let report = inspect(source);
            assert_eq!(
                report
                    .violations
                    .iter()
                    .filter(|violation| violation.code == "workflow-release-secret-scope")
                    .count(),
                1
            );
        }
    }

    #[test]
    fn draft_operator_rejects_extra_secrets_and_shell_composition() {
        for source in [
            r#"
jobs:
  draft:
    steps:
      - env:
          GITHUB_TOKEN: secret
          CARGO_REGISTRY_TOKEN: other-secret
        run: ./target/debug/stab-release create-draft --assets target/releases/assets --tag "$RELEASE_TAG" --confirm-version 0.2.0
"#,
            r#"
jobs:
  draft:
    steps:
      - env:
          GITHUB_TOKEN: secret
        run: ./target/debug/stab-release create-draft --assets target/releases/assets --tag "$RELEASE_TAG" --confirm-version 0.2.0; echo leaked
"#,
        ] {
            let report = inspect(source);
            assert_eq!(
                report
                    .violations
                    .iter()
                    .filter(|violation| violation.code == "workflow-release-secret-scope")
                    .count(),
                1
            );
        }
    }

    #[test]
    fn release_credential_aliases_and_inline_expressions_are_rejected() {
        for source in [
            r#"
jobs:
  draft:
    steps:
      - env:
          TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: cargo build
"#,
            r#"
jobs:
  draft:
    steps:
      - run: 'echo ${{ secrets.GITHUB_TOKEN }}'
"#,
            r#"
jobs:
  draft:
    steps:
      - uses: owner/action@0123456789abcdef0123456789abcdef01234567
        with:
          token: ${{ github.token }}
"#,
        ] {
            let report = inspect(source);
            assert_eq!(
                report
                    .violations
                    .iter()
                    .filter(|violation| violation.code == "workflow-release-secret-scope")
                    .count(),
                1
            );
        }
    }

    #[test]
    fn malformed_workflow_yaml_is_rejected() {
        let report = inspect("jobs:\n  build: [\n");
        assert_eq!(report.action_use_count, 0);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(
            report.violations.first().map(|violation| violation.code),
            Some("workflow-invalid-yaml")
        );
    }

    #[cfg(unix)]
    #[test]
    fn workflow_symlink_entries_are_rejected_without_following_target() {
        let repository = tempfile::tempdir().expect("create repository");
        let workflows = repository.path().join(WORKFLOW_DIRECTORY);
        std::fs::create_dir_all(&workflows).expect("create workflow directory");
        let target = repository.path().join("target.yml");
        std::fs::write(
            &target,
            "jobs:\n  build:\n    steps:\n      - uses: actions/checkout@v7\n",
        )
        .expect("write target workflow");
        std::os::unix::fs::symlink(&target, workflows.join("ci.yml"))
            .expect("create workflow symlink");

        let report = scan_workflow_actions(repository.path()).expect("scan workflows");

        assert_eq!(report.action_use_count, 0);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(
            report.violations.first().map(|violation| violation.code),
            Some("workflow-not-regular-file")
        );
    }

    #[cfg(unix)]
    #[test]
    fn workflow_scan_reads_validated_descriptor_after_path_replacement() {
        let repository = tempfile::tempdir().expect("create repository");
        let workflows = repository.path().join(WORKFLOW_DIRECTORY);
        std::fs::create_dir_all(&workflows).expect("create workflow directory");
        let path = workflows.join("ci.yml");
        std::fs::write(
            &path,
            "jobs:\n  build:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1\n",
        )
        .expect("write initial workflow");
        let malicious = repository.path().join("malicious.yml");
        std::fs::write(
            &malicious,
            "jobs:\n  build:\n    steps:\n      - uses: actions/checkout@v7\n",
        )
        .expect("write replacement target");
        let mut report = WorkflowActionReport {
            action_use_count: 0,
            violations: Vec::new(),
        };

        inspect_workflow_with_post_open_hook(repository.path(), &path, &mut report, || {
            std::fs::remove_file(&path).expect("remove original workflow path");
            std::os::unix::fs::symlink(&malicious, &path).expect("replace path with symlink");
        })
        .expect("inspect workflow");

        assert_eq!(report.action_use_count, 1);
        assert!(report.violations.is_empty(), "{:?}", report.violations);
    }

    #[test]
    fn repository_workflows_use_only_immutable_action_refs() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = scan_workflow_actions(&root).expect("scan repository workflows");
        assert_eq!(report.action_use_count, 10);
        assert!(report.violations.is_empty(), "{:?}", report.violations);
    }
}
