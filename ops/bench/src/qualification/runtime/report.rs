use std::collections::BTreeSet;
use std::path::PathBuf;

use clap::Args;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::calibration::{CalibrationProbe, calibrate};
use super::correctness::CorrectnessPreflightStatus;
use super::protocol::{EvidenceMode, Implementation, InputDigest, ProtocolId, SemanticDigest};
use super::run::{
    ClaimClass, PairExecution, QualificationReport, QualificationTier, REPORT_SCHEMA_VERSION,
    TimingAttempt, TimingAttemptKind, sha256_hex,
};
use super::statistics::{GateOutcome, PairOrder, PairedSample, pair_measurements, summarize};
use crate::config::{STIM_COMMIT, STIM_TAG};

mod markdown;
mod published;

pub(super) use published::{
    MAX_PUBLISHED_PREFLIGHT_BYTES, MAX_PUBLISHED_REPORT_BYTES, load_validated_published_evidence,
    load_validated_published_report, run,
};

pub(super) fn render_markdown(
    report: &QualificationReport,
    report_sha256: &str,
) -> Result<String, ReportError> {
    markdown::render(report, report_sha256)
}

const PREFLIGHT_SCHEMA_VERSION: u32 = 5;
const EXPECTED_WARMUPS: usize = 3;
const EXPECTED_MAXIMUM_TIMING_ATTEMPTS: usize = 2;
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
    scale_id: String,
    work_items: u64,
    group_contract_sha256: String,
    claim_class: ClaimClass,
    baseline_eligibility: super::group::BaselineEligibility,
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
    timing_attempts: usize,
    authoritative_attempt_index: usize,
    sample_pairs: usize,
    promotable: bool,
}

pub(super) fn validate_report(
    root: &crate::root::RepoRoot,
    report: &QualificationReport,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
) -> Result<(), ReportError> {
    if report.schema_version != REPORT_SCHEMA_VERSION {
        return Err(ReportError::SchemaVersion {
            actual: report.schema_version,
            expected: REPORT_SCHEMA_VERSION,
        });
    }
    if report.stim_tag != STIM_TAG || report.stim_commit != STIM_COMMIT {
        return Err(ReportError::Identity);
    }
    if report.command.output.is_empty()
        || !valid_output_path(&report.command.output)
        || report.command.group_id.is_empty()
        || report.command.scale_id.is_empty()
        || report.command.work_items == 0
        || report.command.warmup_batches != EXPECTED_WARMUPS
        || report.command.paired_samples != expected_pair_count(report.tier)
        || report.command.maximum_timing_attempts != EXPECTED_MAXIMUM_TIMING_ATTEMPTS
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
    if report.performance_inventory_sha256 != expected_performance_inventory_sha256
        || report.correctness_inventory_sha256 != expected_correctness_inventory_sha256
    {
        return Err(ReportError::InventoryEvidence);
    }
    validate_sha256("group_contract_sha256", &report.group_contract_sha256)?;
    let resolved_group = super::group::load_group(
        root,
        expected_performance_inventory_sha256,
        &report.group_id,
    )?;
    if report.group_contract_sha256 != resolved_group.source_sha256
        || report.claim_class != resolved_group.contract.claim_class
        || report.baseline_eligibility != resolved_group.contract.baseline_eligibility
        || report.owner != resolved_group.contract.owner.to_string()
        || report.profiler_note != resolved_group.contract.profiler_note
    {
        return Err(ReportError::GroupEvidence);
    }
    let scale = resolved_group.contract.scale(&report.scale_id)?;
    if report.command.group_id != report.group_id
        || report.command.scale_id != report.scale_id
        || report.command.work_items != scale.work_items.get()
    {
        return Err(ReportError::GroupEvidence);
    }
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
    validate_sha256(
        "contract_preflight_sha256",
        &report.workers.contract_preflight_sha256,
    )?;
    if !report.contract_preflight.validates_source_contract()
        || report.workers.contract_preflight_sha256 != report.contract_preflight.sha256()
    {
        return Err(ReportError::WorkerReceipt);
    }
    if !report.adapter_receipt.validates_report_identity(
        &report.workers.stim_source_sha256,
        &report.workers.stim_build_fingerprint,
        &report.workers.stim_binary_sha256,
    ) {
        return Err(ReportError::AdapterReceipt);
    }
    if !report
        .adapter_receipt
        .validates_comparator_sources(&resolved_group.contract.comparator_sources)
    {
        return Err(ReportError::ComparatorSourceReceipt);
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
    report.toolchain.validate_current(root)?;
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
    validate_all_worker_receipts(report, &resolved_group.contract)?;
    validate_correctness_evidence(root, report, &resolved_group.contract)?;
    validate_pair_execution(&report.semantic_preflight, EvidenceMode::Timing)?;
    validate_calibration(report)?;
    validate_timing_attempts(report)?;
    validate_failure_evidence(report)?;
    validate_memory(report)?;
    validate_claim(report, &resolved_group.contract)?;
    Ok(())
}

fn validate_failure_evidence(report: &QualificationReport) -> Result<(), ReportError> {
    require_failure_evidence(
        report.claim_class,
        &report.timing_attempts,
        report.profiler_note.is_some(),
    )
}

fn require_failure_evidence(
    claim_class: ClaimClass,
    timing_attempts: &[TimingAttempt],
    has_profiler_note: bool,
) -> Result<(), ReportError> {
    let failed_or_noisy = timing_attempts.iter().any(|attempt| {
        attempt
            .statistics
            .iter()
            .any(|summary| summary.outcome != GateOutcome::Passed)
    });
    if claim_class == ClaimClass::PromotablePerformance && failed_or_noisy && !has_profiler_note {
        return Err(ReportError::FailureEvidence);
    }
    Ok(())
}

fn validate_timing_attempts(report: &QualificationReport) -> Result<(), ReportError> {
    validate_timing_attempt_policy(&report.timing_attempts)?;
    for attempt in &report.timing_attempts {
        validate_timing_attempt(report, attempt)?;
    }
    Ok(())
}

fn validate_timing_attempt_policy(attempts: &[TimingAttempt]) -> Result<(), ReportError> {
    let initial = attempts.first().ok_or(ReportError::TimingAttemptCount(0))?;
    let expected_attempts = if initial.requires_noisy_rerun() { 2 } else { 1 };
    if attempts.len() != expected_attempts {
        return Err(ReportError::TimingAttemptCount(attempts.len()));
    }
    for (attempt_index, attempt) in attempts.iter().enumerate() {
        let expected_kind = if attempt_index == 0 {
            TimingAttemptKind::Initial
        } else {
            TimingAttemptKind::PairedRatioNoiseRerun
        };
        if attempt.attempt_index != attempt_index || attempt.kind != expected_kind {
            return Err(ReportError::TimingAttemptIdentity);
        }
    }
    Ok(())
}

fn validate_timing_attempt(
    report: &QualificationReport,
    attempt: &TimingAttempt,
) -> Result<(), ReportError> {
    if attempt.warmups.len() != EXPECTED_WARMUPS {
        return Err(ReportError::WarmupCount(attempt.warmups.len()));
    }
    for (index, warmup) in attempt.warmups.iter().enumerate() {
        if warmup.pair_index != index {
            return Err(ReportError::PairIndex);
        }
        validate_pair_execution(warmup, EvidenceMode::Timing)?;
    }
    if attempt.samples.len() != expected_pair_count(report.tier) {
        return Err(ReportError::SampleCount(attempt.samples.len()));
    }
    let mut reconstructed = Vec::new();
    for (index, sample) in attempt.samples.iter().enumerate() {
        if sample.pair_index != index {
            return Err(ReportError::PairIndex);
        }
        reconstructed.extend(validate_pair_execution(sample, EvidenceMode::Timing)?);
    }
    if !paired_samples_equivalent(&reconstructed, &attempt.paired_samples) {
        return Err(ReportError::PairedSamples);
    }
    let measurement_ids = attempt
        .paired_samples
        .iter()
        .map(|sample| sample.measurement_id.clone())
        .collect::<BTreeSet<_>>();
    if measurement_ids.len() != attempt.statistics.len() || measurement_ids.is_empty() {
        return Err(ReportError::StatisticsSet);
    }
    let mut reconstructed_statistics = Vec::new();
    for measurement_id in measurement_ids {
        let selected = attempt
            .paired_samples
            .iter()
            .filter(|sample| sample.measurement_id == measurement_id)
            .cloned()
            .collect::<Vec<PairedSample>>();
        reconstructed_statistics.push(summarize(measurement_id, &selected, EXPECTED_THRESHOLD)?);
    }
    if !statistics_equivalent(&reconstructed_statistics, &attempt.statistics) {
        return Err(ReportError::StatisticsMismatch {
            expected: format!("{reconstructed_statistics:?}"),
            actual: format!("{:?}", attempt.statistics),
        });
    }
    let worst = reconstructed_statistics
        .iter()
        .map(|summary| summary.confidence_interval_upper)
        .reduce(f64::max)
        .ok_or(ReportError::StatisticsSet)?;
    if !approximately_equal(worst, attempt.worst_confidence_interval_upper) {
        return Err(ReportError::WorstUpperBound);
    }
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

fn validate_correctness_evidence(
    root: &crate::root::RepoRoot,
    report: &QualificationReport,
    group: &super::group::GroupContract,
) -> Result<(), ReportError> {
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
                || report.command.correctness_output.is_some()
                || report.command.correctness_request_sha256.is_some()
                || report.command.correctness_completion_sha256.is_some()
            {
                return Err(ReportError::CorrectnessEvidence);
            }
        }
        CorrectnessPreflightStatus::Passed => {
            if evidence.case_ids != group.correctness_case_ids
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
            let output = report
                .command
                .correctness_output
                .as_deref()
                .ok_or(ReportError::CorrectnessEvidence)?;
            let request_sha256 = report
                .command
                .correctness_request_sha256
                .as_deref()
                .ok_or(ReportError::CorrectnessEvidence)?;
            let completion_sha256 = report
                .command
                .correctness_completion_sha256
                .as_deref()
                .ok_or(ReportError::CorrectnessEvidence)?;
            if evidence.source_directory.as_deref() != Some(output)
                || evidence.request_sha256.as_deref() != Some(request_sha256)
                || evidence.completion_sha256.as_deref() != Some(completion_sha256)
            {
                return Err(ReportError::CorrectnessEvidence);
            }
            let reconstructed = super::correctness::validate(
                root,
                super::correctness::CorrectnessRequirement::Required {
                    output: std::path::Path::new(output),
                    case_ids: &group.correctness_case_ids,
                    expected_manifest_sha256: &report.correctness_inventory_sha256,
                    expected_stab_commit: &report.repository.commit_before,
                    expected_request_sha256: request_sha256,
                    expected_completion_sha256: completion_sha256,
                },
            )?;
            if *evidence != reconstructed {
                return Err(ReportError::CorrectnessEvidence);
            }
        }
    }
    Ok(())
}

fn validate_all_worker_receipts(
    report: &QualificationReport,
    group: &super::group::GroupContract,
) -> Result<(), ReportError> {
    let workload_id = &group.workload_id;
    let measurement_id = group.single_measurement()?;
    let scale = group.scale(&report.scale_id)?;
    let identity = ReceiptIdentity::from_report(report, scale)?;
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
        workload_id,
        measurement_id,
        output_digest: Some(expected_output_digest),
    };
    validate_pair_receipts(&identity, &report.semantic_preflight, &common_timing)?;
    for probe in &report.calibration.stim.probes {
        let phase = PhaseExpectation {
            evidence_mode: EvidenceMode::Timing,
            iterations: probe.iterations,
            workload_id,
            measurement_id,
            output_digest: None,
        };
        validate_invocation_receipt(&identity, &probe.invocation, Implementation::Stim, &phase)?;
    }
    for probe in &report.calibration.stab.probes {
        let phase = PhaseExpectation {
            evidence_mode: EvidenceMode::Timing,
            iterations: probe.iterations,
            workload_id,
            measurement_id,
            output_digest: None,
        };
        validate_invocation_receipt(&identity, &probe.invocation, Implementation::Stab, &phase)?;
    }
    validate_pair_receipts(
        &identity,
        &report.calibration.common_validation,
        &common_timing,
    )?;
    for attempt in &report.timing_attempts {
        for pair in &attempt.warmups {
            validate_pair_receipts(&identity, pair, &common_timing)?;
        }
        for pair in &attempt.samples {
            validate_pair_receipts(&identity, pair, &common_timing)?;
        }
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
    input_bytes: u64,
    input_digest: &'a InputDigest,
    invocation_timeout_seconds: f64,
    expected_cpu: u32,
    stim_commit: &'a str,
    stim_source: &'a str,
    stim_build: &'a str,
    stab_source: &'a str,
    stab_build: &'a str,
}

impl<'a> ReceiptIdentity<'a> {
    fn from_report(
        report: &'a QualificationReport,
        scale: &'a super::group::ScaleContract,
    ) -> Result<Self, ReportError> {
        Ok(Self {
            work_items: report.command.work_items,
            input_bytes: scale.input_bytes,
            input_digest: &scale.input_digest,
            invocation_timeout_seconds: report.command.invocation_timeout_seconds as f64,
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
        || invocation.process_wall_seconds > identity.invocation_timeout_seconds
        || invocation.process_wall_seconds < row.elapsed_seconds
        || row.implementation != implementation
        || row.evidence_mode != phase.evidence_mode
        || row.workload_id != *phase.workload_id
        || row.measurement_id != *phase.measurement_id
        || row.iteration_count != phase.iterations
        || row.affinity_cpu != Some(identity.expected_cpu)
        || row.stim_commit.as_str() != identity.stim_commit
        || row.work_count != expected_work_count
        || row.input_bytes != identity.input_bytes
        || row.input_digest != *identity.input_digest
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

fn validate_claim(
    report: &QualificationReport,
    group: &super::group::GroupContract,
) -> Result<(), ReportError> {
    match report.claim_class {
        ClaimClass::DiagnosticInfrastructure => {
            if report.promotable
                || group.baseline_eligibility != super::group::BaselineEligibility::ReportOnly
                || report.correctness_preflight.status != CorrectnessPreflightStatus::NotApplicable
                || !report.correctness_preflight.case_ids.is_empty()
                || !group.correctness_case_ids.is_empty()
            {
                return Err(ReportError::Claim);
            }
        }
        ClaimClass::PromotablePerformance => {
            if group.baseline_eligibility != super::group::BaselineEligibility::ThresholdEligible
                || report.correctness_preflight.case_ids != group.correctness_case_ids
                || report.promotable
                    != promotion_eligibility(PromotionEvidence {
                        claim_class: report.claim_class,
                        allow_unverified_host: report.command.allow_unverified_host,
                        tier: report.tier,
                        local_modifications_before: report.repository.local_modifications_before,
                        local_modifications_after: report.repository.local_modifications_after,
                        host_verified: report.host.verified,
                        correctness_status: report.correctness_preflight.status,
                        correctness_case_count: report.correctness_preflight.case_ids.len(),
                    })
            {
                return Err(ReportError::Claim);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct PromotionEvidence {
    pub(super) claim_class: ClaimClass,
    pub(super) allow_unverified_host: bool,
    pub(super) tier: QualificationTier,
    pub(super) local_modifications_before: bool,
    pub(super) local_modifications_after: bool,
    pub(super) host_verified: bool,
    pub(super) correctness_status: CorrectnessPreflightStatus,
    pub(super) correctness_case_count: usize,
}

fn promotable_claim_requirements(evidence: PromotionEvidence) -> bool {
    !evidence.allow_unverified_host
        && matches!(
            evidence.tier,
            QualificationTier::Full | QualificationTier::Soak
        )
        && !evidence.local_modifications_before
        && !evidence.local_modifications_after
        && evidence.host_verified
        && evidence.correctness_status == CorrectnessPreflightStatus::Passed
        && evidence.correctness_case_count > 0
}

pub(super) fn promotion_eligibility(evidence: PromotionEvidence) -> bool {
    evidence.claim_class == ClaimClass::PromotablePerformance
        && promotable_claim_requirements(evidence)
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
    report: &QualificationReport,
    report_json: &[u8],
) -> Result<PerformancePreflightArtifact, ReportError> {
    let authoritative = authoritative_timing_attempt(report)?;
    Ok(PerformancePreflightArtifact {
        schema_version: PREFLIGHT_SCHEMA_VERSION,
        report_sha256: sha256_hex(report_json),
        group_id: report.group_id.clone(),
        scale_id: report.scale_id.clone(),
        work_items: report.command.work_items,
        group_contract_sha256: report.group_contract_sha256.clone(),
        claim_class: report.claim_class,
        baseline_eligibility: report.baseline_eligibility,
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
        timing_attempts: report.timing_attempts.len(),
        authoritative_attempt_index: authoritative.attempt_index,
        sample_pairs: authoritative.samples.len(),
        promotable: report.promotable,
    })
}

pub(super) fn authoritative_timing_attempt(
    report: &QualificationReport,
) -> Result<&TimingAttempt, ReportError> {
    report
        .timing_attempts
        .last()
        .ok_or(ReportError::TimingAttemptCount(0))
}

#[derive(Debug, Error)]
pub(super) enum ReportError {
    #[error("qualification report schema is {actual}, expected {expected}")]
    SchemaVersion { actual: u32, expected: u32 },
    #[error("qualification report has stale group or Stim identity")]
    Identity,
    #[error("qualification report group contract evidence is stale or inconsistent")]
    GroupEvidence,
    #[error("failed or noisy product evidence lacks source-owned failure ownership")]
    FailureEvidence,
    #[error("qualification report inventory evidence differs from the checked inventories")]
    InventoryEvidence,
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
    #[error("qualification report comparator sources differ from the adapter build receipt")]
    ComparatorSourceReceipt,
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
    #[error("qualification report has an invalid timing-attempt count: {0}")]
    TimingAttemptCount(usize),
    #[error("qualification report timing-attempt indices or reasons are invalid")]
    TimingAttemptIdentity,
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
    Correctness(#[from] super::correctness::CorrectnessError),
    #[error(transparent)]
    Host(#[from] super::host::HostError),
    #[error(transparent)]
    Group(#[from] super::group::GroupError),
    #[error(transparent)]
    Toolchain(#[from] super::toolchain::ToolchainError),
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),
    #[error("qualification report JSON must be nonempty and newline terminated")]
    ReportBoundary,
    #[error(
        "qualification report JSON is not canonical at byte {offset}: actual={actual:?} expected={expected:?} actual_length={actual_length} expected_length={expected_length}"
    )]
    NonCanonicalReport {
        offset: usize,
        actual: Option<u8>,
        expected: Option<u8>,
        actual_length: usize,
        expected_length: usize,
    },
    #[error("qualification report output path does not match the validated directory")]
    OutputBinding,
    #[error("qualification preflight does not exactly reproduce from report.json")]
    PreflightBinding,
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
        let accepted = |allow_unverified_host, tier, before, after, host, status, cases| {
            promotable_claim_requirements(PromotionEvidence {
                claim_class: ClaimClass::PromotablePerformance,
                allow_unverified_host,
                tier,
                local_modifications_before: before,
                local_modifications_after: after,
                host_verified: host,
                correctness_status: status,
                correctness_case_count: cases,
            })
        };
        assert!(accepted(
            false,
            QualificationTier::Full,
            false,
            false,
            true,
            CorrectnessPreflightStatus::Passed,
            1,
        ));
        assert!(!accepted(
            true,
            QualificationTier::Full,
            false,
            false,
            true,
            CorrectnessPreflightStatus::Passed,
            1,
        ));
        assert!(!accepted(
            false,
            QualificationTier::Full,
            true,
            false,
            true,
            CorrectnessPreflightStatus::Passed,
            1,
        ));
        assert!(!accepted(
            false,
            QualificationTier::Pr,
            false,
            false,
            true,
            CorrectnessPreflightStatus::Passed,
            1,
        ));
        assert!(!accepted(
            false,
            QualificationTier::Soak,
            false,
            false,
            false,
            CorrectnessPreflightStatus::Passed,
            1,
        ));
        assert!(!accepted(
            false,
            QualificationTier::Soak,
            false,
            false,
            true,
            CorrectnessPreflightStatus::NotApplicable,
            0,
        ));
        assert!(!promotion_eligibility(PromotionEvidence {
            claim_class: ClaimClass::PromotablePerformance,
            allow_unverified_host: false,
            tier: QualificationTier::Pr,
            local_modifications_before: false,
            local_modifications_after: false,
            host_verified: true,
            correctness_status: CorrectnessPreflightStatus::Passed,
            correctness_case_count: 1,
        }));
        assert!(!promotion_eligibility(PromotionEvidence {
            claim_class: ClaimClass::DiagnosticInfrastructure,
            allow_unverified_host: false,
            tier: QualificationTier::Full,
            local_modifications_before: false,
            local_modifications_after: false,
            host_verified: true,
            correctness_status: CorrectnessPreflightStatus::Passed,
            correctness_case_count: 1,
        }));
    }
}
