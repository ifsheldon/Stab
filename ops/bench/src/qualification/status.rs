use std::collections::BTreeMap;
use std::path::Path;

use clap::Args;
use serde::Deserialize;

use super::model::{ChecklistScope, PerformanceDisposition, QualificationSuite};
use crate::config::PREFIX;
use crate::error::BenchError;
use crate::root::RepoRoot;

const STATUS_PATH: &str = "docs/qualification-status.md";
const RUNTIME_GROUPS_PATH: &str = "benchmarks/qualification-runtime-groups.json";
const COMPLETION_CHECKPOINT_PATH: &str = "benchmarks/qualification-completion-checkpoint.json";
const COMPLETION_CHECKPOINT_SCHEMA_VERSION: u32 = 1;
const COMPLETION_SCOPE: &str = "dem-r6";
const MAX_SOURCE_BYTES: usize = 32 << 20;

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
    if runtime.schema_version != 7 || runtime.performance_inventory_sha256 != suite.semantic_digest
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
    validate_completion_checkpoint(
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
    })
}

fn validate_completion_checkpoint(
    checkpoint: &CompletionCheckpoint,
    performance_inventory_sha256: &str,
    correctness_inventory_sha256: &str,
) -> Result<(), BenchError> {
    if checkpoint.schema_version != COMPLETION_CHECKPOINT_SCHEMA_VERSION {
        return Err(BenchError::Qualification(
            "qualification completion checkpoint schema is unsupported".to_string(),
        ));
    }
    let Some(current) = &checkpoint.current else {
        return Ok(());
    };
    if current.scope_id != COMPLETION_SCOPE
        || current.performance_inventory_sha256 != performance_inventory_sha256
        || current.correctness_inventory_sha256 != correctness_inventory_sha256
        || !valid_sha256(&current.report_sha256)
        || !valid_git_commit(&current.stab_commit)
        || !valid_identity_token(&current.architecture)
        || super::runtime::validate_status_artifact_path(Path::new(&current.path)).is_err()
    {
        return Err(BenchError::Qualification(
            "qualification completion checkpoint is stale or malformed".to_string(),
        ));
    }
    Ok(())
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
    let checkpoint = data.completion.as_ref().map_or_else(
        || {
            "Formal repaired-contract completion: **not started**. Historical reports remain historical under their recorded source identities.".to_string()
        },
        |current| {
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
        },
    );
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

    #[test]
    fn generated_status_is_derived_from_cross_checked_source_contracts() {
        let root = RepoRoot::resolve(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("repository root");
        let suite = super::super::read(&root).expect("performance inventory");
        let data = collect(&root, &suite).expect("status data");
        let rendered = render(&data);

        assert_eq!(
            data.release_groups + data.diagnostic_groups,
            20,
            "every runtime contract is classified"
        );
        assert!(data.correctness_counts.values().sum::<usize>() > 1_000);
        let completion = data.completion.as_ref().expect("current DEM completion");
        assert_eq!(completion.scope_id, COMPLETION_SCOPE);
        assert!(rendered.contains(&format!(
            "Stim parity `{}`",
            completion.parity_outcome.as_str()
        )));
        assert!(rendered.contains(&format!(
            "Stab regression `{}`",
            completion.regression_outcome.as_str()
        )));
        assert!(!rendered.contains("Formal repaired-contract completion: **not started**"));
        assert!(rendered.contains(&data.performance_digest));
        assert!(rendered.contains(&data.parity_policy_sha256));
        assert!(rendered.contains(&data.regression_policy_sha256));
        assert!(rendered.contains(&data.regression_baselines_sha256));
    }

    #[test]
    fn completion_checkpoint_rejects_malformed_current_identity() {
        let checkpoint = CompletionCheckpoint {
            schema_version: COMPLETION_CHECKPOINT_SCHEMA_VERSION,
            current: Some(CurrentCompletion {
                scope_id: COMPLETION_SCOPE.to_string(),
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
            validate_completion_checkpoint(&checkpoint, &"2".repeat(64), &"3".repeat(64)).is_err()
        );
    }
}
