use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::Duration;

use clap::Args;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::artifact::{DirectQualificationArtifactPath, QualificationOutput, RepositoryBinding};
use super::calibration::{CalibrationProbe, calibrate};
use super::group::{GroupContract, ParityEligibility, ResolvedGroupContract, ScaleContract};
use super::host::{HostEvidence, HostGuard};
use super::invocation::{
    CLIFFORD_NON_IDENTITY_GROUP_ID, DiagnosticInvocationRequest, DiagnosticWorkerIdentityEvidence,
    InvocationError, InvocationRecord, PreparedDiagnosticWorker, SIMD_BITS_XOR_GROUP_ID,
};
use super::protocol::{
    EvidenceMode, RAW_WORK_TIMING_BOUNDARY, SemanticDigest, TimingBoundary, WorkerMeasurement,
};
use super::run::{
    ClaimClass, INVOCATION_TIMEOUT, QualificationTier, RepositoryEvidence, WARMUP_BATCHES,
};
use super::stab_build::{StabBuildReceipt, StabBuildVariant};
use super::statistics::{
    StatisticsError, bootstrap_interval, median, relative_mad, validate_positive_finite,
};
use crate::qualification::model::{SizeClass, TimingBatchPolicy};
use crate::root::RepoRoot;

mod replay;

pub(crate) use replay::SimdReportArgs;

const REPORT_SCHEMA_VERSION: u32 = 1;
const DEFAULT_OUTPUT: &str = "target/benchmarks/qualification/a6-simd-compare-latest";
const SUITE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const GROUP_IDS: [&str; 2] = [SIMD_BITS_XOR_GROUP_ID, CLIFFORD_NON_IDENTITY_GROUP_ID];
const SCALE_IDS: [&str; 2] = ["medium", "large"];

#[derive(Clone, Debug, Args)]
pub(crate) struct SimdCompareArgs {
    /// Diagnostic tier controlling retained paired samples.
    #[arg(long, value_enum, default_value = "full")]
    tier: QualificationTier,

    /// Immutable report directory below target/benchmarks/qualification.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    out: PathBuf,

    /// Preserve diagnostic evidence when controlled-host limits are unavailable.
    #[arg(long)]
    allow_unverified_host: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ComparisonScope {
    ScalarVsPortableSimd,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum VariantPairOrder {
    ScalarThenPortable,
    PortableThenScalar,
}

impl VariantPairOrder {
    const fn for_pair(pair_index: usize) -> Self {
        if pair_index.is_multiple_of(2) {
            Self::ScalarThenPortable
        } else {
            Self::PortableThenScalar
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SimdCompareReport {
    schema_version: u32,
    scope: ComparisonScope,
    timing_boundary: TimingBoundary,
    generated_unix_epoch_seconds: u64,
    performance_inventory_sha256: String,
    correctness_inventory_sha256: String,
    command: CommandEvidence,
    repository: RepositoryEvidence,
    host: HostEvidence,
    toolchain: super::toolchain::ToolchainEvidence,
    scalar_worker: VariantWorkerEvidence,
    portable_worker: VariantWorkerEvidence,
    groups: Vec<GroupEvidence>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CommandEvidence {
    output: String,
    group_ids: Vec<String>,
    scale_ids: Vec<String>,
    tier: QualificationTier,
    allow_unverified_host: bool,
    warmup_pairs: usize,
    retained_pairs: usize,
    invocation_timeout_seconds: u64,
    suite_timeout_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VariantWorkerEvidence {
    variant: StabBuildVariant,
    identity: DiagnosticWorkerIdentityEvidence,
    build_receipt: StabBuildReceipt,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GroupEvidence {
    group_id: String,
    group_contract_sha256: String,
    workload_id: String,
    measurement_id: String,
    owner: String,
    scales: Vec<ScaleEvidence>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScaleEvidence {
    scale_id: String,
    family_id: String,
    size_class: SizeClass,
    work_items: u64,
    input_bytes: u64,
    input_digest: String,
    scalar_calibration: VariantCalibration,
    portable_calibration: VariantCalibration,
    common_iterations: u64,
    semantic_validation: VariantPairExecution,
    warmups: Vec<VariantPairExecution>,
    samples: Vec<VariantPairExecution>,
    paired_samples: Vec<VariantPairedSample>,
    summary: VariantSummary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VariantCalibration {
    variant: StabBuildVariant,
    selected_iterations: u64,
    selected_measured_seconds: f64,
    probes: Vec<VariantCalibrationProbe>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VariantCalibrationProbe {
    iterations: u64,
    invocation: InvocationRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VariantPairExecution {
    pair_index: usize,
    order: VariantPairOrder,
    scalar: InvocationRecord,
    portable: InvocationRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VariantPairedSample {
    pair_index: usize,
    order: VariantPairOrder,
    scalar_elapsed_seconds: f64,
    portable_elapsed_seconds: f64,
    scalar_work_count: u64,
    portable_work_count: u64,
    scalar_work_per_second: f64,
    portable_work_per_second: f64,
    portable_over_scalar_ratio: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VariantSummary {
    pair_count: usize,
    median_portable_over_scalar_ratio: f64,
    confidence_interval_lower: f64,
    confidence_interval_upper: f64,
    scalar_relative_mad: f64,
    portable_relative_mad: f64,
    ratio_relative_mad: f64,
    material_benefit: bool,
}

pub(super) fn run_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    live_repository: &RepositoryBinding,
    performance_inventory_sha256: &str,
    correctness_inventory_sha256: &str,
    args: SimdCompareArgs,
) -> Result<PathBuf, SimdCompareError> {
    let output_path = DirectQualificationArtifactPath::try_new(&args.out)?;
    QualificationOutput::require_absent_with_repository(root, live_repository, &output_path)?;
    let repository_before = super::run::bound_repository_state(root, live_repository)?;
    if repository_before.local_modifications {
        return Err(SimdCompareError::DirtyRepository);
    }
    let groups = load_groups(source_root, performance_inventory_sha256)?;

    live_repository.require_current(root)?;
    let mut host_guard = HostGuard::prepare(source_root, args.allow_unverified_host)?;
    let toolchain = super::toolchain::collect(source_root)?;
    live_repository.require_current(root)?;
    let mut scalar = PreparedDiagnosticWorker::prepare_variant(
        source_root,
        &repository_before.commit,
        &toolchain,
        StabBuildVariant::Scalar,
    )?;
    live_repository.require_current(root)?;
    let mut portable = PreparedDiagnosticWorker::prepare_variant(
        source_root,
        &repository_before.commit,
        &toolchain,
        StabBuildVariant::PortableSimd,
    )?;
    scalar.pin_to_cpu(host_guard.selected_cpu());
    portable.pin_to_cpu(host_guard.selected_cpu());
    let deadline = SuiteDeadline::start(SUITE_TIMEOUT)?;

    let mut group_evidence = Vec::with_capacity(groups.len());
    for group in &groups {
        group_evidence.push(run_group(&scalar, &portable, group, args.tier, &deadline)?);
    }
    deadline.require_remaining()?;
    scalar.verify()?;
    portable.verify()?;
    let scalar_evidence = worker_evidence(&scalar, StabBuildVariant::Scalar)?;
    let portable_evidence = worker_evidence(&portable, StabBuildVariant::PortableSimd)?;
    let host = host_guard.finish()?;
    let repository_after = super::run::bound_repository_state(root, live_repository)?;
    if repository_before.commit != repository_after.commit || repository_after.local_modifications {
        return Err(SimdCompareError::RepositoryChanged {
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
    let report = SimdCompareReport {
        schema_version: REPORT_SCHEMA_VERSION,
        scope: ComparisonScope::ScalarVsPortableSimd,
        timing_boundary: RAW_WORK_TIMING_BOUNDARY,
        generated_unix_epoch_seconds: super::run::unix_epoch_seconds()?,
        performance_inventory_sha256: performance_inventory_sha256.to_string(),
        correctness_inventory_sha256: correctness_inventory_sha256.to_string(),
        command: CommandEvidence {
            output: output_path.as_path().to_string_lossy().into_owned(),
            group_ids: GROUP_IDS.iter().map(ToString::to_string).collect(),
            scale_ids: SCALE_IDS.iter().map(ToString::to_string).collect(),
            tier: args.tier,
            allow_unverified_host: args.allow_unverified_host,
            warmup_pairs: WARMUP_BATCHES,
            retained_pairs: args.tier.sample_count(),
            invocation_timeout_seconds: INVOCATION_TIMEOUT.as_secs(),
            suite_timeout_seconds: SUITE_TIMEOUT.as_secs(),
        },
        repository,
        host,
        toolchain,
        scalar_worker: scalar_evidence,
        portable_worker: portable_evidence,
        groups: group_evidence,
    };
    replay::validate_report(
        root,
        source_root,
        live_repository,
        performance_inventory_sha256,
        correctness_inventory_sha256,
        &groups,
        &report,
        Some(&output_path),
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

pub(super) fn run_report_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    live_repository: &RepositoryBinding,
    performance_inventory_sha256: &str,
    correctness_inventory_sha256: &str,
    args: SimdReportArgs,
) -> Result<PathBuf, SimdCompareError> {
    replay::run_with_repository(
        root,
        source_root,
        live_repository,
        performance_inventory_sha256,
        correctness_inventory_sha256,
        args,
    )
}

fn worker_evidence(
    worker: &PreparedDiagnosticWorker,
    variant: StabBuildVariant,
) -> Result<VariantWorkerEvidence, SimdCompareError> {
    let build_receipt = worker.build_receipt().clone();
    if build_receipt.variant() != variant {
        return Err(SimdCompareError::WorkerVariant {
            expected: variant,
            actual: build_receipt.variant(),
        });
    }
    Ok(VariantWorkerEvidence {
        variant,
        identity: worker.identity_evidence(),
        build_receipt,
    })
}

fn load_groups(
    root: &RepoRoot,
    inventory_digest: &str,
) -> Result<Vec<ResolvedGroupContract>, SimdCompareError> {
    GROUP_IDS
        .iter()
        .map(|group_id| {
            let group = super::group::load_group(root, inventory_digest, group_id)?;
            validate_group(&group.contract)?;
            Ok(group)
        })
        .collect()
}

fn validate_group(group: &GroupContract) -> Result<(), SimdCompareError> {
    let expected = GROUP_IDS
        .iter()
        .any(|candidate| *candidate == group.id.to_string());
    let scales_match = SCALE_IDS.iter().all(|scale_id| {
        group
            .scales
            .iter()
            .any(|scale| scale.id.to_string() == *scale_id)
    });
    if expected
        && scales_match
        && group.claim_class == ClaimClass::PromotablePerformance
        && group.parity_eligibility == ParityEligibility::ThresholdEligible
        && group.timing_batch_policy == TimingBatchPolicy::CommonIterations
        && group.measurement_ids.len() == 1
    {
        Ok(())
    } else {
        Err(SimdCompareError::GroupContract(group.id.to_string()))
    }
}

fn run_group(
    scalar: &PreparedDiagnosticWorker,
    portable: &PreparedDiagnosticWorker,
    resolved: &ResolvedGroupContract,
    tier: QualificationTier,
    deadline: &SuiteDeadline,
) -> Result<GroupEvidence, SimdCompareError> {
    let group = &resolved.contract;
    let mut scales = Vec::with_capacity(SCALE_IDS.len());
    for scale_id in SCALE_IDS {
        scales.push(run_scale(
            scalar,
            portable,
            group,
            group.scale(scale_id)?,
            tier,
            deadline,
        )?);
    }
    Ok(GroupEvidence {
        group_id: group.id.to_string(),
        group_contract_sha256: resolved.source_sha256.clone(),
        workload_id: group.workload_id.to_string(),
        measurement_id: group.single_measurement()?.to_string(),
        owner: group.owner.to_string(),
        scales,
    })
}

fn run_scale(
    scalar: &PreparedDiagnosticWorker,
    portable: &PreparedDiagnosticWorker,
    group: &GroupContract,
    scale: &ScaleContract,
    tier: QualificationTier,
    deadline: &SuiteDeadline,
) -> Result<ScaleEvidence, SimdCompareError> {
    let scalar_calibration =
        calibrate_variant(scalar, StabBuildVariant::Scalar, group, scale, deadline)?;
    let portable_calibration = calibrate_variant(
        portable,
        StabBuildVariant::PortableSimd,
        group,
        scale,
        deadline,
    )?;
    let common_iterations = NonZeroU64::new(
        scalar_calibration
            .selected_iterations
            .max(portable_calibration.selected_iterations),
    )
    .ok_or(SimdCompareError::InvalidIterations)?;
    let pair_context = VariantPairContext {
        scalar,
        portable,
        group,
        scale,
        deadline,
        iterations: common_iterations,
    };
    let semantic_validation = execute_pair(&pair_context, 0, None)?;
    let expected_output = exact_pair_output(&semantic_validation)?;

    let mut warmups = Vec::with_capacity(WARMUP_BATCHES);
    for pair_index in 0..WARMUP_BATCHES {
        warmups.push(execute_pair(
            &pair_context,
            pair_index,
            Some(&expected_output),
        )?);
    }
    let mut samples = Vec::with_capacity(tier.sample_count());
    let mut paired_samples = Vec::with_capacity(tier.sample_count());
    for pair_index in 0..tier.sample_count() {
        let pair = execute_pair(&pair_context, pair_index, Some(&expected_output))?;
        paired_samples.push(pair_sample(&pair)?);
        samples.push(pair);
    }
    let summary = summarize(&paired_samples)?;
    Ok(ScaleEvidence {
        scale_id: scale.id.to_string(),
        family_id: scale.family_id.to_string(),
        size_class: scale.size_class,
        work_items: scale.work_items.get(),
        input_bytes: scale.input_bytes,
        input_digest: scale.input_digest.as_str().to_string(),
        scalar_calibration,
        portable_calibration,
        common_iterations: common_iterations.get(),
        semantic_validation,
        warmups,
        samples,
        paired_samples,
        summary,
    })
}

fn calibrate_variant(
    worker: &PreparedDiagnosticWorker,
    variant: StabBuildVariant,
    group: &GroupContract,
    scale: &ScaleContract,
    deadline: &SuiteDeadline,
) -> Result<VariantCalibration, SimdCompareError> {
    let mut probes = Vec::new();
    let decision = calibrate(super::run::calibration_policy()?, |iterations| {
        let invocation = invoke(worker, group, scale, iterations, None, deadline)
            .map_err(|error| error.to_string())?;
        let measured = invocation
            .measured_duration()
            .map_err(|error| error.to_string())?;
        let wall = invocation
            .wall_duration()
            .map_err(|error| error.to_string())?;
        probes.push(VariantCalibrationProbe {
            iterations: iterations.get(),
            invocation,
        });
        Ok(CalibrationProbe { measured, wall })
    })?;
    Ok(VariantCalibration {
        variant,
        selected_iterations: decision.iterations.get(),
        selected_measured_seconds: decision.measured.as_secs_f64(),
        probes,
    })
}

struct VariantPairContext<'a> {
    scalar: &'a PreparedDiagnosticWorker,
    portable: &'a PreparedDiagnosticWorker,
    group: &'a GroupContract,
    scale: &'a ScaleContract,
    deadline: &'a SuiteDeadline,
    iterations: NonZeroU64,
}

fn execute_pair(
    context: &VariantPairContext<'_>,
    pair_index: usize,
    expected_output: Option<&SemanticDigest>,
) -> Result<VariantPairExecution, SimdCompareError> {
    let order = VariantPairOrder::for_pair(pair_index);
    let (scalar_record, portable_record) = match order {
        VariantPairOrder::ScalarThenPortable => {
            let scalar_record = invoke(
                context.scalar,
                context.group,
                context.scale,
                context.iterations,
                expected_output,
                context.deadline,
            )?;
            let output = expected_output
                .cloned()
                .unwrap_or(exact_output(&scalar_record)?);
            let portable_record = invoke(
                context.portable,
                context.group,
                context.scale,
                context.iterations,
                Some(&output),
                context.deadline,
            )?;
            (scalar_record, portable_record)
        }
        VariantPairOrder::PortableThenScalar => {
            let portable_record = invoke(
                context.portable,
                context.group,
                context.scale,
                context.iterations,
                expected_output,
                context.deadline,
            )?;
            let output = expected_output
                .cloned()
                .unwrap_or(exact_output(&portable_record)?);
            let scalar_record = invoke(
                context.scalar,
                context.group,
                context.scale,
                context.iterations,
                Some(&output),
                context.deadline,
            )?;
            (scalar_record, portable_record)
        }
    };
    let pair = VariantPairExecution {
        pair_index,
        order,
        scalar: scalar_record,
        portable: portable_record,
    };
    exact_pair_output(&pair)?;
    Ok(pair)
}

fn invoke(
    worker: &PreparedDiagnosticWorker,
    group: &GroupContract,
    scale: &ScaleContract,
    iterations: NonZeroU64,
    expected_output_digest: Option<&SemanticDigest>,
    deadline: &SuiteDeadline,
) -> Result<InvocationRecord, SimdCompareError> {
    Ok(
        worker.invoke_variant_comparison(DiagnosticInvocationRequest {
            group,
            evidence_mode: EvidenceMode::Timing,
            iterations,
            scale,
            expected_output_digest,
            timeout: deadline.invocation_timeout()?,
        })?,
    )
}

fn exact_output(invocation: &InvocationRecord) -> Result<SemanticDigest, SimdCompareError> {
    Ok(only_row(&invocation.rows)?.output_digest.clone())
}

fn exact_pair_output(pair: &VariantPairExecution) -> Result<SemanticDigest, SimdCompareError> {
    let scalar = only_row(&pair.scalar.rows)?;
    let portable = only_row(&pair.portable.rows)?;
    if scalar.iteration_count != portable.iteration_count
        || scalar.work_count != portable.work_count
        || scalar.input_digest != portable.input_digest
        || scalar.output_digest != portable.output_digest
    {
        return Err(SimdCompareError::SemanticMismatch);
    }
    Ok(scalar.output_digest.clone())
}

fn pair_sample(pair: &VariantPairExecution) -> Result<VariantPairedSample, SimdCompareError> {
    let scalar = only_row(&pair.scalar.rows)?;
    let portable = only_row(&pair.portable.rows)?;
    exact_pair_output(pair)?;
    if scalar.work_count == 0 || portable.work_count == 0 {
        return Err(SimdCompareError::SemanticMismatch);
    }
    let scalar_work_per_second = scalar.work_count as f64 / scalar.elapsed_seconds;
    let portable_work_per_second = portable.work_count as f64 / portable.elapsed_seconds;
    let portable_over_scalar_ratio = (portable.elapsed_seconds / portable.work_count as f64)
        / (scalar.elapsed_seconds / scalar.work_count as f64);
    validate_positive_finite(&[
        scalar_work_per_second,
        portable_work_per_second,
        portable_over_scalar_ratio,
    ])?;
    Ok(VariantPairedSample {
        pair_index: pair.pair_index,
        order: pair.order,
        scalar_elapsed_seconds: scalar.elapsed_seconds,
        portable_elapsed_seconds: portable.elapsed_seconds,
        scalar_work_count: scalar.work_count,
        portable_work_count: portable.work_count,
        scalar_work_per_second,
        portable_work_per_second,
        portable_over_scalar_ratio,
    })
}

fn summarize(samples: &[VariantPairedSample]) -> Result<VariantSummary, SimdCompareError> {
    if samples.is_empty() {
        return Err(SimdCompareError::InvalidReport);
    }
    let scalar = samples
        .iter()
        .map(|sample| sample.scalar_elapsed_seconds / sample.scalar_work_count as f64)
        .collect::<Vec<_>>();
    let portable = samples
        .iter()
        .map(|sample| sample.portable_elapsed_seconds / sample.portable_work_count as f64)
        .collect::<Vec<_>>();
    let ratios = samples
        .iter()
        .map(|sample| sample.portable_over_scalar_ratio)
        .collect::<Vec<_>>();
    validate_positive_finite(&scalar)?;
    validate_positive_finite(&portable)?;
    validate_positive_finite(&ratios)?;
    let median_ratio = median(&ratios)?;
    let (confidence_interval_lower, confidence_interval_upper) = bootstrap_interval(&ratios)?;
    Ok(VariantSummary {
        pair_count: samples.len(),
        median_portable_over_scalar_ratio: median_ratio,
        confidence_interval_lower,
        confidence_interval_upper,
        scalar_relative_mad: relative_mad(&scalar)?,
        portable_relative_mad: relative_mad(&portable)?,
        ratio_relative_mad: relative_mad(&ratios)?,
        material_benefit: confidence_interval_upper < 1.0,
    })
}

fn only_row(rows: &[WorkerMeasurement]) -> Result<&WorkerMeasurement, SimdCompareError> {
    let [row] = rows else {
        return Err(SimdCompareError::MeasurementCount(rows.len()));
    };
    Ok(row)
}

fn render_json(value: &impl Serialize) -> Result<Vec<u8>, SimdCompareError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn render_markdown(report: &SimdCompareReport, report_sha256: &str) -> String {
    let mut output = format!(
        "# Scalar Versus Portable SIMD Diagnostic\n\n- Scope: `scalar-vs-portable-simd`\n- Timing boundary: `raw-work-v2`\n- Report SHA-256: `{report_sha256}`\n- Ratio direction: portable SIMD seconds per work item divided by scalar seconds per work item\n- Stim parity: not evaluated\n- Stab self-regression: not evaluated\n- Material benefit: confidence-interval upper bound below `1.0`\n\n## Measurements\n\n| Group | Scale | Pairs | Median portable/scalar | 95% interval | Ratio relative MAD | Material benefit |\n|---|---|---:|---:|---:|---:|---|\n"
    );
    for group in &report.groups {
        for scale in &group.scales {
            output.push_str(&format!(
                "| `{}` | `{}` | {} | {:.6}x | {:.6}x–{:.6}x | {:.6} | {} |\n",
                group.group_id,
                scale.scale_id,
                scale.summary.pair_count,
                scale.summary.median_portable_over_scalar_ratio,
                scale.summary.confidence_interval_lower,
                scale.summary.confidence_interval_upper,
                scale.summary.ratio_relative_mad,
                if scale.summary.material_benefit {
                    "yes"
                } else {
                    "no"
                }
            ));
        }
    }
    output
}

struct SuiteDeadline {
    ends_at: std::time::Instant,
}

impl SuiteDeadline {
    fn start(timeout: Duration) -> Result<Self, SimdCompareError> {
        let ends_at = std::time::Instant::now()
            .checked_add(timeout)
            .ok_or(SimdCompareError::SuiteDeadlineOverflow)?;
        Ok(Self { ends_at })
    }

    fn invocation_timeout(&self) -> Result<Duration, SimdCompareError> {
        let remaining = self
            .ends_at
            .checked_duration_since(std::time::Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or(SimdCompareError::SuiteTimeout)?;
        Ok(remaining.min(INVOCATION_TIMEOUT))
    }

    fn require_remaining(&self) -> Result<(), SimdCompareError> {
        self.invocation_timeout().map(|_| ())
    }
}

#[derive(Debug, Error)]
pub(super) enum SimdCompareError {
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),
    #[error(transparent)]
    Calibration(#[from] super::calibration::CalibrationError),
    #[error(transparent)]
    Group(#[from] super::group::GroupError),
    #[error(transparent)]
    Host(#[from] super::host::HostError),
    #[error(transparent)]
    Invocation(#[from] InvocationError),
    #[error(transparent)]
    Protocol(#[from] super::protocol::ProtocolError),
    #[error(transparent)]
    Run(#[from] super::run::RunError),
    #[error(transparent)]
    Statistics(#[from] StatisticsError),
    #[error(transparent)]
    Toolchain(#[from] super::toolchain::ToolchainError),
    #[error("scalar-versus-SIMD evidence requires a clean repository")]
    DirtyRepository,
    #[error("runtime group {0} is not an eligible A6 SIMD comparison contract")]
    GroupContract(String),
    #[error("scalar and portable workers produced different semantic outputs")]
    SemanticMismatch,
    #[error("SIMD comparison invocation produced {0} measurements instead of one")]
    MeasurementCount(usize),
    #[error("SIMD comparison selected an invalid iteration count")]
    InvalidIterations,
    #[error("SIMD comparison report failed internal validation")]
    InvalidReport,
    #[error("SIMD comparison report inventory identities are stale")]
    InventoryEvidence,
    #[error("SIMD comparison report repository identity is stale")]
    RepositoryEvidence,
    #[error("SIMD comparison worker build evidence is stale or inconsistent")]
    WorkerEvidence,
    #[error("SIMD comparison group or scale evidence differs from its runtime contract")]
    GroupEvidence,
    #[error("SIMD comparison calibration evidence does not replay")]
    CalibrationEvidence,
    #[error("SIMD comparison raw invocation evidence is invalid")]
    RawInvocation,
    #[error("SIMD comparison paired samples do not reproduce from raw invocation rows")]
    DerivedSamples,
    #[error("SIMD comparison summary does not reproduce from raw paired samples")]
    SummaryEvidence,
    #[error("SIMD comparison report JSON must be nonempty and newline terminated")]
    ReportBoundary,
    #[error("SIMD comparison report JSON is not canonical")]
    NonCanonicalReport,
    #[error("SIMD comparison Markdown does not reproduce from report.json")]
    MarkdownBinding,
    #[error("SIMD comparison suite exceeded its 30-minute deadline")]
    SuiteTimeout,
    #[error("SIMD comparison suite deadline cannot be represented")]
    SuiteDeadlineOverflow,
    #[error("repository changed during SIMD comparison: {before} -> {after}")]
    RepositoryChanged { before: String, after: String },
    #[error("expected {expected:?} worker, but receipt identifies {actual:?}")]
    WorkerVariant {
        expected: StabBuildVariant,
        actual: StabBuildVariant,
    },
    #[error("failed to process SIMD comparison JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_order_alternates_without_changing_ratio_direction() {
        let first = sample(0, 2.0, 1.0);
        let second = sample(1, 4.0, 2.0);
        assert_eq!(first.order, VariantPairOrder::ScalarThenPortable);
        assert_eq!(second.order, VariantPairOrder::PortableThenScalar);
        assert_eq!(first.portable_over_scalar_ratio, 0.5);
        assert_eq!(second.portable_over_scalar_ratio, 0.5);
    }

    #[test]
    fn material_benefit_requires_the_complete_interval_below_scalar() {
        let clear = (0..9)
            .map(|index| sample(index, 2.0, 1.0))
            .collect::<Vec<_>>();
        let ambiguous = (0..9)
            .map(|index| {
                if index < 4 {
                    sample(index, 1.0, 2.0)
                } else {
                    sample(index, 2.0, 1.0)
                }
            })
            .collect::<Vec<_>>();
        assert!(summarize(&clear).expect("clear summary").material_benefit);
        assert!(
            !summarize(&ambiguous)
                .expect("ambiguous summary")
                .material_benefit
        );
    }

    fn sample(
        pair_index: usize,
        scalar_elapsed_seconds: f64,
        portable_elapsed_seconds: f64,
    ) -> VariantPairedSample {
        VariantPairedSample {
            pair_index,
            order: VariantPairOrder::for_pair(pair_index),
            scalar_elapsed_seconds,
            portable_elapsed_seconds,
            scalar_work_count: 1,
            portable_work_count: 1,
            scalar_work_per_second: 1.0 / scalar_elapsed_seconds,
            portable_work_per_second: 1.0 / portable_elapsed_seconds,
            portable_over_scalar_ratio: portable_elapsed_seconds / scalar_elapsed_seconds,
        }
    }
}
