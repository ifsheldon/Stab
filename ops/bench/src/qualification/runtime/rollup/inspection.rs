use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{
    AssemblyContext, DirectQualificationArtifactPath, GroupContract, LoadedCandidate,
    MAX_ROLLUP_MARKDOWN_BYTES, MAX_ROLLUP_PREFLIGHT_BYTES, MAX_ROLLUP_REPORT_BYTES,
    RepositoryBinding, RollupError, RollupRegressionMeasurement, RollupRegressionScale,
    RollupReplayEvidence, RollupReport, RollupSourceEvidence, assemble, collect_input_paths,
    expected_stab_commit, load_candidates, parse_existing_rollup, preflight, render_json,
    render_markdown, require_current_correctness, require_reconstruction, sha256_hex,
};
use crate::qualification::runtime::protocol::RAW_WORK_TIMING_BOUNDARY;
use crate::qualification::runtime::run::RepositoryEvidence;
use crate::root::RepoRoot;

pub(in crate::qualification::runtime) fn inspect_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    live_repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    output_path: DirectQualificationArtifactPath,
) -> Result<RollupReplayEvidence, RollupError> {
    let repository_before = super::super::run::bound_repository_state(root, live_repository)?;
    super::require_clean_repository(&repository_before)?;
    let existing_report_json = read_artifact(
        root,
        live_repository,
        &output_path,
        "report.json",
        MAX_ROLLUP_REPORT_BYTES,
    )?;
    let existing_preflight_json = read_artifact(
        root,
        live_repository,
        &output_path,
        "preflight.json",
        MAX_ROLLUP_PREFLIGHT_BYTES,
    )?;
    let existing_markdown = read_artifact(
        root,
        live_repository,
        &output_path,
        "report.md",
        MAX_ROLLUP_MARKDOWN_BYTES,
    )?;
    let existing_report = parse_existing_rollup(
        &existing_report_json,
        &existing_preflight_json,
        &output_path,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?;
    let resolved = super::super::group::load_group(
        source_root,
        expected_performance_inventory_sha256,
        &existing_report.group_id,
    )?;
    if existing_report.scales.len() != resolved.contract.scales.len() {
        return Err(RollupError::InputCount {
            actual: existing_report.scales.len(),
            expected: resolved.contract.scales.len(),
        });
    }
    let input_paths = collect_input_paths(
        existing_report
            .scales
            .iter()
            .map(|scale| Path::new(&scale.source.path)),
        &output_path,
    )?;
    let loaded = load_candidates(
        root,
        source_root,
        live_repository,
        &input_paths,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?;
    let expected_stab_commit = expected_stab_commit(&loaded)?;
    let producer_repository =
        validate_recorded_producer(&existing_report.producer_repository, &expected_stab_commit)?;
    let reconstructed = assemble(
        AssemblyContext {
            contract: &resolved.contract,
            group_contract_sha256: &resolved.source_sha256,
            expected_performance_inventory_sha256,
            expected_correctness_inventory_sha256,
            tier: existing_report.tier,
            output_path: &output_path,
            producer_repository,
        },
        loaded
            .iter()
            .map(|evidence| evidence.candidate.clone())
            .collect(),
    )?;
    let report_json = require_reconstruction(&existing_report_json, &reconstructed)?;
    let preflight_json = render_json(&preflight(&reconstructed, &report_json))?;
    if preflight_json != existing_preflight_json {
        return Err(RollupError::PreflightBinding);
    }
    let markdown = render_markdown(&reconstructed, &sha256_hex(&report_json));
    if markdown.as_bytes() != existing_markdown {
        return Err(RollupError::Reconstruction);
    }

    require_stable_artifact(
        root,
        live_repository,
        &output_path,
        "report.json",
        MAX_ROLLUP_REPORT_BYTES,
        &existing_report_json,
    )?;
    require_stable_artifact(
        root,
        live_repository,
        &output_path,
        "preflight.json",
        MAX_ROLLUP_PREFLIGHT_BYTES,
        &existing_preflight_json,
    )?;
    require_stable_artifact(
        root,
        live_repository,
        &output_path,
        "report.md",
        MAX_ROLLUP_MARKDOWN_BYTES,
        &existing_markdown,
    )?;
    require_stable_sources(root, live_repository, &loaded)?;
    require_current_correctness(&loaded)?;
    live_repository.require_current(root)?;
    let repository_after = super::super::run::bound_repository_state(root, live_repository)?;
    let current_commit = repository_before.commit.clone();
    super::bind_producer_repository(repository_before, repository_after, &current_commit)?;

    build_replay_evidence(
        output_path,
        reconstructed,
        &resolved.contract,
        &loaded,
        &report_json,
        &preflight_json,
        markdown.as_bytes(),
    )
}

pub(super) fn validate_recorded_producer(
    producer: &RepositoryEvidence,
    expected_stab_commit: &str,
) -> Result<RepositoryEvidence, RollupError> {
    if producer.local_modifications_before || producer.local_modifications_after {
        return Err(RollupError::DirtyProducer);
    }
    if producer.commit_before != producer.commit_after {
        return Err(RollupError::RepositoryChanged {
            before: producer.commit_before.clone(),
            after: producer.commit_after.clone(),
        });
    }
    if producer.commit_before != expected_stab_commit {
        return Err(RollupError::ProducerCommit {
            actual: producer.commit_before.clone(),
            expected: expected_stab_commit.to_string(),
        });
    }
    Ok(producer.clone())
}

fn require_stable_sources(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    loaded: &[LoadedCandidate],
) -> Result<(), RollupError> {
    for evidence in loaded {
        require_stable_digest(
            root,
            repository,
            &evidence.path,
            "report.json",
            super::super::report::MAX_PUBLISHED_REPORT_BYTES,
            &evidence.report_sha256,
        )?;
        require_stable_digest(
            root,
            repository,
            &evidence.path,
            "preflight.json",
            super::super::report::MAX_PUBLISHED_PREFLIGHT_BYTES,
            &evidence.preflight_sha256,
        )?;
        require_stable_digest(
            root,
            repository,
            &evidence.path,
            "report.md",
            super::super::report::MAX_PUBLISHED_MARKDOWN_BYTES,
            &evidence.markdown_sha256,
        )?;
    }
    Ok(())
}

fn require_stable_digest(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    path: &DirectQualificationArtifactPath,
    name: &'static str,
    maximum_bytes: usize,
    expected_sha256: &str,
) -> Result<(), RollupError> {
    let bytes = read_artifact(root, repository, path, name, maximum_bytes)?;
    if sha256_hex(&bytes) != expected_sha256 {
        return Err(RollupError::Reconstruction);
    }
    Ok(())
}

fn require_stable_artifact(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    path: &DirectQualificationArtifactPath,
    name: &'static str,
    maximum_bytes: usize,
    expected: &[u8],
) -> Result<(), RollupError> {
    if read_artifact(root, repository, path, name, maximum_bytes)? != expected {
        return Err(RollupError::Reconstruction);
    }
    Ok(())
}

fn read_artifact(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    path: &DirectQualificationArtifactPath,
    name: &'static str,
    maximum_bytes: usize,
) -> Result<Vec<u8>, RollupError> {
    Ok(
        super::super::artifact::read_artifact_bounded_with_repository(
            root,
            repository,
            path,
            name,
            maximum_bytes,
        )?,
    )
}

pub(super) fn build_replay_evidence(
    output_path: DirectQualificationArtifactPath,
    reconstructed: RollupReport,
    contract: &GroupContract,
    loaded: &[LoadedCandidate],
    report_json: &[u8],
    preflight_json: &[u8],
    markdown: &[u8],
) -> Result<RollupReplayEvidence, RollupError> {
    let regression_scales = reconstructed
        .scales
        .iter()
        .map(|scale| RollupRegressionScale {
            scale_id: scale.scale_id.clone(),
            family_id: scale.family_id.clone(),
            size_class: scale.size_class,
            work_items: scale.work_items,
            input_digest: scale.input_digest.clone(),
            measurements: scale
                .measurements
                .iter()
                .map(|measurement| RollupRegressionMeasurement {
                    measurement_id: measurement.measurement_id.clone(),
                    median_ratio: measurement.median_ratio,
                    confidence_interval_upper: measurement.confidence_interval_upper,
                    outcome: measurement.outcome,
                })
                .collect(),
            memory: scale.memory.clone(),
        })
        .collect();
    let comparator_sources = contract
        .comparator_sources
        .iter()
        .map(|source| {
            (
                source.path.as_str().to_string(),
                source.sha256.as_str().to_string(),
            )
        })
        .collect();
    Ok(RollupReplayEvidence {
        output: output_path.into_path_buf(),
        report_sha256: sha256_hex(report_json),
        preflight_sha256: sha256_hex(preflight_json),
        markdown_sha256: sha256_hex(markdown),
        group_id: reconstructed.group_id,
        group_contract_sha256: reconstructed.group_contract_sha256,
        tier: reconstructed.tier,
        performance_inventory_sha256: reconstructed.performance_inventory_sha256,
        stab_commit: reconstructed.stab_commit,
        stim_commit: reconstructed.stim_commit,
        host_policy_sha256: reconstructed.host_policy_sha256,
        host_profile_id: reconstructed.host_profile_id,
        operating_system: reconstructed.operating_system,
        architecture: reconstructed.architecture,
        cpu_identity: reconstructed.cpu_identity,
        rust_toolchain: reconstructed.rust_toolchain,
        target_triple: reconstructed.target_triple,
        toolchain_sha256: reconstructed.toolchain_sha256,
        timing_boundary: RAW_WORK_TIMING_BOUNDARY,
        workload_id: contract.workload_id.to_string(),
        timing_batch_policy: contract.timing_batch_policy,
        comparator_sources,
        workers: reconstructed.workers,
        correctness_preflight: reconstructed.correctness_preflight,
        correctness_bindings: loaded
            .iter()
            .map(|candidate| Arc::clone(&candidate.correctness_binding))
            .collect(),
        overall_outcome: reconstructed.overall_outcome,
        scales: regression_scales,
        sources: reconstructed
            .scales
            .into_iter()
            .map(|scale| {
                let markdown_sha256 = loaded
                    .iter()
                    .find(|candidate| candidate.candidate.scale_id == scale.scale_id)
                    .map(|candidate| candidate.markdown_sha256.clone())
                    .ok_or_else(|| RollupError::ScaleContract(scale.scale_id.clone()))?;
                Ok(RollupSourceEvidence {
                    scale_id: scale.scale_id,
                    path: PathBuf::from(scale.source.path),
                    report_sha256: scale.source.report_sha256,
                    preflight_sha256: scale.source.preflight_sha256,
                    markdown_sha256,
                })
            })
            .collect::<Result<Vec<_>, RollupError>>()?,
    })
}
