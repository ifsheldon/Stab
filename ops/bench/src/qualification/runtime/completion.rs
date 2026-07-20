use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Args;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::artifact::{DirectQualificationArtifactPath, QualificationOutput, RepositoryBinding};
use super::group::{BaselineEligibility, GroupContract};
use super::invocation::WorkerIdentityEvidence;
use super::probe::AdapterProbeReceipt;
use super::run::{
    ClaimClass, QualificationReport, QualificationTier, RepositoryEvidence, sha256_hex,
};
use super::statistics::GateOutcome;
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::root::RepoRoot;

mod model;
mod validation;
mod workflow;

use model::{
    ArtifactReceipt, CompletionEnvironmentEvidence, CompletionPreflight, CompletionReceipt,
    CompletionStep, CompletionStepKind, CompletionStepResult, EvidenceDirectoryReceipt,
};

const COMPLETION_SCHEMA_VERSION: u32 = 1;
const PREFLIGHT_SCHEMA_VERSION: u32 = 1;
const DEFAULT_OUTPUT: &str = "target/benchmarks/qualification/completion-latest";
const MAX_COMPLETION_REPORT_BYTES: usize = 8 << 20;
const MAX_COMPLETION_PREFLIGHT_BYTES: usize = 2 << 20;
const MAX_COMPLETION_MARKDOWN_BYTES: usize = 4 << 20;
const MAX_BOUND_ARTIFACT_BYTES: usize = 4 << 20;
const MAX_SOURCE_REPORTS: usize = 128;

#[derive(Clone, Debug, Args)]
pub(crate) struct CompletionArgs {
    /// Source-owned promotable runtime group being closed.
    #[arg(long)]
    group: String,

    /// Full-tier source report; repeat exactly once per source-owned scale.
    #[arg(long = "full-input", required = true)]
    full_inputs: Vec<PathBuf>,

    /// Soak-tier source report; repeat exactly once per source-owned scale.
    #[arg(long = "soak-input", required = true)]
    soak_inputs: Vec<PathBuf>,

    /// Replayed full-tier scale-family rollup.
    #[arg(long)]
    full_rollup: PathBuf,

    /// Replayed soak-tier scale-family rollup.
    #[arg(long)]
    soak_rollup: PathBuf,

    /// Atomic completion-receipt directory beside its source evidence.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    out: PathBuf,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct CompletionReportArgs {
    /// Published completion-receipt directory to replay.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    input: PathBuf,
}

struct SelectedReport {
    path: DirectQualificationArtifactPath,
    tier: QualificationTier,
    scale_id: String,
    workers: WorkerIdentityEvidence,
    artifacts: Vec<ArtifactReceipt>,
    correctness_binding: Arc<super::correctness::CorrectnessArtifactBinding>,
}

struct AdmittedCompletionPaths {
    output: DirectQualificationArtifactPath,
    full_inputs: Vec<DirectQualificationArtifactPath>,
    soak_inputs: Vec<DirectQualificationArtifactPath>,
    full_rollup: DirectQualificationArtifactPath,
    soak_rollup: DirectQualificationArtifactPath,
}

struct ExecutionOptions<'a> {
    generated_unix_epoch_seconds: u64,
    existing_report_json: Option<&'a [u8]>,
    existing_preflight_json: Option<&'a [u8]>,
    existing_markdown: Option<&'a [u8]>,
}

struct SourceSelectionContext<'a> {
    root: &'a RepoRoot,
    source_root: &'a RepoRoot,
    repository: &'a RepositoryBinding,
    contract: &'a GroupContract,
    expected_commit: &'a str,
    performance_inventory_sha256: &'a str,
    correctness_inventory_sha256: &'a str,
}

struct CompletionPublication<'a> {
    root: &'a RepoRoot,
    repository: &'a RepositoryBinding,
    output_path: &'a DirectQualificationArtifactPath,
    receipt: &'a CompletionReceipt,
    report_json: &'a [u8],
    preflight_json: &'a [u8],
    markdown: &'a str,
    existing_report_json: Option<&'a [u8]>,
    existing_preflight_json: Option<&'a [u8]>,
    existing_markdown: Option<&'a [u8]>,
    correctness_bindings: &'a [Arc<super::correctness::CorrectnessArtifactBinding>],
}

impl CompletionPublication<'_> {
    fn publish(
        self,
        repository_before: &super::git::RepositoryState,
    ) -> Result<(), CompletionError> {
        let root = self.root;
        let correctness_bindings = self.correctness_bindings;
        self.publish_production_with(|repository| {
            repository.require_current(root)?;
            let source_root = repository.descriptor_root(root)?;
            let repository_at_publication = super::git::repository_state(&source_root)?;
            repository.require_current(root)?;
            require_same_clean_repository(repository_before, &repository_at_publication)?;
            require_current_correctness(correctness_bindings)
        })
    }

    fn publish_production_with<ValidateSource>(
        self,
        validate_source: ValidateSource,
    ) -> Result<(), CompletionError>
    where
        ValidateSource: FnMut(&super::artifact::BoundRepository<'_>) -> Result<(), CompletionError>,
    {
        let replay = self.existing_report_json.is_some();
        self.publish_with(
            || Ok(()),
            |output| commit_completion_output(output, replay, validate_source),
        )
    }

    fn publish_with<BeforeCommit, Commit>(
        self,
        before_commit: BeforeCommit,
        commit: Commit,
    ) -> Result<(), CompletionError>
    where
        BeforeCommit: FnOnce() -> Result<(), CompletionError>,
        Commit: FnOnce(QualificationOutput) -> Result<(), CompletionError>,
    {
        let mut output = if self.existing_report_json.is_some() {
            QualificationOutput::begin_with_repository(
                self.root,
                self.repository,
                self.output_path,
            )?
        } else {
            QualificationOutput::begin_new_with_repository(
                self.root,
                self.repository,
                self.output_path,
            )?
        };
        if let Some(existing) = self.existing_report_json {
            output.require_current_artifact("report.json", existing)?;
        }
        if let Some(existing) = self.existing_preflight_json {
            output.require_current_artifact("preflight.json", existing)?;
        }
        if let Some(existing) = self.existing_markdown {
            output.require_current_artifact("report.md", existing)?;
        }
        output.write("report.json", self.report_json)?;
        output.write("preflight.json", self.preflight_json)?;
        output.write("report.md", self.markdown.as_bytes())?;
        require_current_evidence(&mut output, self.receipt)?;
        before_commit()?;
        commit(output)
    }
}

fn commit_completion_output<ValidateSource>(
    output: QualificationOutput,
    replay: bool,
    mut validate_source: ValidateSource,
) -> Result<(), CompletionError>
where
    ValidateSource: FnMut(&super::artifact::BoundRepository<'_>) -> Result<(), CompletionError>,
{
    let validate = |repository: &super::artifact::BoundRepository<'_>| {
        validate_source(repository).map_err(|_| {
            super::artifact::ArtifactError::ExternalSourceChanged("completion source evidence")
        })
    };
    if replay {
        output.commit_with_source_validation(validate)
    } else {
        output.commit_new_with_source_validation(validate)
    }
    .map_err(CompletionError::Artifact)
}

#[derive(Clone, Copy)]
struct ReplayContext<'a> {
    root: &'a RepoRoot,
    repository: &'a RepositoryBinding,
    performance_inventory_sha256: &'a str,
    correctness_inventory_sha256: &'a str,
    contract: &'a GroupContract,
    repository_commit: &'a str,
}

pub(super) fn run(
    root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: CompletionArgs,
) -> Result<PathBuf, CompletionError> {
    execute(
        root,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        args,
        ExecutionOptions {
            generated_unix_epoch_seconds: current_unix_epoch_seconds()?,
            existing_report_json: None,
            existing_preflight_json: None,
            existing_markdown: None,
        },
        None,
    )
}

pub(super) fn run_report(
    root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: CompletionReportArgs,
) -> Result<PathBuf, CompletionError> {
    let input_path = DirectQualificationArtifactPath::try_new(&args.input)?;
    let live_repository = RepositoryBinding::open(root)?;
    let report_json = super::artifact::read_artifact_bounded_with_repository(
        root,
        &live_repository,
        &input_path,
        "report.json",
        MAX_COMPLETION_REPORT_BYTES,
    )?;
    let preflight_json = super::artifact::read_artifact_bounded_with_repository(
        root,
        &live_repository,
        &input_path,
        "preflight.json",
        MAX_COMPLETION_PREFLIGHT_BYTES,
    )?;
    let markdown = super::artifact::read_artifact_bounded_with_repository(
        root,
        &live_repository,
        &input_path,
        "report.md",
        MAX_COMPLETION_MARKDOWN_BYTES,
    )?;
    let receipt: CompletionReceipt = parse_canonical(&report_json)?;
    let preflight: CompletionPreflight = parse_canonical(&preflight_json)?;
    validate_existing_boundary(
        &receipt,
        &preflight,
        &report_json,
        input_path.as_path(),
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?;
    let replay_args = arguments_from_receipt(&receipt)?;
    execute(
        root,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        replay_args,
        ExecutionOptions {
            generated_unix_epoch_seconds: receipt.generated_unix_epoch_seconds,
            existing_report_json: Some(&report_json),
            existing_preflight_json: Some(&preflight_json),
            existing_markdown: Some(&markdown),
        },
        Some(live_repository),
    )
}

fn execute(
    root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: CompletionArgs,
    options: ExecutionOptions<'_>,
    repository: Option<RepositoryBinding>,
) -> Result<PathBuf, CompletionError> {
    let paths = admit_completion_paths(&args)?;
    let live_repository = match repository {
        Some(repository) => repository,
        None => RepositoryBinding::open(root)?,
    };
    if options.existing_report_json.is_none() {
        QualificationOutput::require_absent_with_repository(root, &live_repository, &paths.output)?;
    }
    let source_root = live_repository.descriptor_root(root)?;
    let repository_before = super::run::bound_repository_state(root, &live_repository)?;
    require_clean_repository(&repository_before)?;
    let resolved = super::group::load_group(
        &source_root,
        expected_performance_inventory_sha256,
        &args.group,
    )?;
    live_repository.require_current(root)?;
    require_completion_group(&resolved.contract)?;
    if resolved.contract.scales.len() > MAX_SOURCE_REPORTS {
        return Err(CompletionError::SourceReportCount {
            actual: resolved.contract.scales.len(),
            expected: MAX_SOURCE_REPORTS,
        });
    }
    let selection = SourceSelectionContext {
        root,
        source_root: &source_root,
        repository: &live_repository,
        contract: &resolved.contract,
        expected_commit: &repository_before.commit,
        performance_inventory_sha256: expected_performance_inventory_sha256,
        correctness_inventory_sha256: expected_correctness_inventory_sha256,
    };
    let full = select_reports(&selection, QualificationTier::Full, &paths.full_inputs)?;
    let soak = select_reports(&selection, QualificationTier::Soak, &paths.soak_inputs)?;
    let correctness_bindings = full
        .iter()
        .chain(&soak)
        .map(|report| Arc::clone(&report.correctness_binding))
        .collect::<Vec<_>>();
    let replay = ReplayContext {
        root,
        repository: &live_repository,
        performance_inventory_sha256: expected_performance_inventory_sha256,
        correctness_inventory_sha256: expected_correctness_inventory_sha256,
        contract: &resolved.contract,
        repository_commit: &repository_before.commit,
    };
    let closure = workflow::run(
        &replay,
        &args.group,
        full,
        soak,
        paths.full_rollup,
        paths.soak_rollup,
        &mut workflow::ProductionActions,
    )?;

    let repository_after = super::run::bound_repository_state(root, &live_repository)?;
    require_same_clean_repository(&repository_before, &repository_after)?;
    let output = path_text(paths.output.as_path())?;
    let receipt = CompletionReceipt {
        schema_version: COMPLETION_SCHEMA_VERSION,
        output,
        generated_unix_epoch_seconds: options.generated_unix_epoch_seconds,
        group_id: args.group,
        group_contract_sha256: resolved.source_sha256,
        performance_inventory_sha256: expected_performance_inventory_sha256.to_string(),
        correctness_inventory_sha256: expected_correctness_inventory_sha256.to_string(),
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        repository: RepositoryEvidence {
            commit_before: repository_before.commit.clone(),
            commit_after: repository_after.commit,
            local_modifications_before: repository_before.local_modifications,
            local_modifications_after: repository_after.local_modifications,
        },
        environment: CompletionEnvironmentEvidence {
            host_policy_sha256: closure.full_rollup.host_policy_sha256.clone(),
            host_profile_id: closure.full_rollup.host_profile_id.clone(),
            architecture: closure.full_rollup.architecture.clone(),
            cpu_identity: closure.full_rollup.cpu_identity.clone(),
            target_triple: closure.full_rollup.target_triple.clone(),
            toolchain_sha256: closure.full_rollup.toolchain_sha256.clone(),
        },
        workers: closure.workers,
        correctness_preflight: closure.full_rollup.correctness_preflight.clone(),
        source_reports: closure.source_reports,
        rollups: closure.rollups,
        steps: closure.steps,
    };
    validation::validate(&receipt)?;
    let report_json = canonical_json(&receipt)?;
    let preflight = completion_preflight(&receipt, &report_json)?;
    let preflight_json = canonical_json(&preflight)?;
    let markdown = render_markdown(&receipt, &sha256_hex(&report_json));
    if options
        .existing_report_json
        .is_some_and(|existing| existing != report_json)
        || options
            .existing_preflight_json
            .is_some_and(|existing| existing != preflight_json)
        || options
            .existing_markdown
            .is_some_and(|existing| existing != markdown.as_bytes())
    {
        return Err(CompletionError::Reconstruction);
    }

    CompletionPublication {
        root,
        repository: &live_repository,
        output_path: &paths.output,
        receipt: &receipt,
        report_json: &report_json,
        preflight_json: &preflight_json,
        markdown: &markdown,
        existing_report_json: options.existing_report_json,
        existing_preflight_json: options.existing_preflight_json,
        existing_markdown: options.existing_markdown,
        correctness_bindings: &correctness_bindings,
    }
    .publish(&repository_before)?;
    Ok(paths.output.into_path_buf())
}

fn admit_completion_paths(
    args: &CompletionArgs,
) -> Result<AdmittedCompletionPaths, CompletionError> {
    let output = DirectQualificationArtifactPath::try_new(&args.out)?;
    let full_inputs = args
        .full_inputs
        .iter()
        .map(|path| DirectQualificationArtifactPath::try_new(path))
        .collect::<Result<Vec<_>, _>>()?;
    let soak_inputs = args
        .soak_inputs
        .iter()
        .map(|path| DirectQualificationArtifactPath::try_new(path))
        .collect::<Result<Vec<_>, _>>()?;
    let full_rollup = DirectQualificationArtifactPath::try_new(&args.full_rollup)?;
    let soak_rollup = DirectQualificationArtifactPath::try_new(&args.soak_rollup)?;
    require_unique_paths(
        &output,
        &full_inputs,
        &soak_inputs,
        &full_rollup,
        &soak_rollup,
    )?;
    Ok(AdmittedCompletionPaths {
        output,
        full_inputs,
        soak_inputs,
        full_rollup,
        soak_rollup,
    })
}

fn select_reports(
    context: &SourceSelectionContext<'_>,
    tier: QualificationTier,
    inputs: &[DirectQualificationArtifactPath],
) -> Result<Vec<SelectedReport>, CompletionError> {
    if inputs.len() != context.contract.scales.len() {
        return Err(CompletionError::SourceReportCount {
            actual: inputs.len(),
            expected: context.contract.scales.len(),
        });
    }
    let mut by_scale = BTreeMap::new();
    for path in inputs {
        let evidence = super::report::load_validated_published_evidence(
            context.root,
            context.source_root,
            context.repository,
            path,
            context.performance_inventory_sha256,
            context.correctness_inventory_sha256,
        )?;
        validate_source_report(
            &evidence.report,
            context.contract,
            tier,
            context.expected_commit,
        )?;
        let artifacts = read_artifact_receipts(context.root, context.repository, path)?;
        require_artifact_digest(&artifacts, "report.json", &evidence.report_sha256)?;
        require_artifact_digest(&artifacts, "preflight.json", &evidence.preflight_sha256)?;
        let selected = SelectedReport {
            path: path.clone(),
            tier: evidence.report.tier,
            scale_id: evidence.report.scale_id,
            workers: evidence.report.workers,
            artifacts,
            correctness_binding: evidence.correctness_binding,
        };
        if by_scale
            .insert(selected.scale_id.clone(), selected)
            .is_some()
        {
            return Err(CompletionError::DuplicateScale);
        }
    }
    let mut ordered = Vec::with_capacity(context.contract.scales.len());
    for scale in &context.contract.scales {
        let scale_id = scale.id.to_string();
        ordered.push(
            by_scale
                .remove(&scale_id)
                .ok_or(CompletionError::MissingScale(scale_id))?,
        );
    }
    if !by_scale.is_empty() {
        return Err(CompletionError::UnknownScale(
            by_scale.into_keys().next().unwrap_or_default(),
        ));
    }
    Ok(ordered)
}

fn validate_source_report(
    report: &QualificationReport,
    contract: &GroupContract,
    tier: QualificationTier,
    expected_commit: &str,
) -> Result<(), CompletionError> {
    if report.group_id != contract.id.to_string()
        || report.claim_class != ClaimClass::PromotablePerformance
        || report.baseline_eligibility != BaselineEligibility::ThresholdEligible
        || report.tier != tier
        || !report.promotable
        || report.repository.commit_before != expected_commit
        || report.repository.commit_after != expected_commit
        || report.repository.local_modifications_before
        || report.repository.local_modifications_after
    {
        return Err(CompletionError::SourceReportIdentity);
    }
    Ok(())
}

fn validate_rollup(
    evidence: &super::rollup::RollupReplayEvidence,
    contract: &GroupContract,
    tier: QualificationTier,
    expected_commit: &str,
    source_reports: &[EvidenceDirectoryReceipt],
    workers: &WorkerIdentityEvidence,
) -> Result<(), CompletionError> {
    if evidence.group_id != contract.id.to_string()
        || evidence.tier != tier
        || evidence.stab_commit != expected_commit
        || evidence.workers != *workers
        || evidence.overall_outcome != GateOutcome::Passed
        || evidence.sources.len() != contract.scales.len()
    {
        return Err(CompletionError::RollupIdentity);
    }
    let expected = source_reports
        .iter()
        .filter(|receipt| receipt.tier == tier)
        .map(|receipt| {
            Ok((
                receipt
                    .scale_id
                    .clone()
                    .ok_or(CompletionError::RollupIdentity)?,
                receipt.path.clone(),
                artifact_digest(&receipt.artifacts, "report.json")?.to_string(),
                artifact_digest(&receipt.artifacts, "preflight.json")?.to_string(),
            ))
        })
        .collect::<Result<Vec<_>, CompletionError>>()?;
    let actual = evidence
        .sources
        .iter()
        .map(|source| {
            Ok((
                source.scale_id.clone(),
                path_text(&source.path)?,
                source.report_sha256.clone(),
                source.preflight_sha256.clone(),
            ))
        })
        .collect::<Result<Vec<_>, CompletionError>>()?;
    if actual != expected {
        return Err(CompletionError::RollupSources);
    }
    Ok(())
}

fn require_matching_rollup_identity(
    full: &super::rollup::RollupReplayEvidence,
    soak: &super::rollup::RollupReplayEvidence,
) -> Result<(), CompletionError> {
    if full.host_policy_sha256 != soak.host_policy_sha256
        || full.host_profile_id != soak.host_profile_id
        || full.architecture != soak.architecture
        || full.cpu_identity != soak.cpu_identity
        || full.target_triple != soak.target_triple
        || full.toolchain_sha256 != soak.toolchain_sha256
        || full.workers != soak.workers
        || full.correctness_preflight != soak.correctness_preflight
    {
        return Err(CompletionError::MixedRollupIdentity);
    }
    Ok(())
}

fn require_completion_group(contract: &GroupContract) -> Result<(), CompletionError> {
    if contract.claim_class != ClaimClass::PromotablePerformance
        || contract.baseline_eligibility != BaselineEligibility::ThresholdEligible
        || contract.scales.is_empty()
        || contract.measurement_ids.is_empty()
    {
        return Err(CompletionError::GroupDisposition(contract.id.to_string()));
    }
    Ok(())
}

fn shared_workers(
    full: &[SelectedReport],
    soak: &[SelectedReport],
) -> Result<WorkerIdentityEvidence, CompletionError> {
    let first = full
        .first()
        .ok_or(CompletionError::SourceReportCount {
            actual: 0,
            expected: 1,
        })?
        .workers
        .clone();
    if full
        .iter()
        .chain(soak)
        .any(|selected| selected.workers != first)
    {
        return Err(CompletionError::WorkerIdentity);
    }
    Ok(first)
}

fn validate_probe(
    probe: &AdapterProbeReceipt,
    workers: &WorkerIdentityEvidence,
    expected_group: &str,
) -> Result<(), CompletionError> {
    if probe.runtime_group_id != expected_group
        || probe.evidence_mode != "timing"
        || probe.work_count == 0
        || probe.stim_source_sha256 != workers.stim_source_sha256
        || probe.stim_build_fingerprint != workers.stim_build_fingerprint
        || probe.stim_binary_sha256 != workers.stim_binary_sha256
        || probe.stab_source_sha256 != workers.stab_source_sha256
    {
        return Err(CompletionError::ProbeIdentity);
    }
    Ok(())
}

fn checked_action<T, E, F>(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_commit: &str,
    name: &'static str,
    action: F,
) -> Result<T, CompletionError>
where
    E: Display,
    F: FnOnce(&RepoRoot) -> Result<T, E>,
{
    let source_root = repository.descriptor_root(root)?;
    let before = super::run::bound_repository_state(root, repository)?;
    require_expected_repository(&before, expected_commit)?;
    let result = action(&source_root).map_err(|source| CompletionError::Action {
        name,
        detail: source.to_string(),
    })?;
    let after = super::run::bound_repository_state(root, repository)?;
    require_expected_repository(&after, expected_commit)?;
    Ok(result)
}

fn push_step(
    steps: &mut Vec<CompletionStep>,
    kind: CompletionStepKind,
    repository_commit: &str,
    canonical_arguments: Vec<String>,
    inputs: Vec<ArtifactReceipt>,
    outputs: Vec<ArtifactReceipt>,
    result: CompletionStepResult,
) {
    steps.push(CompletionStep {
        index: steps.len(),
        kind,
        repository_commit: repository_commit.to_string(),
        canonical_arguments,
        inputs,
        exit_status: 0,
        outputs,
        result,
    });
}

fn read_artifact_receipts(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    path: &DirectQualificationArtifactPath,
) -> Result<Vec<ArtifactReceipt>, CompletionError> {
    ["report.json", "preflight.json", "report.md"]
        .into_iter()
        .map(|name| {
            let bytes = super::artifact::read_artifact_bounded_with_repository(
                root,
                repository,
                path,
                name,
                MAX_BOUND_ARTIFACT_BYTES,
            )?;
            Ok(ArtifactReceipt {
                path: path_text(path.as_path())?,
                name: name.to_string(),
                bytes: u64::try_from(bytes.len()).map_err(|_| CompletionError::ArtifactSize)?,
                sha256: sha256_hex(&bytes),
            })
        })
        .collect()
}

fn require_current_evidence(
    output: &mut QualificationOutput,
    receipt: &CompletionReceipt,
) -> Result<(), CompletionError> {
    for directory in receipt.source_reports.iter().chain(&receipt.rollups) {
        let path = DirectQualificationArtifactPath::try_new(Path::new(&directory.path))?;
        for artifact in &directory.artifacts {
            let name = artifact_name(&artifact.name)?;
            output.require_sibling_artifact_digest(
                &path,
                name,
                &artifact.sha256,
                MAX_BOUND_ARTIFACT_BYTES,
            )?;
        }
    }
    Ok(())
}

fn require_current_correctness(
    bindings: &[Arc<super::correctness::CorrectnessArtifactBinding>],
) -> Result<(), CompletionError> {
    for binding in bindings {
        binding
            .require_current()
            .map_err(|_| CompletionError::CorrectnessEvidenceChanged)?;
    }
    Ok(())
}

fn completion_preflight(
    receipt: &CompletionReceipt,
    report_json: &[u8],
) -> Result<CompletionPreflight, CompletionError> {
    Ok(CompletionPreflight {
        schema_version: PREFLIGHT_SCHEMA_VERSION,
        report_sha256: sha256_hex(report_json),
        output: receipt.output.clone(),
        group_id: receipt.group_id.clone(),
        performance_inventory_sha256: receipt.performance_inventory_sha256.clone(),
        correctness_inventory_sha256: receipt.correctness_inventory_sha256.clone(),
        stab_commit: receipt.repository.commit_after.clone(),
        workers: receipt.workers.clone(),
        source_reports: receipt.source_reports.clone(),
        rollups: receipt.rollups.clone(),
        step_count: receipt.steps.len(),
        steps_sha256: sha256_hex(&canonical_json(&receipt.steps)?),
    })
}

fn validate_existing_boundary(
    receipt: &CompletionReceipt,
    preflight: &CompletionPreflight,
    report_json: &[u8],
    input: &Path,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
) -> Result<(), CompletionError> {
    validation::validate(receipt)?;
    if receipt.schema_version != COMPLETION_SCHEMA_VERSION
        || preflight.schema_version != PREFLIGHT_SCHEMA_VERSION
        || receipt.output != path_text(input)?
        || receipt.performance_inventory_sha256 != expected_performance_inventory_sha256
        || receipt.correctness_inventory_sha256 != expected_correctness_inventory_sha256
        || receipt.stim_tag != STIM_TAG
        || receipt.stim_commit != STIM_COMMIT
        || receipt.repository.commit_before != receipt.repository.commit_after
        || receipt.repository.local_modifications_before
        || receipt.repository.local_modifications_after
        || *preflight != completion_preflight(receipt, report_json)?
    {
        return Err(CompletionError::Boundary);
    }
    Ok(())
}

fn arguments_from_receipt(receipt: &CompletionReceipt) -> Result<CompletionArgs, CompletionError> {
    let full_inputs = receipt
        .source_reports
        .iter()
        .filter(|source| source.tier == QualificationTier::Full)
        .map(|source| PathBuf::from(&source.path))
        .collect();
    let soak_inputs = receipt
        .source_reports
        .iter()
        .filter(|source| source.tier == QualificationTier::Soak)
        .map(|source| PathBuf::from(&source.path))
        .collect();
    let full_rollup = unique_rollup(receipt, QualificationTier::Full)?;
    let soak_rollup = unique_rollup(receipt, QualificationTier::Soak)?;
    Ok(CompletionArgs {
        group: receipt.group_id.clone(),
        full_inputs,
        soak_inputs,
        full_rollup,
        soak_rollup,
        out: PathBuf::from(&receipt.output),
    })
}

fn unique_rollup(
    receipt: &CompletionReceipt,
    tier: QualificationTier,
) -> Result<PathBuf, CompletionError> {
    let mut matching = receipt.rollups.iter().filter(|rollup| rollup.tier == tier);
    let path = matching
        .next()
        .ok_or(CompletionError::RollupIdentity)?
        .path
        .clone();
    if matching.next().is_some() {
        return Err(CompletionError::RollupIdentity);
    }
    Ok(PathBuf::from(path))
}

fn require_unique_paths(
    output: &DirectQualificationArtifactPath,
    full: &[DirectQualificationArtifactPath],
    soak: &[DirectQualificationArtifactPath],
    full_rollup: &DirectQualificationArtifactPath,
    soak_rollup: &DirectQualificationArtifactPath,
) -> Result<(), CompletionError> {
    let mut paths = BTreeSet::new();
    for path in full.iter().chain(soak).chain([full_rollup, soak_rollup]) {
        if path == output || !paths.insert(path.clone()) {
            return Err(CompletionError::DuplicatePath(path.clone().into_path_buf()));
        }
    }
    Ok(())
}

fn require_artifact_digest(
    artifacts: &[ArtifactReceipt],
    name: &str,
    expected: &str,
) -> Result<(), CompletionError> {
    if artifact_digest(artifacts, name)? == expected {
        Ok(())
    } else {
        Err(CompletionError::ArtifactBinding(name.to_string()))
    }
}

fn artifact_digest<'a>(
    artifacts: &'a [ArtifactReceipt],
    name: &str,
) -> Result<&'a str, CompletionError> {
    let mut matching = artifacts.iter().filter(|artifact| artifact.name == name);
    let digest = matching
        .next()
        .ok_or_else(|| CompletionError::ArtifactBinding(name.to_string()))?
        .sha256
        .as_str();
    if matching.next().is_some() {
        return Err(CompletionError::ArtifactBinding(name.to_string()));
    }
    Ok(digest)
}

fn artifact_name(value: &str) -> Result<&'static str, CompletionError> {
    match value {
        "report.json" => Ok("report.json"),
        "preflight.json" => Ok("preflight.json"),
        "report.md" => Ok("report.md"),
        _ => Err(CompletionError::ArtifactBinding(value.to_string())),
    }
}

fn probe_arguments(probe: &AdapterProbeReceipt) -> Vec<String> {
    vec![
        "qualification-probe".to_string(),
        "--group".to_string(),
        probe.probe_id.clone(),
        "--iterations".to_string(),
        probe.iteration_count.to_string(),
        "--work-items".to_string(),
        probe.work_items.to_string(),
        "--evidence-mode".to_string(),
        probe.evidence_mode.clone(),
    ]
}

fn require_clean_repository(state: &super::git::RepositoryState) -> Result<(), CompletionError> {
    if state.local_modifications {
        Err(CompletionError::DirtyRepository)
    } else {
        Ok(())
    }
}

fn require_expected_repository(
    state: &super::git::RepositoryState,
    expected_commit: &str,
) -> Result<(), CompletionError> {
    require_clean_repository(state)?;
    if state.commit == expected_commit {
        Ok(())
    } else {
        Err(CompletionError::RepositoryChanged {
            before: expected_commit.to_string(),
            after: state.commit.clone(),
        })
    }
}

fn require_same_clean_repository(
    before: &super::git::RepositoryState,
    after: &super::git::RepositoryState,
) -> Result<(), CompletionError> {
    require_clean_repository(before)?;
    require_clean_repository(after)?;
    if before.commit == after.commit {
        Ok(())
    } else {
        Err(CompletionError::RepositoryChanged {
            before: before.commit.clone(),
            after: after.commit.clone(),
        })
    }
}

fn canonical_json(value: &impl Serialize) -> Result<Vec<u8>, CompletionError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn parse_canonical<T>(bytes: &[u8]) -> Result<T, CompletionError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(CompletionError::Boundary);
    }
    let value = serde_json::from_slice(bytes)?;
    if canonical_json(&value)? != bytes {
        return Err(CompletionError::NonCanonical);
    }
    Ok(value)
}

fn path_text(path: &Path) -> Result<String, CompletionError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| CompletionError::InvalidPath(path.to_path_buf()))
}

fn current_unix_epoch_seconds() -> Result<u64, CompletionError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(CompletionError::Clock)
}

fn render_markdown(receipt: &CompletionReceipt, report_sha256: &str) -> String {
    let group = super::markdown::inline_code(&receipt.group_id);
    let commit = super::markdown::inline_code(&receipt.repository.commit_after);
    let architecture = super::markdown::inline_code(&receipt.environment.architecture);
    let cpu_identity = super::markdown::inline_code(&receipt.environment.cpu_identity);
    let target = super::markdown::inline_code(&receipt.environment.target_triple);
    let digest = super::markdown::inline_code(report_sha256);
    let mut markdown = format!(
        "# Performance Qualification Completion Receipt\n\n- Group: {group}\n- Stab commit: {commit}\n- Architecture: {architecture} ({target})\n- CPU identity: {cpu_identity}\n- Source reports: `{}`\n- Rollups: `{}`\n- Machine-checked steps: `{}`\n- Completion report SHA-256: {digest}\n\nHuman milestone audit and independent code review are intentionally not self-certified by this receipt.\n\n## Steps\n\n| # | Operation | Exit | Inputs | Outputs |\n| ---: | --- | ---: | ---: | ---: |\n",
        receipt.source_reports.len(),
        receipt.rollups.len(),
        receipt.steps.len(),
    );
    for step in &receipt.steps {
        markdown.push_str(&format!(
            "| {} | `{:?}` | {} | {} | {} |\n",
            step.index,
            step.kind,
            step.exit_status,
            step.inputs.len(),
            step.outputs.len(),
        ));
    }
    markdown
}

#[derive(Debug, Error)]
pub(super) enum CompletionError {
    #[error("qualification completion requires a clean repository")]
    DirtyRepository,
    #[error("qualification completion repository changed from {before} to {after}")]
    RepositoryChanged { before: String, after: String },
    #[error("qualification group {0} is not eligible for completion evidence")]
    GroupDisposition(String),
    #[error("qualification completion has {actual} source reports, expected {expected}")]
    SourceReportCount { actual: usize, expected: usize },
    #[error("qualification completion repeats a source scale")]
    DuplicateScale,
    #[error("qualification completion is missing source scale {0}")]
    MissingScale(String),
    #[error("qualification completion contains unknown source scale {0}")]
    UnknownScale(String),
    #[error("qualification completion repeats or collides with evidence path {0}")]
    DuplicatePath(PathBuf),
    #[error("qualification source report identity is stale or nonpromotable")]
    SourceReportIdentity,
    #[error("qualification workers differ across reports or reproducibility evidence")]
    WorkerIdentity,
    #[error("qualification adapter probe does not match the source report workers")]
    ProbeIdentity,
    #[error("qualification regression did not gate every source-owned measurement")]
    RegressionDisposition,
    #[error("qualification rollup identity is stale or nonpassing")]
    RollupIdentity,
    #[error("qualification rollup source bindings differ from the replayed reports")]
    RollupSources,
    #[error(
        "full and soak rollups do not share exact host, toolchain, correctness, and worker identity"
    )]
    MixedRollupIdentity,
    #[error("qualification replay changed source evidence at {0}")]
    NonIdempotentReplay(PathBuf),
    #[error("qualification completion artifact {0} is missing, duplicated, or stale")]
    ArtifactBinding(String),
    #[error("qualification correctness evidence changed during completion publication")]
    CorrectnessEvidenceChanged,
    #[error("qualification completion artifact size does not fit u64")]
    ArtifactSize,
    #[error("qualification completion path is not valid UTF-8: {0}")]
    InvalidPath(PathBuf),
    #[error("qualification completion report or preflight boundary is invalid")]
    Boundary,
    #[error("qualification completion JSON is not canonical")]
    NonCanonical,
    #[error("qualification completion receipt cannot be reconstructed from current evidence")]
    Reconstruction,
    #[error("qualification completion action {name} failed: {detail}")]
    Action { name: &'static str, detail: String },
    #[error("system clock is before the Unix epoch: {0}")]
    Clock(std::time::SystemTimeError),
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),
    #[error(transparent)]
    Git(#[from] super::git::GitError),
    #[error(transparent)]
    Group(#[from] super::group::GroupError),
    #[error(transparent)]
    Report(#[from] super::report::ReportError),
    #[error("failed to serialize qualification completion evidence: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests;
