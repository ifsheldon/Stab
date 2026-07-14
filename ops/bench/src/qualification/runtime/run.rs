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
use super::invocation::{InvocationRecord, PreparedWorkers, WorkerIdentityEvidence, protocol_ids};
use super::protocol::{EvidenceMode, Implementation, SemanticDigest, WorkerMeasurement};
use super::statistics::{PairOrder, PairedSample, StatisticsSummary, pair_measurements, summarize};
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::root::RepoRoot;

pub(super) const REPORT_SCHEMA_VERSION: u32 = 8;
const DEFAULT_OUTPUT: &str = "target/benchmarks/qualification/latest";
const CALIBRATION_ACCEPTANCE_MINIMUM: Duration = Duration::from_millis(250);
const CALIBRATION_TARGET_MINIMUM: Duration = Duration::from_millis(350);
const CALIBRATION_MAXIMUM: Duration = Duration::from_secs(2);
const INVOCATION_TIMEOUT: Duration = Duration::from_secs(30);
const MAXIMUM_ITERATIONS: u64 = 1_000_000_000;
const WARMUP_BATCHES: usize = 3;
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
    /// Qualification tier controlling the number of retained pairs.
    #[arg(long, value_enum)]
    tier: QualificationTier,

    /// Atomic report directory below target/benchmarks/qualification.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    out: PathBuf,

    /// Preserve a diagnostic report when source-owned host limits are not met.
    #[arg(long)]
    allow_unverified_host: bool,

    /// Semantic work items per protocol-smoke iteration.
    #[arg(long, default_value = "4096")]
    work_items: NonZeroU64,

    /// CQ1 report directory used by future promotable product groups.
    #[arg(long)]
    correctness_out: Option<PathBuf>,

    /// Exact CQ1 case required by a future promotable product group.
    #[arg(long, requires = "correctness_out")]
    correctness_case: Vec<String>,
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
    pub(super) claim_class: ClaimClass,
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
    pub(super) adapter_receipt: super::adapter::AdapterBuildReceipt,
    pub(super) stab_build_receipt: super::stab_build::StabBuildReceipt,
    pub(super) correctness_preflight: CorrectnessPreflightEvidence,
    pub(super) semantic_preflight: PairExecution,
    pub(super) calibration: CalibrationEvidence,
    pub(super) warmups: Vec<PairExecution>,
    pub(super) samples: Vec<PairExecution>,
    pub(super) paired_samples: Vec<PairedSample>,
    pub(super) statistics: Vec<StatisticsSummary>,
    pub(super) worst_confidence_interval_upper: f64,
    pub(super) memory: MemoryEvidence,
    pub(super) promotable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunCommandEvidence {
    pub(super) output: String,
    pub(super) work_items: u64,
    pub(super) allow_unverified_host: bool,
    pub(super) warmup_batches: usize,
    pub(super) paired_samples: usize,
    pub(super) invocation_timeout_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    pub(super) stim: ImplementationCalibration,
    pub(super) stab: ImplementationCalibration,
    pub(super) common_iterations: u64,
    pub(super) common_validation: PairExecution,
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
    let repository_before = super::git::repository_state(root)?;
    let claim_class = ClaimClass::DiagnosticInfrastructure;
    let correctness_preflight = correctness_preflight(
        root,
        claim_class,
        &args,
        correctness_inventory_sha256,
        &repository_before.commit,
    )?;
    let toolchain = super::toolchain::collect(root)?;
    let mut workers = PreparedWorkers::prepare(root, &repository_before.commit, &toolchain)?;
    let host_guard = HostGuard::prepare(root, args.allow_unverified_host)?;
    workers.pin_to_cpu(host_guard.selected_cpu());

    let policy = calibration_policy()?;
    let (stim_decision, stim_probes) =
        calibrate_worker(&workers, Implementation::Stim, args.work_items, policy)?;
    let (stab_decision, stab_probes) =
        calibrate_worker(&workers, Implementation::Stab, args.work_items, policy)?;
    let common_iterations = stim_decision.iterations.max(stab_decision.iterations);
    let semantic_preflight = execute_pair(
        &workers,
        0,
        common_iterations,
        args.work_items,
        EvidenceMode::Timing,
        None,
    )?;
    pair_execution(&semantic_preflight)?;
    let expected_output_digest = only_row(&semantic_preflight.stim.rows)?
        .output_digest
        .clone();
    let common_validation = execute_pair(
        &workers,
        0,
        common_iterations,
        args.work_items,
        EvidenceMode::Timing,
        Some(&expected_output_digest),
    )?;
    pair_execution(&common_validation)?;
    validate_common_calibration(&common_validation)?;
    let calibration = CalibrationEvidence {
        acceptance_minimum_seconds: CALIBRATION_ACCEPTANCE_MINIMUM.as_secs_f64(),
        target_minimum_seconds: CALIBRATION_TARGET_MINIMUM.as_secs_f64(),
        maximum_seconds: CALIBRATION_MAXIMUM.as_secs_f64(),
        stim: calibration_evidence(Implementation::Stim, stim_decision, stim_probes),
        stab: calibration_evidence(Implementation::Stab, stab_decision, stab_probes),
        common_iterations: common_iterations.get(),
        common_validation,
    };

    let mut warmups = Vec::with_capacity(WARMUP_BATCHES);
    for pair_index in 0..WARMUP_BATCHES {
        let execution = execute_pair(
            &workers,
            pair_index,
            common_iterations,
            args.work_items,
            EvidenceMode::Timing,
            Some(&expected_output_digest),
        )?;
        pair_execution(&execution)?;
        warmups.push(execution);
    }

    let mut samples = Vec::with_capacity(args.tier.pair_count());
    let mut paired_samples = Vec::with_capacity(args.tier.pair_count());
    for pair_index in 0..args.tier.pair_count() {
        let execution = execute_pair(
            &workers,
            pair_index,
            common_iterations,
            args.work_items,
            EvidenceMode::Timing,
            Some(&expected_output_digest),
        )?;
        paired_samples.extend(pair_execution(&execution)?);
        samples.push(execution);
    }
    let (_, measurement_id) = protocol_ids()?;
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

    let memory_execution = execute_pair(
        &workers,
        0,
        common_iterations,
        args.work_items,
        EvidenceMode::Memory,
        Some(&expected_output_digest),
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
    let report = QualificationReport {
        schema_version: REPORT_SCHEMA_VERSION,
        group_id: "pq1-adapter-protocol-smoke".to_string(),
        claim_class,
        tier: args.tier,
        command: RunCommandEvidence {
            output: args.out.to_string_lossy().into_owned(),
            work_items: args.work_items.get(),
            allow_unverified_host: args.allow_unverified_host,
            warmup_batches: WARMUP_BATCHES,
            paired_samples: args.tier.pair_count(),
            invocation_timeout_seconds: INVOCATION_TIMEOUT.as_secs(),
        },
        generated_unix_epoch_seconds: unix_epoch_seconds()?,
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        performance_inventory_sha256: performance_inventory_sha256.to_string(),
        correctness_inventory_sha256: correctness_inventory_sha256.to_string(),
        repository,
        host,
        toolchain,
        workers: workers.identity_evidence(),
        adapter_receipt: workers.adapter_receipt().clone(),
        stab_build_receipt: workers.stab_build_receipt().clone(),
        correctness_preflight,
        semantic_preflight,
        calibration,
        warmups,
        samples,
        paired_samples,
        statistics,
        worst_confidence_interval_upper,
        memory,
        promotable: false,
    };
    super::report::validate_report(&report)?;
    let report_json = render_json(&report)?;
    let preflight = super::report::preflight_artifact(&report, &report_json)?;
    let preflight_json = render_json(&preflight)?;
    let markdown = super::report::render_markdown(&report, &sha256_hex(&report_json));
    let output = QualificationOutput::begin(root, &args.out)?;
    output.write("report.json", &report_json)?;
    output.write("preflight.json", &preflight_json)?;
    output.write("report.md", markdown.as_bytes())?;
    let relative = output.relative().to_path_buf();
    output.commit()?;
    Ok(relative)
}

fn correctness_preflight(
    root: &RepoRoot,
    claim_class: ClaimClass,
    args: &RunArgs,
    correctness_inventory_sha256: &str,
    stab_commit: &str,
) -> Result<CorrectnessPreflightEvidence, RunError> {
    let requirement = match claim_class {
        ClaimClass::DiagnosticInfrastructure => {
            if args.correctness_out.is_some() || !args.correctness_case.is_empty() {
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
            case_ids: &args.correctness_case,
            expected_manifest_sha256: correctness_inventory_sha256,
            expected_stab_commit: stab_commit,
        },
    };
    Ok(super::correctness::validate(root, requirement)?)
}

fn calibrate_worker(
    workers: &PreparedWorkers,
    implementation: Implementation,
    work_items: NonZeroU64,
    policy: CalibrationPolicy,
) -> Result<(CalibrationDecision, Vec<CalibrationProbeEvidence>), RunError> {
    let mut evidence = Vec::new();
    let decision = calibrate(policy, |iterations| {
        let invocation = workers
            .invoke(
                implementation,
                EvidenceMode::Timing,
                iterations,
                work_items,
                None,
                INVOCATION_TIMEOUT,
            )
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

fn calibration_policy() -> Result<CalibrationPolicy, RunError> {
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

fn execute_pair(
    workers: &PreparedWorkers,
    pair_index: usize,
    iterations: NonZeroU64,
    work_items: NonZeroU64,
    evidence_mode: EvidenceMode,
    expected_output_digest: Option<&SemanticDigest>,
) -> Result<PairExecution, RunError> {
    let order = PairOrder::for_pair(pair_index);
    let invoke = |implementation| {
        workers.invoke(
            implementation,
            evidence_mode,
            iterations,
            work_items,
            expected_output_digest,
            INVOCATION_TIMEOUT,
        )
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

fn pair_execution(execution: &PairExecution) -> Result<Vec<PairedSample>, RunError> {
    Ok(pair_measurements(
        execution.pair_index,
        execution.order,
        &execution.stim.rows,
        &execution.stab.rows,
    )?)
}

fn validate_common_calibration(execution: &PairExecution) -> Result<(), RunError> {
    for invocation in [&execution.stim, &execution.stab] {
        let measured = invocation.measured_duration()?;
        if !common_calibration_duration_is_accepted(measured) {
            return Err(RunError::CommonCalibrationOutOfBounds {
                implementation: invocation.implementation,
                measured,
            });
        }
    }
    Ok(())
}

fn common_calibration_duration_is_accepted(measured: Duration) -> bool {
    measured >= CALIBRATION_ACCEPTANCE_MINIMUM && measured <= CALIBRATION_MAXIMUM
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
    Report(#[from] super::report::ReportError),
    #[error("qualification maximum iteration cap must be positive")]
    InvalidIterationCap,
    #[error("diagnostic PQ1 protocol-smoke does not accept product correctness evidence")]
    UnexpectedCorrectnessInput,
    #[error("promotable performance evidence requires a CQ1 report directory and exact cases")]
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
mod tests {
    use super::*;

    #[test]
    fn tiers_have_source_owned_pair_counts() {
        assert_eq!(QualificationTier::Pr.pair_count(), 3);
        assert_eq!(QualificationTier::Full.pair_count(), 9);
        assert_eq!(QualificationTier::Soak.pair_count(), 15);
    }

    #[test]
    fn diagnostic_claim_cannot_be_promotable() {
        assert_ne!(
            ClaimClass::DiagnosticInfrastructure,
            ClaimClass::PromotablePerformance
        );
    }

    #[test]
    fn calibration_guard_band_preserves_the_acceptance_floor() {
        let policy = calibration_policy().expect("calibration policy");
        assert_eq!(policy.minimum, Duration::from_millis(350));
        assert_eq!(CALIBRATION_ACCEPTANCE_MINIMUM, Duration::from_millis(250));
        assert!(policy.minimum > CALIBRATION_ACCEPTANCE_MINIMUM);
        assert!(!common_calibration_duration_is_accepted(
            Duration::from_micros(249_999)
        ));
        assert!(common_calibration_duration_is_accepted(
            Duration::from_millis(250)
        ));
        assert!(common_calibration_duration_is_accepted(
            Duration::from_secs(2)
        ));
        assert!(!common_calibration_duration_is_accepted(
            Duration::from_micros(2_000_001)
        ));
    }
}
