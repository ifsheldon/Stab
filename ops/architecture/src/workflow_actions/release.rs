use std::path::{Path, PathBuf};

use yaml_rust2::{Yaml, YamlLoader};

use super::{WorkflowActionReport, mapping_value, yaml_scalar};
use crate::Violation;

const RELEASE_WORKFLOW_PATH: &str = ".github/workflows/release.yml";
const REHEARSAL_WORKFLOW_PATH: &str = ".github/workflows/release-rehearsal.yml";
const DRAFT_OPERATOR_COMMAND: &str = "\"$RELEASE_OPERATOR\" create-draft --assets target/releases/assets --tag \"$RELEASE_TAG\" --confirm-version 0.2.0";
const DRAFT_OPERATOR_STEP_NAME: &str = "Verify retained assets and create digest-checked draft";
const GITHUB_TOKEN_SECRET: &str = "${{ secrets.GITHUB_TOKEN }}";
const RELEASE_OPERATOR_PATH: &str =
    "${{ runner.temp }}/stab-release-operator-${{ github.sha }}/debug/stab-release";
const RELEASE_TAG_INPUT: &str = "${{ inputs.tag }}";
const REHEARSAL_DRAFT_OPERATOR_COMMAND: &str = "\"$RELEASE_OPERATOR\" create-draft --assets target/releases/assets --tag \"$RELEASE_TAG\" --confirm-repository ifsheldon/Stab-release-rehearsal";
const REHEARSAL_DRAFT_OPERATOR_STEP_NAME: &str =
    "Verify retained assets and create digest-checked rehearsal draft";
const REQUIRED_WORKFLOW_PATHS: [&str; 2] = [RELEASE_WORKFLOW_PATH, REHEARSAL_WORKFLOW_PATH];
const REHEARSAL_OPERATOR_PATH: &str = "${{ runner.temp }}/stab-release-rehearsal-operator-${{ github.sha }}/debug/stab-release-rehearsal";
const REHEARSAL_TAG: &str = "v0.2.0-rehearsal-${{ github.sha }}";

const EXPECTED_RELEASE_WORKFLOW: &str = r#"name: Release

on:
  workflow_dispatch:
    inputs:
      tag:
        description: Existing annotated release tag to build into a draft release
        required: true
        type: string

permissions:
  contents: read

concurrency:
  group: release-${{ inputs.tag }}
  cancel-in-progress: false

jobs:
  build:
    name: Build stab (${{ matrix.name }})
    strategy:
      fail-fast: false
      matrix:
        include:
          - name: linux-aarch64
            os: ubuntu-24.04-arm
          - name: macos-aarch64
            os: macos-15
    runs-on: ${{ matrix.os }}
    timeout-minutes: 30
    steps:
      - name: Checkout tagged source
        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
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

      - name: Show toolchain
        run: rustup show active-toolchain

      - name: Build and validate tagged binary
        env:
          RELEASE_TAG: ${{ inputs.tag }}
          RELEASE_TARGET: ${{ matrix.name }}
        run: cargo run -q --locked -p stab-release -- build-binary --target "$RELEASE_TARGET" --out "target/releases/binary-$RELEASE_TARGET" --tag "$RELEASE_TAG"

      - name: Retain validated assets
        uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a
        with:
          name: stab-${{ matrix.name }}
          path: target/releases/binary-${{ matrix.name }}/*
          if-no-files-found: error
          retention-days: 7

  draft:
    name: Create verified private draft
    needs: build
    runs-on: ubuntu-24.04
    timeout-minutes: 45
    permissions:
      contents: write
    steps:
      - name: Checkout tagged source
        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
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

      - name: Download all validated assets
        uses: actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c
        with:
          pattern: stab-*
          merge-multiple: true
          path: target/releases/assets

      - name: Build credential-free release operator
        env:
          CARGO_TARGET_DIR: ${{ runner.temp }}/stab-release-operator-${{ github.sha }}
        run: cargo build -q --locked -p stab-release

      - name: Verify retained assets and create digest-checked draft
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          RELEASE_OPERATOR: ${{ runner.temp }}/stab-release-operator-${{ github.sha }}/debug/stab-release
          RELEASE_TAG: ${{ inputs.tag }}
        run: '"$RELEASE_OPERATOR" create-draft --assets target/releases/assets --tag "$RELEASE_TAG" --confirm-version 0.2.0'
"#;

const EXPECTED_REHEARSAL_WORKFLOW: &str = r#"name: Release Rehearsal

on:
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: release-rehearsal-${{ github.sha }}
  cancel-in-progress: false

jobs:
  build:
    name: Build stab rehearsal (${{ matrix.name }})
    if: github.repository == 'ifsheldon/Stab-release-rehearsal' && github.repository_id == '1342241032'
    strategy:
      fail-fast: false
      matrix:
        include:
          - name: linux-aarch64
            os: ubuntu-24.04-arm
          - name: macos-aarch64
            os: macos-15
    runs-on: ${{ matrix.os }}
    timeout-minutes: 30
    steps:
      - name: Checkout tagged source
        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          ref: ${{ github.sha }}
          fetch-depth: 0
          persist-credentials: false

      - name: Verify immutable rehearsal revision
        env:
          REHEARSAL_REPOSITORY: ${{ github.repository }}
          REHEARSAL_REPOSITORY_ID: ${{ github.repository_id }}
          RELEASE_TAG: v0.2.0-rehearsal-${{ github.sha }}
          RELEASE_REF: ${{ github.ref }}
          RELEASE_SHA: ${{ github.sha }}
        run: |
          test "$REHEARSAL_REPOSITORY" = "ifsheldon/Stab-release-rehearsal"
          test "$REHEARSAL_REPOSITORY_ID" = "1342241032"
          test "${#RELEASE_SHA}" -eq 40
          case "$RELEASE_SHA" in *[!0-9a-f]*) exit 1 ;; esac
          test "$RELEASE_TAG" = "v0.2.0-rehearsal-$RELEASE_SHA"
          test "$RELEASE_REF" = "refs/tags/$RELEASE_TAG"
          test "$(git rev-parse HEAD)" = "$RELEASE_SHA"
          test "$(git cat-file -t "$RELEASE_TAG")" = "tag"
          test "$(git rev-parse "${RELEASE_TAG}^{commit}")" = "$RELEASE_SHA"
          test "$(git rev-parse "${RELEASE_REF}^{commit}")" = "$RELEASE_SHA"

      - name: Show toolchain
        run: rustup show active-toolchain

      - name: Build and validate tagged binary
        env:
          RELEASE_TAG: v0.2.0-rehearsal-${{ github.sha }}
          RELEASE_TARGET: ${{ matrix.name }}
        run: cargo run -q --locked -p stab-release --bin stab-release-rehearsal -- build-binary --target "$RELEASE_TARGET" --out "target/releases/binary-$RELEASE_TARGET" --tag "$RELEASE_TAG"

      - name: Retain validated assets
        uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a
        with:
          name: stab-${{ matrix.name }}
          path: target/releases/binary-${{ matrix.name }}/*
          if-no-files-found: error
          retention-days: 7

  draft:
    name: Create verified rehearsal draft
    needs: build
    if: github.repository == 'ifsheldon/Stab-release-rehearsal' && github.repository_id == '1342241032'
    runs-on: ubuntu-24.04
    timeout-minutes: 45
    permissions:
      contents: write
    steps:
      - name: Checkout tagged source
        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          ref: ${{ github.sha }}
          fetch-depth: 0
          persist-credentials: false

      - name: Verify immutable rehearsal revision
        env:
          REHEARSAL_REPOSITORY: ${{ github.repository }}
          REHEARSAL_REPOSITORY_ID: ${{ github.repository_id }}
          RELEASE_TAG: v0.2.0-rehearsal-${{ github.sha }}
          RELEASE_REF: ${{ github.ref }}
          RELEASE_SHA: ${{ github.sha }}
        run: |
          test "$REHEARSAL_REPOSITORY" = "ifsheldon/Stab-release-rehearsal"
          test "$REHEARSAL_REPOSITORY_ID" = "1342241032"
          test "${#RELEASE_SHA}" -eq 40
          case "$RELEASE_SHA" in *[!0-9a-f]*) exit 1 ;; esac
          test "$RELEASE_TAG" = "v0.2.0-rehearsal-$RELEASE_SHA"
          test "$RELEASE_REF" = "refs/tags/$RELEASE_TAG"
          test "$(git rev-parse HEAD)" = "$RELEASE_SHA"
          test "$(git cat-file -t "$RELEASE_TAG")" = "tag"
          test "$(git rev-parse "${RELEASE_TAG}^{commit}")" = "$RELEASE_SHA"
          test "$(git rev-parse "${RELEASE_REF}^{commit}")" = "$RELEASE_SHA"

      - name: Download all validated assets
        uses: actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c
        with:
          pattern: stab-*
          merge-multiple: true
          path: target/releases/assets

      - name: Build credential-free rehearsal operator
        env:
          CARGO_TARGET_DIR: ${{ runner.temp }}/stab-release-rehearsal-operator-${{ github.sha }}
        run: cargo build -q --locked -p stab-release --bin stab-release-rehearsal

      - name: Verify retained assets and create digest-checked rehearsal draft
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          RELEASE_OPERATOR: ${{ runner.temp }}/stab-release-rehearsal-operator-${{ github.sha }}/debug/stab-release-rehearsal
          RELEASE_TAG: v0.2.0-rehearsal-${{ github.sha }}
        run: '"$RELEASE_OPERATOR" create-draft --assets target/releases/assets --tag "$RELEASE_TAG" --confirm-repository ifsheldon/Stab-release-rehearsal'
"#;

pub(super) fn inspect(
    path: &Path,
    root: &yaml_rust2::yaml::Hash,
    report: &mut WorkflowActionReport,
) {
    let expected_source = if path == Path::new(RELEASE_WORKFLOW_PATH) {
        EXPECTED_RELEASE_WORKFLOW
    } else if path == Path::new(REHEARSAL_WORKFLOW_PATH) {
        EXPECTED_REHEARSAL_WORKFLOW
    } else {
        return;
    };
    let Ok(expected) = YamlLoader::load_from_str(expected_source) else {
        report.violations.push(Violation::new(
            "workflow-release-invalid-policy",
            format!(
                "the source-owned release workflow policy for {} is invalid YAML",
                path.display()
            ),
        ));
        return;
    };
    let [expected] = expected.as_slice() else {
        report.violations.push(Violation::new(
            "workflow-release-invalid-policy",
            "the source-owned release workflow policy must contain one document",
        ));
        return;
    };
    if expected.as_hash() != Some(root) {
        report.violations.push(Violation::new(
            "workflow-release-execution-context",
            format!(
                "workflow {} must match the exact reviewed release jobs, runners, permissions, steps, shells, actions, and commands",
                path.display()
            ),
        ));
    }
}

pub(super) fn inspect_required_workflow_paths(
    paths: &[PathBuf],
    report: &mut WorkflowActionReport,
) {
    for required in REQUIRED_WORKFLOW_PATHS {
        if !paths.iter().any(|path| path == Path::new(required)) {
            report.violations.push(Violation::new(
                "workflow-release-missing",
                format!("required release workflow {required} is missing or renamed"),
            ));
        }
    }
}

pub(super) fn is_final_draft_step(
    path: &Path,
    job_name: &str,
    index: usize,
    step_count: usize,
) -> bool {
    (path == Path::new(RELEASE_WORKFLOW_PATH) || path == Path::new(REHEARSAL_WORKFLOW_PATH))
        && job_name == "draft"
        && index.saturating_add(1) == step_count
}

pub(super) fn is_exact_operator_invocation(step: &yaml_rust2::yaml::Hash) -> bool {
    is_operator_invocation(
        step,
        DRAFT_OPERATOR_STEP_NAME,
        DRAFT_OPERATOR_COMMAND,
        RELEASE_OPERATOR_PATH,
        RELEASE_TAG_INPUT,
    ) || is_operator_invocation(
        step,
        REHEARSAL_DRAFT_OPERATOR_STEP_NAME,
        REHEARSAL_DRAFT_OPERATOR_COMMAND,
        REHEARSAL_OPERATOR_PATH,
        REHEARSAL_TAG,
    )
}

fn is_operator_invocation(
    step: &yaml_rust2::yaml::Hash,
    step_name: &str,
    command: &str,
    operator_path: &str,
    tag: &str,
) -> bool {
    if step.len() != 3
        || mapping_value(step, "name").and_then(yaml_scalar) != Some(step_name)
        || mapping_value(step, "run").and_then(yaml_scalar) != Some(command)
        || mapping_value(step, "uses").is_some()
    {
        return false;
    }
    let Some(environment) = mapping_value(step, "env").and_then(Yaml::as_hash) else {
        return false;
    };
    environment.len() == 3
        && mapping_value(environment, "GITHUB_TOKEN").and_then(yaml_scalar)
            == Some(GITHUB_TOKEN_SECRET)
        && mapping_value(environment, "RELEASE_OPERATOR").and_then(yaml_scalar)
            == Some(operator_path)
        && mapping_value(environment, "RELEASE_TAG").and_then(yaml_scalar) == Some(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inspect(source: &str) -> WorkflowActionReport {
        inspect_at(Path::new(RELEASE_WORKFLOW_PATH), source)
    }

    fn inspect_at(path: &Path, source: &str) -> WorkflowActionReport {
        let documents = YamlLoader::load_from_str(source).expect("valid test workflow");
        let [document] = documents.as_slice() else {
            panic!("one test workflow document");
        };
        let root = document.as_hash().expect("test workflow mapping");
        let mut report = WorkflowActionReport {
            action_use_count: 0,
            violations: Vec::new(),
        };
        super::inspect(path, root, &mut report);
        report
    }

    fn inspect_all_contracts(path: &Path, source: &str) -> WorkflowActionReport {
        let mut report = WorkflowActionReport {
            action_use_count: 0,
            violations: Vec::new(),
        };
        super::super::inspect_workflow_source(path, source, &mut report);
        report
    }

    fn assert_rejected(source: &str) {
        assert_rejected_at(Path::new(RELEASE_WORKFLOW_PATH), source);
    }

    fn assert_rejected_at(path: &Path, source: &str) {
        let report = inspect_at(path, source);
        assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
        assert_eq!(
            report.violations.first().map(|violation| violation.code),
            Some("workflow-release-execution-context")
        );
    }

    #[test]
    fn exact_release_workflow_is_accepted() {
        assert!(inspect(EXPECTED_RELEASE_WORKFLOW).violations.is_empty());
    }

    #[test]
    fn exact_rehearsal_workflow_and_secret_scope_are_accepted() {
        let path = Path::new(REHEARSAL_WORKFLOW_PATH);
        assert!(
            inspect_at(path, EXPECTED_REHEARSAL_WORKFLOW)
                .violations
                .is_empty()
        );
        let report = inspect_all_contracts(path, EXPECTED_REHEARSAL_WORKFLOW);
        assert_eq!(report.action_use_count, 4);
        assert!(report.violations.is_empty(), "{:?}", report.violations);
    }

    #[test]
    fn both_release_workflow_paths_are_required() {
        let exact = REQUIRED_WORKFLOW_PATHS
            .map(PathBuf::from)
            .into_iter()
            .collect::<Vec<_>>();
        let mut report = WorkflowActionReport {
            action_use_count: 0,
            violations: Vec::new(),
        };
        inspect_required_workflow_paths(&exact, &mut report);
        assert!(report.violations.is_empty());

        for retained in [
            vec![PathBuf::from(RELEASE_WORKFLOW_PATH)],
            vec![PathBuf::from(REHEARSAL_WORKFLOW_PATH)],
            vec![
                PathBuf::from(RELEASE_WORKFLOW_PATH),
                PathBuf::from(".github/workflows/renamed-rehearsal.yml"),
            ],
        ] {
            let mut report = WorkflowActionReport {
                action_use_count: 0,
                violations: Vec::new(),
            };
            inspect_required_workflow_paths(&retained, &mut report);
            assert!(
                report
                    .violations
                    .iter()
                    .any(|violation| violation.code == "workflow-release-missing")
            );
        }
    }

    #[test]
    fn release_workflow_rejects_revision_and_operator_mutations() {
        for source in [
            EXPECTED_RELEASE_WORKFLOW.replace(
                "test \"$RELEASE_REF\" = \"refs/tags/v0.2.0\"",
                "test \"$RELEASE_REF\" = \"refs/heads/main\"",
            ),
            EXPECTED_RELEASE_WORKFLOW.replace("ref: ${{ github.sha }}", "ref: ${{ inputs.tag }}"),
            EXPECTED_RELEASE_WORKFLOW.replace("fetch-depth: 0", "fetch-depth: 1"),
            EXPECTED_RELEASE_WORKFLOW
                .replace("persist-credentials: false", "persist-credentials: true"),
            EXPECTED_RELEASE_WORKFLOW.replace(
                "${{ runner.temp }}/stab-release-operator-${{ github.sha }}",
                "target",
            ),
        ] {
            assert_rejected(&source);
        }
    }

    #[test]
    fn release_workflow_rejects_inherited_execution_modifiers() {
        for source in [
            EXPECTED_RELEASE_WORKFLOW.replace(
                "permissions:\n  contents: read",
                "defaults:\n  run:\n    shell: python\n\npermissions:\n  contents: read",
            ),
            EXPECTED_RELEASE_WORKFLOW.replace(
                "  draft:\n    name: Create verified private draft",
                "  draft:\n    container: ubuntu:latest\n    name: Create verified private draft",
            ),
            EXPECTED_RELEASE_WORKFLOW.replace(
                "  draft:\n    name: Create verified private draft",
                "  draft:\n    services:\n      helper:\n        image: alpine:latest\n    name: Create verified private draft",
            ),
            EXPECTED_RELEASE_WORKFLOW.replace("runs-on: ubuntu-24.04", "runs-on: ubuntu-latest"),
            EXPECTED_RELEASE_WORKFLOW.replace(
                "    permissions:\n      contents: write",
                "    permissions:\n      contents: write\n      actions: write",
            ),
        ] {
            assert_rejected(&source);
        }
    }

    #[test]
    fn release_workflow_rejects_extra_privileged_steps_and_step_keys() {
        for source in [
            EXPECTED_RELEASE_WORKFLOW.replace(
                "      - name: Build credential-free release operator",
                "      - name: Unexpected privileged action\n        uses: owner/action@0123456789abcdef0123456789abcdef01234567\n\n      - name: Build credential-free release operator",
            ),
            EXPECTED_RELEASE_WORKFLOW.replace(
                "      - name: Build credential-free release operator\n        env:",
                "      - name: Build credential-free release operator\n        shell: python\n        env:",
            ),
            EXPECTED_RELEASE_WORKFLOW.replace(
                "      - name: Build credential-free release operator\n        env:",
                "      - name: Build credential-free release operator\n        timeout-minutes: 1\n        env:",
            ),
        ] {
            assert_rejected(&source);
        }
    }

    #[test]
    fn rehearsal_workflow_rejects_destination_and_revision_mutations() {
        let path = Path::new(REHEARSAL_WORKFLOW_PATH);
        for source in [
            EXPECTED_REHEARSAL_WORKFLOW
                .replace("ifsheldon/Stab-release-rehearsal", "ifsheldon/Stab"),
            EXPECTED_REHEARSAL_WORKFLOW.replace("1342241032", "1342241033"),
            EXPECTED_REHEARSAL_WORKFLOW.replace(
                "v0.2.0-rehearsal-${{ github.sha }}",
                "v0.2.0-rehearsal-${{ github.ref_name }}",
            ),
            EXPECTED_REHEARSAL_WORKFLOW.replace(
                "test \"${#RELEASE_SHA}\" -eq 40",
                "test \"${#RELEASE_SHA}\" -ge 7",
            ),
            EXPECTED_REHEARSAL_WORKFLOW.replace(
                "case \"$RELEASE_SHA\" in *[!0-9a-f]*) exit 1 ;; esac",
                "true",
            ),
            EXPECTED_REHEARSAL_WORKFLOW.replace(
                "test \"$(git cat-file -t \"$RELEASE_TAG\")\" = \"tag\"",
                "true",
            ),
        ] {
            assert_rejected_at(path, &source);
        }
    }

    #[test]
    fn rehearsal_workflow_rejects_runner_action_and_checkout_mutations() {
        let path = Path::new(REHEARSAL_WORKFLOW_PATH);
        for source in [
            EXPECTED_REHEARSAL_WORKFLOW.replace("ubuntu-24.04-arm", "ubuntu-latest"),
            EXPECTED_REHEARSAL_WORKFLOW.replace("macos-15", "macos-latest"),
            EXPECTED_REHEARSAL_WORKFLOW.replace(
                "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
                "actions/checkout@1111111111111111111111111111111111111111",
            ),
            EXPECTED_REHEARSAL_WORKFLOW.replace("fetch-depth: 0", "fetch-depth: 1"),
            EXPECTED_REHEARSAL_WORKFLOW
                .replace("persist-credentials: false", "persist-credentials: true"),
            EXPECTED_REHEARSAL_WORKFLOW.replace(
                "    permissions:\n      contents: write",
                "    permissions:\n      contents: write\n      actions: write",
            ),
        ] {
            assert_rejected_at(path, &source);
        }
    }

    #[test]
    fn production_and_rehearsal_workflows_reject_cross_wired_operators() {
        for source in [
            EXPECTED_RELEASE_WORKFLOW.replace(
                "cargo build -q --locked -p stab-release",
                "cargo build -q --locked -p stab-release --bin stab-release-rehearsal",
            ),
            EXPECTED_RELEASE_WORKFLOW
                .replace("/debug/stab-release", "/debug/stab-release-rehearsal"),
        ] {
            assert_rejected(&source);
        }

        let path = Path::new(REHEARSAL_WORKFLOW_PATH);
        for source in [
            EXPECTED_REHEARSAL_WORKFLOW
                .replace("--bin stab-release-rehearsal", "--bin stab-release"),
            EXPECTED_REHEARSAL_WORKFLOW.replace(
                "stab-release-rehearsal-operator-${{ github.sha }}",
                "stab-release-operator-${{ github.sha }}",
            ),
            EXPECTED_REHEARSAL_WORKFLOW
                .replace("/debug/stab-release-rehearsal", "/debug/stab-release"),
            EXPECTED_REHEARSAL_WORKFLOW.replace(
                "--confirm-repository ifsheldon/Stab-release-rehearsal",
                "--confirm-version 0.2.0",
            ),
        ] {
            assert_rejected_at(path, &source);
        }
    }

    #[test]
    fn rehearsal_secret_is_rejected_outside_exact_final_operator_step() {
        let path = Path::new(REHEARSAL_WORKFLOW_PATH);
        for source in [
            EXPECTED_REHEARSAL_WORKFLOW.replace(
                "      - name: Build credential-free rehearsal operator",
                "      - name: Expose token before operator build\n        env:\n          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}\n        run: true\n\n      - name: Build credential-free rehearsal operator",
            ),
            EXPECTED_REHEARSAL_WORKFLOW.replace(
                "          RELEASE_TAG: v0.2.0-rehearsal-${{ github.sha }}\n        run: '\"$RELEASE_OPERATOR\" create-draft",
                "          RELEASE_TAG: v0.2.0-rehearsal-${{ github.sha }}\n          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}\n        run: '\"$RELEASE_OPERATOR\" create-draft",
            ),
            EXPECTED_REHEARSAL_WORKFLOW.replace(
                "        run: '\"$RELEASE_OPERATOR\" create-draft --assets target/releases/assets --tag \"$RELEASE_TAG\" --confirm-repository ifsheldon/Stab-release-rehearsal'",
                "        run: '\"$RELEASE_OPERATOR\" create-draft --assets target/releases/assets --tag \"$RELEASE_TAG\" --confirm-repository ifsheldon/Stab-release-rehearsal; true'",
            ),
        ] {
            let report = inspect_all_contracts(path, &source);
            assert!(
                report
                    .violations
                    .iter()
                    .any(|violation| violation.code == "workflow-release-secret-scope"),
                "{:?}",
                report.violations
            );
        }
    }
}
