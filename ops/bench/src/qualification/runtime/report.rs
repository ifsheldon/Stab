use std::collections::BTreeSet;
use std::path::PathBuf;

use clap::Args;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::calibration::{CalibrationProbe, calibrate};
use super::correctness::CorrectnessPreflightStatus;
use super::invocation::protocol_ids;
use super::protocol::{EvidenceMode, Implementation, ProtocolId, SemanticDigest};
use super::run::{
    ClaimClass, PairExecution, QualificationReport, QualificationTier, REPORT_SCHEMA_VERSION,
    sha256_hex,
};
use super::statistics::{PairOrder, PairedSample, pair_measurements, summarize};
use crate::config::{STIM_COMMIT, STIM_TAG};

const PREFLIGHT_SCHEMA_VERSION: u32 = 2;
const EXPECTED_WARMUPS: usize = 3;
const EXPECTED_THRESHOLD: f64 = 1.25;

#[derive(Clone, Debug, Args)]
pub(crate) struct ReportArgs {
    /// Published qualification directory to validate and refresh.
    #[arg(long, default_value = "target/benchmarks/qualification/latest")]
    input: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PerformancePreflightArtifact {
    schema_version: u32,
    report_sha256: String,
    group_id: String,
    claim_class: ClaimClass,
    tier: QualificationTier,
    performance_inventory_sha256: String,
    correctness_inventory_sha256: String,
    stab_commit: String,
    local_modifications: bool,
    stim_commit: String,
    host_profile_id: String,
    host_verified: bool,
    rust_toolchain: String,
    target_triple: String,
    correctness_status: CorrectnessPreflightStatus,
    correctness_case_ids: Vec<String>,
    semantic_preflight_passed: bool,
    sample_pairs: usize,
    promotable: bool,
}

pub(super) fn validate_report(
    root: &crate::root::RepoRoot,
    report: &QualificationReport,
) -> Result<(), ReportError> {
    if report.schema_version != REPORT_SCHEMA_VERSION {
        return Err(ReportError::SchemaVersion {
            actual: report.schema_version,
            expected: REPORT_SCHEMA_VERSION,
        });
    }
    if report.group_id != "pq1-adapter-protocol-smoke"
        || report.stim_tag != STIM_TAG
        || report.stim_commit != STIM_COMMIT
    {
        return Err(ReportError::Identity);
    }
    if report.command.output.is_empty()
        || !valid_output_path(&report.command.output)
        || report.command.work_items == 0
        || report.command.warmup_batches != EXPECTED_WARMUPS
        || report.command.paired_samples != expected_pair_count(report.tier)
        || report.command.invocation_timeout_seconds != 30
        || !report.host.verified && !report.command.allow_unverified_host
    {
        return Err(ReportError::CommandEvidence);
    }
    validate_sha256(
        "performance_inventory_sha256",
        &report.performance_inventory_sha256,
    )?;
    validate_sha256(
        "correctness_inventory_sha256",
        &report.correctness_inventory_sha256,
    )?;
    validate_sha256("host_policy_sha256", &report.host.policy_sha256)?;
    validate_sha256("stim_source_sha256", &report.workers.stim_source_sha256)?;
    validate_sha256(
        "stim_build_fingerprint",
        &report.workers.stim_build_fingerprint,
    )?;
    validate_sha256("stim_binary_sha256", &report.workers.stim_binary_sha256)?;
    validate_sha256("stab_source_sha256", &report.workers.stab_source_sha256)?;
    validate_sha256(
        "stab_build_fingerprint",
        &report.workers.stab_build_fingerprint,
    )?;
    validate_sha256("stab_binary_sha256", &report.workers.stab_binary_sha256)?;
    if !report.adapter_receipt.validates_report_identity(
        &report.workers.stim_source_sha256,
        &report.workers.stim_build_fingerprint,
        &report.workers.stim_binary_sha256,
    ) {
        return Err(ReportError::AdapterReceipt);
    }
    if !report.stab_build_receipt.validates_report_identity(
        &report.workers.stab_source_sha256,
        &report.workers.stab_build_fingerprint,
        &report.workers.stab_binary_sha256,
        &report.repository.commit_before,
        &report.toolchain,
    ) {
        return Err(ReportError::StabBuildReceipt);
    }
    if report.repository.commit_before != report.repository.commit_after
        || !valid_git_commit(&report.repository.commit_before)
    {
        return Err(ReportError::RepositoryIdentity);
    }
    validate_sha256("rustup_sha256", &report.toolchain.rustup_sha256)?;
    validate_sha256("cargo_sha256", &report.toolchain.cargo_sha256)?;
    validate_sha256("rustc_sha256", &report.toolchain.rustc_sha256)?;
    if report.toolchain.rust_toolchain != "nightly-2026-06-20"
        || report.toolchain.cargo_profile != "release"
        || !std::path::Path::new(&report.toolchain.rustup_path).is_absolute()
        || !std::path::Path::new(&report.toolchain.cargo_path).is_absolute()
        || !std::path::Path::new(&report.toolchain.rustc_path).is_absolute()
        || report.toolchain.cargo_verbose_version.is_empty()
        || report.toolchain.rustc_verbose_version.is_empty()
        || !report
            .toolchain
            .target_triple
            .starts_with(report.host.architecture.as_str())
    {
        return Err(ReportError::ToolchainEvidence);
    }
    if report.host.allowed_cpus.is_empty()
        || report.host.logical_cpu_count != report.host.allowed_cpus.len()
        || report.host.cpu_identity.is_empty()
        || !report.host.allowed_cpus.contains(&report.host.selected_cpu)
        || report.host.verified != report.host.violations.is_empty()
        || report.host.maximum_temperature_millidegrees_celsius <= 0
        || !thermal_readings_valid(&report.host.thermal_readings_before)
        || !thermal_readings_valid(&report.host.thermal_readings_after)
        || report.host.thermal_probe_available
            != (!report.host.thermal_readings_before.is_empty()
                && !report.host.thermal_readings_after.is_empty())
        || thermal_zone_keys(&report.host.thermal_readings_before)
            != thermal_zone_keys(&report.host.thermal_readings_after)
        || report.host.verified
            && report
                .host
                .thermal_readings_before
                .iter()
                .chain(&report.host.thermal_readings_after)
                .any(|reading| {
                    reading.millidegrees_celsius
                        > report.host.maximum_temperature_millidegrees_celsius
                })
        || report.host.verified
            && (report.host.frequency_governor_before.is_none()
                || report.host.frequency_governor_before != report.host.frequency_governor_after)
    {
        return Err(ReportError::HostEvidence);
    }
    report.host.validate_against_policy(root)?;
    validate_all_worker_receipts(report)?;
    validate_correctness_evidence(report)?;
    validate_pair_execution(&report.semantic_preflight, EvidenceMode::Timing)?;
    validate_calibration(report)?;
    if report.warmups.len() != EXPECTED_WARMUPS {
        return Err(ReportError::WarmupCount(report.warmups.len()));
    }
    for (index, warmup) in report.warmups.iter().enumerate() {
        if warmup.pair_index != index {
            return Err(ReportError::PairIndex);
        }
        validate_pair_execution(warmup, EvidenceMode::Timing)?;
    }
    if report.samples.len() != expected_pair_count(report.tier) {
        return Err(ReportError::SampleCount(report.samples.len()));
    }
    let mut reconstructed = Vec::new();
    for (index, sample) in report.samples.iter().enumerate() {
        if sample.pair_index != index {
            return Err(ReportError::PairIndex);
        }
        reconstructed.extend(validate_pair_execution(sample, EvidenceMode::Timing)?);
    }
    if !paired_samples_equivalent(&reconstructed, &report.paired_samples) {
        return Err(ReportError::PairedSamples);
    }
    let measurement_ids = report
        .paired_samples
        .iter()
        .map(|sample| sample.measurement_id.clone())
        .collect::<BTreeSet<_>>();
    if measurement_ids.len() != report.statistics.len() || measurement_ids.is_empty() {
        return Err(ReportError::StatisticsSet);
    }
    let mut reconstructed_statistics = Vec::new();
    for measurement_id in measurement_ids {
        let selected = report
            .paired_samples
            .iter()
            .filter(|sample| sample.measurement_id == measurement_id)
            .cloned()
            .collect::<Vec<PairedSample>>();
        reconstructed_statistics.push(summarize(measurement_id, &selected, EXPECTED_THRESHOLD)?);
    }
    if !statistics_equivalent(&reconstructed_statistics, &report.statistics) {
        return Err(ReportError::StatisticsMismatch {
            expected: format!("{reconstructed_statistics:?}"),
            actual: format!("{:?}", report.statistics),
        });
    }
    let worst = reconstructed_statistics
        .iter()
        .map(|summary| summary.confidence_interval_upper)
        .reduce(f64::max)
        .ok_or(ReportError::StatisticsSet)?;
    if !approximately_equal(worst, report.worst_confidence_interval_upper) {
        return Err(ReportError::WorstUpperBound);
    }
    validate_memory(report)?;
    validate_claim(report)?;
    Ok(())
}

fn paired_samples_equivalent(expected: &[PairedSample], actual: &[PairedSample]) -> bool {
    expected.len() == actual.len()
        && expected.iter().zip(actual).all(|(expected, actual)| {
            expected.pair_index == actual.pair_index
                && expected.order == actual.order
                && expected.measurement_id == actual.measurement_id
                && expected.work_count == actual.work_count
                && approximately_equal(expected.stim_elapsed_seconds, actual.stim_elapsed_seconds)
                && approximately_equal(expected.stab_elapsed_seconds, actual.stab_elapsed_seconds)
                && approximately_equal(expected.stim_work_per_second, actual.stim_work_per_second)
                && approximately_equal(expected.stab_work_per_second, actual.stab_work_per_second)
                && approximately_equal(expected.ratio, actual.ratio)
        })
}

fn approximately_equal(left: f64, right: f64) -> bool {
    left.is_finite()
        && right.is_finite()
        && (left - right).abs() <= f64::EPSILON * 16.0 * left.abs().max(right.abs()).max(1.0)
}

fn statistics_equivalent(
    expected: &[super::statistics::StatisticsSummary],
    actual: &[super::statistics::StatisticsSummary],
) -> bool {
    expected.len() == actual.len()
        && expected.iter().zip(actual).all(|(expected, actual)| {
            expected.measurement_id == actual.measurement_id
                && expected.pair_count == actual.pair_count
                && expected.outcome == actual.outcome
                && approximately_equal(expected.median_ratio, actual.median_ratio)
                && approximately_equal(
                    expected.confidence_interval_lower,
                    actual.confidence_interval_lower,
                )
                && approximately_equal(
                    expected.confidence_interval_upper,
                    actual.confidence_interval_upper,
                )
                && approximately_equal(expected.stim_relative_mad, actual.stim_relative_mad)
                && approximately_equal(expected.stab_relative_mad, actual.stab_relative_mad)
                && approximately_equal(expected.ratio_relative_mad, actual.ratio_relative_mad)
                && approximately_equal(expected.threshold, actual.threshold)
        })
}

fn validate_correctness_evidence(report: &QualificationReport) -> Result<(), ReportError> {
    let evidence = &report.correctness_preflight;
    match evidence.status {
        CorrectnessPreflightStatus::NotApplicable => {
            if evidence.reason.trim().is_empty()
                || !evidence.case_ids.is_empty()
                || evidence.source_directory.is_some()
                || evidence.qualification_manifest_sha256.is_some()
                || evidence.request_sha256.is_some()
                || evidence.completion_sha256.is_some()
                || evidence.report_sha256.is_some()
                || evidence.preflight_sha256.is_some()
            {
                return Err(ReportError::CorrectnessEvidence);
            }
        }
        CorrectnessPreflightStatus::Passed => {
            if evidence.case_ids.is_empty()
                || evidence.reason.trim().is_empty()
                || evidence
                    .source_directory
                    .as_deref()
                    .is_none_or(str::is_empty)
                || evidence.qualification_manifest_sha256.as_deref()
                    != Some(report.correctness_inventory_sha256.as_str())
                || [
                    evidence.request_sha256.as_deref(),
                    evidence.completion_sha256.as_deref(),
                    evidence.report_sha256.as_deref(),
                    evidence.preflight_sha256.as_deref(),
                ]
                .into_iter()
                .any(|value| value.is_none_or(|value| validate_sha256_value(value).is_err()))
            {
                return Err(ReportError::CorrectnessEvidence);
            }
        }
    }
    Ok(())
}

fn validate_all_worker_receipts(report: &QualificationReport) -> Result<(), ReportError> {
    let (workload_id, measurement_id) = protocol_ids()?;
    let identity = ReceiptIdentity::from_report(report)?;
    let preflight_stim = only_row(&report.semantic_preflight.stim.rows)?;
    let preflight_stab = only_row(&report.semantic_preflight.stab.rows)?;
    if preflight_stim.output_digest != preflight_stab.output_digest {
        return Err(ReportError::WorkerReceipt);
    }
    if report.semantic_preflight.pair_index != 0
        || report.calibration.common_validation.pair_index != 0
        || report.memory.execution.pair_index != 0
    {
        return Err(ReportError::PairIndex);
    }
    let expected_output_digest = &preflight_stim.output_digest;
    let common_timing = PhaseExpectation {
        evidence_mode: EvidenceMode::Timing,
        iterations: report.calibration.common_iterations,
        workload_id: &workload_id,
        measurement_id: &measurement_id,
        output_digest: Some(expected_output_digest),
    };
    validate_pair_receipts(&identity, &report.semantic_preflight, &common_timing)?;
    for probe in &report.calibration.stim.probes {
        let phase = PhaseExpectation {
            evidence_mode: EvidenceMode::Timing,
            iterations: probe.iterations,
            workload_id: &workload_id,
            measurement_id: &measurement_id,
            output_digest: None,
        };
        validate_invocation_receipt(&identity, &probe.invocation, Implementation::Stim, &phase)?;
    }
    for probe in &report.calibration.stab.probes {
        let phase = PhaseExpectation {
            evidence_mode: EvidenceMode::Timing,
            iterations: probe.iterations,
            workload_id: &workload_id,
            measurement_id: &measurement_id,
            output_digest: None,
        };
        validate_invocation_receipt(&identity, &probe.invocation, Implementation::Stab, &phase)?;
    }
    validate_pair_receipts(
        &identity,
        &report.calibration.common_validation,
        &common_timing,
    )?;
    for pair in &report.warmups {
        validate_pair_receipts(&identity, pair, &common_timing)?;
    }
    for pair in &report.samples {
        validate_pair_receipts(&identity, pair, &common_timing)?;
    }
    let common_memory = PhaseExpectation {
        evidence_mode: EvidenceMode::Memory,
        ..common_timing
    };
    validate_pair_receipts(&identity, &report.memory.execution, &common_memory)?;
    Ok(())
}

struct ReceiptIdentity<'a> {
    work_items: u64,
    expected_cpu: u32,
    stim_commit: &'a str,
    stim_source: &'a str,
    stim_build: &'a str,
    stab_source: &'a str,
    stab_build: &'a str,
}

impl<'a> ReceiptIdentity<'a> {
    fn from_report(report: &'a QualificationReport) -> Result<Self, ReportError> {
        Ok(Self {
            work_items: report.command.work_items,
            expected_cpu: u32::try_from(report.host.selected_cpu)
                .map_err(|_| ReportError::HostEvidence)?,
            stim_commit: &report.stim_commit,
            stim_source: &report.workers.stim_source_sha256,
            stim_build: &report.workers.stim_build_fingerprint,
            stab_source: &report.workers.stab_source_sha256,
            stab_build: &report.workers.stab_build_fingerprint,
        })
    }

    fn source_and_build(&self, implementation: Implementation) -> (&str, &str) {
        match implementation {
            Implementation::Stim => (self.stim_source, self.stim_build),
            Implementation::Stab => (self.stab_source, self.stab_build),
        }
    }
}

#[derive(Clone, Copy)]
struct PhaseExpectation<'a> {
    evidence_mode: EvidenceMode,
    iterations: u64,
    workload_id: &'a ProtocolId,
    measurement_id: &'a ProtocolId,
    output_digest: Option<&'a SemanticDigest>,
}

fn validate_pair_receipts(
    identity: &ReceiptIdentity<'_>,
    pair: &PairExecution,
    phase: &PhaseExpectation<'_>,
) -> Result<(), ReportError> {
    validate_invocation_receipt(identity, &pair.stim, Implementation::Stim, phase)?;
    validate_invocation_receipt(identity, &pair.stab, Implementation::Stab, phase)
}

fn validate_invocation_receipt(
    identity: &ReceiptIdentity<'_>,
    invocation: &super::invocation::InvocationRecord,
    implementation: Implementation,
    phase: &PhaseExpectation<'_>,
) -> Result<(), ReportError> {
    let [row] = invocation.rows.as_slice() else {
        return Err(ReportError::MeasurementCount(invocation.rows.len()));
    };
    row.validate_values()?;
    let expected_work_count = phase
        .iterations
        .checked_mul(identity.work_items)
        .ok_or(ReportError::WorkOverflow)?;
    let (source, build) = identity.source_and_build(implementation);
    if invocation.implementation != implementation
        || invocation.evidence_mode != phase.evidence_mode
        || !invocation.process_wall_seconds.is_finite()
        || invocation.process_wall_seconds <= 0.0
        || invocation.process_wall_seconds < row.elapsed_seconds
        || row.implementation != implementation
        || row.evidence_mode != phase.evidence_mode
        || row.workload_id != *phase.workload_id
        || row.measurement_id != *phase.measurement_id
        || row.iteration_count != phase.iterations
        || row.affinity_cpu != Some(identity.expected_cpu)
        || row.stim_commit.as_str() != identity.stim_commit
        || row.work_count != expected_work_count
        || phase
            .output_digest
            .is_some_and(|expected| row.output_digest != *expected)
        || row.source_digest.as_str() != source
        || row.build_fingerprint.as_str() != build
    {
        return Err(ReportError::WorkerReceipt);
    }
    Ok(())
}

pub(super) fn run(root: &crate::root::RepoRoot, args: ReportArgs) -> Result<PathBuf, ReportError> {
    let report_json = super::artifact::read_artifact(root, &args.input, "report.json")?;
    if report_json.is_empty() || !report_json.ends_with(b"\n") {
        return Err(ReportError::ReportBoundary);
    }
    let report: QualificationReport =
        serde_json::from_slice(&report_json).map_err(ReportError::Json)?;
    validate_report(root, &report)?;
    if std::path::Path::new(&report.command.output) != args.input {
        return Err(ReportError::OutputBinding);
    }
    let preflight = preflight_artifact(root, &report, &report_json)?;
    let mut preflight_json = serde_json::to_vec_pretty(&preflight).map_err(ReportError::Json)?;
    preflight_json.push(b'\n');
    let markdown = render_markdown(&report, &sha256_hex(&report_json));
    let output = super::artifact::QualificationOutput::begin(root, &args.input)?;
    output.write("report.json", &report_json)?;
    output.write("preflight.json", &preflight_json)?;
    output.write("report.md", markdown.as_bytes())?;
    let relative = output.relative().to_path_buf();
    output.commit()?;
    Ok(relative)
}

fn validate_calibration(report: &QualificationReport) -> Result<(), ReportError> {
    let calibration = &report.calibration;
    if calibration.acceptance_minimum_seconds != 0.25
        || calibration.target_minimum_seconds != 0.35
        || calibration.target_minimum_seconds <= calibration.acceptance_minimum_seconds
        || calibration.maximum_seconds != 2.0
        || calibration.common_iterations == 0
    {
        return Err(ReportError::Calibration);
    }
    for (expected, implementation) in [
        (Implementation::Stim, &calibration.stim),
        (Implementation::Stab, &calibration.stab),
    ] {
        if implementation.implementation != expected
            || implementation.selected_iterations == 0
            || implementation.probes.is_empty()
        {
            return Err(ReportError::Calibration);
        }
        replay_calibration(implementation)?;
    }
    if calibration.common_iterations
        != calibration
            .stim
            .selected_iterations
            .max(calibration.stab.selected_iterations)
    {
        return Err(ReportError::Calibration);
    }
    validate_pair_execution(&calibration.common_validation, EvidenceMode::Timing)?;
    for invocation in [
        &calibration.common_validation.stim,
        &calibration.common_validation.stab,
    ] {
        let measured = invocation.measured_duration()?.as_secs_f64();
        if measured < calibration.acceptance_minimum_seconds
            || measured > calibration.maximum_seconds
        {
            return Err(ReportError::Calibration);
        }
    }
    Ok(())
}

fn replay_calibration(
    implementation: &super::run::ImplementationCalibration,
) -> Result<(), ReportError> {
    let policy = super::run::calibration_policy().map_err(|_| ReportError::Calibration)?;
    let mut probes = implementation.probes.iter();
    let decision = calibrate(policy, |expected_iterations| {
        let probe = probes
            .next()
            .ok_or_else(|| "calibration evidence ended before the decision".to_string())?;
        let [row] = probe.invocation.rows.as_slice() else {
            return Err("calibration invocation must contain exactly one row".to_string());
        };
        if probe.iterations != expected_iterations.get()
            || row.iteration_count != probe.iterations
            || probe.invocation.implementation != implementation.implementation
            || probe.invocation.evidence_mode != EvidenceMode::Timing
        {
            return Err("calibration phase identity or iterations do not replay".to_string());
        }
        let measured = probe
            .invocation
            .measured_duration()
            .map_err(|error| error.to_string())?;
        let wall = probe
            .invocation
            .wall_duration()
            .map_err(|error| error.to_string())?;
        Ok(CalibrationProbe { measured, wall })
    })
    .map_err(|_| ReportError::Calibration)?;
    if probes.next().is_some()
        || decision.probes.len() != implementation.probes.len()
        || decision.iterations.get() != implementation.selected_iterations
        || !approximately_equal(
            decision.measured.as_secs_f64(),
            implementation.selected_measured_seconds,
        )
    {
        return Err(ReportError::Calibration);
    }
    Ok(())
}

fn validate_memory(report: &QualificationReport) -> Result<(), ReportError> {
    let memory = &report.memory;
    if memory.evidence_mode != EvidenceMode::Memory
        || memory.iterations != report.calibration.common_iterations
        || memory.work_count == 0
    {
        return Err(ReportError::Memory);
    }
    validate_pair_shape(&memory.execution, EvidenceMode::Memory)?;
    let stim = only_row(&memory.execution.stim.rows)?;
    let stab = only_row(&memory.execution.stab.rows)?;
    if !memory_receipts_match(memory, stim, stab) {
        return Err(ReportError::Memory);
    }
    Ok(())
}

fn memory_receipts_match(
    memory: &super::run::MemoryEvidence,
    stim: &super::protocol::WorkerMeasurement,
    stab: &super::protocol::WorkerMeasurement,
) -> bool {
    stim.work_count == memory.work_count
        && stim.work_count == stab.work_count
        && stim.output_digest == stab.output_digest
        && stim.setup_rss_bytes == Some(memory.stim_setup_rss_bytes)
        && stim.peak_rss_bytes == Some(memory.stim_peak_rss_bytes)
        && stab.setup_rss_bytes == Some(memory.stab_setup_rss_bytes)
        && stab.peak_rss_bytes == Some(memory.stab_peak_rss_bytes)
        && memory.stim_parent_observed_peak_rss_bytes
            == memory.execution.stim.parent_observed_peak_rss_bytes
        && memory.stab_parent_observed_peak_rss_bytes
            == memory.execution.stab.parent_observed_peak_rss_bytes
        && memory.stim_peak_rss_bytes >= memory.stim_setup_rss_bytes
        && memory.stab_peak_rss_bytes >= memory.stab_setup_rss_bytes
}

fn validate_claim(report: &QualificationReport) -> Result<(), ReportError> {
    match report.claim_class {
        ClaimClass::DiagnosticInfrastructure => {
            if report.promotable
                || report.correctness_preflight.status != CorrectnessPreflightStatus::NotApplicable
                || !report.correctness_preflight.case_ids.is_empty()
            {
                return Err(ReportError::Claim);
            }
        }
        ClaimClass::PromotablePerformance => {
            if !promotable_claim_requirements(
                report.promotable,
                report.tier,
                report.repository.local_modifications_before,
                report.repository.local_modifications_after,
                report.host.verified,
                report.correctness_preflight.status,
                report.correctness_preflight.case_ids.len(),
            ) {
                return Err(ReportError::Claim);
            }
        }
    }
    Ok(())
}

fn promotable_claim_requirements(
    promotable: bool,
    tier: QualificationTier,
    local_modifications_before: bool,
    local_modifications_after: bool,
    host_verified: bool,
    correctness_status: CorrectnessPreflightStatus,
    correctness_case_count: usize,
) -> bool {
    promotable
        && matches!(tier, QualificationTier::Full | QualificationTier::Soak)
        && !local_modifications_before
        && !local_modifications_after
        && host_verified
        && correctness_status == CorrectnessPreflightStatus::Passed
        && correctness_case_count > 0
}

fn validate_pair_execution(
    execution: &PairExecution,
    mode: EvidenceMode,
) -> Result<Vec<PairedSample>, ReportError> {
    validate_pair_shape(execution, mode)?;
    if mode != EvidenceMode::Timing {
        return Err(ReportError::MemoryUsedAsTiming);
    }
    Ok(pair_measurements(
        execution.pair_index,
        execution.order,
        &execution.stim.rows,
        &execution.stab.rows,
    )?)
}

fn validate_pair_shape(execution: &PairExecution, mode: EvidenceMode) -> Result<(), ReportError> {
    if execution.order != PairOrder::for_pair(execution.pair_index)
        || execution.stim.implementation != Implementation::Stim
        || execution.stab.implementation != Implementation::Stab
        || execution.stim.evidence_mode != mode
        || execution.stab.evidence_mode != mode
        || !execution.stim.process_wall_seconds.is_finite()
        || execution.stim.process_wall_seconds <= 0.0
        || !execution.stab.process_wall_seconds.is_finite()
        || execution.stab.process_wall_seconds <= 0.0
    {
        return Err(ReportError::PairShape);
    }
    Ok(())
}

fn only_row(
    rows: &[super::protocol::WorkerMeasurement],
) -> Result<&super::protocol::WorkerMeasurement, ReportError> {
    let [row] = rows else {
        return Err(ReportError::MeasurementCount(rows.len()));
    };
    Ok(row)
}

fn expected_pair_count(tier: QualificationTier) -> usize {
    match tier {
        QualificationTier::Pr => 3,
        QualificationTier::Full => 9,
        QualificationTier::Soak => 15,
    }
}

fn valid_git_commit(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_output_path(value: &str) -> bool {
    let path = std::path::Path::new(value);
    let mut components = path.components();
    !path.is_absolute()
        && components.next() == Some(std::path::Component::Normal("target".as_ref()))
        && components.next() == Some(std::path::Component::Normal("benchmarks".as_ref()))
        && components.next() == Some(std::path::Component::Normal("qualification".as_ref()))
        && components.next().is_some()
        && components.all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn validate_sha256(field: &'static str, value: &str) -> Result<(), ReportError> {
    validate_sha256_value(value).map_err(|()| ReportError::Digest(field))
}

fn validate_sha256_value(value: &str) -> Result<(), ()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(())
    }
}

fn thermal_readings_valid(readings: &[super::host::ThermalReading]) -> bool {
    readings.len() <= 128
        && readings.iter().all(|reading| {
            !reading.zone.is_empty()
                && reading.zone.len() <= 256
                && reading.zone.is_ascii()
                && !reading.kind.is_empty()
                && reading.kind.len() <= 256
                && reading.kind.is_ascii()
                && (-273_150..=300_000).contains(&reading.millidegrees_celsius)
        })
        && thermal_zone_keys(readings).len() == readings.len()
}

fn thermal_zone_keys(readings: &[super::host::ThermalReading]) -> BTreeSet<(&str, &str)> {
    readings
        .iter()
        .map(|reading| (reading.zone.as_str(), reading.kind.as_str()))
        .collect()
}

pub(super) fn preflight_artifact(
    root: &crate::root::RepoRoot,
    report: &QualificationReport,
    report_json: &[u8],
) -> Result<PerformancePreflightArtifact, ReportError> {
    validate_report(root, report)?;
    Ok(PerformancePreflightArtifact {
        schema_version: PREFLIGHT_SCHEMA_VERSION,
        report_sha256: sha256_hex(report_json),
        group_id: report.group_id.clone(),
        claim_class: report.claim_class,
        tier: report.tier,
        performance_inventory_sha256: report.performance_inventory_sha256.clone(),
        correctness_inventory_sha256: report.correctness_inventory_sha256.clone(),
        stab_commit: report.repository.commit_after.clone(),
        local_modifications: report.repository.local_modifications_before
            || report.repository.local_modifications_after,
        stim_commit: report.stim_commit.clone(),
        host_profile_id: report.host.profile_id.clone(),
        host_verified: report.host.verified,
        rust_toolchain: report.toolchain.rust_toolchain.clone(),
        target_triple: report.toolchain.target_triple.clone(),
        correctness_status: report.correctness_preflight.status,
        correctness_case_ids: report.correctness_preflight.case_ids.clone(),
        semantic_preflight_passed: true,
        sample_pairs: report.samples.len(),
        promotable: report.promotable,
    })
}

pub(super) fn render_markdown(report: &QualificationReport, report_sha256: &str) -> String {
    let summary = report.statistics.first();
    let median = summary.map_or("n/a".to_string(), |value| {
        format!("{:.6}", value.median_ratio)
    });
    let upper = summary.map_or("n/a".to_string(), |value| {
        format!("{:.6}", value.confidence_interval_upper)
    });
    let outcome = summary.map_or("n/a".to_string(), |value| {
        format!("{:?}", value.outcome).to_ascii_lowercase()
    });
    let maximum_temperature = |readings: &[super::host::ThermalReading]| {
        readings
            .iter()
            .map(|reading| reading.millidegrees_celsius)
            .max()
            .map_or("unavailable".to_string(), |value| value.to_string())
    };
    format!(
        "# PQ1 Qualification Harness Report\n\n- Group: `{}`\n- Claim class: diagnostic infrastructure\n- Tier: `{:?}`\n- Stim: `{}` (`{}`)\n- Stab commit: `{}`\n- Local modifications: `{}`\n- Host profile: `{}`\n- Host verified: `{}`\n- CPU: `{}` on `{}`\n- Frequency governor: `{:?}`\n- Maximum thermal reading before: `{}` millidegrees Celsius\n- Maximum thermal reading after: `{}` millidegrees Celsius\n- Rust toolchain: `{}`\n- Target: `{}`\n- Calibration target: `{:.3}` seconds\n- Calibration acceptance floor: `{:.3}` seconds\n- Warmups: `{}`\n- Paired samples: `{}`\n- Median diagnostic ratio: `{}`\n- Upper bootstrap bound: `{}`\n- Diagnostic 1.25 outcome: `{}`\n- Process memory evidence: separate from timing\n- Promotable product claim: `false`\n- Report SHA-256: `{}`\n",
        report.group_id,
        report.tier,
        report.stim_tag,
        report.stim_commit,
        report.repository.commit_after,
        report.repository.local_modifications_before || report.repository.local_modifications_after,
        report.host.profile_id,
        report.host.verified,
        report.host.selected_cpu,
        report.host.cpu_identity,
        report.host.frequency_governor_before,
        maximum_temperature(&report.host.thermal_readings_before),
        maximum_temperature(&report.host.thermal_readings_after),
        report.toolchain.rust_toolchain,
        report.toolchain.target_triple,
        report.calibration.target_minimum_seconds,
        report.calibration.acceptance_minimum_seconds,
        report.warmups.len(),
        report.samples.len(),
        median,
        upper,
        outcome,
        report_sha256,
    )
}

#[derive(Debug, Error)]
pub(super) enum ReportError {
    #[error("qualification report schema is {actual}, expected {expected}")]
    SchemaVersion { actual: u32, expected: u32 },
    #[error("qualification report has stale group or Stim identity")]
    Identity,
    #[error("qualification report command evidence is invalid")]
    CommandEvidence,
    #[error("qualification report field {0} is not a lowercase SHA-256 digest")]
    Digest(&'static str),
    #[error("qualification report repository identity is invalid")]
    RepositoryIdentity,
    #[error("qualification report Rust toolchain evidence is invalid")]
    ToolchainEvidence,
    #[error("qualification report host evidence is internally inconsistent")]
    HostEvidence,
    #[error("qualification report adapter build receipt is stale or inconsistent")]
    AdapterReceipt,
    #[error("qualification report Stab build receipt is stale or inconsistent")]
    StabBuildReceipt,
    #[error("qualification report worker receipt is stale or inconsistent")]
    WorkerReceipt,
    #[error("qualification report semantic work count overflows u64")]
    WorkOverflow,
    #[error("qualification report correctness evidence is structurally invalid")]
    CorrectnessEvidence,
    #[error("qualification report calibration evidence is invalid")]
    Calibration,
    #[error("qualification report has {0} warmups, expected three")]
    WarmupCount(usize),
    #[error("qualification report has {0} samples for its selected tier")]
    SampleCount(usize),
    #[error("qualification report pair indices or order are invalid")]
    PairIndex,
    #[error("qualification report pair shape is invalid")]
    PairShape,
    #[error("qualification report paired samples do not reproduce from raw worker rows")]
    PairedSamples,
    #[error("qualification report statistics measurement set is invalid")]
    StatisticsSet,
    #[error(
        "qualification report statistics do not reproduce from raw paired samples: expected {expected}; actual {actual}"
    )]
    StatisticsMismatch { expected: String, actual: String },
    #[error("qualification report worst upper bound is invalid")]
    WorstUpperBound,
    #[error("qualification report memory evidence is invalid")]
    Memory,
    #[error("qualification report claim classification is invalid")]
    Claim,
    #[error("memory-instrumented evidence cannot be consumed as timing evidence")]
    MemoryUsedAsTiming,
    #[error("qualification report expected one measurement but found {0}")]
    MeasurementCount(usize),
    #[error(transparent)]
    Invocation(#[from] super::invocation::InvocationError),
    #[error(transparent)]
    Statistics(#[from] super::statistics::StatisticsError),
    #[error(transparent)]
    Protocol(#[from] super::protocol::ProtocolError),
    #[error(transparent)]
    Host(#[from] super::host::HostError),
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),
    #[error("qualification report JSON must be nonempty and newline terminated")]
    ReportBoundary,
    #[error("qualification report output path does not match the validated directory")]
    OutputBinding,
    #[error("qualification report JSON is invalid: {0}")]
    Json(serde_json::Error),
}

#[cfg(test)]
#[path = "report/adversarial_tests.rs"]
mod adversarial_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_or_unverified_evidence_cannot_be_promoted() {
        let accepted = |tier, before, after, host, status, cases| {
            promotable_claim_requirements(true, tier, before, after, host, status, cases)
        };
        assert!(accepted(
            QualificationTier::Full,
            false,
            false,
            true,
            CorrectnessPreflightStatus::Passed,
            1,
        ));
        assert!(!accepted(
            QualificationTier::Full,
            true,
            false,
            true,
            CorrectnessPreflightStatus::Passed,
            1,
        ));
        assert!(!accepted(
            QualificationTier::Pr,
            false,
            false,
            true,
            CorrectnessPreflightStatus::Passed,
            1,
        ));
        assert!(!accepted(
            QualificationTier::Soak,
            false,
            false,
            false,
            CorrectnessPreflightStatus::Passed,
            1,
        ));
        assert!(!accepted(
            QualificationTier::Soak,
            false,
            false,
            true,
            CorrectnessPreflightStatus::NotApplicable,
            0,
        ));
    }
}
