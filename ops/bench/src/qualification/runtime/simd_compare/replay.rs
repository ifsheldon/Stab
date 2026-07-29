use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::path::PathBuf;

use clap::Args;

use super::{
    ComparisonScope, GROUP_IDS, GroupEvidence, REPORT_SCHEMA_VERSION, SCALE_IDS, ScaleEvidence,
    SimdCompareError, SimdCompareReport, VariantCalibration, VariantPairExecution,
    VariantPairedSample, VariantSummary, WARMUP_BATCHES, exact_pair_output, pair_sample,
    render_json, render_markdown, summarize,
};
use crate::config::STIM_COMMIT;
use crate::qualification::runtime::artifact::{
    DirectQualificationArtifactPath, RepositoryBinding, read_artifact_bounded_with_repository,
};
use crate::qualification::runtime::calibration::{CalibrationProbe, calibrate};
use crate::qualification::runtime::group::{GroupContract, ResolvedGroupContract, ScaleContract};
use crate::qualification::runtime::protocol::{
    EvidenceMode, GitCommit, Implementation, ProtocolExpectation, RAW_WORK_TIMING_BOUNDARY,
    SemanticDigest, Sha256Digest,
};
use crate::qualification::runtime::run::{
    INVOCATION_TIMEOUT, WARMUP_BATCHES as RUN_WARMUP_BATCHES,
};
use crate::qualification::runtime::stab_build::StabBuildVariant;
use crate::root::RepoRoot;

const MAX_REPORT_BYTES: usize = 4 << 20;
const MAX_MARKDOWN_BYTES: usize = 1 << 20;

#[derive(Clone, Debug, Args)]
pub(crate) struct SimdReportArgs {
    /// Published scalar-versus-portable-SIMD diagnostic directory to replay.
    #[arg(
        long,
        default_value = "target/benchmarks/qualification/a6-simd-compare-latest"
    )]
    input: PathBuf,
}

pub(super) fn run_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    live_repository: &RepositoryBinding,
    performance_inventory_sha256: &str,
    correctness_inventory_sha256: &str,
    args: SimdReportArgs,
) -> Result<PathBuf, SimdCompareError> {
    let input = DirectQualificationArtifactPath::try_new(&args.input)?;
    let report_json = read_artifact_bounded_with_repository(
        root,
        live_repository,
        &input,
        "report.json",
        MAX_REPORT_BYTES,
    )?;
    if report_json.is_empty() || !report_json.ends_with(b"\n") {
        return Err(SimdCompareError::ReportBoundary);
    }
    let report: SimdCompareReport = serde_json::from_slice(&report_json)?;
    if render_json(&report)? != report_json {
        return Err(SimdCompareError::NonCanonicalReport);
    }
    let groups = super::load_groups(source_root, performance_inventory_sha256)?;
    validate_report(
        root,
        source_root,
        live_repository,
        performance_inventory_sha256,
        correctness_inventory_sha256,
        &groups,
        &report,
        Some(&input),
    )?;
    let markdown = read_artifact_bounded_with_repository(
        root,
        live_repository,
        &input,
        "report.md",
        MAX_MARKDOWN_BYTES,
    )?;
    let expected_markdown = render_markdown(&report, &super::super::run::sha256_hex(&report_json));
    if markdown != expected_markdown.as_bytes() {
        return Err(SimdCompareError::MarkdownBinding);
    }
    Ok(input.into_path_buf())
}

#[allow(
    clippy::too_many_arguments,
    reason = "replay validates one report against its complete source identity tuple"
)]
pub(super) fn validate_report(
    root: &RepoRoot,
    source_root: &RepoRoot,
    live_repository: &RepositoryBinding,
    performance_inventory_sha256: &str,
    correctness_inventory_sha256: &str,
    groups: &[ResolvedGroupContract],
    report: &SimdCompareReport,
    expected_output: Option<&DirectQualificationArtifactPath>,
) -> Result<(), SimdCompareError> {
    validate_report_shape(report, expected_output)?;
    if report.performance_inventory_sha256 != performance_inventory_sha256
        || report.correctness_inventory_sha256 != correctness_inventory_sha256
    {
        return Err(SimdCompareError::InventoryEvidence);
    }
    let current = super::super::run::bound_repository_state(root, live_repository)?;
    if current.commit != report.repository.commit_after
        || current.local_modifications != report.repository.local_modifications_after
    {
        return Err(SimdCompareError::RepositoryEvidence);
    }
    report.toolchain.validate_current(source_root)?;
    report.host.validate_against_policy(source_root)?;
    validate_worker_evidence(source_root, report)?;
    if groups.len() != report.groups.len() {
        return Err(SimdCompareError::GroupEvidence);
    }
    for ((evidence, resolved), expected_group_id) in report.groups.iter().zip(groups).zip(GROUP_IDS)
    {
        validate_group_binding(evidence, resolved, expected_group_id)?;
        validate_group_runtime(report, evidence, resolved)?;
    }
    Ok(())
}

fn validate_report_shape(
    report: &SimdCompareReport,
    expected_output: Option<&DirectQualificationArtifactPath>,
) -> Result<(), SimdCompareError> {
    let output_matches = expected_output
        .is_none_or(|expected| std::path::Path::new(&report.command.output) == expected.as_path());
    if report.schema_version != REPORT_SCHEMA_VERSION
        || report.scope != ComparisonScope::ScalarVsPortableSimd
        || report.timing_boundary != RAW_WORK_TIMING_BOUNDARY
        || report.generated_unix_epoch_seconds == 0
        || report.command.group_ids != GROUP_IDS.map(str::to_string)
        || report.command.scale_ids != SCALE_IDS.map(str::to_string)
        || report.command.warmup_pairs != WARMUP_BATCHES
        || report.command.retained_pairs != report.command.tier.sample_count()
        || report.command.invocation_timeout_seconds != INVOCATION_TIMEOUT.as_secs()
        || report.command.suite_timeout_seconds != super::SUITE_TIMEOUT.as_secs()
        || !output_matches
        || report.repository.commit_before != report.repository.commit_after
        || report.repository.commit_before.len() != 40
        || !report
            .repository
            .commit_before
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || report.repository.local_modifications_before
        || report.repository.local_modifications_after
        || !report.host.verified && !report.command.allow_unverified_host
        || !worker_variants_match(
            report.scalar_worker.variant,
            report.scalar_worker.build_receipt.variant(),
            report.portable_worker.variant,
            report.portable_worker.build_receipt.variant(),
        )
        || report.groups.len() != GROUP_IDS.len()
    {
        return Err(SimdCompareError::InvalidReport);
    }
    Ok(())
}

fn worker_variants_match(
    scalar: StabBuildVariant,
    scalar_receipt: StabBuildVariant,
    portable: StabBuildVariant,
    portable_receipt: StabBuildVariant,
) -> bool {
    scalar == StabBuildVariant::Scalar
        && scalar_receipt == StabBuildVariant::Scalar
        && portable == StabBuildVariant::PortableSimd
        && portable_receipt == StabBuildVariant::PortableSimd
}

fn validate_worker_evidence(
    source_root: &RepoRoot,
    report: &SimdCompareReport,
) -> Result<(), SimdCompareError> {
    let commit = &report.repository.commit_before;
    for worker in [&report.scalar_worker, &report.portable_worker] {
        worker
            .build_receipt
            .validate_replayed_identity(
                source_root,
                &worker.identity.stab_source_sha256,
                &worker.identity.stab_build_fingerprint,
                &worker.identity.stab_binary_sha256,
                commit,
                &report.toolchain,
            )
            .map_err(|_| SimdCompareError::WorkerEvidence)?;
    }
    Ok(())
}

fn validate_group_binding(
    evidence: &GroupEvidence,
    resolved: &ResolvedGroupContract,
    expected_group_id: &str,
) -> Result<(), SimdCompareError> {
    let contract = &resolved.contract;
    if evidence.group_id != expected_group_id
        || evidence.group_id != contract.id.to_string()
        || evidence.group_contract_sha256 != resolved.source_sha256
        || evidence.workload_id != contract.workload_id.to_string()
        || evidence.measurement_id != contract.single_measurement()?.to_string()
        || evidence.owner != contract.owner.to_string()
        || evidence.scales.len() != SCALE_IDS.len()
    {
        return Err(SimdCompareError::GroupEvidence);
    }
    for (scale, expected_scale_id) in evidence.scales.iter().zip(SCALE_IDS) {
        let contract_scale = contract.scale(expected_scale_id)?;
        if scale.scale_id != expected_scale_id
            || scale.family_id != contract_scale.family_id.to_string()
            || scale.size_class != contract_scale.size_class
            || scale.work_items != contract_scale.work_items.get()
            || scale.input_bytes != contract_scale.input_bytes
            || scale.input_digest != contract_scale.input_digest.as_str()
        {
            return Err(SimdCompareError::GroupEvidence);
        }
    }
    Ok(())
}

fn validate_group_runtime(
    report: &SimdCompareReport,
    evidence: &GroupEvidence,
    resolved: &ResolvedGroupContract,
) -> Result<(), SimdCompareError> {
    for scale in &evidence.scales {
        validate_scale_runtime(
            report,
            &resolved.contract,
            resolved.contract.scale(&scale.scale_id)?,
            scale,
        )?;
    }
    Ok(())
}

fn validate_scale_runtime(
    report: &SimdCompareReport,
    group: &GroupContract,
    contract_scale: &ScaleContract,
    scale: &ScaleEvidence,
) -> Result<(), SimdCompareError> {
    validate_calibration(
        report,
        group,
        contract_scale,
        &scale.scalar_calibration,
        StabBuildVariant::Scalar,
    )?;
    validate_calibration(
        report,
        group,
        contract_scale,
        &scale.portable_calibration,
        StabBuildVariant::PortableSimd,
    )?;
    let common_iterations = scale
        .scalar_calibration
        .selected_iterations
        .max(scale.portable_calibration.selected_iterations);
    if scale.common_iterations != common_iterations || common_iterations == 0 {
        return Err(SimdCompareError::InvalidIterations);
    }
    let iterations =
        NonZeroU64::new(common_iterations).ok_or(SimdCompareError::InvalidIterations)?;
    let output = validate_pair_runtime(
        report,
        group,
        contract_scale,
        &scale.semantic_validation,
        0,
        iterations,
        None,
    )?;
    if scale.warmups.len() != RUN_WARMUP_BATCHES
        || scale.samples.len() != report.command.retained_pairs
        || scale.paired_samples.len() != report.command.retained_pairs
    {
        return Err(SimdCompareError::InvalidReport);
    }
    for (pair_index, pair) in scale.warmups.iter().enumerate() {
        validate_pair_runtime(
            report,
            group,
            contract_scale,
            pair,
            pair_index,
            iterations,
            Some(&output),
        )?;
    }
    for (pair_index, pair) in scale.samples.iter().enumerate() {
        validate_pair_runtime(
            report,
            group,
            contract_scale,
            pair,
            pair_index,
            iterations,
            Some(&output),
        )?;
    }
    reconstruct_scale_derivations(scale)
}

fn validate_calibration(
    report: &SimdCompareReport,
    group: &GroupContract,
    scale: &ScaleContract,
    calibration: &VariantCalibration,
    variant: StabBuildVariant,
) -> Result<(), SimdCompareError> {
    if calibration.variant != variant || calibration.probes.is_empty() {
        return Err(SimdCompareError::CalibrationEvidence);
    }
    for probe in &calibration.probes {
        let iterations =
            NonZeroU64::new(probe.iterations).ok_or(SimdCompareError::InvalidIterations)?;
        validate_invocation(
            report,
            group,
            scale,
            &probe.invocation,
            variant,
            iterations,
            None,
        )?;
    }
    let mut probe_index = 0;
    let decision = calibrate(super::super::run::calibration_policy()?, |iterations| {
        let probe = calibration
            .probes
            .get(probe_index)
            .ok_or_else(|| "stored calibration ended before convergence".to_string())?;
        if probe.iterations != iterations.get() {
            return Err("stored calibration iteration sequence differs".to_string());
        }
        probe_index += 1;
        Ok(CalibrationProbe {
            measured: probe
                .invocation
                .measured_duration()
                .map_err(|error| error.to_string())?,
            wall: probe
                .invocation
                .wall_duration()
                .map_err(|error| error.to_string())?,
        })
    })
    .map_err(|_| SimdCompareError::CalibrationEvidence)?;
    if probe_index != calibration.probes.len()
        || decision.iterations.get() != calibration.selected_iterations
        || decision.measured.as_secs_f64().to_bits()
            != calibration.selected_measured_seconds.to_bits()
    {
        return Err(SimdCompareError::CalibrationEvidence);
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "pair replay keeps the complete runtime contract explicit at the call site"
)]
fn validate_pair_runtime(
    report: &SimdCompareReport,
    group: &GroupContract,
    scale: &ScaleContract,
    pair: &VariantPairExecution,
    pair_index: usize,
    iterations: NonZeroU64,
    expected_output: Option<&SemanticDigest>,
) -> Result<SemanticDigest, SimdCompareError> {
    if pair.pair_index != pair_index || pair.order != super::VariantPairOrder::for_pair(pair_index)
    {
        return Err(SimdCompareError::InvalidReport);
    }
    validate_invocation(
        report,
        group,
        scale,
        &pair.scalar,
        StabBuildVariant::Scalar,
        iterations,
        expected_output,
    )?;
    validate_invocation(
        report,
        group,
        scale,
        &pair.portable,
        StabBuildVariant::PortableSimd,
        iterations,
        expected_output,
    )?;
    exact_pair_output(pair)
}

#[allow(
    clippy::too_many_arguments,
    reason = "protocol expectations require the complete report, group, scale, and worker tuple"
)]
fn validate_invocation(
    report: &SimdCompareReport,
    group: &GroupContract,
    scale: &ScaleContract,
    invocation: &super::super::invocation::InvocationRecord,
    variant: StabBuildVariant,
    iterations: NonZeroU64,
    expected_output: Option<&SemanticDigest>,
) -> Result<(), SimdCompareError> {
    if invocation.implementation != Implementation::Stab
        || invocation.evidence_mode != EvidenceMode::Timing
        || !invocation.process_wall_seconds.is_finite()
        || invocation.process_wall_seconds <= 0.0
    {
        return Err(SimdCompareError::RawInvocation);
    }
    for row in &invocation.rows {
        row.validate_values()?;
    }
    let worker = match variant {
        StabBuildVariant::Scalar => &report.scalar_worker,
        StabBuildVariant::PortableSimd => &report.portable_worker,
        StabBuildVariant::LegacyDefault => return Err(SimdCompareError::WorkerEvidence),
    };
    let expected_work_count = iterations
        .get()
        .checked_mul(scale.work_items.get())
        .ok_or(SimdCompareError::InvalidIterations)?;
    let selected_cpu =
        u32::try_from(report.host.selected_cpu).map_err(|_| SimdCompareError::RawInvocation)?;
    ProtocolExpectation {
        implementation: Implementation::Stab,
        evidence_mode: EvidenceMode::Timing,
        workload_id: group.workload_id.clone(),
        measurement_ids: BTreeSet::from([group.single_measurement()?.clone()]),
        iteration_count: iterations.get(),
        expected_work_count,
        expected_input_bytes: scale.input_bytes,
        expected_input_digest: scale.input_digest.clone(),
        expected_output_digest: expected_output.cloned(),
        affinity_cpu: Some(selected_cpu),
        stim_commit: GitCommit::try_new(STIM_COMMIT)?,
        source_digest: Sha256Digest::try_new(worker.identity.stab_source_sha256.clone())?,
        build_fingerprint: Sha256Digest::try_new(worker.identity.stab_build_fingerprint.clone())?,
    }
    .validate(&invocation.rows)?;
    Ok(())
}

fn reconstruct_scale_derivations(scale: &ScaleEvidence) -> Result<(), SimdCompareError> {
    if scale.samples.len() != scale.paired_samples.len() {
        return Err(SimdCompareError::DerivedSamples);
    }
    let mut reconstructed = Vec::with_capacity(scale.samples.len());
    for (pair, stored) in scale.samples.iter().zip(&scale.paired_samples) {
        let derived = pair_sample(pair)?;
        if !paired_sample_matches(stored, &derived) {
            return Err(SimdCompareError::DerivedSamples);
        }
        reconstructed.push(derived);
    }
    let summary = summarize(&reconstructed)?;
    if !summary_matches(&scale.summary, &summary) {
        return Err(SimdCompareError::SummaryEvidence);
    }
    Ok(())
}

fn paired_sample_matches(left: &VariantPairedSample, right: &VariantPairedSample) -> bool {
    left.pair_index == right.pair_index
        && left.order == right.order
        && left.scalar_elapsed_seconds.to_bits() == right.scalar_elapsed_seconds.to_bits()
        && left.portable_elapsed_seconds.to_bits() == right.portable_elapsed_seconds.to_bits()
        && left.scalar_work_count == right.scalar_work_count
        && left.portable_work_count == right.portable_work_count
        && left.scalar_work_per_second.to_bits() == right.scalar_work_per_second.to_bits()
        && left.portable_work_per_second.to_bits() == right.portable_work_per_second.to_bits()
        && left.portable_over_scalar_ratio.to_bits() == right.portable_over_scalar_ratio.to_bits()
}

fn summary_matches(left: &VariantSummary, right: &VariantSummary) -> bool {
    left.pair_count == right.pair_count
        && left.median_portable_over_scalar_ratio.to_bits()
            == right.median_portable_over_scalar_ratio.to_bits()
        && left.confidence_interval_lower.to_bits() == right.confidence_interval_lower.to_bits()
        && left.confidence_interval_upper.to_bits() == right.confidence_interval_upper.to_bits()
        && left.scalar_relative_mad.to_bits() == right.scalar_relative_mad.to_bits()
        && left.portable_relative_mad.to_bits() == right.portable_relative_mad.to_bits()
        && left.ratio_relative_mad.to_bits() == right.ratio_relative_mad.to_bits()
        && left.material_benefit == right.material_benefit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qualification::model::SizeClass;
    use crate::qualification::model::TimingBatchPolicy;
    use crate::qualification::runtime::group::ParityEligibility;
    use crate::qualification::runtime::protocol::{
        InputDigest, PROTOCOL_SCHEMA_VERSION, ProtocolId, WorkerMeasurement,
    };
    use crate::qualification::runtime::run::ClaimClass;

    fn digest(byte: char) -> String {
        std::iter::repeat_n(byte, 64).collect()
    }

    fn measurement(elapsed_seconds: f64, work_count: u64, source: char) -> WorkerMeasurement {
        WorkerMeasurement {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            implementation: Implementation::Stab,
            evidence_mode: EvidenceMode::Timing,
            timing_boundary: RAW_WORK_TIMING_BOUNDARY,
            workload_id: ProtocolId::try_new("simd-bits-xor").expect("workload"),
            measurement_id: ProtocolId::try_new("xor-complete-vector").expect("measurement"),
            iteration_count: 1,
            elapsed_seconds,
            work_count,
            input_bytes: 8,
            input_digest: InputDigest::try_new(digest('a')).expect("input digest"),
            output_digest: SemanticDigest::try_new(digest('d')).expect("output digest"),
            setup_rss_bytes: None,
            peak_rss_bytes: None,
            affinity_cpu: Some(0),
            stim_commit: GitCommit::try_new(STIM_COMMIT).expect("Stim commit"),
            source_digest: Sha256Digest::try_new(digest(source)).expect("source digest"),
            build_fingerprint: Sha256Digest::try_new(digest(if source == 'b' { 'e' } else { 'f' }))
                .expect("build fingerprint"),
        }
    }

    fn invocation(
        elapsed_seconds: f64,
        work_count: u64,
        source: char,
    ) -> super::super::super::invocation::InvocationRecord {
        super::super::super::invocation::InvocationRecord {
            implementation: Implementation::Stab,
            evidence_mode: EvidenceMode::Timing,
            process_wall_seconds: elapsed_seconds + 0.01,
            parent_observed_peak_rss_bytes: None,
            rows: vec![measurement(elapsed_seconds, work_count, source)],
        }
    }

    fn pair(pair_index: usize) -> VariantPairExecution {
        VariantPairExecution {
            pair_index,
            order: super::super::VariantPairOrder::for_pair(pair_index),
            scalar: invocation(2.0 + pair_index as f64 * 0.1, 8, 'b'),
            portable: invocation(1.0 + pair_index as f64 * 0.1, 8, 'c'),
        }
    }

    fn scale_evidence() -> ScaleEvidence {
        let samples = (0..3).map(pair).collect::<Vec<_>>();
        let paired_samples = samples
            .iter()
            .map(pair_sample)
            .collect::<Result<Vec<_>, _>>()
            .expect("paired samples");
        let summary = summarize(&paired_samples).expect("summary");
        let calibration = |variant| VariantCalibration {
            variant,
            selected_iterations: 1,
            selected_measured_seconds: 0.5,
            probes: Vec::new(),
        };
        ScaleEvidence {
            scale_id: "medium".to_string(),
            family_id: "default".to_string(),
            size_class: SizeClass::Medium,
            work_items: 8,
            input_bytes: 8,
            input_digest: digest('a'),
            scalar_calibration: calibration(StabBuildVariant::Scalar),
            portable_calibration: calibration(StabBuildVariant::PortableSimd),
            common_iterations: 1,
            semantic_validation: pair(0),
            warmups: Vec::new(),
            samples,
            paired_samples,
            summary,
        }
    }

    fn contract() -> ResolvedGroupContract {
        ResolvedGroupContract {
            source_sha256: digest('9'),
            product_diagnostic_suite_timeout_seconds: NonZeroU64::new(1).expect("timeout"),
            contract: GroupContract {
                id: ProtocolId::try_new(GROUP_IDS[0]).expect("group"),
                claim_class: ClaimClass::PromotablePerformance,
                parity_eligibility: ParityEligibility::ThresholdEligible,
                timing_batch_policy: TimingBatchPolicy::CommonIterations,
                workload_id: ProtocolId::try_new("simd-bits-xor").expect("workload"),
                measurement_ids: vec![
                    ProtocolId::try_new("xor-complete-vector").expect("measurement"),
                ],
                scales: vec![ScaleContract {
                    id: ProtocolId::try_new("medium").expect("scale"),
                    family_id: ProtocolId::try_new("default").expect("family"),
                    size_class: SizeClass::Medium,
                    work_items: NonZeroU64::new(8).expect("work"),
                    input_bytes: 8,
                    input_digest: InputDigest::try_new(digest('a')).expect("input digest"),
                }],
                correctness_case_ids: Vec::new(),
                owner: ProtocolId::try_new("stab-bits/bit-vector").expect("owner"),
                profiler_note: None,
                comparator_sources: Vec::new(),
            },
        }
    }

    fn group_evidence() -> GroupEvidence {
        GroupEvidence {
            group_id: GROUP_IDS[0].to_string(),
            group_contract_sha256: digest('9'),
            workload_id: "simd-bits-xor".to_string(),
            measurement_id: "xor-complete-vector".to_string(),
            owner: "stab-bits/bit-vector".to_string(),
            scales: vec![scale_evidence()],
        }
    }

    #[test]
    fn deterministic_replay_reconstructs_samples_and_summary() {
        let scale = scale_evidence();
        reconstruct_scale_derivations(&scale).expect("first replay");
        reconstruct_scale_derivations(&scale).expect("deterministic second replay");
    }

    #[test]
    fn replay_rejects_scalar_and_portable_raw_timing_tampering() {
        let mut scalar = scale_evidence();
        scalar
            .samples
            .first_mut()
            .and_then(|pair| pair.scalar.rows.first_mut())
            .expect("scalar raw row")
            .elapsed_seconds *= 2.0;
        assert!(matches!(
            reconstruct_scale_derivations(&scalar),
            Err(SimdCompareError::DerivedSamples)
        ));

        let mut portable = scale_evidence();
        portable
            .samples
            .get_mut(1)
            .and_then(|pair| pair.portable.rows.first_mut())
            .expect("portable raw row")
            .elapsed_seconds *= 2.0;
        assert!(matches!(
            reconstruct_scale_derivations(&portable),
            Err(SimdCompareError::DerivedSamples)
        ));
    }

    #[test]
    fn replay_rejects_raw_work_and_derived_sample_tampering() {
        let mut raw = scale_evidence();
        let pair = raw.samples.first_mut().expect("raw pair");
        pair.scalar
            .rows
            .first_mut()
            .expect("scalar raw row")
            .work_count *= 2;
        pair.portable
            .rows
            .first_mut()
            .expect("portable raw row")
            .work_count *= 2;
        assert!(matches!(
            reconstruct_scale_derivations(&raw),
            Err(SimdCompareError::DerivedSamples)
        ));

        let mut derived = scale_evidence();
        derived
            .paired_samples
            .first_mut()
            .expect("derived sample")
            .portable_over_scalar_ratio *= 2.0;
        assert!(matches!(
            reconstruct_scale_derivations(&derived),
            Err(SimdCompareError::DerivedSamples)
        ));
    }

    #[test]
    fn replay_rejects_summary_tampering() {
        let mut scale = scale_evidence();
        scale.summary.confidence_interval_upper *= 2.0;
        assert!(matches!(
            reconstruct_scale_derivations(&scale),
            Err(SimdCompareError::SummaryEvidence)
        ));
    }

    #[test]
    fn replay_rejects_build_variant_mismatch() {
        assert!(worker_variants_match(
            StabBuildVariant::Scalar,
            StabBuildVariant::Scalar,
            StabBuildVariant::PortableSimd,
            StabBuildVariant::PortableSimd,
        ));
        assert!(!worker_variants_match(
            StabBuildVariant::Scalar,
            StabBuildVariant::PortableSimd,
            StabBuildVariant::PortableSimd,
            StabBuildVariant::PortableSimd,
        ));
    }

    #[test]
    fn replay_rejects_group_and_scale_contract_mismatch() {
        let resolved = contract();
        let mut evidence = group_evidence();
        assert!(validate_group_binding(&evidence, &resolved, GROUP_IDS[0]).is_err());

        evidence.scales.push({
            let mut large = scale_evidence();
            large.scale_id = "large".to_string();
            large.size_class = SizeClass::Large;
            large
        });
        let mut complete = contract();
        complete.contract.scales.push(ScaleContract {
            id: ProtocolId::try_new("large").expect("scale"),
            family_id: ProtocolId::try_new("default").expect("family"),
            size_class: SizeClass::Large,
            work_items: NonZeroU64::new(8).expect("work"),
            input_bytes: 8,
            input_digest: InputDigest::try_new(digest('a')).expect("input digest"),
        });
        validate_group_binding(&evidence, &complete, GROUP_IDS[0]).expect("matching contract");

        evidence.group_contract_sha256 = digest('8');
        assert!(matches!(
            validate_group_binding(&evidence, &complete, GROUP_IDS[0]),
            Err(SimdCompareError::GroupEvidence)
        ));
        evidence.group_contract_sha256 = digest('9');
        evidence.scales.get_mut(1).expect("large scale").input_bytes += 1;
        assert!(matches!(
            validate_group_binding(&evidence, &complete, GROUP_IDS[0]),
            Err(SimdCompareError::GroupEvidence)
        ));
    }
}
