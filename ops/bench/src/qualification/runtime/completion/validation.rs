use std::collections::BTreeSet;
use std::path::Path;

use super::model::{
    CompletionReceipt, CompletionStep, CompletionStepKind, CompletionStepResult,
    EvidenceDirectoryReceipt,
};
use super::{CompletionError, probe_arguments};
use crate::qualification::runtime::run::QualificationTier;
use crate::qualification::runtime::statistics::GateOutcome;

pub(super) fn validate(receipt: &CompletionReceipt) -> Result<(), CompletionError> {
    validate_environment(receipt)?;
    let (full_sources, soak_sources) = validate_directories(receipt)?;
    let expected_steps = receipt
        .source_reports
        .len()
        .checked_mul(2)
        .and_then(|count| count.checked_add(4))
        .ok_or(CompletionError::Boundary)?;
    if receipt.steps.len() != expected_steps {
        return Err(CompletionError::Boundary);
    }
    for (index, step) in receipt.steps.iter().enumerate() {
        if step.index != index
            || step.exit_status != 0
            || step.repository_commit != receipt.repository.commit_after
            || step.canonical_arguments.is_empty()
        {
            return Err(CompletionError::Boundary);
        }
    }

    let mut cursor = 0;
    let worker = next_step(&receipt.steps, &mut cursor)?;
    if worker.kind != CompletionStepKind::WorkerReproducibility
        || worker.canonical_arguments != ["qualification-worker-reproducibility"]
        || !worker.inputs.is_empty()
        || !worker.outputs.is_empty()
        || !matches!(
            &worker.result,
            CompletionStepResult::WorkerReproducibility { workers }
                if workers == &receipt.workers
        )
    {
        return Err(CompletionError::Boundary);
    }

    let probe = next_step(&receipt.steps, &mut cursor)?;
    let CompletionStepResult::AdapterProbe {
        probe: probe_result,
    } = &probe.result
    else {
        return Err(CompletionError::Boundary);
    };
    if probe.kind != CompletionStepKind::AdapterProbe
        || probe.canonical_arguments != probe_arguments(probe_result)
        || probe_result.runtime_group_id != receipt.group_id
        || !probe.inputs.is_empty()
        || !probe.outputs.is_empty()
    {
        return Err(CompletionError::Boundary);
    }

    for source in &receipt.source_reports {
        validate_source_steps(receipt, source, &mut cursor)?;
    }
    for rollup in &receipt.rollups {
        validate_rollup_step(receipt, rollup, &mut cursor)?;
    }
    if cursor != receipt.steps.len() || full_sources != soak_sources {
        return Err(CompletionError::Boundary);
    }
    Ok(())
}

fn validate_environment(receipt: &CompletionReceipt) -> Result<(), CompletionError> {
    let environment = &receipt.environment;
    if !valid_sha256(&environment.host_policy_sha256)
        || environment.host_profile_id.is_empty()
        || environment.architecture.is_empty()
        || environment.cpu_identity.is_empty()
        || environment.cpu_identity.len() > 1024
        || !environment.cpu_identity.is_ascii()
        || environment.target_triple.is_empty()
        || !valid_sha256(&environment.toolchain_sha256)
    {
        return Err(CompletionError::Boundary);
    }
    Ok(())
}

fn validate_directories(
    receipt: &CompletionReceipt,
) -> Result<(Vec<String>, Vec<String>), CompletionError> {
    if receipt.source_reports.is_empty() || receipt.rollups.len() != 2 {
        return Err(CompletionError::Boundary);
    }
    let mut paths = BTreeSet::new();
    let mut full = Vec::new();
    let mut soak = Vec::new();
    let mut saw_soak = false;
    for source in &receipt.source_reports {
        validate_directory(source, true, &mut paths)?;
        let scale = source.scale_id.clone().ok_or(CompletionError::Boundary)?;
        match source.tier {
            QualificationTier::Full if !saw_soak => full.push(scale),
            QualificationTier::Soak => {
                saw_soak = true;
                soak.push(scale);
            }
            QualificationTier::Pr | QualificationTier::Full => {
                return Err(CompletionError::Boundary);
            }
        }
    }
    let [full_rollup, soak_rollup] = receipt.rollups.as_slice() else {
        return Err(CompletionError::Boundary);
    };
    if full_rollup.tier != QualificationTier::Full || soak_rollup.tier != QualificationTier::Soak {
        return Err(CompletionError::Boundary);
    }
    for rollup in &receipt.rollups {
        validate_directory(rollup, false, &mut paths)?;
    }
    if paths.contains(&receipt.output) || full.is_empty() {
        return Err(CompletionError::Boundary);
    }
    Ok((full, soak))
}

fn validate_directory(
    directory: &EvidenceDirectoryReceipt,
    requires_scale: bool,
    paths: &mut BTreeSet<String>,
) -> Result<(), CompletionError> {
    if directory.path.is_empty()
        || !paths.insert(directory.path.clone())
        || directory.scale_id.is_some() != requires_scale
        || directory.artifacts.len() != 3
    {
        return Err(CompletionError::Boundary);
    }
    let expected_names = ["report.json", "preflight.json", "report.md"];
    for (artifact, expected_name) in directory.artifacts.iter().zip(expected_names) {
        if artifact.path != directory.path
            || artifact.name != expected_name
            || artifact.bytes == 0
            || !valid_sha256(&artifact.sha256)
        {
            return Err(CompletionError::Boundary);
        }
    }
    super::super::artifact::DirectQualificationArtifactPath::try_new(Path::new(&directory.path))?;
    Ok(())
}

fn validate_source_steps(
    receipt: &CompletionReceipt,
    source: &EvidenceDirectoryReceipt,
    cursor: &mut usize,
) -> Result<(), CompletionError> {
    let scale_id = source.scale_id.as_ref().ok_or(CompletionError::Boundary)?;
    let report = next_step(&receipt.steps, cursor)?;
    if report.kind != CompletionStepKind::ReportReplay
        || report.canonical_arguments != ["qualification-report", "--input", source.path.as_str()]
        || report.inputs != source.artifacts
        || report.outputs != source.artifacts
        || !matches!(
            &report.result,
            CompletionStepResult::ReportReplay { tier, scale_id: actual }
                if tier == &source.tier && actual == scale_id
        )
    {
        return Err(CompletionError::Boundary);
    }
    let regression = next_step(&receipt.steps, cursor)?;
    if regression.kind != CompletionStepKind::Regression
        || regression.canonical_arguments
            != [
                "qualification-regression",
                "--input",
                source.path.as_str(),
                "--baseline",
                super::super::regression::DEFAULT_BASELINE,
            ]
        || regression.inputs != source.artifacts
        || !regression.outputs.is_empty()
        || !matches!(
            &regression.result,
            CompletionStepResult::Regression {
                group_id,
                checked_measurements,
                report_only,
            } if group_id == &receipt.group_id && *checked_measurements > 0 && !report_only
        )
    {
        return Err(CompletionError::Boundary);
    }
    Ok(())
}

fn validate_rollup_step(
    receipt: &CompletionReceipt,
    rollup: &EvidenceDirectoryReceipt,
    cursor: &mut usize,
) -> Result<(), CompletionError> {
    let step = next_step(&receipt.steps, cursor)?;
    let source_count = receipt
        .source_reports
        .iter()
        .filter(|source| source.tier == rollup.tier)
        .count();
    if step.kind != CompletionStepKind::RollupReplay
        || step.canonical_arguments
            != [
                "qualification-rollup-report",
                "--input",
                rollup.path.as_str(),
            ]
        || step.inputs != rollup.artifacts
        || step.outputs != rollup.artifacts
        || !matches!(
            &step.result,
            CompletionStepResult::RollupReplay {
                tier,
                scale_count,
                overall_outcome,
            } if tier == &rollup.tier
                && *scale_count == source_count
                && *overall_outcome == GateOutcome::Passed
        )
    {
        return Err(CompletionError::Boundary);
    }
    Ok(())
}

fn next_step<'a>(
    steps: &'a [CompletionStep],
    cursor: &mut usize,
) -> Result<&'a CompletionStep, CompletionError> {
    let step = steps.get(*cursor).ok_or(CompletionError::Boundary)?;
    *cursor = cursor.checked_add(1).ok_or(CompletionError::Boundary)?;
    Ok(step)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
