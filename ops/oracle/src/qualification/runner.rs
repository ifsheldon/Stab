use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::artifact::QualificationOutputDir;
use super::executables::{QualificationExecutables, QualificationMetadataExecutables};
use super::model::{Comparator, EvidenceCase, FeatureId, QualificationManifest, SelectorKind};
use super::receipt::{RequestedCase, RunRequestReceipt, RunRequestReceiptInput};
use super::report::{
    CaseOutcome, CaseResult, DomainComparatorCount, DomainDispositionCount, QualificationReport,
    ResourceExecution, SelectionSummary,
};
use super::tier::{CaseSelection, QualificationTier, select_cases};
use crate::{RepoRoot, fixtures::QualificationFixtureRunner};

const MAX_FAILURE_REASON_ARTIFACT_BYTES: usize = 4 << 10;

#[derive(Debug, Error)]
pub(crate) enum RunError {
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),

    #[error(transparent)]
    Report(#[from] super::report::ReportError),

    #[error("unknown qualification feature {0:?}")]
    UnknownFeature(String),

    #[error("unknown qualification case {0:?}")]
    UnknownCase(String),

    #[error("explicit selection contains non-executable cases: {0}")]
    NonExecutableSelection(String),

    #[error("explicit selection contains cases outside the requested tier: {0}")]
    OutOfTierSelection(String),

    #[error("explicit case and feature filters disagree for cases: {0}")]
    CaseFeatureMismatch(String),

    #[error("selection contains deferred cases without --allow-deferred: {0}")]
    DeferredSelection(String),

    #[error("qualification selection contains no executable cases")]
    EmptySelection,

    #[error("failed to inspect qualification environment: {0}")]
    Environment(Box<str>),

    #[error("correctness qualification was interrupted before evidence publication: {0}")]
    Interrupted(Box<str>),

    #[error("statistical qualification plan is invalid: {0}")]
    Statistics(Box<str>),

    #[error("qualification report cannot be bound to its manifest selection: {0}")]
    ReportContract(Box<str>),

    #[error("correctness qualification completed with {failed} failed case(s); report: {report}")]
    CasesFailed { failed: usize, report: PathBuf },
}

pub(super) struct RunRequest {
    pub(super) tier: QualificationTier,
    pub(super) features: Vec<String>,
    pub(super) cases: Vec<String>,
    pub(super) allow_deferred: bool,
    pub(super) output: PathBuf,
}

fn ensure_supported_qualification_host() -> Result<(), RunError> {
    validate_qualification_host(std::env::consts::OS)
}

fn validate_qualification_host(operating_system: &str) -> Result<(), RunError> {
    if operating_system == "linux" {
        Ok(())
    } else {
        Err(RunError::Environment(
            "correctness qualification requires Linux process-group termination and atomic directory exchange"
                .into(),
        ))
    }
}

pub(super) fn run(
    root: &RepoRoot,
    manifest: &QualificationManifest,
    request: RunRequest,
) -> Result<(), RunError> {
    ensure_supported_qualification_host()?;
    let output = QualificationOutputDir::parse(root, &request.output)?.begin_run()?;
    let features = parse_features(&request.features)?;
    let case_ids = parse_case_ids(manifest, &request.cases)?;
    validate_case_feature_filters(manifest, &features, &case_ids)?;
    let borrowed_ids = case_ids.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let selection = select_cases(
        &manifest.evidence_cases,
        request.tier,
        &features,
        &borrowed_ids,
    );
    validate_selection_policy(
        &selection,
        !request.cases.is_empty(),
        request.allow_deferred,
    )?;

    let metadata_executables = QualificationMetadataExecutables::prepare(root)
        .map_err(|source| RunError::Environment(source.to_string().into_boxed_str()))?;
    let statistical = statistical_run_plan(root, &selection.selected, request.tier)?;
    let metadata = environment_metadata_from_tools(root, manifest, &metadata_executables)?;
    let executables =
        QualificationExecutables::prepare_with_metadata(root, metadata_executables)
            .map_err(|source| RunError::Environment(source.to_string().into_boxed_str()))?;
    let blocker_selectors = crate::blocker_ledger::qualification_existing_selectors(root)
        .map_err(|source| RunError::Environment(source.to_string().into_boxed_str()))?;
    let mut fixtures = QualificationFixtureRunner::for_run(root, &executables)
        .map_err(|source| RunError::Environment(source.to_string().into_boxed_str()))?;
    let selector_digests = selection
        .selected
        .iter()
        .map(|case| {
            let digest = resolved_selector_sha256(case, &blocker_selectors, &fixtures)
                .map_err(|reason| RunError::ReportContract(reason.into_boxed_str()))?;
            Ok((case.id.to_string(), digest))
        })
        .collect::<Result<BTreeMap<_, _>, RunError>>()?;
    let run_request = RunRequestReceipt::new(RunRequestReceiptInput {
        qualification_manifest_digest: manifest.semantic_digest.to_string(),
        stab_commit: metadata.stab_commit.clone(),
        worktree_was_clean: !metadata.local_modifications,
        stim_tag: metadata.stim_tag.clone(),
        stim_commit: metadata.stim_commit.clone(),
        tier: request.tier,
        feature_filters: features.iter().copied().collect(),
        case_filters: case_ids.iter().cloned().collect(),
        allow_deferred: request.allow_deferred,
        executables: executables.identities().to_vec(),
        execution_environment_sha256: executables.environment_sha256().to_string(),
        selected_cases: selection
            .selected
            .iter()
            .map(|case| {
                Ok(RequestedCase {
                    case_id: case.id.to_string(),
                    selector_sha256: selector_digests.get(case.id.as_str()).cloned().ok_or_else(
                        || {
                            RunError::ReportContract(
                                format!("selected case {} has no selector digest", case.id)
                                    .into_boxed_str(),
                            )
                        },
                    )?,
                    case_contract_sha256: super::receipt::case_contract_sha256(*case).map_err(
                        |source| RunError::ReportContract(source.to_string().into_boxed_str()),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, RunError>>()?,
        planned_case_ids: selection
            .planned
            .iter()
            .map(|case| case.id.to_string())
            .collect(),
        deferred_case_ids: selection
            .deferred
            .iter()
            .map(|case| case.id.to_string())
            .collect(),
    });
    let run_request_sha256 = run_request
        .publish(&output)
        .map_err(|source| RunError::ReportContract(source.to_string().into_boxed_str()))?;
    let mut report = QualificationReport::new(
        metadata,
        SelectionSummary {
            tier: request.tier,
            feature_filters: features.iter().copied().collect(),
            case_filters: case_ids.into_iter().collect(),
            allow_deferred: request.allow_deferred,
            selected_count: selection.selected.len(),
            planned_count: selection.planned.len(),
            deferred_count: selection.deferred.len(),
        },
    );
    report.run_request_sha256 = run_request_sha256.clone();
    report.statistical_declared_budget =
        super::report::ProbabilityBound::try_new(statistical.declared_bound)?;
    report.statistical_planned_shots = statistical.shots;
    report.statistical_planned_seeds = statistical.seeds.clone();
    report.property_corpus_ids = selection
        .selected
        .iter()
        .filter(|case| case.comparator == Comparator::Property)
        .map(|case| case.source_id.clone())
        .collect();
    report.resource_case_count = selection
        .selected
        .iter()
        .filter(|case| case.comparator == Comparator::Resource)
        .count();
    report.resource_contracts = selection
        .selected
        .iter()
        .copied()
        .filter(|case| case.comparator == Comparator::Resource)
        .map(|case| ResourceExecution {
            case_id: case.id.to_string(),
            kind: case.resource_contract.kind.as_str().to_string(),
            detail: case.resource_contract.detail.clone(),
            negative_axes: case.negative_axes.clone(),
            timeout_ms: case.execution.timeout_ms,
            stdout_limit_bytes: case.execution.stdout_limit_bytes,
            stderr_limit_bytes: case.execution.stderr_limit_bytes,
            artifact_limit_bytes: case.execution.artifact_limit_bytes,
        })
        .collect();
    report.upstream_dispositions = upstream_disposition_counts(manifest, &features);
    report.deferred_products = deferred_product_counts(&selection.deferred);
    for case in &selection.selected {
        let selector_sha256 = selector_digests
            .get(case.id.as_str())
            .cloned()
            .ok_or_else(|| {
                RunError::ReportContract(
                    format!("selected case {} lost its selector digest", case.id).into_boxed_str(),
                )
            })?;
        let execution = execute_case(
            root,
            case,
            &blocker_selectors,
            &mut fixtures,
            &executables,
            statistical.seeds.get(case.id.as_str()).map(Vec::as_slice),
            statistical.attempt_contracts.get(case.id.as_str()),
        );
        record_statistical_attempts(&mut report, case, &execution, &statistical)?;
        report.results.push(case_result(
            case,
            execution,
            selector_sha256,
            &run_request_sha256,
            &output,
            executables.identities(),
            executables.environment_sha256(),
        )?);
    }
    report.case_counts =
        domain_comparator_counts(&report.results, &selection.planned, &selection.deferred);
    report.finish();
    executables
        .verify_support()
        .map_err(|source| RunError::Environment(source.to_string().into_boxed_str()))?;
    let expectation = report_expectation(root, manifest, &run_request, &run_request_sha256)?;
    crate::process::ensure_qualification_active()
        .map_err(|source| RunError::Interrupted(source.to_string().into_boxed_str()))?;
    let publication = report.publish(&output, &expectation)?;
    let report_path = output.relative().join("report.json");
    crate::process::ensure_qualification_active()
        .map_err(|source| RunError::Interrupted(source.to_string().into_boxed_str()))?;
    output.commit()?;
    println!(
        "[stab-oracle] correctness tier={} selected={} passed={} failed={} request_sha256={} report_sha256={} completion_sha256={} report={}",
        request.tier.as_str(),
        report.selected_count,
        report.passed_count,
        report.failed_count,
        run_request_sha256,
        publication.report_sha256,
        publication.completion_sha256,
        report_path.display()
    );
    if report.failed_count != 0 {
        return Err(RunError::CasesFailed {
            failed: report.failed_count,
            report: report_path,
        });
    }
    Ok(())
}

fn validate_selection_policy(
    selection: &CaseSelection<'_>,
    explicit_cases: bool,
    allow_deferred: bool,
) -> Result<(), RunError> {
    if explicit_cases && !selection.planned.is_empty() {
        return Err(RunError::NonExecutableSelection(join_case_ids(
            &selection.planned,
        )));
    }
    if explicit_cases && !selection.out_of_tier.is_empty() {
        return Err(RunError::OutOfTierSelection(join_case_ids(
            &selection.out_of_tier,
        )));
    }
    if !explicit_cases && allow_deferred {
        return Err(RunError::DeferredSelection(
            "--allow-deferred requires explicit --case filters".to_string(),
        ));
    }
    if allow_deferred && selection.deferred.is_empty() {
        return Err(RunError::DeferredSelection(
            "--allow-deferred did not select any deferred case".to_string(),
        ));
    }
    if !allow_deferred && !selection.deferred.is_empty() {
        return Err(RunError::DeferredSelection(join_case_ids(
            &selection.deferred,
        )));
    }
    if selection.selected.is_empty() && selection.deferred.is_empty() {
        return Err(RunError::EmptySelection);
    }

    Ok(())
}

fn upstream_disposition_counts(
    manifest: &QualificationManifest,
    features: &BTreeSet<FeatureId>,
) -> Vec<DomainDispositionCount> {
    let mut counts = BTreeMap::new();
    for case in &manifest.upstream_cases {
        for feature_id in &case.domain_ids {
            if features.is_empty() || features.contains(feature_id) {
                *counts.entry((*feature_id, case.disposition)).or_default() += 1;
            }
        }
    }
    counts
        .into_iter()
        .map(
            |((feature_id, disposition), count)| DomainDispositionCount {
                feature_id,
                disposition,
                count,
            },
        )
        .collect()
}

fn deferred_product_counts(cases: &[&EvidenceCase]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for case in cases {
        if let Some(product) = case.deferred_product {
            *counts.entry(product.as_str().to_string()).or_default() += 1;
        }
    }
    counts
}

fn domain_comparator_counts(
    results: &[CaseResult],
    planned: &[&EvidenceCase],
    deferred: &[&EvidenceCase],
) -> Vec<DomainComparatorCount> {
    let mut counts = BTreeMap::<(FeatureId, Comparator), [usize; 4]>::new();
    for result in results {
        let values = counts
            .entry((result.feature_id, result.comparator))
            .or_default();
        match result.outcome {
            CaseOutcome::Passed => values[0] += 1,
            CaseOutcome::Failed => values[1] += 1,
        }
    }
    for case in planned {
        counts
            .entry((case.feature_id, case.comparator))
            .or_default()[2] += 1;
    }
    for case in deferred {
        counts
            .entry((case.feature_id, case.comparator))
            .or_default()[3] += 1;
    }
    counts
        .into_iter()
        .map(
            |((feature_id, comparator), [passed, failed, planned, deferred])| {
                DomainComparatorCount {
                    feature_id,
                    comparator,
                    passed,
                    failed,
                    planned,
                    deferred,
                }
            },
        )
        .collect()
}

struct StatisticalRunPlan {
    declared_bound: f64,
    shots: u64,
    seeds: BTreeMap<String, Vec<u64>>,
    shots_per_attempt: BTreeMap<String, u64>,
    exact_bound_per_attempt: BTreeMap<String, f64>,
    attempt_contracts: BTreeMap<String, StatisticalAttemptContract>,
}

#[derive(Clone, Debug)]
struct StatisticalAttemptContract {
    plan_id: String,
    shots_per_batch: u64,
    comparisons_per_attempt: u32,
    batches_per_attempt: u32,
    batches_per_comparison: u32,
    shots_per_attempt: u64,
    exact_bound_per_attempt: f64,
}

fn statistical_run_plan(
    root: &RepoRoot,
    selected: &[&EvidenceCase],
    tier: QualificationTier,
) -> Result<StatisticalRunPlan, RunError> {
    let catalog = super::statistics::source_plan_summaries(root)
        .map_err(|source| RunError::Statistics(source.to_string().into_boxed_str()))?;
    let by_id = catalog
        .iter()
        .map(|plan| (plan.id().as_str(), plan))
        .collect::<BTreeMap<_, _>>();
    let mut plans = Vec::new();
    let mut case_plans = Vec::new();
    for case in selected
        .iter()
        .copied()
        .filter(|case| case.comparator == Comparator::Statistical)
    {
        let reference = case.statistical_plan.as_ref().ok_or_else(|| {
            RunError::Statistics(
                format!("case {} has no statistical plan reference", case.id).into_boxed_str(),
            )
        })?;
        let plan = by_id.get(reference.id.as_str()).copied().ok_or_else(|| {
            RunError::Statistics(
                format!(
                    "case {} references missing statistical plan {:?}",
                    case.id, reference.id
                )
                .into_boxed_str(),
            )
        })?;
        plans.push(plan);
        case_plans.push((case, plan));
    }
    let budget = super::statistics::validate_selected_suite(plans)
        .map_err(|source| RunError::Statistics(source.to_string().into_boxed_str()))?;
    let mut declared_bound = 0.0;
    let mut exact_bound = 0.0;
    let mut shots = 0u64;
    let mut seeds = BTreeMap::new();
    let mut shots_per_attempt = BTreeMap::new();
    let mut exact_bound_per_attempt = BTreeMap::new();
    let mut attempt_contracts = BTreeMap::new();
    for (case, plan) in case_plans {
        let panel = if tier == QualificationTier::Soak && plan.seed_override_executable() {
            super::statistics::expand_budgeted_soak_seed_panel(plan, 3)
                .map_err(|source| RunError::Statistics(source.to_string().into_boxed_str()))?
        } else {
            super::statistics::StatisticalSeedPanel::try_new(plan.primary_seed(), Vec::new())
                .map_err(|source| RunError::Statistics(source.to_string().into_boxed_str()))?
        };
        let values = panel.seeds().map(|seed| seed.get()).collect::<Vec<_>>();
        let attempts = values.len() as f64;
        let case_declared_bound = plan.declared_familywise_bound().get();
        let attempt_exact_bound = plan.exact_bound_per_attempt();
        let case_exact_bound = attempt_exact_bound * attempts;
        if case_exact_bound > case_declared_bound
            || case_exact_bound > super::statistics::MAX_CASE_FAMILYWISE_BOUND
        {
            return Err(RunError::Statistics(
                format!(
                    "tier-expanded statistical case {} exact union bound {case_exact_bound:.6e} exceeds its declared case bound {case_declared_bound:.6e} or the {:.6e} case cap",
                    case.id,
                    super::statistics::MAX_CASE_FAMILYWISE_BOUND
                )
                .into_boxed_str(),
            ));
        }
        declared_bound += case_declared_bound;
        exact_bound += case_exact_bound;
        let attempt_count = u64::try_from(values.len())
            .map_err(|_| RunError::Statistics("statistical seed count exceeds u64".into()))?;
        shots = shots
            .checked_add(
                plan.shots_per_attempt()
                    .get()
                    .checked_mul(attempt_count)
                    .ok_or_else(|| {
                        RunError::Statistics("statistical shot count overflow".into())
                    })?,
            )
            .ok_or_else(|| RunError::Statistics("statistical shot total overflow".into()))?;
        seeds.insert(case.id.to_string(), values);
        shots_per_attempt.insert(case.id.to_string(), plan.shots_per_attempt().get());
        exact_bound_per_attempt.insert(case.id.to_string(), attempt_exact_bound);
        attempt_contracts.insert(
            case.id.to_string(),
            StatisticalAttemptContract {
                plan_id: plan.id().as_str().to_string(),
                shots_per_batch: plan.shots().get(),
                comparisons_per_attempt: plan.independent_comparisons_per_attempt().get(),
                batches_per_attempt: plan.shot_batches_per_attempt().get(),
                batches_per_comparison: plan.shot_batches_per_comparison().get(),
                shots_per_attempt: plan.shots_per_attempt().get(),
                exact_bound_per_attempt: attempt_exact_bound,
            },
        );
    }
    if declared_bound > super::statistics::MAX_SELECTED_SUITE_FAMILYWISE_BOUND
        || exact_bound > super::statistics::MAX_SELECTED_SUITE_FAMILYWISE_BOUND
    {
        return Err(RunError::Statistics(
            format!(
                "tier-expanded statistical bound declared={declared_bound:.6e} exact={exact_bound:.6e} exceeds {:.6e}",
                super::statistics::MAX_SELECTED_SUITE_FAMILYWISE_BOUND
            )
            .into_boxed_str(),
        ));
    }
    debug_assert_eq!(budget.plan_count(), seeds.len());
    Ok(StatisticalRunPlan {
        declared_bound,
        shots,
        seeds,
        shots_per_attempt,
        exact_bound_per_attempt,
        attempt_contracts,
    })
}

fn parse_features(values: &[String]) -> Result<BTreeSet<FeatureId>, RunError> {
    values
        .iter()
        .map(|value| FeatureId::parse(value).ok_or_else(|| RunError::UnknownFeature(value.clone())))
        .collect()
}

fn parse_case_ids(
    manifest: &QualificationManifest,
    values: &[String],
) -> Result<BTreeSet<String>, RunError> {
    let known = manifest
        .evidence_cases
        .iter()
        .map(|case| case.id.as_str())
        .collect::<BTreeSet<_>>();
    values
        .iter()
        .map(|value| {
            if known.contains(value.as_str()) {
                Ok(value.clone())
            } else {
                Err(RunError::UnknownCase(value.clone()))
            }
        })
        .collect()
}

fn validate_case_feature_filters(
    manifest: &QualificationManifest,
    features: &BTreeSet<FeatureId>,
    case_ids: &BTreeSet<String>,
) -> Result<(), RunError> {
    if features.is_empty() || case_ids.is_empty() {
        return Ok(());
    }
    let mismatched = manifest
        .evidence_cases
        .iter()
        .filter(|case| case_ids.contains(case.id.as_str()) && !features.contains(&case.feature_id))
        .map(|case| case.id.as_str())
        .collect::<Vec<_>>();
    if mismatched.is_empty() {
        Ok(())
    } else {
        Err(RunError::CaseFeatureMismatch(mismatched.join(", ")))
    }
}

fn join_case_ids(cases: &[&EvidenceCase]) -> String {
    cases
        .iter()
        .map(|case| case.id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn resolved_selector_sha256(
    case: &EvidenceCase,
    blocker_selectors: &BTreeMap<String, Vec<String>>,
    fixtures: &QualificationFixtureRunner<'_>,
) -> Result<String, String> {
    match case.primary_selector.kind {
        SelectorKind::CargoTest | SelectorKind::PropertyTarget => {
            super::report::selector_sha256(&case.primary_selector)
                .map_err(|source| source.to_string())
        }
        SelectorKind::OracleFixture => {
            let [id] = case.primary_selector.value.as_slice() else {
                return Err("oracle selector must contain one fixture id".to_string());
            };
            fixtures
                .selector_sha256(id)
                .map_err(|source| source.to_string())
        }
        SelectorKind::OpsCheck => {
            let selector = blocker_selectors.get(&case.source_id).ok_or_else(|| {
                "blocker selector is missing from the validated ledger".to_string()
            })?;
            super::report::selector_sha256(&super::model::EvidenceSelector {
                state: super::model::EvidenceState::Existing,
                kind: SelectorKind::CargoTest,
                value: selector.clone(),
            })
            .map_err(|source| source.to_string())
        }
    }
}

pub(super) fn expected_selector_digests(
    root: &RepoRoot,
    manifest: &QualificationManifest,
    case_ids: &[String],
) -> Result<BTreeMap<String, String>, RunError> {
    let blockers = crate::blocker_ledger::qualification_existing_selectors(root)
        .map_err(|source| RunError::Environment(source.to_string().into_boxed_str()))?;
    let fixtures = QualificationFixtureRunner::new(root)
        .map_err(|source| RunError::Environment(source.to_string().into_boxed_str()))?;
    case_ids
        .iter()
        .map(|case_id| {
            let case = manifest
                .evidence_cases
                .iter()
                .find(|case| case.id.as_str() == case_id)
                .ok_or_else(|| RunError::UnknownCase(case_id.clone()))?;
            let digest = resolved_selector_sha256(case, &blockers, &fixtures)
                .map_err(|reason| RunError::Environment(reason.into_boxed_str()))?;
            Ok((case_id.clone(), digest))
        })
        .collect()
}

mod report_binding;
pub(super) use report_binding::report_expectation;

struct ExecutionEvidence {
    outcome: CaseOutcome,
    completed_execution: bool,
    exit_status: Option<i32>,
    exact_test_count: Option<usize>,
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
    stdout_bytes: Option<usize>,
    stderr_bytes: Option<usize>,
    stdout_digest: Option<String>,
    stderr_digest: Option<String>,
    failure: Option<Vec<u8>>,
    property_regression: Option<Vec<u8>>,
    statistical_attempts: Vec<StatisticalAttemptEvidence>,
}

struct StatisticalAttemptEvidence {
    seed: u64,
    outcome: CaseOutcome,
    completed_shots: u64,
    completed_comparisons: u32,
    completed_batches: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct StatisticalCompletion {
    shots: u64,
    comparisons: u32,
    batches: u32,
}

impl StatisticalCompletion {
    const fn none() -> Self {
        Self {
            shots: 0,
            comparisons: 0,
            batches: 0,
        }
    }

    fn complete(contract: Option<&StatisticalAttemptContract>) -> Self {
        contract.map_or_else(Self::none, |contract| Self {
            shots: contract.shots_per_attempt,
            comparisons: contract.comparisons_per_attempt,
            batches: contract.batches_per_attempt,
        })
    }
}

fn execute_case(
    root: &RepoRoot,
    case: &EvidenceCase,
    blocker_selectors: &std::collections::BTreeMap<String, Vec<String>>,
    fixtures: &mut QualificationFixtureRunner<'_>,
    executables: &QualificationExecutables,
    statistical_seeds: Option<&[u64]>,
    statistical_contract: Option<&StatisticalAttemptContract>,
) -> ExecutionEvidence {
    let execution = match case.primary_selector.kind {
        SelectorKind::CargoTest => attach_fixed_statistical_attempt(
            execute_cargo(
                root,
                executables,
                &case.primary_selector.value,
                case.execution.timeout_ms,
                statistical_contract,
            ),
            statistical_seeds,
            statistical_contract,
        ),
        SelectorKind::OracleFixture => {
            let Some(id) = case.primary_selector.value.first() else {
                return failure("oracle selector contains no fixture id");
            };
            if let Some(seeds) = statistical_seeds {
                let mut stdout_hasher = Sha256::new();
                let mut stderr_hasher = Sha256::new();
                let mut stdout_bytes = 0_usize;
                let mut stderr_bytes = 0_usize;
                let mut exit_status = None;
                let mut attempts = super::statistics::StatisticalAttemptHistory::new();
                let mut attempt_evidence = Vec::new();
                for seed in seeds {
                    match fixtures.run_with_seed(
                        id,
                        Duration::from_millis(case.execution.timeout_ms),
                        Some(*seed),
                    ) {
                        Ok(output) => {
                            let Some(next_stdout_bytes) =
                                stdout_bytes.checked_add(output.stdout.bytes.len())
                            else {
                                return failure("statistical stdout byte count overflowed");
                            };
                            let Some(next_stderr_bytes) =
                                stderr_bytes.checked_add(output.stderr.bytes.len())
                            else {
                                return failure("statistical stderr byte count overflowed");
                            };
                            stdout_bytes = next_stdout_bytes;
                            stderr_bytes = next_stderr_bytes;
                            exit_status = output.status;
                            update_seeded_digest(
                                &mut stdout_hasher,
                                *seed,
                                Some(&output.stdout.bytes),
                            );
                            update_seeded_digest(
                                &mut stderr_hasher,
                                *seed,
                                Some(&output.stderr.bytes),
                            );
                            if let Some(reason) = attempt_capture_violation(case, &output) {
                                let record = record_statistical_attempt(
                                    &mut attempts,
                                    &mut attempt_evidence,
                                    *seed,
                                    CaseOutcome::Failed,
                                    statistical_contract,
                                    StatisticalCompletion::complete(statistical_contract),
                                );
                                let reason = match record {
                                    Ok(()) => reason,
                                    Err(history_error) => {
                                        format!(
                                            "{reason}; failed to retain statistical attempt: {history_error}"
                                        )
                                    }
                                };
                                return failure_with_digests(
                                    reason,
                                    Some(output.stdout.bytes),
                                    Some(output.stderr.bytes),
                                    finalize_sha256(stdout_hasher),
                                    finalize_sha256(stderr_hasher),
                                    stdout_bytes,
                                    stderr_bytes,
                                    output.status,
                                    true,
                                    attempt_evidence,
                                );
                            }
                            if let Err(source) = record_statistical_attempt(
                                &mut attempts,
                                &mut attempt_evidence,
                                *seed,
                                CaseOutcome::Passed,
                                statistical_contract,
                                StatisticalCompletion::complete(statistical_contract),
                            ) {
                                return failure_with_digests(
                                    source,
                                    Some(output.stdout.bytes),
                                    Some(output.stderr.bytes),
                                    finalize_sha256(stdout_hasher),
                                    finalize_sha256(stderr_hasher),
                                    stdout_bytes,
                                    stderr_bytes,
                                    output.status,
                                    true,
                                    attempt_evidence,
                                );
                            }
                        }
                        Err(source) => {
                            let parts = source.into_parts();
                            let reason = parts.reason;
                            let failed_stdout = parts.stdout;
                            let failed_stderr = parts.stderr;
                            let completed = parts.completed;
                            let completion = StatisticalCompletion {
                                shots: parts.completed_statistical_shots,
                                comparisons: parts.completed_statistical_comparisons,
                                batches: parts.completed_statistical_batches,
                            };
                            let status = parts.status;
                            let failed_stdout_len = failed_stdout.as_ref().map_or(0, Vec::len);
                            let failed_stderr_len = failed_stderr.as_ref().map_or(0, Vec::len);
                            let Some(next_stdout_bytes) =
                                stdout_bytes.checked_add(failed_stdout_len)
                            else {
                                return failure("statistical stdout byte count overflowed");
                            };
                            let Some(next_stderr_bytes) =
                                stderr_bytes.checked_add(failed_stderr_len)
                            else {
                                return failure("statistical stderr byte count overflowed");
                            };
                            stdout_bytes = next_stdout_bytes;
                            stderr_bytes = next_stderr_bytes;
                            update_seeded_digest(
                                &mut stdout_hasher,
                                *seed,
                                failed_stdout.as_deref(),
                            );
                            update_seeded_digest(
                                &mut stderr_hasher,
                                *seed,
                                failed_stderr.as_deref(),
                            );
                            let reason = match record_statistical_attempt(
                                &mut attempts,
                                &mut attempt_evidence,
                                *seed,
                                CaseOutcome::Failed,
                                statistical_contract,
                                completion,
                            ) {
                                Ok(()) => reason.to_string(),
                                Err(history_error) => format!(
                                    "{reason}; failed to retain statistical attempt: {history_error}"
                                ),
                            };
                            return failure_with_digests(
                                reason,
                                failed_stdout,
                                failed_stderr,
                                finalize_sha256(stdout_hasher),
                                finalize_sha256(stderr_hasher),
                                stdout_bytes,
                                stderr_bytes,
                                status,
                                completed,
                                attempt_evidence,
                            );
                        }
                    }
                }
                let mut execution = success_with_digests(
                    finalize_sha256(stdout_hasher),
                    finalize_sha256(stderr_hasher),
                    stdout_bytes,
                    stderr_bytes,
                    exit_status,
                );
                execution.statistical_attempts = attempt_evidence;
                execution
            } else {
                match fixtures.run(id, Duration::from_millis(case.execution.timeout_ms)) {
                    Ok(output) => success(output, None),
                    Err(source) => fixture_failure(source),
                }
            }
        }
        SelectorKind::OpsCheck => {
            let Some(selector) = blocker_selectors.get(&case.source_id) else {
                return failure("blocker selector is missing from the validated ledger");
            };
            attach_fixed_statistical_attempt(
                execute_cargo(
                    root,
                    executables,
                    selector,
                    case.execution.timeout_ms,
                    statistical_contract,
                ),
                statistical_seeds,
                statistical_contract,
            )
        }
        SelectorKind::PropertyTarget => execute_property_target(root, executables, case),
    };
    enforce_capture_limits(case, execution)
}

fn update_seeded_digest(hasher: &mut Sha256, seed: u64, bytes: Option<&[u8]>) {
    hasher.update(seed.to_le_bytes());
    match bytes {
        Some(bytes) => {
            hasher.update([1]);
            hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
            hasher.update(bytes);
        }
        None => {
            hasher.update([0]);
            hasher.update(0_u64.to_le_bytes());
        }
    }
}

fn finalize_sha256(hasher: Sha256) -> String {
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn attempt_capture_violation(case: &EvidenceCase, output: &crate::ProcessOutput) -> Option<String> {
    if output.stdout.bytes.len() > case.execution.stdout_limit_bytes {
        Some(format!(
            "qualification case {} exceeded its per-attempt stdout limit of {} bytes",
            case.id, case.execution.stdout_limit_bytes
        ))
    } else if output.stderr.bytes.len() > case.execution.stderr_limit_bytes {
        Some(format!(
            "qualification case {} exceeded its per-attempt stderr limit of {} bytes",
            case.id, case.execution.stderr_limit_bytes
        ))
    } else {
        None
    }
}

fn record_statistical_attempt(
    history: &mut super::statistics::StatisticalAttemptHistory,
    evidence: &mut Vec<StatisticalAttemptEvidence>,
    seed: u64,
    outcome: CaseOutcome,
    contract: Option<&StatisticalAttemptContract>,
    completion: StatisticalCompletion,
) -> Result<(), String> {
    let contract =
        contract.ok_or_else(|| "statistical attempt has no frozen contract".to_string())?;
    validate_statistical_completion(contract, completion, outcome)?;
    let history_outcome = match outcome {
        CaseOutcome::Passed => super::statistics::StatisticalAttemptOutcome::Passed,
        CaseOutcome::Failed => super::statistics::StatisticalAttemptOutcome::Failed,
    };
    history
        .record(super::statistics::StatisticalAttempt::new(
            super::statistics::StatisticalSeed::new(seed),
            history_outcome,
        ))
        .map_err(|source| source.to_string())?;
    evidence.push(StatisticalAttemptEvidence {
        seed,
        outcome,
        completed_shots: completion.shots,
        completed_comparisons: completion.comparisons,
        completed_batches: completion.batches,
    });
    Ok(())
}

fn validate_statistical_completion(
    contract: &StatisticalAttemptContract,
    completion: StatisticalCompletion,
    outcome: CaseOutcome,
) -> Result<(), String> {
    let expected_batches = completion
        .comparisons
        .checked_mul(contract.batches_per_comparison)
        .ok_or_else(|| "statistical completed batch count overflowed".to_string())?;
    let expected_shots = contract
        .shots_per_batch
        .checked_mul(u64::from(completion.batches))
        .ok_or_else(|| "statistical completed shot count overflowed".to_string())?;
    if completion.shots != expected_shots
        || completion.batches != expected_batches
        || completion.comparisons > contract.comparisons_per_attempt
        || completion.batches > contract.batches_per_attempt
    {
        return Err(format!(
            "statistical completion reports shots={} comparisons={} batches={}, outside frozen shots-per-batch={} comparisons={} batches={} batches-per-comparison={}",
            completion.shots,
            completion.comparisons,
            completion.batches,
            contract.shots_per_batch,
            contract.comparisons_per_attempt,
            contract.batches_per_attempt,
            contract.batches_per_comparison
        ));
    }
    if outcome == CaseOutcome::Passed
        && (completion.shots != contract.shots_per_attempt
            || completion.comparisons != contract.comparisons_per_attempt
            || completion.batches != contract.batches_per_attempt)
    {
        return Err("passing statistical attempt lacks complete frozen work evidence".to_string());
    }
    Ok(())
}

fn enforce_capture_limits(case: &EvidenceCase, execution: ExecutionEvidence) -> ExecutionEvidence {
    let violation = if execution
        .stdout
        .as_ref()
        .is_some_and(|bytes| bytes.len() > case.execution.stdout_limit_bytes)
    {
        Some(("stdout", case.execution.stdout_limit_bytes))
    } else if execution
        .stderr
        .as_ref()
        .is_some_and(|bytes| bytes.len() > case.execution.stderr_limit_bytes)
    {
        Some(("stderr", case.execution.stderr_limit_bytes))
    } else {
        None
    };
    if let Some((label, limit)) = violation {
        return capture_limit_failure(
            execution,
            &format!(
                "qualification case {} exceeded its {label} limit of {limit} bytes",
                case.id
            ),
        );
    }
    execution
}

fn attach_fixed_statistical_attempt(
    mut execution: ExecutionEvidence,
    statistical_seeds: Option<&[u64]>,
    statistical_contract: Option<&StatisticalAttemptContract>,
) -> ExecutionEvidence {
    let Some(seeds) = statistical_seeds else {
        return execution;
    };
    let [seed] = seeds else {
        return failure_with_parts(
            "fixed statistical selector requires exactly one frozen seed",
            execution.stdout,
            execution.stderr,
            Vec::new(),
        );
    };
    let Some(contract) = statistical_contract else {
        return failure_with_parts(
            "fixed statistical selector has no frozen completion contract",
            execution.stdout,
            execution.stderr,
            Vec::new(),
        );
    };
    let completion =
        match execution::statistical_completion_from_output(&execution, contract, *seed) {
            Ok(completion) => completion,
            Err(error) => {
                execution.outcome = CaseOutcome::Failed;
                execution.failure = failure(&error.reason).failure;
                error.completion
            }
        };
    if let Err(reason) = validate_statistical_completion(contract, completion, execution.outcome) {
        execution.outcome = CaseOutcome::Failed;
        execution.failure = failure(&reason).failure;
    }
    execution.statistical_attempts = vec![StatisticalAttemptEvidence {
        seed: *seed,
        outcome: execution.outcome,
        completed_shots: completion.shots,
        completed_comparisons: completion.comparisons,
        completed_batches: completion.batches,
    }];
    execution
}

fn record_statistical_attempts(
    report: &mut QualificationReport,
    case: &EvidenceCase,
    execution: &ExecutionEvidence,
    statistical: &StatisticalRunPlan,
) -> Result<(), RunError> {
    for attempt in &execution.statistical_attempts {
        let contract = statistical
            .attempt_contracts
            .get(case.id.as_str())
            .ok_or_else(|| {
                RunError::Statistics(
                    format!(
                        "case {} has a statistical attempt without a completion contract",
                        case.id
                    )
                    .into_boxed_str(),
                )
            })?;
        let completion = StatisticalCompletion {
            shots: attempt.completed_shots,
            comparisons: attempt.completed_comparisons,
            batches: attempt.completed_batches,
        };
        validate_statistical_completion(contract, completion, attempt.outcome)
            .map_err(|reason| RunError::Statistics(reason.into_boxed_str()))?;
        if attempt.completed_shots != 0 {
            report.statistical_shots = report
                .statistical_shots
                .checked_add(attempt.completed_shots)
                .ok_or_else(|| RunError::Statistics("executed shot total overflow".into()))?;
        }
        if attempt.completed_comparisons != 0 {
            let completed_bound = contract.exact_bound_per_attempt
                * f64::from(attempt.completed_comparisons)
                / f64::from(contract.comparisons_per_attempt);
            report.statistical_consumed_bound = super::report::ProbabilityBound::try_new(
                report.statistical_consumed_bound.get() + completed_bound,
            )?;
        }
        report
            .statistical_seeds
            .entry(case.id.to_string())
            .or_default()
            .push(attempt.seed);
        report
            .statistical_attempts
            .push(super::report::StatisticalAttempt {
                case_id: case.id.to_string(),
                seed: attempt.seed,
                completed_shots: attempt.completed_shots,
                completed_comparisons: attempt.completed_comparisons,
                completed_batches: attempt.completed_batches,
                outcome: attempt.outcome,
            });
    }
    Ok(())
}

mod evidence;
#[cfg(test)]
use evidence::sha256;
use evidence::{
    capture_limit_failure, case_result, failure, failure_with_digests, failure_with_parts,
    fixture_failure, oracle_failure, process_failure, success, success_with_digests,
};

mod execution;
use execution::{execute_cargo, execute_property_target};

mod environment;
use environment::current_environment_evidence;
pub(super) use environment::environment_metadata_from_tools;

#[cfg(test)]
#[path = "runner/tests.rs"]
mod tests;
