use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::artifact::QualificationOutput;
use super::calibration::{CalibrationDecision, CalibrationPolicy, CalibrationProbe, calibrate};
use super::correctness::{CorrectnessPreflightEvidence, CorrectnessRequirement};
use super::host::{HostEvidence, HostGuard};
use super::invocation::{
    InvocationRecord, InvocationRequest, PreparedWorkers, WorkerIdentityEvidence,
};
use super::protocol::{EvidenceMode, Implementation, SemanticDigest, WorkerMeasurement};
use super::statistics::{
    GateOutcome, PairOrder, PairedSample, StatisticsSummary, pair_measurements_with_policy,
    summarize,
};
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::qualification::model::TimingBatchPolicy;
use crate::root::RepoRoot;

pub(super) const REPORT_SCHEMA_VERSION: u32 = 31;
const DEFAULT_OUTPUT: &str = "target/benchmarks/qualification/latest";
const CALIBRATION_ACCEPTANCE_MINIMUM: Duration = Duration::from_millis(250);
const CALIBRATION_TARGET_MINIMUM: Duration = Duration::from_millis(350);
const CALIBRATION_MAXIMUM: Duration = Duration::from_secs(2);
const CALIBRATION_WIDE_RATIO_MAXIMUM: Duration = Duration::from_secs(20);
const INVOCATION_TIMEOUT: Duration = Duration::from_secs(30);
const MAXIMUM_ITERATIONS: u64 = 1_000_000_000;
const WARMUP_BATCHES: usize = 3;
const MAXIMUM_TIMING_ATTEMPTS: usize = 2;
const PRIMARY_THRESHOLD: f64 = 1.25;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum QualificationTier {
    Pr,
    Full,
    Soak,
}

impl QualificationTier {
    fn pair_count(self) -> usize {
        match self {
            Self::Pr => 3,
            Self::Full => 9,
            Self::Soak => 15,
        }
    }
}

#[derive(Clone, Debug, Args)]
pub(crate) struct RunArgs {
    /// Source-owned runtime group to execute.
    #[arg(long, default_value = "pq1-adapter-protocol-smoke")]
    group: String,

    /// Source-owned workload scale within the selected group.
    #[arg(long, default_value = "default")]
    scale: String,

    /// Qualification tier controlling the number of retained pairs.
    #[arg(long, value_enum)]
    tier: QualificationTier,

    /// Atomic report directory below target/benchmarks/qualification.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    out: PathBuf,

    /// Preserve a diagnostic report when source-owned host limits are not met.
    #[arg(long)]
    allow_unverified_host: bool,

    /// Correctness qualification report directory for a promotable product group.
    #[arg(long)]
    correctness_out: Option<PathBuf>,

    /// Controller-approved SHA-256 of the CQ1 request artifact.
    #[arg(
        long,
        requires_all = ["correctness_out", "correctness_completion_sha256"]
    )]
    correctness_request_sha256: Option<String>,

    /// Controller-approved SHA-256 of the CQ1 completion artifact.
    #[arg(
        long,
        requires_all = ["correctness_out", "correctness_request_sha256"]
    )]
    correctness_completion_sha256: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ClaimClass {
    DiagnosticInfrastructure,
    PromotablePerformance,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QualificationReport {
    pub(super) schema_version: u32,
    pub(super) group_id: String,
    pub(super) scale_id: String,
    pub(super) group_contract_sha256: String,
    pub(super) claim_class: ClaimClass,
    pub(super) baseline_eligibility: super::group::BaselineEligibility,
    pub(super) owner: String,
    pub(super) profiler_note: Option<super::group::ProfilerNoteContract>,
    pub(super) tier: QualificationTier,
    pub(super) command: RunCommandEvidence,
    pub(super) generated_unix_epoch_seconds: u64,
    pub(super) stim_tag: String,
    pub(super) stim_commit: String,
    pub(super) performance_inventory_sha256: String,
    pub(super) correctness_inventory_sha256: String,
    pub(super) repository: RepositoryEvidence,
    pub(super) host: HostEvidence,
    pub(super) toolchain: super::toolchain::ToolchainEvidence,
    pub(super) workers: WorkerIdentityEvidence,
    pub(super) contract_preflight: super::invocation::WorkerContractPreflightEvidence,
    pub(super) adapter_receipt: super::adapter::AdapterBuildReceipt,
    pub(super) stab_build_receipt: super::stab_build::StabBuildReceipt,
    pub(super) correctness_preflight: CorrectnessPreflightEvidence,
    pub(super) semantic_preflight: PairExecution,
    pub(super) calibration: CalibrationEvidence,
    pub(super) timing_attempts: Vec<TimingAttempt>,
    pub(super) memory: MemoryEvidence,
    pub(super) promotable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunCommandEvidence {
    pub(super) output: String,
    pub(super) group_id: String,
    pub(super) scale_id: String,
    pub(super) work_items: u64,
    pub(super) allow_unverified_host: bool,
    pub(super) warmup_batches: usize,
    pub(super) paired_samples: usize,
    pub(super) maximum_timing_attempts: usize,
    pub(super) invocation_timeout_seconds: u64,
    pub(super) correctness_output: Option<String>,
    pub(super) correctness_request_sha256: Option<String>,
    pub(super) correctness_completion_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RepositoryEvidence {
    pub(super) commit_before: String,
    pub(super) commit_after: String,
    pub(super) local_modifications_before: bool,
    pub(super) local_modifications_after: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PairExecution {
    pub(super) pair_index: usize,
    pub(super) order: PairOrder,
    pub(super) stim: InvocationRecord,
    pub(super) stab: InvocationRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CalibrationProbeEvidence {
    pub(super) iterations: u64,
    pub(super) invocation: InvocationRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ImplementationCalibration {
    pub(super) implementation: Implementation,
    pub(super) selected_iterations: u64,
    pub(super) selected_measured_seconds: f64,
    pub(super) probes: Vec<CalibrationProbeEvidence>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CalibrationEvidence {
    pub(super) acceptance_minimum_seconds: f64,
    pub(super) target_minimum_seconds: f64,
    pub(super) maximum_seconds: f64,
    pub(super) wide_ratio_maximum_seconds: f64,
    pub(super) batch_policy: TimingBatchPolicy,
    pub(super) common_batch_mode: CommonBatchMode,
    pub(super) stim: ImplementationCalibration,
    pub(super) stab: ImplementationCalibration,
    pub(super) common_iterations: u64,
    pub(super) common_validation: PairExecution,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CommonBatchMode {
    Standard,
    WideRatio,
    IndependentThroughput,
}

impl CommonBatchMode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::WideRatio => "wide-ratio",
            Self::IndependentThroughput => "independent-throughput",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum TimingAttemptKind {
    Initial,
    PairedRatioNoiseRerun,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TimingAttempt {
    pub(super) attempt_index: usize,
    pub(super) kind: TimingAttemptKind,
    pub(super) warmups: Vec<PairExecution>,
    pub(super) samples: Vec<PairExecution>,
    pub(super) paired_samples: Vec<PairedSample>,
    pub(super) statistics: Vec<StatisticsSummary>,
    pub(super) worst_confidence_interval_upper: f64,
}

impl TimingAttempt {
    pub(super) fn requires_noisy_rerun(&self) -> bool {
        self.statistics
            .iter()
            .any(|summary| summary.outcome == GateOutcome::Noisy)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MemoryEvidence {
    pub(super) evidence_mode: EvidenceMode,
    pub(super) iterations: u64,
    pub(super) work_count: u64,
    pub(super) stim_setup_rss_bytes: u64,
    pub(super) stim_peak_rss_bytes: u64,
    pub(super) stim_parent_observed_peak_rss_bytes: Option<u64>,
    pub(super) stab_setup_rss_bytes: u64,
    pub(super) stab_peak_rss_bytes: u64,
    pub(super) stab_parent_observed_peak_rss_bytes: Option<u64>,
    pub(super) execution: PairExecution,
}

pub(super) fn run(
    root: &RepoRoot,
    performance_inventory_sha256: &str,
    correctness_inventory_sha256: &str,
    args: RunArgs,
) -> Result<PathBuf, RunError> {
    QualificationOutput::require_absent(root, &args.out)?;
    let repository_before = super::git::repository_state(root)?;
    let resolved_group = super::group::load_group(root, performance_inventory_sha256, &args.group)?;
    let scale = resolved_group.contract.scale(&args.scale)?;
    let scale_id = scale.id.clone();
    let workload_id = resolved_group.contract.workload_id.clone();
    let measurement_id = resolved_group.contract.single_measurement()?.clone();
    resolved_group
        .contract
        .validate_worker_shape(&workload_id, &measurement_id)?;
    let claim_class = resolved_group.contract.claim_class;
    let correctness_preflight = correctness_preflight(
        root,
        &resolved_group.contract,
        &args,
        correctness_inventory_sha256,
        &repository_before.commit,
    )?;
    let mut host_guard = HostGuard::prepare(root, args.allow_unverified_host)?;
    let toolchain = super::toolchain::collect(root)?;
    let mut workers = PreparedWorkers::prepare(root, &repository_before.commit, &toolchain)?;
    workers.pin_to_cpu(host_guard.selected_cpu());

    let policy = calibration_policy()?;
    let (stim_decision, stim_probes) = calibrate_worker(
        &workers,
        &resolved_group.contract,
        Implementation::Stim,
        scale,
        policy,
    )?;
    let (stab_decision, stab_probes) = calibrate_worker(
        &workers,
        &resolved_group.contract,
        Implementation::Stab,
        scale,
        policy,
    )?;
    let batch_policy = resolved_group.contract.timing_batch_policy;
    let common_iterations = match batch_policy {
        TimingBatchPolicy::CommonIterations => {
            stim_decision.iterations.max(stab_decision.iterations)
        }
        TimingBatchPolicy::IndependentThroughput => {
            stim_decision.iterations.min(stab_decision.iterations)
        }
    };
    let common_batch = WorkloadBatch::common(common_iterations, scale);
    let semantic_preflight = execute_pair(
        &workers,
        &resolved_group.contract,
        0,
        common_batch,
        EvidenceMode::Timing,
        None,
    )?;
    pair_execution(&semantic_preflight, TimingBatchPolicy::CommonIterations)?;
    let expected_output_digest = only_row(&semantic_preflight.stim.rows)?
        .output_digest
        .clone();
    let common_validation = execute_pair(
        &workers,
        &resolved_group.contract,
        0,
        common_batch,
        EvidenceMode::Timing,
        Some(ExpectedOutputDigests::same(&expected_output_digest)),
    )?;
    pair_execution(&common_validation, TimingBatchPolicy::CommonIterations)?;
    let common_batch_mode = validate_common_calibration(
        batch_policy,
        &common_validation,
        stim_decision.iterations.get(),
        stab_decision.iterations.get(),
    )?;
    let stim_selected_iterations = stim_decision.iterations;
    let stab_selected_iterations = stab_decision.iterations;
    let timing_batch = match batch_policy {
        TimingBatchPolicy::CommonIterations => common_batch,
        TimingBatchPolicy::IndependentThroughput => WorkloadBatch {
            stim_iterations: stim_selected_iterations,
            stab_iterations: stab_selected_iterations,
            scale,
        },
    };
    let stim_selected_digest =
        selected_calibration_output_digest(&stim_probes, stim_selected_iterations)?;
    let stab_selected_digest =
        selected_calibration_output_digest(&stab_probes, stab_selected_iterations)?;
    for (implementation, selected_iterations, selected_digest) in [
        (
            Implementation::Stim,
            stim_selected_iterations.get(),
            &stim_selected_digest,
        ),
        (
            Implementation::Stab,
            stab_selected_iterations.get(),
            &stab_selected_digest,
        ),
    ] {
        if !selected_output_matches_common(
            common_iterations.get(),
            selected_iterations,
            &expected_output_digest,
            selected_digest,
        ) {
            return Err(RunError::SelectedCalibrationSemanticMismatch(
                implementation,
            ));
        }
    }
    let timing_output_digests = match batch_policy {
        TimingBatchPolicy::CommonIterations => ExpectedOutputDigests::same(&expected_output_digest),
        TimingBatchPolicy::IndependentThroughput => ExpectedOutputDigests {
            stim: &stim_selected_digest,
            stab: &stab_selected_digest,
        },
    };
    let timing_plan = TimingAttemptPlan {
        tier: args.tier,
        batch: timing_batch,
        expected_output_digests: timing_output_digests,
        batch_policy,
    };
    let calibration = CalibrationEvidence {
        acceptance_minimum_seconds: CALIBRATION_ACCEPTANCE_MINIMUM.as_secs_f64(),
        target_minimum_seconds: CALIBRATION_TARGET_MINIMUM.as_secs_f64(),
        maximum_seconds: CALIBRATION_MAXIMUM.as_secs_f64(),
        wide_ratio_maximum_seconds: CALIBRATION_WIDE_RATIO_MAXIMUM.as_secs_f64(),
        batch_policy,
        common_batch_mode,
        stim: calibration_evidence(Implementation::Stim, stim_decision, stim_probes),
        stab: calibration_evidence(Implementation::Stab, stab_decision, stab_probes),
        common_iterations: common_iterations.get(),
        common_validation,
    };

    let mut timing_attempts = vec![execute_timing_attempt(
        &workers,
        &resolved_group.contract,
        0,
        TimingAttemptKind::Initial,
        timing_plan,
    )?];
    if timing_attempts
        .first()
        .is_some_and(TimingAttempt::requires_noisy_rerun)
    {
        timing_attempts.push(execute_timing_attempt(
            &workers,
            &resolved_group.contract,
            1,
            TimingAttemptKind::PairedRatioNoiseRerun,
            timing_plan,
        )?);
    }

    let memory_execution = execute_pair(
        &workers,
        &resolved_group.contract,
        0,
        common_batch,
        EvidenceMode::Memory,
        Some(ExpectedOutputDigests::same(&expected_output_digest)),
    )?;
    let memory = memory_evidence(memory_execution, common_iterations)?;
    workers.verify()?;
    let host = host_guard.finish()?;
    let repository_after = super::git::repository_state(root)?;
    if repository_before.commit != repository_after.commit {
        return Err(RunError::RepositoryChanged {
            before: repository_before.commit,
            after: repository_after.commit,
        });
    }
    let repository = RepositoryEvidence {
        commit_before: repository_before.commit,
        commit_after: repository_after.commit,
        local_modifications_before: repository_before.local_modifications,
        local_modifications_after: repository_after.local_modifications,
    };
    let promotable = super::report::promotion_eligibility(super::report::PromotionEvidence {
        claim_class,
        allow_unverified_host: args.allow_unverified_host,
        tier: args.tier,
        local_modifications_before: repository.local_modifications_before,
        local_modifications_after: repository.local_modifications_after,
        host_verified: host.verified,
        correctness_status: correctness_preflight.status,
        correctness_case_count: correctness_preflight.case_ids.len(),
    });
    let report = QualificationReport {
        schema_version: REPORT_SCHEMA_VERSION,
        group_id: resolved_group.contract.id.to_string(),
        scale_id: scale_id.to_string(),
        group_contract_sha256: resolved_group.source_sha256,
        claim_class,
        baseline_eligibility: resolved_group.contract.baseline_eligibility,
        owner: resolved_group.contract.owner.to_string(),
        profiler_note: resolved_group.contract.profiler_note.clone(),
        tier: args.tier,
        command: RunCommandEvidence {
            output: args.out.to_string_lossy().into_owned(),
            group_id: resolved_group.contract.id.to_string(),
            scale_id: scale_id.to_string(),
            work_items: scale.work_items.get(),
            allow_unverified_host: args.allow_unverified_host,
            warmup_batches: WARMUP_BATCHES,
            paired_samples: args.tier.pair_count(),
            maximum_timing_attempts: MAXIMUM_TIMING_ATTEMPTS,
            invocation_timeout_seconds: INVOCATION_TIMEOUT.as_secs(),
            correctness_output: args
                .correctness_out
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            correctness_request_sha256: args.correctness_request_sha256.clone(),
            correctness_completion_sha256: args.correctness_completion_sha256.clone(),
        },
        generated_unix_epoch_seconds: unix_epoch_seconds()?,
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        performance_inventory_sha256: performance_inventory_sha256.to_string(),
        correctness_inventory_sha256: correctness_inventory_sha256.to_string(),
        repository,
        host,
        toolchain,
        workers: workers.identity_evidence()?,
        contract_preflight: workers.contract_preflight_evidence()?.clone(),
        adapter_receipt: workers.adapter_receipt().clone(),
        stab_build_receipt: workers.stab_build_receipt().clone(),
        correctness_preflight,
        semantic_preflight,
        calibration,
        timing_attempts,
        memory,
        promotable,
    };
    super::report::validate_report(
        root,
        &report,
        performance_inventory_sha256,
        correctness_inventory_sha256,
    )?;
    let report_json = render_json(&report)?;
    let preflight = super::report::preflight_artifact(&report, &report_json)?;
    let preflight_json = render_json(&preflight)?;
    let markdown = super::report::render_markdown(&report, &sha256_hex(&report_json))?;
    let output = QualificationOutput::begin_new(root, &args.out)?;
    output.write("report.json", &report_json)?;
    output.write("preflight.json", &preflight_json)?;
    output.write("report.md", markdown.as_bytes())?;
    let relative = output.relative().to_path_buf();
    output.commit_new()?;
    Ok(relative)
}

fn correctness_preflight(
    root: &RepoRoot,
    group: &super::group::GroupContract,
    args: &RunArgs,
    correctness_inventory_sha256: &str,
    stab_commit: &str,
) -> Result<CorrectnessPreflightEvidence, RunError> {
    let requirement = match group.claim_class {
        ClaimClass::DiagnosticInfrastructure => {
            if args.correctness_out.is_some()
                || args.correctness_request_sha256.is_some()
                || args.correctness_completion_sha256.is_some()
            {
                return Err(RunError::UnexpectedCorrectnessInput);
            }
            CorrectnessRequirement::NotApplicable {
                reason: "PQ1 protocol-smoke validates benchmark infrastructure and cannot support a product performance claim",
            }
        }
        ClaimClass::PromotablePerformance => CorrectnessRequirement::Required {
            output: args
                .correctness_out
                .as_deref()
                .ok_or(RunError::MissingCorrectnessInput)?,
            case_ids: &group.correctness_case_ids,
            expected_manifest_sha256: correctness_inventory_sha256,
            expected_stab_commit: stab_commit,
            expected_request_sha256: args
                .correctness_request_sha256
                .as_deref()
                .ok_or(RunError::MissingCorrectnessInput)?,
            expected_completion_sha256: args
                .correctness_completion_sha256
                .as_deref()
                .ok_or(RunError::MissingCorrectnessInput)?,
        },
    };
    Ok(super::correctness::validate(root, requirement)?)
}

fn calibrate_worker(
    workers: &PreparedWorkers,
    group: &super::group::GroupContract,
    implementation: Implementation,
    scale: &super::group::ScaleContract,
    policy: CalibrationPolicy,
) -> Result<(CalibrationDecision, Vec<CalibrationProbeEvidence>), RunError> {
    let mut evidence = Vec::new();
    let decision = calibrate(policy, |iterations| {
        let invocation = workers
            .invoke(InvocationRequest {
                group,
                implementation,
                evidence_mode: EvidenceMode::Timing,
                iterations,
                scale,
                expected_output_digest: None,
                timeout: INVOCATION_TIMEOUT,
            })
            .map_err(|error| error.to_string())?;
        let measured = invocation
            .measured_duration()
            .map_err(|error| error.to_string())?;
        let wall = invocation
            .wall_duration()
            .map_err(|error| error.to_string())?;
        evidence.push(CalibrationProbeEvidence {
            iterations: iterations.get(),
            invocation,
        });
        Ok(CalibrationProbe { measured, wall })
    })?;
    Ok((decision, evidence))
}

pub(super) fn calibration_policy() -> Result<CalibrationPolicy, RunError> {
    Ok(CalibrationPolicy {
        minimum: CALIBRATION_TARGET_MINIMUM,
        maximum: CALIBRATION_MAXIMUM,
        timeout: INVOCATION_TIMEOUT,
        maximum_iterations: NonZeroU64::new(MAXIMUM_ITERATIONS)
            .ok_or(RunError::InvalidIterationCap)?,
    })
}

fn calibration_evidence(
    implementation: Implementation,
    decision: CalibrationDecision,
    probes: Vec<CalibrationProbeEvidence>,
) -> ImplementationCalibration {
    ImplementationCalibration {
        implementation,
        selected_iterations: decision.iterations.get(),
        selected_measured_seconds: decision.measured.as_secs_f64(),
        probes,
    }
}

#[derive(Clone, Copy)]
struct WorkloadBatch<'a> {
    stim_iterations: NonZeroU64,
    stab_iterations: NonZeroU64,
    scale: &'a super::group::ScaleContract,
}

impl<'a> WorkloadBatch<'a> {
    const fn common(iterations: NonZeroU64, scale: &'a super::group::ScaleContract) -> Self {
        Self {
            stim_iterations: iterations,
            stab_iterations: iterations,
            scale,
        }
    }

    const fn iterations(self, implementation: Implementation) -> NonZeroU64 {
        match implementation {
            Implementation::Stim => self.stim_iterations,
            Implementation::Stab => self.stab_iterations,
        }
    }
}

#[derive(Clone, Copy)]
struct ExpectedOutputDigests<'a> {
    stim: &'a SemanticDigest,
    stab: &'a SemanticDigest,
}

impl<'a> ExpectedOutputDigests<'a> {
    const fn same(digest: &'a SemanticDigest) -> Self {
        Self {
            stim: digest,
            stab: digest,
        }
    }

    const fn for_implementation(self, implementation: Implementation) -> &'a SemanticDigest {
        match implementation {
            Implementation::Stim => self.stim,
            Implementation::Stab => self.stab,
        }
    }
}

#[derive(Clone, Copy)]
struct TimingAttemptPlan<'a> {
    tier: QualificationTier,
    batch: WorkloadBatch<'a>,
    expected_output_digests: ExpectedOutputDigests<'a>,
    batch_policy: TimingBatchPolicy,
}

fn selected_calibration_output_digest(
    probes: &[CalibrationProbeEvidence],
    selected_iterations: NonZeroU64,
) -> Result<SemanticDigest, RunError> {
    let probe = probes.last().ok_or(RunError::MissingCalibrationProbe)?;
    if probe.iterations != selected_iterations.get() {
        return Err(RunError::MissingCalibrationProbe);
    }
    Ok(only_row(&probe.invocation.rows)?.output_digest.clone())
}

pub(super) fn selected_output_matches_common(
    common_iterations: u64,
    selected_iterations: u64,
    common_digest: &SemanticDigest,
    selected_digest: &SemanticDigest,
) -> bool {
    selected_iterations != common_iterations || selected_digest == common_digest
}

fn execute_timing_attempt(
    workers: &PreparedWorkers,
    group: &super::group::GroupContract,
    attempt_index: usize,
    kind: TimingAttemptKind,
    plan: TimingAttemptPlan<'_>,
) -> Result<TimingAttempt, RunError> {
    let mut warmups = Vec::with_capacity(WARMUP_BATCHES);
    for pair_index in 0..WARMUP_BATCHES {
        let execution = execute_pair(
            workers,
            group,
            pair_index,
            plan.batch,
            EvidenceMode::Timing,
            Some(plan.expected_output_digests),
        )?;
        pair_execution(&execution, plan.batch_policy)?;
        warmups.push(execution);
    }

    let mut samples = Vec::with_capacity(plan.tier.pair_count());
    let mut paired_samples = Vec::with_capacity(plan.tier.pair_count());
    for pair_index in 0..plan.tier.pair_count() {
        let execution = execute_pair(
            workers,
            group,
            pair_index,
            plan.batch,
            EvidenceMode::Timing,
            Some(plan.expected_output_digests),
        )?;
        paired_samples.extend(pair_execution(&execution, plan.batch_policy)?);
        samples.push(execution);
    }
    let measurement_id = group.single_measurement()?.clone();
    let statistics = vec![summarize(
        measurement_id,
        &paired_samples,
        PRIMARY_THRESHOLD,
    )?];
    let worst_confidence_interval_upper = statistics
        .iter()
        .map(|summary| summary.confidence_interval_upper)
        .reduce(f64::max)
        .ok_or(RunError::MissingStatistics)?;
    Ok(TimingAttempt {
        attempt_index,
        kind,
        warmups,
        samples,
        paired_samples,
        statistics,
        worst_confidence_interval_upper,
    })
}

fn execute_pair(
    workers: &PreparedWorkers,
    group: &super::group::GroupContract,
    pair_index: usize,
    batch: WorkloadBatch<'_>,
    evidence_mode: EvidenceMode,
    expected_output_digests: Option<ExpectedOutputDigests<'_>>,
) -> Result<PairExecution, RunError> {
    let order = PairOrder::for_pair(pair_index);
    let invoke = |implementation| {
        workers.invoke(InvocationRequest {
            group,
            implementation,
            evidence_mode,
            iterations: batch.iterations(implementation),
            scale: batch.scale,
            expected_output_digest: expected_output_digests
                .map(|digests| digests.for_implementation(implementation)),
            timeout: INVOCATION_TIMEOUT,
        })
    };
    let (stim, stab) = match order {
        PairOrder::StimThenStab => {
            let stim = invoke(Implementation::Stim)?;
            let stab = invoke(Implementation::Stab)?;
            (stim, stab)
        }
        PairOrder::StabThenStim => {
            let stab = invoke(Implementation::Stab)?;
            let stim = invoke(Implementation::Stim)?;
            (stim, stab)
        }
    };
    Ok(PairExecution {
        pair_index,
        order,
        stim,
        stab,
    })
}

fn pair_execution(
    execution: &PairExecution,
    batch_policy: TimingBatchPolicy,
) -> Result<Vec<PairedSample>, RunError> {
    Ok(pair_measurements_with_policy(
        execution.pair_index,
        execution.order,
        &execution.stim.rows,
        &execution.stab.rows,
        batch_policy,
    )?)
}

fn validate_common_calibration(
    batch_policy: TimingBatchPolicy,
    execution: &PairExecution,
    stim_selected_iterations: u64,
    stab_selected_iterations: u64,
) -> Result<CommonBatchMode, RunError> {
    classify_common_calibration(
        batch_policy,
        stim_selected_iterations,
        stab_selected_iterations,
        execution.stim.measured_duration()?,
        execution.stab.measured_duration()?,
    )
}

pub(super) fn classify_common_calibration(
    batch_policy: TimingBatchPolicy,
    stim_selected_iterations: u64,
    stab_selected_iterations: u64,
    stim_measured: Duration,
    stab_measured: Duration,
) -> Result<CommonBatchMode, RunError> {
    if batch_policy == TimingBatchPolicy::IndependentThroughput {
        for (implementation, measured) in [
            (Implementation::Stim, stim_measured),
            (Implementation::Stab, stab_measured),
        ] {
            if measured.is_zero() || measured > CALIBRATION_MAXIMUM {
                return Err(RunError::CommonCalibrationOutOfBounds {
                    implementation,
                    measured,
                });
            }
        }
        return Ok(CommonBatchMode::IndependentThroughput);
    }
    for (implementation, measured) in [
        (Implementation::Stim, stim_measured),
        (Implementation::Stab, stab_measured),
    ] {
        if measured < CALIBRATION_ACCEPTANCE_MINIMUM || measured > CALIBRATION_WIDE_RATIO_MAXIMUM {
            return Err(RunError::CommonCalibrationOutOfBounds {
                implementation,
                measured,
            });
        }
    }
    if stim_measured <= CALIBRATION_MAXIMUM && stab_measured <= CALIBRATION_MAXIMUM {
        return Ok(CommonBatchMode::Standard);
    }
    let wide_ratio_is_valid = if stim_selected_iterations < stab_selected_iterations {
        stim_measured > CALIBRATION_MAXIMUM && stab_measured <= CALIBRATION_MAXIMUM
    } else if stab_selected_iterations < stim_selected_iterations {
        stab_measured > CALIBRATION_MAXIMUM && stim_measured <= CALIBRATION_MAXIMUM
    } else {
        false
    };
    if wide_ratio_is_valid {
        return Ok(CommonBatchMode::WideRatio);
    }
    let (implementation, measured) = if stim_measured > CALIBRATION_MAXIMUM {
        (Implementation::Stim, stim_measured)
    } else {
        (Implementation::Stab, stab_measured)
    };
    Err(RunError::CommonCalibrationOutOfBounds {
        implementation,
        measured,
    })
}

fn memory_evidence(
    execution: PairExecution,
    iterations: NonZeroU64,
) -> Result<MemoryEvidence, RunError> {
    let stim = only_row(&execution.stim.rows)?;
    let stab = only_row(&execution.stab.rows)?;
    if stim.evidence_mode != EvidenceMode::Memory
        || stab.evidence_mode != EvidenceMode::Memory
        || stim.work_count != stab.work_count
        || stim.output_digest != stab.output_digest
        || stim.iteration_count != stab.iteration_count
    {
        return Err(RunError::MemorySemanticMismatch);
    }
    let stim_setup = stim.setup_rss_bytes.ok_or(RunError::MissingMemory)?;
    let stim_peak = stim.peak_rss_bytes.ok_or(RunError::MissingMemory)?;
    let stab_setup = stab.setup_rss_bytes.ok_or(RunError::MissingMemory)?;
    let stab_peak = stab.peak_rss_bytes.ok_or(RunError::MissingMemory)?;
    Ok(MemoryEvidence {
        evidence_mode: EvidenceMode::Memory,
        iterations: iterations.get(),
        work_count: stim.work_count,
        stim_setup_rss_bytes: stim_setup,
        stim_peak_rss_bytes: stim_peak,
        stim_parent_observed_peak_rss_bytes: execution.stim.parent_observed_peak_rss_bytes,
        stab_setup_rss_bytes: stab_setup,
        stab_peak_rss_bytes: stab_peak,
        stab_parent_observed_peak_rss_bytes: execution.stab.parent_observed_peak_rss_bytes,
        execution,
    })
}

fn only_row(rows: &[WorkerMeasurement]) -> Result<&WorkerMeasurement, RunError> {
    let [row] = rows else {
        return Err(RunError::ExpectedOneMeasurement(rows.len()));
    };
    Ok(row)
}

fn render_json(value: &impl Serialize) -> Result<Vec<u8>, RunError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len().saturating_mul(2));
    for byte in digest {
        output.push(hex_digit(byte >> 4));
        output.push(hex_digit(byte & 0x0f));
    }
    output
}

fn hex_digit(value: u8) -> char {
    char::from(if value < 10 {
        b'0' + value
    } else {
        b'a' + (value - 10)
    })
}

fn unix_epoch_seconds() -> Result<u64, RunError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(RunError::Clock)
}

#[derive(Debug, Error)]
pub(super) enum RunError {
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),
    #[error(transparent)]
    Host(#[from] super::host::HostError),
    #[error(transparent)]
    Invocation(#[from] super::invocation::InvocationError),
    #[error(transparent)]
    Calibration(#[from] super::calibration::CalibrationError),
    #[error(transparent)]
    Correctness(#[from] super::correctness::CorrectnessError),
    #[error(transparent)]
    Toolchain(#[from] super::toolchain::ToolchainError),
    #[error(transparent)]
    Statistics(#[from] super::statistics::StatisticsError),
    #[error(transparent)]
    Process(#[from] super::process::ProcessError),
    #[error(transparent)]
    Git(#[from] super::git::GitError),
    #[error(transparent)]
    Group(#[from] super::group::GroupError),
    #[error(transparent)]
    Report(#[from] super::report::ReportError),
    #[error("qualification maximum iteration cap must be positive")]
    InvalidIterationCap,
    #[error("diagnostic PQ1 protocol-smoke does not accept product correctness evidence")]
    UnexpectedCorrectnessInput,
    #[error(
        "promotable performance evidence requires a CQ1 report directory plus controller-approved request and completion digests"
    )]
    MissingCorrectnessInput,
    #[error(
        "common calibrated batch measured {measured:?} for {implementation}, outside the source-owned bounds"
    )]
    CommonCalibrationOutOfBounds {
        implementation: Implementation,
        measured: Duration,
    },
    #[error("qualification produced no measurement statistics")]
    MissingStatistics,
    #[error("qualification calibration evidence has no selected output receipt")]
    MissingCalibrationProbe,
    #[error(
        "{0:?} selected calibration output differs from the exact common semantic output at the same iteration count"
    )]
    SelectedCalibrationSemanticMismatch(Implementation),
    #[error("memory evidence differs in mode, semantic work, digest, or iteration count")]
    MemorySemanticMismatch,
    #[error("memory evidence omits setup or peak RSS")]
    MissingMemory,
    #[error("qualification expected one measurement but received {0}")]
    ExpectedOneMeasurement(usize),
    #[error("repository commit changed during qualification: {before} -> {after}")]
    RepositoryChanged { before: String, after: String },
    #[error("system clock is before the Unix epoch: {0}")]
    Clock(std::time::SystemTimeError),
    #[error("failed to serialize qualification evidence: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
#[path = "run/tests.rs"]
mod tests;
