use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Args;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::artifact::{DirectQualificationArtifactPath, QualificationOutput, RepositoryBinding};
use super::calibration::{CalibrationProbe, calibrate};
use super::group::{
    GroupContract, ProductDiagnosticBatchPolicy, ProductDiagnosticPolicy,
    ProductDiagnosticScalePolicy, ScaleContract,
};
use super::host::{HostEvidence, HostGuard};
use super::invocation::{
    DiagnosticInvocationRequest, DiagnosticWorkerIdentityEvidence, InvocationError,
    InvocationRecord, PreparedDiagnosticWorker,
};
use super::protocol::{
    EvidenceMode, Implementation, RAW_WORK_TIMING_BOUNDARY, SemanticDigest, TimingBoundary,
    WorkerMeasurement,
};
use super::run::{
    ClaimClass, INVOCATION_TIMEOUT, QualificationTier, RepositoryEvidence, WARMUP_BATCHES,
};
use super::statistics::median_in_place;
use crate::qualification::model::SizeClass;
use crate::root::RepoRoot;

const DIAGNOSTIC_REPORT_SCHEMA_VERSION: u32 = 3;
const DEFAULT_OUTPUT: &str = "target/benchmarks/qualification/a2-diagnostic-latest";

#[derive(Clone, Debug, Args)]
pub(crate) struct DiagnosticArgs {
    /// Source-owned Stab-only product diagnostic group.
    #[arg(long)]
    group: String,

    /// One source-owned workload scale. Defaults to small unless --all-scales is used.
    #[arg(long, conflicts_with = "all_scales")]
    scale: Option<String>,

    /// Run every source-owned scale in declaration order.
    #[arg(long)]
    all_scales: bool,

    /// Diagnostic tier controlling the number of retained raw samples.
    #[arg(long, value_enum, default_value = "pr")]
    tier: QualificationTier,

    /// Atomic report directory below target/benchmarks/qualification.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    out: PathBuf,

    /// Preserve a diagnostic report when source-owned host limits are not met.
    #[arg(long)]
    allow_unverified_host: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DiagnosticScope {
    StabOnlyProduct,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticReport {
    schema_version: u32,
    scope: DiagnosticScope,
    timing_boundary: TimingBoundary,
    group_id: String,
    group_contract_sha256: String,
    claim_class: ClaimClass,
    owner: String,
    correctness_case_ids: Vec<String>,
    generated_unix_epoch_seconds: u64,
    performance_inventory_sha256: String,
    correctness_inventory_sha256: String,
    command: DiagnosticCommandEvidence,
    repository: RepositoryEvidence,
    host: HostEvidence,
    toolchain: super::toolchain::ToolchainEvidence,
    worker: DiagnosticWorkerIdentityEvidence,
    stab_build_receipt: super::stab_build::StabBuildReceipt,
    scales: Vec<DiagnosticScaleEvidence>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticCommandEvidence {
    output: String,
    scale_ids: Vec<String>,
    tier: QualificationTier,
    allow_unverified_host: bool,
    warmup_batches: usize,
    retained_samples: usize,
    invocation_timeout_seconds: u64,
    suite_timeout_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticScaleEvidence {
    scale_id: String,
    family_id: String,
    size_class: SizeClass,
    work_items: u64,
    input_bytes: u64,
    input_digest: String,
    batch_policy: ProductDiagnosticBatchPolicy,
    witness_case_id: String,
    expected_output_digest: String,
    calibration: DiagnosticCalibrationEvidence,
    semantic_validation: InvocationRecord,
    warmups: Vec<InvocationRecord>,
    samples: Vec<InvocationRecord>,
    memory: Option<DiagnosticMemoryEvidence>,
    summary: DiagnosticScaleSummary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticMemoryEvidence {
    max_worker_peak_rss_bytes: u64,
    setup_rss_bytes: u64,
    peak_rss_bytes: u64,
    peak_delta_bytes: u64,
    parent_observed_peak_rss_bytes: Option<u64>,
    invocation: InvocationRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticScaleSummary {
    median_batch_seconds: f64,
    median_seconds_per_work_item: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticCalibrationEvidence {
    selected_iterations: u64,
    selected_measured_seconds: f64,
    probes: Vec<DiagnosticCalibrationProbe>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticCalibrationProbe {
    iterations: u64,
    invocation: InvocationRecord,
}

pub(super) fn run_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    live_repository: &RepositoryBinding,
    performance_inventory_sha256: &str,
    correctness_inventory_sha256: &str,
    args: DiagnosticArgs,
) -> Result<PathBuf, DiagnosticError> {
    let output_path = DirectQualificationArtifactPath::try_new(&args.out)?;
    QualificationOutput::require_absent_with_repository(root, live_repository, &output_path)?;
    let repository_before = super::run::bound_repository_state(root, live_repository)?;
    let resolved_group =
        super::group::load_group(source_root, performance_inventory_sha256, &args.group)?;
    require_product_diagnostic(&resolved_group.contract)?;
    let diagnostic_policy = resolved_group
        .product_diagnostic_policy
        .as_ref()
        .ok_or(DiagnosticError::MissingSourceOwnedWitness)?;
    let scales = selected_scales(&resolved_group.contract, &args)?;
    let scale_ids = scales
        .iter()
        .map(|scale| scale.id.to_string())
        .collect::<Vec<_>>();

    live_repository.require_current(root)?;
    let mut host_guard = HostGuard::prepare(source_root, args.allow_unverified_host)?;
    let toolchain = super::toolchain::collect(source_root)?;
    live_repository.require_current(root)?;
    let mut worker =
        PreparedDiagnosticWorker::prepare(source_root, &repository_before.commit, &toolchain)?;
    worker.pin_to_cpu(host_guard.selected_cpu());
    let suite_timeout_seconds = resolved_group
        .product_diagnostic_suite_timeout_seconds
        .get();
    let deadline = SuiteDeadline::start(Duration::from_secs(suite_timeout_seconds))?;

    let mut scale_evidence = Vec::with_capacity(scales.len());
    for scale in scales {
        scale_evidence.push(run_scale(
            &worker,
            &resolved_group.contract,
            diagnostic_policy,
            scale,
            args.tier,
            &deadline,
        )?);
    }
    deadline.require_remaining()?;
    worker.verify()?;
    let worker_identity = worker.identity_evidence();
    let build_receipt = worker.build_receipt().clone();
    let host = host_guard.finish()?;
    let repository_after = super::run::bound_repository_state(root, live_repository)?;
    if repository_before.commit != repository_after.commit {
        return Err(DiagnosticError::RepositoryChanged {
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
    let report = DiagnosticReport {
        schema_version: DIAGNOSTIC_REPORT_SCHEMA_VERSION,
        scope: DiagnosticScope::StabOnlyProduct,
        timing_boundary: RAW_WORK_TIMING_BOUNDARY,
        group_id: resolved_group.contract.id.to_string(),
        group_contract_sha256: resolved_group.source_sha256,
        claim_class: resolved_group.contract.claim_class,
        owner: resolved_group.contract.owner.to_string(),
        correctness_case_ids: resolved_group.contract.correctness_case_ids.clone(),
        generated_unix_epoch_seconds: super::run::unix_epoch_seconds()?,
        performance_inventory_sha256: performance_inventory_sha256.to_string(),
        correctness_inventory_sha256: correctness_inventory_sha256.to_string(),
        command: DiagnosticCommandEvidence {
            output: output_path.as_path().to_string_lossy().into_owned(),
            scale_ids,
            tier: args.tier,
            allow_unverified_host: args.allow_unverified_host,
            warmup_batches: WARMUP_BATCHES,
            retained_samples: args.tier.sample_count(),
            invocation_timeout_seconds: INVOCATION_TIMEOUT.as_secs(),
            suite_timeout_seconds,
        },
        repository,
        host,
        toolchain,
        worker: worker_identity,
        stab_build_receipt: build_receipt,
        scales: scale_evidence,
    };
    validate_report(
        &report,
        &resolved_group.contract,
        diagnostic_policy,
        suite_timeout_seconds,
    )?;
    let report_json = render_json(&report)?;
    let report_markdown = render_markdown(&report, &super::run::sha256_hex(&report_json));
    let repository_evidence = report.repository.clone();
    let mut output =
        QualificationOutput::begin_new_with_repository(root, live_repository, &output_path)?;
    output.write("report.json", &report_json)?;
    output.write("report.md", report_markdown.as_bytes())?;
    let relative = output.relative().to_path_buf();
    output.commit_new_with_source_validation(|repository| {
        super::run::require_current_repository(root, &repository_evidence, repository)
    })?;
    Ok(relative)
}

fn run_scale(
    worker: &PreparedDiagnosticWorker,
    group: &GroupContract,
    diagnostic_policy: &ProductDiagnosticPolicy,
    scale: &ScaleContract,
    tier: QualificationTier,
    deadline: &SuiteDeadline,
) -> Result<DiagnosticScaleEvidence, DiagnosticError> {
    let scale_policy = diagnostic_policy.scale(&scale.id)?;
    let semantic_validation = invoke(
        worker,
        group,
        scale,
        EvidenceMode::Contract,
        NonZeroU64::MIN,
        Some(&scale_policy.expected_output_digest),
        deadline,
    )?;
    let (selected_iterations, selected_measured_seconds, probes) =
        calibrate_scale(worker, group, scale_policy, scale, deadline)?;
    let mut warmups = Vec::with_capacity(WARMUP_BATCHES);
    for _ in 0..WARMUP_BATCHES {
        warmups.push(invoke(
            worker,
            group,
            scale,
            EvidenceMode::Timing,
            selected_iterations,
            Some(&scale_policy.expected_output_digest),
            deadline,
        )?);
    }
    let mut samples = Vec::with_capacity(tier.sample_count());
    for _ in 0..tier.sample_count() {
        samples.push(invoke(
            worker,
            group,
            scale,
            EvidenceMode::Timing,
            selected_iterations,
            Some(&scale_policy.expected_output_digest),
            deadline,
        )?);
    }
    let memory = scale_policy
        .max_worker_peak_rss_bytes
        .map(|maximum| {
            run_memory(
                worker,
                group,
                scale,
                maximum.get(),
                &scale_policy.expected_output_digest,
                deadline,
            )
        })
        .transpose()?;
    let summary = summarize_samples(&samples)?;
    Ok(DiagnosticScaleEvidence {
        scale_id: scale.id.to_string(),
        family_id: scale.family_id.to_string(),
        size_class: scale.size_class,
        work_items: scale.work_items.get(),
        input_bytes: scale.input_bytes,
        input_digest: scale.input_digest.as_str().to_string(),
        batch_policy: scale_policy.batch_policy,
        witness_case_id: scale_policy.witness_case_id.clone(),
        expected_output_digest: scale_policy.expected_output_digest.as_str().to_string(),
        calibration: DiagnosticCalibrationEvidence {
            selected_iterations: selected_iterations.get(),
            selected_measured_seconds,
            probes,
        },
        semantic_validation,
        warmups,
        samples,
        memory,
        summary,
    })
}

fn calibrate_scale(
    worker: &PreparedDiagnosticWorker,
    group: &GroupContract,
    scale_policy: &ProductDiagnosticScalePolicy,
    scale: &ScaleContract,
    deadline: &SuiteDeadline,
) -> Result<(NonZeroU64, f64, Vec<DiagnosticCalibrationProbe>), DiagnosticError> {
    let source_expected = &scale_policy.expected_output_digest;
    if scale_policy.batch_policy == ProductDiagnosticBatchPolicy::SinglePass {
        let iterations = NonZeroU64::MIN;
        let invocation = invoke(
            worker,
            group,
            scale,
            EvidenceMode::Timing,
            iterations,
            Some(source_expected),
            deadline,
        )?;
        let measured = invocation.measured_duration()?.as_secs_f64();
        return Ok((
            iterations,
            measured,
            vec![DiagnosticCalibrationProbe {
                iterations: iterations.get(),
                invocation,
            }],
        ));
    }

    let mut probes = Vec::new();
    let decision = calibrate(super::run::calibration_policy()?, |iterations| {
        let invocation = invoke(
            worker,
            group,
            scale,
            EvidenceMode::Timing,
            iterations,
            Some(source_expected),
            deadline,
        )
        .map_err(|error| error.to_string())?;
        let measured = invocation
            .measured_duration()
            .map_err(|error| error.to_string())?;
        let wall = invocation
            .wall_duration()
            .map_err(|error| error.to_string())?;
        probes.push(DiagnosticCalibrationProbe {
            iterations: iterations.get(),
            invocation,
        });
        Ok(CalibrationProbe { measured, wall })
    })?;
    Ok((decision.iterations, decision.measured.as_secs_f64(), probes))
}

fn run_memory(
    worker: &PreparedDiagnosticWorker,
    group: &GroupContract,
    scale: &ScaleContract,
    max_peak_rss_bytes: u64,
    expected_output_digest: &SemanticDigest,
    deadline: &SuiteDeadline,
) -> Result<DiagnosticMemoryEvidence, DiagnosticError> {
    let invocation = invoke(
        worker,
        group,
        scale,
        EvidenceMode::Memory,
        NonZeroU64::MIN,
        Some(expected_output_digest),
        deadline,
    )?;
    memory_evidence_from_invocation(invocation, group, scale, max_peak_rss_bytes)
}

fn memory_evidence_from_invocation(
    invocation: InvocationRecord,
    group: &GroupContract,
    scale: &ScaleContract,
    max_peak_rss_bytes: u64,
) -> Result<DiagnosticMemoryEvidence, DiagnosticError> {
    let row = only_row(&invocation.rows)?;
    let setup_rss_bytes = row.setup_rss_bytes.ok_or(DiagnosticError::MissingMemory)?;
    let peak_rss_bytes = row.peak_rss_bytes.ok_or(DiagnosticError::MissingMemory)?;
    if peak_rss_bytes > max_peak_rss_bytes {
        return Err(DiagnosticError::MemoryLimitExceeded {
            group: group.id.to_string(),
            scale: scale.id.to_string(),
            actual: peak_rss_bytes,
            maximum: max_peak_rss_bytes,
        });
    }
    Ok(DiagnosticMemoryEvidence {
        max_worker_peak_rss_bytes: max_peak_rss_bytes,
        setup_rss_bytes,
        peak_rss_bytes,
        peak_delta_bytes: peak_rss_bytes.saturating_sub(setup_rss_bytes),
        parent_observed_peak_rss_bytes: invocation.parent_observed_peak_rss_bytes,
        invocation,
    })
}

fn invoke(
    worker: &PreparedDiagnosticWorker,
    group: &GroupContract,
    scale: &ScaleContract,
    evidence_mode: EvidenceMode,
    iterations: NonZeroU64,
    expected_output_digest: Option<&SemanticDigest>,
    deadline: &SuiteDeadline,
) -> Result<InvocationRecord, DiagnosticError> {
    let timeout = deadline.invocation_timeout()?;
    Ok(worker.invoke(DiagnosticInvocationRequest {
        group,
        evidence_mode,
        iterations,
        scale,
        expected_output_digest,
        timeout,
    })?)
}

fn selected_scales<'a>(
    group: &'a GroupContract,
    args: &DiagnosticArgs,
) -> Result<Vec<&'a ScaleContract>, DiagnosticError> {
    if args.all_scales {
        return Ok(group.scales.iter().collect());
    }
    Ok(vec![group.scale(args.scale.as_deref().unwrap_or("small"))?])
}

fn require_product_diagnostic(group: &GroupContract) -> Result<(), DiagnosticError> {
    if group.claim_class == ClaimClass::ProductDiagnostic
        && group.parity_eligibility == super::group::ParityEligibility::ReportOnly
        && group.profiler_note.is_none()
        && group.comparator_sources.is_empty()
        && !group.correctness_case_ids.is_empty()
    {
        Ok(())
    } else {
        Err(DiagnosticError::Scope(group.id.to_string()))
    }
}

fn validate_report(
    report: &DiagnosticReport,
    group: &GroupContract,
    diagnostic_policy: &ProductDiagnosticPolicy,
    suite_timeout_seconds: u64,
) -> Result<(), DiagnosticError> {
    if report.schema_version != DIAGNOSTIC_REPORT_SCHEMA_VERSION
        || report.scope != DiagnosticScope::StabOnlyProduct
        || report.timing_boundary != RAW_WORK_TIMING_BOUNDARY
        || report.claim_class != ClaimClass::ProductDiagnostic
        || report.group_id != group.id.to_string()
        || report.correctness_case_ids != group.correctness_case_ids
        || report.scales.is_empty()
        || report.scales.len() != report.command.scale_ids.len()
        || report.command.scale_ids
            != report
                .scales
                .iter()
                .map(|scale| scale.scale_id.clone())
                .collect::<Vec<_>>()
        || report.command.suite_timeout_seconds != suite_timeout_seconds
    {
        return Err(DiagnosticError::Report);
    }
    for scale in &report.scales {
        let contract_scale = group.scale(&scale.scale_id)?;
        let scale_policy = diagnostic_policy.scale(&contract_scale.id)?;
        let expected_output_digest = &scale_policy.expected_output_digest;
        let expected_memory_limit = scale_policy.max_worker_peak_rss_bytes;
        let memory_matches = match (&scale.memory, expected_memory_limit) {
            (None, None) => true,
            (Some(memory), Some(maximum)) => {
                memory.max_worker_peak_rss_bytes == maximum.get()
                    && memory.setup_rss_bytes <= memory.peak_rss_bytes
                    && memory.peak_delta_bytes
                        == memory.peak_rss_bytes.saturating_sub(memory.setup_rss_bytes)
                    && memory.peak_rss_bytes <= memory.max_worker_peak_rss_bytes
                    && memory.parent_observed_peak_rss_bytes
                        == memory.invocation.parent_observed_peak_rss_bytes
                    && invocation_is_stab(
                        &memory.invocation,
                        group,
                        contract_scale,
                        EvidenceMode::Memory,
                        1,
                        Some(expected_output_digest),
                    )
            }
            _ => false,
        };
        if scale.calibration.probes.is_empty()
            || scale.scale_id != contract_scale.id.to_string()
            || scale.family_id != contract_scale.family_id.to_string()
            || scale.size_class != contract_scale.size_class
            || scale.work_items != contract_scale.work_items.get()
            || scale.input_bytes != contract_scale.input_bytes
            || scale.input_digest != contract_scale.input_digest.as_str()
            || scale.calibration.selected_iterations == 0
            || !scale.calibration.selected_measured_seconds.is_finite()
            || scale.calibration.selected_measured_seconds <= 0.0
            || scale.warmups.len() != WARMUP_BATCHES
            || scale.samples.len() != report.command.retained_samples
            || scale.batch_policy != scale_policy.batch_policy
            || scale.witness_case_id != scale_policy.witness_case_id
            || scale.expected_output_digest != expected_output_digest.as_str()
            || !memory_matches
            || !summaries_match(&scale.summary, &summarize_samples(&scale.samples)?)
            || !invocation_is_stab(
                &scale.semantic_validation,
                group,
                contract_scale,
                EvidenceMode::Contract,
                1,
                Some(expected_output_digest),
            )
            || !scale
                .warmups
                .iter()
                .chain(&scale.samples)
                .all(|invocation| {
                    invocation_is_stab(
                        invocation,
                        group,
                        contract_scale,
                        EvidenceMode::Timing,
                        scale.calibration.selected_iterations,
                        Some(expected_output_digest),
                    )
                })
            || !scale.calibration.probes.iter().all(|probe| {
                probe.iterations > 0
                    && invocation_is_stab(
                        &probe.invocation,
                        group,
                        contract_scale,
                        EvidenceMode::Timing,
                        probe.iterations,
                        Some(expected_output_digest),
                    )
            })
        {
            return Err(DiagnosticError::Report);
        }
    }
    Ok(())
}

struct SuiteDeadline {
    ends_at: Instant,
}

impl SuiteDeadline {
    fn start(timeout: Duration) -> Result<Self, DiagnosticError> {
        let ends_at = Instant::now()
            .checked_add(timeout)
            .ok_or(DiagnosticError::SuiteDeadlineOverflow)?;
        Ok(Self { ends_at })
    }

    fn invocation_timeout(&self) -> Result<Duration, DiagnosticError> {
        self.invocation_timeout_at(Instant::now())
    }

    fn invocation_timeout_at(&self, now: Instant) -> Result<Duration, DiagnosticError> {
        let remaining = self
            .ends_at
            .checked_duration_since(now)
            .filter(|duration| !duration.is_zero())
            .ok_or(DiagnosticError::SuiteTimeout)?;
        Ok(remaining.min(INVOCATION_TIMEOUT))
    }

    fn require_remaining(&self) -> Result<(), DiagnosticError> {
        self.invocation_timeout_at(Instant::now()).map(|_| ())
    }
}

fn invocation_is_stab(
    invocation: &InvocationRecord,
    group: &GroupContract,
    scale: &ScaleContract,
    evidence_mode: EvidenceMode,
    expected_iterations: u64,
    expected_output_digest: Option<&SemanticDigest>,
) -> bool {
    invocation.implementation == Implementation::Stab
        && invocation.evidence_mode == evidence_mode
        && only_row(&invocation.rows).is_ok_and(|row| {
            row.implementation == Implementation::Stab
                && row.evidence_mode == evidence_mode
                && row.timing_boundary == RAW_WORK_TIMING_BOUNDARY
                && row.workload_id == group.workload_id
                && group.measurement_ids.contains(&row.measurement_id)
                && row.iteration_count == expected_iterations
                && expected_iterations
                    .checked_mul(scale.work_items.get())
                    .is_some_and(|work_count| row.work_count == work_count)
                && row.input_bytes == scale.input_bytes
                && row.input_digest == scale.input_digest
                && expected_output_digest.is_none_or(|expected| row.output_digest == *expected)
        })
}

fn only_row(rows: &[WorkerMeasurement]) -> Result<&WorkerMeasurement, DiagnosticError> {
    let [row] = rows else {
        return Err(DiagnosticError::MeasurementCount(rows.len()));
    };
    Ok(row)
}

fn summarize_samples(
    samples: &[InvocationRecord],
) -> Result<DiagnosticScaleSummary, DiagnosticError> {
    if samples.is_empty() {
        return Err(DiagnosticError::Report);
    }
    let mut batch_seconds = Vec::with_capacity(samples.len());
    let mut seconds_per_work_item = Vec::with_capacity(samples.len());
    for sample in samples {
        let row = only_row(&sample.rows)?;
        if row.work_count == 0 || !row.elapsed_seconds.is_finite() || row.elapsed_seconds <= 0.0 {
            return Err(DiagnosticError::Report);
        }
        batch_seconds.push(row.elapsed_seconds);
        seconds_per_work_item.push(row.elapsed_seconds / row.work_count as f64);
    }
    Ok(DiagnosticScaleSummary {
        median_batch_seconds: median_in_place(&mut batch_seconds)
            .map_err(|_| DiagnosticError::Report)?,
        median_seconds_per_work_item: median_in_place(&mut seconds_per_work_item)
            .map_err(|_| DiagnosticError::Report)?,
    })
}

fn summaries_match(left: &DiagnosticScaleSummary, right: &DiagnosticScaleSummary) -> bool {
    left.median_batch_seconds.to_bits() == right.median_batch_seconds.to_bits()
        && left.median_seconds_per_work_item.to_bits()
            == right.median_seconds_per_work_item.to_bits()
}

fn render_json(value: &impl Serialize) -> Result<Vec<u8>, DiagnosticError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn render_markdown(report: &DiagnosticReport, report_sha256: &str) -> String {
    let host_violations = if report.host.violations.is_empty() {
        "none".to_string()
    } else {
        report.host.violations.join("; ")
    };
    let mut output = format!(
        "# Stab Product Diagnostic\n\n- Scope: `stab-only-product`\n- Group: `{}`\n- Timing boundary: `raw-work-v2`\n- Report SHA-256: `{report_sha256}`\n- Host profile: `{}`\n- Host verified: `{}`\n- Unverified host explicitly allowed: `{}`\n- Host-policy violations: `{host_violations}`\n- Stim parity: not applicable\n- Stab self-regression: not evaluated\n\n## Per-Scale Measurements\n\n| Scale | Iterations | Retained samples | Median batch seconds | Median ns/work-item | Raw batch seconds |\n|---|---:|---:|---:|---:|---|\n",
        report.group_id,
        report.host.profile_id,
        report.host.verified,
        report.command.allow_unverified_host,
    );
    for scale in &report.scales {
        let seconds = scale
            .samples
            .iter()
            .filter_map(|sample| sample.measured_duration().ok())
            .map(|duration| format!("{:.9}", duration.as_secs_f64()))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "| `{}` | {} | {} | {:.9} | {:.3} | `{}` |\n",
            scale.scale_id,
            scale.calibration.selected_iterations,
            scale.samples.len(),
            scale.summary.median_batch_seconds,
            scale.summary.median_seconds_per_work_item * 1e9,
            seconds
        ));
    }
    let memory = report
        .scales
        .iter()
        .filter_map(|scale| scale.memory.as_ref().map(|memory| (scale, memory)))
        .collect::<Vec<_>>();
    if !memory.is_empty() {
        output.push_str(
            "\n## Accepted-Maximum Memory\n\n| Scale | Setup RSS bytes | Peak RSS bytes | Peak delta bytes | Cap bytes | Verdict |\n|---|---:|---:|---:|---:|---|\n",
        );
        for (scale, memory) in memory {
            output.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | `{}` |\n",
                scale.scale_id,
                memory.setup_rss_bytes,
                memory.peak_rss_bytes,
                memory.peak_delta_bytes,
                memory.max_worker_peak_rss_bytes,
                "pass",
            ));
        }
    }
    output
}

#[derive(Debug, Error)]
pub(super) enum DiagnosticError {
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),
    #[error(transparent)]
    Host(#[from] super::host::HostError),
    #[error(transparent)]
    Invocation(#[from] InvocationError),
    #[error(transparent)]
    Calibration(#[from] super::calibration::CalibrationError),
    #[error(transparent)]
    Group(#[from] super::group::GroupError),
    #[error(transparent)]
    Toolchain(#[from] super::toolchain::ToolchainError),
    #[error(transparent)]
    Run(#[from] super::run::RunError),
    #[error("runtime group {0} is not a Stab-only product diagnostic")]
    Scope(String),
    #[error("Stab-only diagnostic scale lacks its source-owned semantic witness")]
    MissingSourceOwnedWitness,
    #[error("Stab-only diagnostic memory receipt lacks worker RSS fields")]
    MissingMemory,
    #[error(
        "Stab-only diagnostic {group}/{scale} reached {actual} peak RSS bytes, exceeding its source-owned {maximum}-byte cap"
    )]
    MemoryLimitExceeded {
        group: String,
        scale: String,
        actual: u64,
        maximum: u64,
    },
    #[error("Stab-only diagnostic report has an invalid claim or receipt shape")]
    Report,
    #[error("Stab-only diagnostic invocation produced {0} measurements instead of one")]
    MeasurementCount(usize),
    #[error("repository commit changed during the diagnostic run: {before} -> {after}")]
    RepositoryChanged { before: String, after: String },
    #[error("Stab-only diagnostic suite exceeded its source-owned timeout")]
    SuiteTimeout,
    #[error("Stab-only diagnostic suite deadline cannot be represented")]
    SuiteDeadlineOverflow,
    #[error("failed to serialize Stab-only diagnostic evidence: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::super::group::ParityEligibility;
    use super::super::protocol::{InputDigest, ProtocolId};
    use super::*;
    use crate::qualification::model::TimingBatchPolicy;

    #[derive(clap::Parser)]
    struct TestCli {
        #[command(flatten)]
        args: DiagnosticArgs,
    }

    #[test]
    fn a2_agent_diagnostics_cli_selects_one_scale_or_all_scales() {
        let one = TestCli::try_parse_from([
            "test",
            "--group",
            "PERFQ-A2-CIRCUIT-MODEL-FINGERPRINT",
            "--scale",
            "medium",
        ])
        .expect("one scale");
        assert_eq!(one.args.scale.as_deref(), Some("medium"));
        assert!(!one.args.all_scales);

        let all = TestCli::try_parse_from([
            "test",
            "--group",
            "PERFQ-A2-CIRCUIT-MODEL-FINGERPRINT",
            "--all-scales",
        ])
        .expect("all scales");
        assert!(all.args.all_scales);
        assert!(all.args.scale.is_none());
    }

    #[test]
    fn a2_agent_diagnostics_cli_rejects_conflicting_scale_selection() {
        let result = TestCli::try_parse_from([
            "test",
            "--group",
            "PERFQ-A2-CIRCUIT-MODEL-FINGERPRINT",
            "--scale",
            "small",
            "--all-scales",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn a2_agent_diagnostics_summary_normalizes_each_batch_by_its_exact_work() {
        let samples = [0.4, 0.2, 0.3]
            .into_iter()
            .map(|elapsed_seconds| invocation(elapsed_seconds, 200))
            .collect::<Vec<_>>();

        let summary = summarize_samples(&samples).expect("summary");

        assert_eq!(summary.median_batch_seconds, 0.3);
        assert_eq!(summary.median_seconds_per_work_item, 0.0015);
    }

    #[test]
    fn a2_agent_diagnostic_deadline_caps_each_child_by_remaining_suite_time() {
        let now = Instant::now();
        let deadline = SuiteDeadline {
            ends_at: now + Duration::from_secs(600),
        };
        assert_eq!(
            deadline
                .invocation_timeout_at(now)
                .expect("full child timeout"),
            INVOCATION_TIMEOUT
        );
        assert_eq!(
            deadline
                .invocation_timeout_at(now + Duration::from_secs(595))
                .expect("remaining child timeout"),
            Duration::from_secs(5)
        );
        assert!(matches!(
            deadline.invocation_timeout_at(now + Duration::from_secs(600)),
            Err(DiagnosticError::SuiteTimeout)
        ));
    }

    #[test]
    fn product_diagnostic_memory_evidence_enforces_worker_peak_cap() {
        let contract = GroupContract {
            id: ProtocolId::try_new("product-diagnostic").expect("group"),
            feature_id: crate::qualification::runtime::protocol::ProtocolId::try_new(
                "PERF-RESOURCE-BOUNDARIES",
            )
            .expect("feature id"),
            origin: crate::qualification::model::RowOrigin::Planned,
            claim_class: ClaimClass::ProductDiagnostic,
            parity_eligibility: ParityEligibility::ReportOnly,
            timing_batch_policy: TimingBatchPolicy::CommonIterations,
            workload_id: ProtocolId::try_new("sampling-request-estimate").expect("workload"),
            measurement_ids: vec![ProtocolId::try_new("estimate").expect("measurement")],
            scales: vec![ScaleContract {
                id: ProtocolId::try_new("large").expect("scale"),
                family_id: ProtocolId::try_new("default").expect("family"),
                size_class: SizeClass::Large,
                work_items: NonZeroU64::new(100).expect("work"),
                input_bytes: 1,
                input_digest: InputDigest::try_new("1".repeat(64)).expect("input digest"),
            }],
            correctness_case_ids: vec!["cq-exact".to_string()],
            public_api_item_ids: Vec::new(),
            checklist_item_ids: Vec::new(),
            checklist_child_ids: Vec::new(),
            owner: ProtocolId::try_new("owner").expect("owner"),
            profiler_note: None,
            comparator_sources: Vec::new(),
        };
        let scale = contract.scales.first().expect("large scale");
        let memory_invocation = || {
            let mut invocation = invocation(0.1, 100);
            invocation.evidence_mode = EvidenceMode::Memory;
            invocation.parent_observed_peak_rss_bytes = Some(24);
            let row = invocation.rows.first_mut().expect("memory row");
            row.evidence_mode = EvidenceMode::Memory;
            row.setup_rss_bytes = Some(10);
            row.peak_rss_bytes = Some(20);
            invocation
        };

        let evidence = memory_evidence_from_invocation(memory_invocation(), &contract, scale, 20)
            .expect("memory at cap");
        assert_eq!(evidence.peak_delta_bytes, 10);
        assert_eq!(evidence.max_worker_peak_rss_bytes, 20);
        assert!(matches!(
            memory_evidence_from_invocation(memory_invocation(), &contract, scale, 19),
            Err(DiagnosticError::MemoryLimitExceeded {
                actual: 20,
                maximum: 19,
                ..
            })
        ));
    }

    fn invocation(elapsed_seconds: f64, work_count: u64) -> InvocationRecord {
        use super::super::protocol::{GitCommit, Sha256Digest};

        InvocationRecord {
            implementation: Implementation::Stab,
            evidence_mode: EvidenceMode::Timing,
            process_wall_seconds: elapsed_seconds,
            parent_observed_peak_rss_bytes: None,
            rows: vec![WorkerMeasurement {
                schema_version: super::super::protocol::PROTOCOL_SCHEMA_VERSION,
                implementation: Implementation::Stab,
                evidence_mode: EvidenceMode::Timing,
                timing_boundary: RAW_WORK_TIMING_BOUNDARY,
                workload_id: ProtocolId::try_new("sampling-request-estimate".to_string())
                    .expect("workload"),
                measurement_id: ProtocolId::try_new("estimate".to_string()).expect("measurement"),
                iteration_count: 2,
                elapsed_seconds,
                work_count,
                input_bytes: 1,
                input_digest: InputDigest::try_new("1".repeat(64)).expect("input digest"),
                output_digest: SemanticDigest::try_new("2".repeat(64)).expect("output digest"),
                setup_rss_bytes: Some(1),
                peak_rss_bytes: Some(1),
                affinity_cpu: Some(0),
                stim_commit: GitCommit::try_new(crate::config::STIM_COMMIT).expect("commit"),
                source_digest: Sha256Digest::try_new("3".repeat(64)).expect("source digest"),
                build_fingerprint: Sha256Digest::try_new("4".repeat(64))
                    .expect("build fingerprint"),
            }],
        }
    }
}
