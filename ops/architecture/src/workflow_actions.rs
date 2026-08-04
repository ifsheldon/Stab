use std::fs::File;
use std::io::Read;
use std::path::Path;

use yaml_rust2::{Yaml, YamlLoader};

use crate::{CheckError, Violation};

const WORKFLOW_DIRECTORY: &str = ".github/workflows";
const MAX_WORKFLOW_BYTES: usize = 1 << 20;
const DRAFT_OPERATOR_COMMAND: &str = "\"$RELEASE_OPERATOR\" create-draft --assets target/releases/assets --tag \"$RELEASE_TAG\" --confirm-version 0.2.0";
const RELEASE_WORKFLOW_PATH: &str = ".github/workflows/release.yml";
const RELEASE_CHECKOUT_REF: &str = "${{ github.sha }}";
const RELEASE_TAG_INPUT: &str = "${{ inputs.tag }}";
const RELEASE_EVENT_REF: &str = "${{ github.ref }}";
const RELEASE_EVENT_SHA: &str = "${{ github.sha }}";
const RELEASE_OPERATOR_PATH: &str =
    "${{ runner.temp }}/stab-release-operator-${{ github.sha }}/debug/stab-release";
const RELEASE_OPERATOR_TARGET: &str = "${{ runner.temp }}/stab-release-operator-${{ github.sha }}";
const RELEASE_OPERATOR_BUILD_COMMAND: &str = "cargo build -q --locked -p stab-release";
const GITHUB_TOKEN_SECRET: &str = "${{ secrets.GITHUB_TOKEN }}";
const RELEASE_REVISION_COMMAND: &str = concat!(
    "test \"$RELEASE_TAG\" = \"v0.2.0\"\n",
    "test \"$RELEASE_REF\" = \"refs/tags/v0.2.0\"\n",
    "test \"$(git rev-parse HEAD)\" = \"$RELEASE_SHA\"\n",
    "test \"$(git rev-parse \"${RELEASE_TAG}^{commit}\")\" = \"$RELEASE_SHA\"\n",
    "test \"$(git rev-parse \"${RELEASE_REF}^{commit}\")\" = \"$RELEASE_SHA\"",
);

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
    inspect_release_revision_binding(path, root, report);
    let workflow_secrets = mapping_value(root, "env")
        .map(release_secrets)
        .unwrap_or_default();
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
        let inherited_secrets = workflow_secrets.merged(
            mapping_value(job, "env")
                .map(release_secrets)
                .unwrap_or_default(),
        );
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
            if inherited_secrets.any() || step_secrets.any() {
                inspect_secret_bearing_step(
                    path,
                    &location,
                    step,
                    inherited_secrets,
                    step_secrets,
                    report,
                );
            }
        }
    }
}

fn inspect_release_revision_binding(
    path: &Path,
    root: &yaml_rust2::yaml::Hash,
    report: &mut WorkflowActionReport,
) {
    if path != Path::new(RELEASE_WORKFLOW_PATH) {
        return;
    }
    if !exact_release_dispatch(root) {
        release_revision_violation(
            path,
            "must expose only workflow_dispatch with one required string input named tag",
            report,
        );
    }

    let Some(jobs) = mapping_value(root, "jobs").and_then(Yaml::as_hash) else {
        release_revision_violation(path, "must define build and draft jobs", report);
        return;
    };
    for job_name in ["build", "draft"] {
        let Some(job) = mapping_value(jobs, job_name).and_then(Yaml::as_hash) else {
            release_revision_violation(
                path,
                &format!("job {job_name} must exist and contain revision-bound steps"),
                report,
            );
            continue;
        };
        inspect_release_job_revision(path, job_name, job, report);
    }
}

fn exact_release_dispatch(root: &yaml_rust2::yaml::Hash) -> bool {
    let Some(trigger) = mapping_value(root, "on").and_then(Yaml::as_hash) else {
        return false;
    };
    if trigger.len() != 1 {
        return false;
    }
    let Some(dispatch) = mapping_value(trigger, "workflow_dispatch").and_then(Yaml::as_hash) else {
        return false;
    };
    let Some(inputs) = mapping_value(dispatch, "inputs").and_then(Yaml::as_hash) else {
        return false;
    };
    if inputs.len() != 1 {
        return false;
    }
    let Some(tag) = mapping_value(inputs, "tag").and_then(Yaml::as_hash) else {
        return false;
    };
    matches!(mapping_value(tag, "required"), Some(Yaml::Boolean(true)))
        && mapping_value(tag, "type").and_then(yaml_scalar) == Some("string")
        && mapping_value(tag, "default").is_none()
}

fn inspect_release_job_revision(
    path: &Path,
    job_name: &str,
    job: &yaml_rust2::yaml::Hash,
    report: &mut WorkflowActionReport,
) {
    let Some(steps) = mapping_value(job, "steps").and_then(Yaml::as_vec) else {
        release_revision_violation(
            path,
            &format!("job {job_name} must contain release steps"),
            report,
        );
        return;
    };
    let checkout_indices = steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| {
            let step = step.as_hash()?;
            let action = mapping_value(step, "uses").and_then(yaml_scalar)?;
            action.starts_with("actions/checkout@").then_some(index)
        })
        .collect::<Vec<_>>();
    let [checkout_index] = checkout_indices.as_slice() else {
        release_revision_violation(
            path,
            &format!("job {job_name} must contain exactly one checkout"),
            report,
        );
        return;
    };
    let Some(checkout) = steps.get(*checkout_index).and_then(Yaml::as_hash) else {
        release_revision_violation(
            path,
            &format!("job {job_name} has a malformed checkout step"),
            report,
        );
        return;
    };
    let checkout_ref = mapping_value(checkout, "with")
        .and_then(Yaml::as_hash)
        .and_then(|options| mapping_value(options, "ref"))
        .and_then(yaml_scalar);
    if checkout_ref != Some(RELEASE_CHECKOUT_REF) {
        release_revision_violation(
            path,
            &format!("job {job_name} checkout must use immutable github.sha"),
            report,
        );
    }

    let verification = steps.get(checkout_index.saturating_add(1));
    if !verification.is_some_and(exact_release_revision_step) {
        release_revision_violation(
            path,
            &format!(
                "job {job_name} must verify the v0.2.0 input tag and dispatch ref against github.sha immediately after checkout"
            ),
            report,
        );
    }
    if job_name == "draft" && !steps.iter().any(exact_release_operator_build_step) {
        release_revision_violation(
            path,
            "job draft must build the release operator into the exact SHA-scoped runner target",
            report,
        );
    }
}

fn exact_release_operator_build_step(step: &Yaml) -> bool {
    let Some(step) = step.as_hash() else {
        return false;
    };
    if mapping_value(step, "name").and_then(yaml_scalar)
        != Some("Build credential-free release operator")
        || mapping_value(step, "run").and_then(yaml_scalar) != Some(RELEASE_OPERATOR_BUILD_COMMAND)
    {
        return false;
    }
    let Some(environment) = mapping_value(step, "env").and_then(Yaml::as_hash) else {
        return false;
    };
    environment.len() == 1
        && mapping_value(environment, "CARGO_TARGET_DIR").and_then(yaml_scalar)
            == Some(RELEASE_OPERATOR_TARGET)
}

fn exact_release_revision_step(step: &Yaml) -> bool {
    let Some(step) = step.as_hash() else {
        return false;
    };
    if step.len() != 3
        || mapping_value(step, "name").and_then(yaml_scalar)
            != Some("Verify immutable release revision")
        || mapping_value(step, "run")
            .and_then(yaml_scalar)
            .map(str::trim_end)
            != Some(RELEASE_REVISION_COMMAND)
    {
        return false;
    }
    let Some(environment) = mapping_value(step, "env").and_then(Yaml::as_hash) else {
        return false;
    };
    environment.len() == 3
        && mapping_value(environment, "RELEASE_TAG").and_then(yaml_scalar)
            == Some(RELEASE_TAG_INPUT)
        && mapping_value(environment, "RELEASE_REF").and_then(yaml_scalar)
            == Some(RELEASE_EVENT_REF)
        && mapping_value(environment, "RELEASE_SHA").and_then(yaml_scalar)
            == Some(RELEASE_EVENT_SHA)
}

fn release_revision_violation(path: &Path, detail: &str, report: &mut WorkflowActionReport) {
    report.violations.push(Violation::new(
        "workflow-release-revision-binding",
        format!("workflow {} {detail}", path.display()),
    ));
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

    fn merged(self, other: Self) -> Self {
        Self {
            github_token: self.github_token || other.github_token,
            unexpected: self.unexpected || other.unexpected,
        }
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

fn inspect_secret_bearing_step(
    path: &Path,
    location: &str,
    step: &yaml_rust2::yaml::Hash,
    inherited_secrets: ReleaseSecrets,
    step_secrets: ReleaseSecrets,
    report: &mut WorkflowActionReport,
) {
    let command = mapping_value(step, "run").and_then(yaml_scalar);
    let exact_environment = mapping_value(step, "env")
        .and_then(Yaml::as_hash)
        .is_some_and(|environment| {
            environment.len() == 3
                && mapping_value(environment, "GITHUB_TOKEN").and_then(yaml_scalar)
                    == Some(GITHUB_TOKEN_SECRET)
                && mapping_value(environment, "RELEASE_OPERATOR").and_then(yaml_scalar)
                    == Some(RELEASE_OPERATOR_PATH)
                && mapping_value(environment, "RELEASE_TAG").and_then(yaml_scalar)
                    == Some(RELEASE_TAG_INPUT)
        });
    let exact_operator = !inherited_secrets.any()
        && step_secrets.github_token
        && !step_secrets.unexpected
        && exact_environment
        && mapping_value(step, "uses").is_none()
        && command == Some(DRAFT_OPERATOR_COMMAND);
    if !exact_operator {
        report.violations.push(Violation::new(
            "workflow-release-secret-scope",
            format!(
                "workflow {} step {location} exposes an explicit release-token environment variable outside the prebuilt draft operator",
                path.display()
            ),
        ));
    }
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

    const REVISION_BOUND_RELEASE_WORKFLOW: &str = r#"
on:
  workflow_dispatch:
    inputs:
      tag:
        required: true
        type: string
jobs:
  build:
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          ref: ${{ github.sha }}
          fetch-depth: 0
          persist-credentials: false
      - name: Verify immutable release revision
        env:
          RELEASE_TAG: ${{ inputs.tag }}
          RELEASE_REF: ${{ github.ref }}
          RELEASE_SHA: ${{ github.sha }}
        run: |
          test "$RELEASE_TAG" = "v0.2.0"
          test "$RELEASE_REF" = "refs/tags/v0.2.0"
          test "$(git rev-parse HEAD)" = "$RELEASE_SHA"
          test "$(git rev-parse "${RELEASE_TAG}^{commit}")" = "$RELEASE_SHA"
          test "$(git rev-parse "${RELEASE_REF}^{commit}")" = "$RELEASE_SHA"
  draft:
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          ref: ${{ github.sha }}
          fetch-depth: 0
          persist-credentials: false
      - name: Verify immutable release revision
        env:
          RELEASE_TAG: ${{ inputs.tag }}
          RELEASE_REF: ${{ github.ref }}
          RELEASE_SHA: ${{ github.sha }}
        run: |
          test "$RELEASE_TAG" = "v0.2.0"
          test "$RELEASE_REF" = "refs/tags/v0.2.0"
          test "$(git rev-parse HEAD)" = "$RELEASE_SHA"
          test "$(git rev-parse "${RELEASE_TAG}^{commit}")" = "$RELEASE_SHA"
          test "$(git rev-parse "${RELEASE_REF}^{commit}")" = "$RELEASE_SHA"
      - name: Build credential-free release operator
        env:
          CARGO_TARGET_DIR: ${{ runner.temp }}/stab-release-operator-${{ github.sha }}
        run: cargo build -q --locked -p stab-release
"#;

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
    fn release_workflow_accepts_exact_tag_dispatch_bound_to_event_sha() {
        let report = inspect_at(
            Path::new(".github/workflows/release.yml"),
            REVISION_BOUND_RELEASE_WORKFLOW,
        );

        assert!(report.violations.is_empty(), "{:?}", report.violations);
    }

    #[test]
    fn release_workflow_rejects_default_branch_dispatch_guard() {
        let source = REVISION_BOUND_RELEASE_WORKFLOW.replace(
            "test \"$RELEASE_REF\" = \"refs/tags/v0.2.0\"",
            "test \"$RELEASE_REF\" = \"refs/heads/main\"",
        );
        let report = inspect_at(Path::new(".github/workflows/release.yml"), &source);

        assert!(
            report
                .violations
                .iter()
                .any(|violation| violation.code == "workflow-release-revision-binding")
        );
    }

    #[test]
    fn release_workflow_rejects_mutable_tag_checkout() {
        let source = REVISION_BOUND_RELEASE_WORKFLOW
            .replace("ref: ${{ github.sha }}", "ref: ${{ inputs.tag }}");
        let report = inspect_at(Path::new(".github/workflows/release.yml"), &source);

        assert!(
            report
                .violations
                .iter()
                .any(|violation| violation.code == "workflow-release-revision-binding")
        );
    }

    #[test]
    fn release_workflow_rejects_shared_operator_target() {
        let source = REVISION_BOUND_RELEASE_WORKFLOW.replace(
            "${{ runner.temp }}/stab-release-operator-${{ github.sha }}",
            "target",
        );
        let report = inspect_at(Path::new(".github/workflows/release.yml"), &source);

        assert!(
            report
                .violations
                .iter()
                .any(|violation| violation.code == "workflow-release-revision-binding")
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
    fn release_credential_is_accepted_only_for_the_prebuilt_draft_operator() {
        let report = inspect(
            r#"
jobs:
  draft:
    steps:
      - env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          RELEASE_OPERATOR: ${{ runner.temp }}/stab-release-operator-${{ github.sha }}/debug/stab-release
          RELEASE_TAG: ${{ inputs.tag }}
        run: '"$RELEASE_OPERATOR" create-draft --assets target/releases/assets --tag "$RELEASE_TAG" --confirm-version 0.2.0'
"#,
        );
        assert!(report.violations.is_empty(), "{:?}", report.violations);
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
