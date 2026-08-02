use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use clap::Args;
use serde::Deserialize;

use super::model::{ChecklistScope, PerformanceDisposition, QualificationSuite};
use crate::config::PREFIX;
use crate::error::BenchError;
use crate::process::{
    OutputPolicy, ProcessEnvironment, ProcessLimits, ProcessRequest, ProcessResult,
    run_bounded_process,
};
use crate::root::RepoRoot;

const STATUS_PATH: &str = "docs/qualification-status.md";
const RUNTIME_GROUPS_PATH: &str = "benchmarks/qualification-runtime-groups.json";
const COMPLETION_CHECKPOINT_PATH: &str = "benchmarks/qualification-completion-checkpoint.json";
const COMPLETION_CHECKPOINT_SCHEMA_VERSION: u32 = 1;
const HISTORICAL_DEM_COMPLETION_SCOPE: &str = "dem-r6";
const RELEASE_COMPLETION_SCOPE: &str = "a9-release";
const MAX_SOURCE_BYTES: usize = 32 << 20;
const MAX_GIT_OUTPUT_BYTES: usize = 4 << 20;
const MAX_GIT_DIAGNOSTIC_BYTES: usize = 64 << 10;
const GIT_TIMEOUT: Duration = Duration::from_secs(30);
const A9_STATUS_ONLY_PATHS: [&str; 6] = [
    "benchmarks/qualification-completion-checkpoint.json",
    "docs/qualification-status.md",
    "docs/plans/agent-native-modular-qec-progress-report.md",
    "docs/plans/agent-native-modular-qec-architecture-plan.md",
    "docs/plans/GOAL.md",
    "docs/plans/milestone-spec-gaps.md",
];

#[derive(Clone, Debug, Args)]
pub(crate) struct StatusArgs {
    /// Compare the generated dashboard with the checked file instead of writing it.
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Deserialize)]
struct CorrectnessInventory {
    semantic_digest: String,
    evidence_cases: Vec<CorrectnessCase>,
}

#[derive(Debug, Deserialize)]
struct CorrectnessCase {
    status: CorrectnessStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum CorrectnessStatus {
    Implemented,
    EvidenceClose,
    Planned,
}

#[derive(Debug, Deserialize)]
struct RuntimeContracts {
    schema_version: u32,
    performance_inventory_sha256: String,
    groups: Vec<RuntimeGroup>,
}

#[derive(Debug, Deserialize)]
struct RuntimeGroup {
    claim_class: RuntimeClaimClass,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum RuntimeClaimClass {
    DiagnosticInfrastructure,
    ProductDiagnostic,
    PromotablePerformance,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletionCheckpoint {
    schema_version: u32,
    current: Option<CurrentCompletion>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentCompletion {
    scope_id: String,
    path: String,
    report_sha256: String,
    stab_commit: String,
    architecture: String,
    performance_inventory_sha256: String,
    correctness_inventory_sha256: String,
    parity_outcome: CompletionParityOutcome,
    regression_outcome: CompletionRegressionOutcome,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum CompletionParityOutcome {
    Passed,
}

impl CompletionParityOutcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum CompletionRegressionOutcome {
    Passed,
    Unseeded,
}

impl CompletionRegressionOutcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Unseeded => "unseeded",
        }
    }
}

struct StatusData {
    correctness_digest: String,
    performance_digest: String,
    correctness_counts: BTreeMap<CorrectnessStatus, usize>,
    deferred_checklist_surfaces: usize,
    release_groups: usize,
    diagnostic_groups: usize,
    future_candidates: usize,
    regression_seeded: usize,
    parity_policy_sha256: String,
    maximum_parity_ratio: f64,
    regression_policy_sha256: String,
    regression_baselines_sha256: String,
    regression_default_max_relative_ratio: String,
    completion: Option<CurrentCompletion>,
    completion_is_current: bool,
}

pub(crate) fn run(root: &RepoRoot, args: StatusArgs) -> Result<(), BenchError> {
    let suite = super::read(root)?;
    let data = collect(root, &suite)?;
    let rendered = render(&data);
    let path = root.path.join(STATUS_PATH);
    if args.check {
        let checked = read(root, &path)?;
        if checked != rendered.as_bytes() {
            return Err(BenchError::Qualification(
                "generated qualification dashboard differs from docs/qualification-status.md"
                    .to_string(),
            ));
        }
        println!("[{PREFIX}] generated qualification status is clean");
    } else {
        super::atomic_write(root, &path, rendered.as_bytes())?;
        println!("[{PREFIX}] wrote {STATUS_PATH}");
    }
    Ok(())
}

fn collect(root: &RepoRoot, suite: &QualificationSuite) -> Result<StatusData, BenchError> {
    let correctness: CorrectnessInventory =
        parse(root, &root.correctness_manifest(), "correctness inventory")?;
    if correctness.semantic_digest != suite.correctness_digest {
        return Err(BenchError::Qualification(
            "qualification status found mismatched correctness and performance inventories"
                .to_string(),
        ));
    }
    let runtime: RuntimeContracts = parse(
        root,
        &root.path.join(RUNTIME_GROUPS_PATH),
        "runtime contracts",
    )?;
    if runtime.schema_version != super::runtime::GROUP_CONTRACT_SCHEMA_VERSION
        || runtime.performance_inventory_sha256 != suite.semantic_digest
    {
        return Err(BenchError::Qualification(
            "qualification status found stale runtime contracts".to_string(),
        ));
    }
    let policies = super::runtime::qualification_policy_status(root, &suite.semantic_digest)
        .map_err(|error| {
            BenchError::Qualification(format!(
                "qualification status found an invalid policy contract: {error}"
            ))
        })?;
    let checkpoint: CompletionCheckpoint = parse(
        root,
        &root.path.join(COMPLETION_CHECKPOINT_PATH),
        "completion checkpoint",
    )?;
    let completion_is_current = validate_completion_checkpoint(
        root,
        &checkpoint,
        &suite.semantic_digest,
        &suite.correctness_digest,
    )?;

    let checklist_source = read(root, &root.feature_checklist())?;
    let checklist_text = std::str::from_utf8(&checklist_source).map_err(|error| {
        BenchError::Qualification(format!("feature checklist is not UTF-8: {error}"))
    })?;
    let checklist = super::checklist::parse(checklist_text)?;
    let deferred_checklist_surfaces = checklist
        .iter()
        .filter(|item| item.scope == ChecklistScope::Deferred || item.deferred_remainder)
        .count();
    let correctness_counts = counts(correctness.evidence_cases.iter().map(|case| case.status));
    let release_groups = runtime
        .groups
        .iter()
        .filter(|group| group.claim_class == RuntimeClaimClass::PromotablePerformance)
        .count();
    let diagnostic_groups = runtime.groups.len().saturating_sub(release_groups);
    let future_candidates = suite
        .qualification_groups
        .iter()
        .filter(|group| group.disposition == PerformanceDisposition::FutureCandidate)
        .count();
    Ok(StatusData {
        correctness_digest: suite.correctness_digest.clone(),
        performance_digest: suite.semantic_digest.clone(),
        correctness_counts,
        deferred_checklist_surfaces,
        release_groups,
        diagnostic_groups,
        future_candidates,
        regression_seeded: policies.regression_seeded_identity_count,
        parity_policy_sha256: policies.parity_policy_sha256,
        maximum_parity_ratio: policies.maximum_parity_ratio,
        regression_policy_sha256: policies.regression_policy_sha256,
        regression_baselines_sha256: policies.regression_baselines_sha256,
        regression_default_max_relative_ratio: policies.regression_default_max_relative_ratio,
        completion: checkpoint.current,
        completion_is_current,
    })
}

fn validate_completion_checkpoint(
    root: &RepoRoot,
    checkpoint: &CompletionCheckpoint,
    performance_inventory_sha256: &str,
    correctness_inventory_sha256: &str,
) -> Result<bool, BenchError> {
    if checkpoint.schema_version != COMPLETION_CHECKPOINT_SCHEMA_VERSION {
        return Err(BenchError::Qualification(
            "qualification completion checkpoint schema is unsupported".to_string(),
        ));
    }
    let Some(current) = &checkpoint.current else {
        return Ok(false);
    };
    if !matches!(
        current.scope_id.as_str(),
        HISTORICAL_DEM_COMPLETION_SCOPE | RELEASE_COMPLETION_SCOPE
    ) || !valid_sha256(&current.report_sha256)
        || !valid_sha256(&current.performance_inventory_sha256)
        || !valid_sha256(&current.correctness_inventory_sha256)
        || !valid_git_commit(&current.stab_commit)
        || !valid_identity_token(&current.architecture)
        || super::runtime::validate_status_artifact_path(Path::new(&current.path)).is_err()
    {
        return Err(BenchError::Qualification(
            "qualification completion checkpoint is malformed".to_string(),
        ));
    }
    if current.scope_id != RELEASE_COMPLETION_SCOPE
        || current.performance_inventory_sha256 != performance_inventory_sha256
        || current.correctness_inventory_sha256 != correctness_inventory_sha256
    {
        return Ok(false);
    }
    completion_revision_is_current(root, &current.stab_commit)
}

fn completion_revision_is_current(
    root: &RepoRoot,
    measured_commit: &str,
) -> Result<bool, BenchError> {
    let head_before = status_git_head(root)?;
    let object = run_status_git(
        root,
        [
            OsString::from("cat-file"),
            OsString::from("-e"),
            OsString::from(format!("{measured_commit}^{{commit}}")),
        ],
    )?;
    if object.status != Some(0) {
        return Err(git_contract_error(
            "completion checkpoint references a missing measured commit",
            &object,
        ));
    }

    let ancestor = run_status_git(
        root,
        [
            OsString::from("merge-base"),
            OsString::from("--is-ancestor"),
            OsString::from(measured_commit),
            OsString::from(&head_before),
        ],
    )?;
    match ancestor.status {
        Some(0) => {}
        Some(1) => return Ok(false),
        _ => {
            return Err(git_contract_error(
                "failed to validate the completion checkpoint ancestry",
                &ancestor,
            ));
        }
    }

    let changed = run_status_git(
        root,
        [
            OsString::from("log"),
            OsString::from("--format="),
            OsString::from("--name-only"),
            OsString::from("--no-renames"),
            OsString::from("-m"),
            OsString::from("-z"),
            OsString::from(format!("{measured_commit}..{head_before}")),
            OsString::from("--"),
        ],
    )?;
    if changed.status != Some(0) {
        return Err(git_contract_error(
            "failed to enumerate committed A9 closure paths",
            &changed,
        ));
    }
    let committed_paths_are_status_only = validate_a9_closure_paths(&changed.stdout)?;
    let working_paths_are_status_only = working_tree_is_status_only(root)?;
    if status_git_head(root)? != head_before {
        return Err(BenchError::Qualification(
            "repository HEAD changed while validating the A9 completion checkpoint".to_string(),
        ));
    }
    Ok(committed_paths_are_status_only && working_paths_are_status_only)
}

fn working_tree_is_status_only(root: &RepoRoot) -> Result<bool, BenchError> {
    let changed = run_status_git(
        root,
        [
            OsString::from("diff"),
            OsString::from("--name-only"),
            OsString::from("--no-renames"),
            OsString::from("-z"),
            OsString::from("HEAD"),
            OsString::from("--"),
        ],
    )?;
    if changed.status != Some(0) {
        return Err(git_contract_error(
            "failed to enumerate modified A9 closure paths",
            &changed,
        ));
    }
    if !validate_a9_closure_paths(&changed.stdout)? {
        return Ok(false);
    }

    let untracked = run_status_git(
        root,
        [
            OsString::from("ls-files"),
            OsString::from("--others"),
            OsString::from("--exclude-standard"),
            OsString::from("-z"),
        ],
    )?;
    if untracked.status != Some(0) {
        return Err(git_contract_error(
            "failed to enumerate untracked A9 closure paths",
            &untracked,
        ));
    }
    validate_a9_closure_paths(&untracked.stdout)
}

fn validate_a9_closure_paths(paths: &[u8]) -> Result<bool, BenchError> {
    if paths.is_empty() {
        return Ok(true);
    }
    let Some((terminator, path_bytes)) = paths.split_last() else {
        return Ok(true);
    };
    if *terminator != 0 {
        return Err(BenchError::Qualification(
            "Git returned a malformed A9 closure path list".to_string(),
        ));
    }
    for raw_path in path_bytes.split(|byte| *byte == 0) {
        let path = std::str::from_utf8(raw_path).map_err(|error| {
            BenchError::Qualification(format!("Git returned a non-UTF-8 A9 closure path: {error}"))
        })?;
        if path.is_empty() || !A9_STATUS_ONLY_PATHS.contains(&path) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn run_status_git(
    root: &RepoRoot,
    args: impl IntoIterator<Item = OsString>,
) -> Result<ProcessResult, BenchError> {
    Ok(run_bounded_process(&ProcessRequest {
        program: "git".into(),
        args: args.into_iter().collect(),
        stdin: Vec::new(),
        working_directory: root.process_working_dir(),
        environment: ProcessEnvironment::ClearAndSet(vec![
            (OsString::from("PATH"), OsString::from("/usr/bin:/bin")),
            (OsString::from("LANG"), OsString::from("C")),
            (OsString::from("LC_ALL"), OsString::from("C")),
            (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
            (
                OsString::from("GIT_CONFIG_GLOBAL"),
                OsString::from("/dev/null"),
            ),
        ]),
        affinity_cpu: None,
        limits: ProcessLimits {
            stdin_bytes: 0,
            stdout: OutputPolicy::Capture {
                maximum_bytes: MAX_GIT_OUTPUT_BYTES,
            },
            stderr: OutputPolicy::Capture {
                maximum_bytes: MAX_GIT_DIAGNOSTIC_BYTES,
            },
            regular_file_bytes: None,
            timeout: GIT_TIMEOUT,
        },
    })?)
}

fn status_git_head(root: &RepoRoot) -> Result<String, BenchError> {
    let result = run_status_git(
        root,
        [
            OsString::from("rev-parse"),
            OsString::from("--verify"),
            OsString::from("HEAD^{commit}"),
        ],
    )?;
    if result.status != Some(0) {
        return Err(git_contract_error(
            "failed to resolve repository HEAD for A9 completion status",
            &result,
        ));
    }
    let text = std::str::from_utf8(&result.stdout).map_err(|error| {
        BenchError::Qualification(format!("Git returned a non-UTF-8 HEAD commit: {error}"))
    })?;
    let commit = text.strip_suffix('\n').unwrap_or(text);
    if !valid_git_commit(commit) {
        return Err(BenchError::Qualification(
            "Git returned a malformed HEAD commit for A9 completion status".to_string(),
        ));
    }
    Ok(commit.to_string())
}

fn git_contract_error(context: &str, result: &ProcessResult) -> BenchError {
    let diagnostic = String::from_utf8_lossy(&result.stderr);
    BenchError::Qualification(format!(
        "{context}: Git status {}, stderr: {}",
        result
            .status
            .map_or_else(|| "signal".to_string(), |status| status.to_string()),
        diagnostic.trim()
    ))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_git_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_identity_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn counts<T: Ord>(values: impl IntoIterator<Item = T>) -> BTreeMap<T, usize> {
    let mut result = BTreeMap::new();
    for value in values {
        *result.entry(value).or_default() += 1;
    }
    result
}

fn render(data: &StatusData) -> String {
    let implemented = data
        .correctness_counts
        .get(&CorrectnessStatus::Implemented)
        .copied()
        .unwrap_or_default();
    let evidence_close = data
        .correctness_counts
        .get(&CorrectnessStatus::EvidenceClose)
        .copied()
        .unwrap_or_default();
    let planned = data
        .correctness_counts
        .get(&CorrectnessStatus::Planned)
        .copied()
        .unwrap_or_default();
    let checkpoint = match (&data.completion, data.completion_is_current) {
        (None, _) => {
            "Formal repaired-contract completion: **not started**. Historical reports remain historical under their recorded source identities.".to_string()
        }
        (Some(current), true) => {
            format!(
                "Formal repaired-contract completion: scope `{}` at `{}` on `{}` (`{}`), report `{}`, Stim parity `{}`, Stab regression `{}`.",
                current.scope_id,
                current.stab_commit,
                current.architecture,
                current.path,
                current.report_sha256,
                current.parity_outcome.as_str(),
                current.regression_outcome.as_str(),
            )
        }
        (Some(current), false) => {
            format!(
                "Formal completion for the current inventories: **not started**. The latest historical checkpoint is scope `{}` at `{}` on `{}` (`{}`), report `{}`, with correctness inventory `{}` and performance inventory `{}`.",
                current.scope_id,
                current.stab_commit,
                current.architecture,
                current.path,
                current.report_sha256,
                current.correctness_inventory_sha256,
                current.performance_inventory_sha256,
            )
        }
    };
    format!(
        "<!-- Generated by `just qualification::status`. Do not edit by hand. -->\n# Qualification Status\n\nThis dashboard is generated from the checked correctness inventory, performance inventory, runtime contracts, parity policy, regression policy and baselines, feature checklist, and completion checkpoint.\n\n## Current Checkpoint\n\n{checkpoint}\n\n## Inventory\n\n| Category | Count |\n| --- | ---: |\n| Implemented correctness evidence parents | {implemented} |\n| Evidence-close correctness parents | {evidence_close} |\n| Planned correctness parents | {planned} |\n| Deferred checklist surfaces or remainders | {} |\n| Release runtime groups | {} |\n| Diagnostic runtime groups | {} |\n| Future performance candidates | {} |\n| Seeded self-regression identities | {} |\n\n## Contract Identities\n\n- Correctness inventory: `{}`\n- Performance inventory: `{}`\n- Stim parity policy: `{}`; paired median and confidence upper bound must each be no greater than `{:.2}x` for threshold-eligible groups.\n- Stab self-regression policy: `{}`; the default maximum deterioration is `{}x`.\n- Stab self-regression baselines: `{}`; missing identities are unseeded, never passing.\n\n## Interpretation\n\nImplementation, correctness qualification, Stim parity, Stab self-regression, environment validity, and memory/scaling evidence are separate conclusions. Shared-host scheduled timing is diagnostic and is not authoritative release evidence.\n",
        data.deferred_checklist_surfaces,
        data.release_groups,
        data.diagnostic_groups,
        data.future_candidates,
        data.regression_seeded,
        data.correctness_digest,
        data.performance_digest,
        data.parity_policy_sha256,
        data.maximum_parity_ratio,
        data.regression_policy_sha256,
        data.regression_default_max_relative_ratio,
        data.regression_baselines_sha256,
    )
}

fn parse<T: for<'de> Deserialize<'de>>(
    root: &RepoRoot,
    path: &Path,
    description: &str,
) -> Result<T, BenchError> {
    let bytes = read(root, path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| BenchError::Qualification(format!("invalid {description} JSON: {error}")))
}

fn read(root: &RepoRoot, path: &Path) -> Result<Vec<u8>, BenchError> {
    crate::source_file::read_repo_regular_file_bounded(root, path, MAX_SOURCE_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_git(repository: &Path, arguments: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(arguments)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("run test Git command");
        assert!(
            output.status.success(),
            "Git {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("Git output UTF-8")
            .trim()
            .to_string()
    }

    fn initialized_repository() -> (tempfile::TempDir, RepoRoot, String) {
        let repository = tempfile::tempdir().expect("temporary repository");
        test_git(
            repository.path(),
            &["init", "--quiet", "--initial-branch=main"],
        );
        test_git(repository.path(), &["config", "user.name", "Stab Test"]);
        test_git(
            repository.path(),
            &["config", "user.email", "stab@example.invalid"],
        );
        std::fs::write(repository.path().join("initial"), b"initial\n")
            .expect("write initial file");
        test_git(repository.path(), &["add", "--all"]);
        test_git(repository.path(), &["commit", "--quiet", "-m", "initial"]);
        let revision = test_git(repository.path(), &["rev-parse", "HEAD"]);
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        (repository, root, revision)
    }

    fn release_checkpoint(
        stab_commit: String,
        performance_inventory_sha256: String,
        correctness_inventory_sha256: String,
    ) -> CompletionCheckpoint {
        CompletionCheckpoint {
            schema_version: COMPLETION_CHECKPOINT_SCHEMA_VERSION,
            current: Some(CurrentCompletion {
                scope_id: RELEASE_COMPLETION_SCOPE.to_string(),
                path: "target/benchmarks/qualification/formal".to_string(),
                report_sha256: "4".repeat(64),
                stab_commit,
                architecture: "aarch64".to_string(),
                performance_inventory_sha256,
                correctness_inventory_sha256,
                parity_outcome: CompletionParityOutcome::Passed,
                regression_outcome: CompletionRegressionOutcome::Unseeded,
            }),
        }
    }

    #[test]
    fn generated_status_is_derived_from_cross_checked_source_contracts() {
        let root = RepoRoot::resolve(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("repository root");
        let suite = super::super::read(&root).expect("performance inventory");
        let data = collect(&root, &suite).expect("status data");
        let rendered = render(&data);
        let runtime: RuntimeContracts = parse(
            &root,
            &root.path.join(RUNTIME_GROUPS_PATH),
            "runtime contracts",
        )
        .expect("runtime contracts");

        assert_eq!(
            data.release_groups + data.diagnostic_groups,
            runtime.groups.len(),
            "every runtime contract is classified"
        );
        assert!(data.correctness_counts.values().sum::<usize>() > 1_000);
        let completion = data.completion.as_ref().expect("completion checkpoint");
        assert!(matches!(
            completion.scope_id.as_str(),
            HISTORICAL_DEM_COMPLETION_SCOPE | RELEASE_COMPLETION_SCOPE
        ));
        if data.completion_is_current {
            assert!(rendered.contains(&format!(
                "Stim parity `{}`",
                completion.parity_outcome.as_str()
            )));
            assert!(rendered.contains(&format!(
                "Stab regression `{}`",
                completion.regression_outcome.as_str()
            )));
        } else {
            assert!(
                rendered.contains("Formal completion for the current inventories: **not started**")
            );
            assert!(rendered.contains(&completion.correctness_inventory_sha256));
            assert!(rendered.contains(&completion.performance_inventory_sha256));
        }
        assert!(rendered.contains(&data.performance_digest));
        assert!(rendered.contains(&data.parity_policy_sha256));
        assert!(rendered.contains(&data.regression_policy_sha256));
        assert!(rendered.contains(&data.regression_baselines_sha256));
    }

    #[test]
    fn completion_checkpoint_rejects_malformed_current_identity() {
        let root = RepoRoot::resolve(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("repository root");
        let checkpoint = CompletionCheckpoint {
            schema_version: COMPLETION_CHECKPOINT_SCHEMA_VERSION,
            current: Some(CurrentCompletion {
                scope_id: RELEASE_COMPLETION_SCOPE.to_string(),
                path: "target/benchmarks/qualification/formal".to_string(),
                report_sha256: "not-a-digest".to_string(),
                stab_commit: "1".repeat(40),
                architecture: "aarch64".to_string(),
                performance_inventory_sha256: "2".repeat(64),
                correctness_inventory_sha256: "3".repeat(64),
                parity_outcome: CompletionParityOutcome::Passed,
                regression_outcome: CompletionRegressionOutcome::Unseeded,
            }),
        };
        assert!(
            validate_completion_checkpoint(&root, &checkpoint, &"2".repeat(64), &"3".repeat(64))
                .is_err()
        );
    }

    #[test]
    fn completion_checkpoint_distinguishes_current_and_historical_inventories() {
        let (_repository, root, measured_commit) = initialized_repository();
        let performance = "2".repeat(64);
        let correctness = "3".repeat(64);
        let checkpoint =
            release_checkpoint(measured_commit, performance.clone(), correctness.clone());

        assert!(
            validate_completion_checkpoint(&root, &checkpoint, &performance, &correctness)
                .expect("current checkpoint")
        );
        assert!(
            !validate_completion_checkpoint(&root, &checkpoint, &"5".repeat(64), &correctness)
                .expect("historical checkpoint")
        );

        let dem_checkpoint = CompletionCheckpoint {
            schema_version: COMPLETION_CHECKPOINT_SCHEMA_VERSION,
            current: Some(CurrentCompletion {
                scope_id: HISTORICAL_DEM_COMPLETION_SCOPE.to_string(),
                path: "target/benchmarks/qualification/formal-dem".to_string(),
                report_sha256: "4".repeat(64),
                stab_commit: "1".repeat(40),
                architecture: "aarch64".to_string(),
                performance_inventory_sha256: performance.clone(),
                correctness_inventory_sha256: correctness.clone(),
                parity_outcome: CompletionParityOutcome::Passed,
                regression_outcome: CompletionRegressionOutcome::Unseeded,
            }),
        };
        assert!(
            !validate_completion_checkpoint(&root, &dem_checkpoint, &performance, &correctness)
                .expect("DEM-only checkpoint")
        );
    }

    #[test]
    fn completion_checkpoint_allows_only_exact_a9_status_descendants() {
        let (repository, root, measured_commit) = initialized_repository();
        let allowed = repository
            .path()
            .join("docs/plans/agent-native-modular-qec-progress-report.md");
        std::fs::create_dir_all(allowed.parent().expect("allowed parent"))
            .expect("create allowed parent");
        std::fs::write(&allowed, b"# Closure\n").expect("write allowed closure path");
        test_git(repository.path(), &["add", "--all"]);
        test_git(
            repository.path(),
            &["commit", "--quiet", "-m", "record closure"],
        );

        assert!(
            completion_revision_is_current(&root, &measured_commit)
                .expect("allowed status descendant")
        );

        std::fs::write(repository.path().join("README.md"), b"not status only\n")
            .expect("write forbidden path");
        test_git(repository.path(), &["add", "--all"]);
        test_git(
            repository.path(),
            &["commit", "--quiet", "-m", "change README"],
        );
        assert!(
            !completion_revision_is_current(&root, &measured_commit)
                .expect("forbidden path makes completion historical")
        );
    }

    #[test]
    fn completion_checkpoint_rejects_transient_forbidden_changes_and_non_ancestors() {
        let (repository, root, measured_commit) = initialized_repository();
        let forbidden = repository.path().join("ops/bench/src/transient.rs");
        std::fs::create_dir_all(forbidden.parent().expect("forbidden parent"))
            .expect("create forbidden parent");
        std::fs::write(&forbidden, b"forbidden\n").expect("write forbidden path");
        test_git(repository.path(), &["add", "--all"]);
        test_git(
            repository.path(),
            &["commit", "--quiet", "-m", "transient forbidden change"],
        );
        std::fs::remove_file(&forbidden).expect("remove forbidden path");
        test_git(repository.path(), &["add", "--all"]);
        test_git(
            repository.path(),
            &["commit", "--quiet", "-m", "restore endpoint"],
        );
        assert!(
            !completion_revision_is_current(&root, &measured_commit).expect("history inspection")
        );

        let (repository, root, common) = initialized_repository();
        test_git(
            repository.path(),
            &["checkout", "--quiet", "-b", "evidence"],
        );
        std::fs::write(repository.path().join("side"), b"side\n").expect("write side branch");
        test_git(repository.path(), &["add", "--all"]);
        test_git(
            repository.path(),
            &["commit", "--quiet", "-m", "side evidence"],
        );
        let non_ancestor = test_git(repository.path(), &["rev-parse", "HEAD"]);
        test_git(repository.path(), &["checkout", "--quiet", "main"]);
        assert_eq!(test_git(repository.path(), &["rev-parse", "HEAD"]), common);
        assert!(
            !completion_revision_is_current(&root, &non_ancestor)
                .expect("non-ancestor is historical")
        );
        assert!(completion_revision_is_current(&root, &"0".repeat(40)).is_err());
    }

    #[test]
    fn completion_checkpoint_rejects_dirty_product_paths() {
        let (repository, root, measured_commit) = initialized_repository();
        let allowed = repository.path().join("docs/plans/GOAL.md");
        std::fs::create_dir_all(allowed.parent().expect("allowed parent"))
            .expect("create allowed parent");
        std::fs::write(&allowed, b"status only\n").expect("write allowed status path");
        assert!(
            completion_revision_is_current(&root, &measured_commit)
                .expect("uncommitted status path is allowed")
        );

        std::fs::write(repository.path().join("product.rs"), b"product change\n")
            .expect("write untracked product path");
        assert!(
            !completion_revision_is_current(&root, &measured_commit)
                .expect("untracked product path is rejected")
        );

        test_git(repository.path(), &["add", "product.rs"]);
        assert!(
            !completion_revision_is_current(&root, &measured_commit)
                .expect("staged product path is rejected")
        );
    }

    #[test]
    fn a9_closure_path_parser_rejects_bad_paths_and_malformed_output() {
        assert!(validate_a9_closure_paths(b"").expect("exact commit"));
        assert!(
            validate_a9_closure_paths(b"docs/plans/GOAL.md\0docs/qualification-status.md\0")
                .expect("allowed paths")
        );
        assert!(!validate_a9_closure_paths(b"README.md\0").expect("forbidden path"));
        assert!(validate_a9_closure_paths(b"docs/plans/GOAL.md").is_err());
        assert!(validate_a9_closure_paths(b"\xff\0").is_err());
    }
}
