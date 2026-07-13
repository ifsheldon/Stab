use std::ffi::{OsStr, OsString};
use std::time::Duration;

use super::{
    ExecutionEvidence, StatisticalAttemptContract, StatisticalCompletion, failure, oracle_failure,
    process_failure, success,
};
use crate::RepoRoot;
use crate::blocker_ledger::selector::CargoTestSelector;
use crate::qualification::executables::QualificationExecutables;
use crate::qualification::model::EvidenceCase;

pub(super) fn execute_cargo(
    root: &RepoRoot,
    executables: &QualificationExecutables,
    selector: &[String],
    timeout_ms: u64,
    statistical_contract: Option<&StatisticalAttemptContract>,
) -> ExecutionEvidence {
    let parsed = match CargoTestSelector::parse(selector) {
        Ok(parsed) if parsed.is_exact() => parsed,
        Ok(_) => return failure("Cargo primary selector is not exact"),
        Err(reason) => return failure(reason),
    };
    let mut args = parsed
        .run_args()
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let separator = args
        .iter()
        .position(|argument| argument == OsStr::new("--"))
        .ok_or_else(|| failure("Cargo primary selector omitted its libtest separator"));
    let separator = match separator {
        Ok(separator) => separator,
        Err(execution) => return execution,
    };
    args.splice(
        separator..separator,
        [
            OsString::from("--manifest-path"),
            root.path.join("Cargo.toml").into_os_string(),
        ],
    );
    if statistical_contract.is_some() {
        args.push(OsString::from("--nocapture"));
    }
    let output = crate::process::run_qualification_process_with_timeout(
        &executables.cargo(),
        args,
        &[],
        Some(&executables.cargo_working_dir()),
        Duration::from_millis(timeout_ms),
        executables.environment(),
    );
    if let Err(source) = executables.verify_support() {
        return failure(&format!(
            "qualification support verification failed after Cargo execution: {source}"
        ));
    }
    let output = match output {
        Ok(output) => output,
        Err(source) => return oracle_failure(source),
    };
    let exact_test_count = crate::fixtures::cargo_test_passed_test_count(&output);
    if !output.success() {
        let mut execution = process_failure(&output, "Cargo selector failed");
        execution.exact_test_count = Some(exact_test_count);
        return execution;
    }
    if exact_test_count != 1 {
        let mut execution = process_failure(
            &output,
            &format!("exact Cargo selector passed {exact_test_count} tests instead of one"),
        );
        execution.exact_test_count = Some(exact_test_count);
        return execution;
    }
    success(output, Some(exact_test_count))
}

pub(super) fn statistical_completion_from_output(
    execution: &ExecutionEvidence,
    contract: &StatisticalAttemptContract,
    expected_seed: u64,
) -> Result<StatisticalCompletion, StatisticalCompletionParseError> {
    let Some(stdout) = execution.stdout.as_deref() else {
        return Ok(StatisticalCompletion::none());
    };
    let mut completion = StatisticalCompletion::none();
    let text = std::str::from_utf8(stdout).map_err(|error| {
        completion_parse_error(
            completion,
            format!("statistical Cargo stdout is not UTF-8: {error}"),
        )
    })?;
    for line in text.lines() {
        if !line.starts_with("STAB_CQ1_STATISTICAL") {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        let [prefix, version, plan_id, seed, comparison, shots] = fields.as_slice() else {
            return Err(completion_parse_error(
                completion,
                format!("malformed statistical completion marker {line:?}"),
            ));
        };
        if *prefix != "STAB_CQ1_STATISTICAL" || *version != "1" || *plan_id != contract.plan_id {
            return Err(completion_parse_error(
                completion,
                format!("unexpected statistical completion marker {line:?}"),
            ));
        }
        let seed = seed.parse::<u64>().map_err(|error| {
            completion_parse_error(
                completion,
                format!("invalid statistical completion seed: {error}"),
            )
        })?;
        let comparison = comparison.parse::<u32>().map_err(|error| {
            completion_parse_error(
                completion,
                format!("invalid statistical completion comparison: {error}"),
            )
        })?;
        let shots = shots.parse::<u64>().map_err(|error| {
            completion_parse_error(
                completion,
                format!("invalid statistical completion shot count: {error}"),
            )
        })?;
        if seed != expected_seed || comparison != completion.comparisons {
            return Err(completion_parse_error(
                completion,
                format!(
                    "statistical completion marker has seed {seed} comparison {comparison}, expected seed {expected_seed} comparison {}",
                    completion.comparisons
                ),
            ));
        }
        let expected_shots = contract
            .shots_per_batch
            .checked_mul(u64::from(contract.batches_per_comparison))
            .ok_or_else(|| {
                completion_parse_error(
                    completion,
                    "statistical shots per comparison overflowed u64",
                )
            })?;
        if shots != expected_shots {
            return Err(completion_parse_error(
                completion,
                format!(
                    "statistical completion marker reports {shots} shots instead of the frozen {expected_shots} shots per comparison"
                ),
            ));
        }
        completion.shots = completion.shots.checked_add(shots).ok_or_else(|| {
            completion_parse_error(completion, "statistical completed shot count overflowed")
        })?;
        completion.comparisons = completion.comparisons.checked_add(1).ok_or_else(|| {
            completion_parse_error(
                completion,
                "statistical completed comparison count overflowed",
            )
        })?;
        completion.batches = completion
            .batches
            .checked_add(contract.batches_per_comparison)
            .ok_or_else(|| {
                completion_parse_error(completion, "statistical completed batch count overflowed")
            })?;
    }
    Ok(completion)
}

pub(super) struct StatisticalCompletionParseError {
    pub(super) completion: StatisticalCompletion,
    pub(super) reason: String,
}

fn completion_parse_error(
    completion: StatisticalCompletion,
    reason: impl Into<String>,
) -> StatisticalCompletionParseError {
    StatisticalCompletionParseError {
        completion,
        reason: reason.into(),
    }
}

pub(super) fn execute_property_target(
    root: &RepoRoot,
    executables: &QualificationExecutables,
    case: &EvidenceCase,
) -> ExecutionEvidence {
    let [id] = case.primary_selector.value.as_slice() else {
        return failure("property selector must contain one registered target id");
    };
    if !super::super::property::is_registered_target(id) {
        return failure(&format!("property target {id:?} is not registered"));
    }
    let Some(plan) = case
        .property_plan
        .as_ref()
        .and_then(|reference| reference.plan.as_ref())
    else {
        return failure("property target has no executable manifest plan");
    };
    if !super::super::property::registered_execution_plan_matches(id, plan) {
        return failure("property target manifest plan disagrees with its registered contract");
    }
    let plan_digest = match super::super::property::registered_execution_plan_digest(id) {
        Ok(digest) => digest,
        Err(source) => return failure(&source),
    };
    let channel_parent = root.path.join("target/qualification/property-workers");
    if let Err(source) = crate::safe_file::create_directory_all(&channel_parent) {
        return failure(&format!(
            "failed to prepare property persistence channel: {source}"
        ));
    }
    let channel = match tempfile::Builder::new()
        .prefix(".stab-property-")
        .tempdir_in(&channel_parent)
    {
        Ok(channel) => channel,
        Err(source) => {
            return failure(&format!(
                "failed to reserve property persistence channel: {source}"
            ));
        }
    };
    let persistence_path = channel.path().join("regression.case");
    let args = worker_args(
        root,
        id,
        &plan_digest,
        "--persistence-out",
        &persistence_path,
    );
    let output = match crate::process::run_qualification_process_with_timeout(
        &executables.worker(),
        args,
        &[],
        Some(&root.path),
        Duration::from_millis(case.execution.timeout_ms),
        executables.environment(),
    ) {
        Ok(output) => output,
        Err(source) => return oracle_failure(source),
    };
    if output.success() {
        return success(output, None);
    }

    let mut execution = process_failure(&output, "property worker failed");
    let persistence = match crate::safe_file::read_regular_file_bounded(
        &persistence_path,
        super::super::property::MAX_TARGET_PERSISTENCE_BYTES,
    ) {
        Ok(bytes) => bytes,
        Err(source) => {
            execution.failure = failure(&format!(
                "property worker failed without bounded persistence evidence: {source}"
            ))
            .failure;
            return execution;
        }
    };
    let replay_args = worker_args(root, id, &plan_digest, "--replay-in", &persistence_path);
    match crate::process::run_qualification_process_with_timeout(
        &executables.worker(),
        replay_args,
        &[],
        Some(&root.path),
        Duration::from_millis(case.execution.timeout_ms),
        executables.environment(),
    ) {
        Ok(replay) if replay.success() => {
            execution.property_regression = Some(persistence);
        }
        Ok(replay) => {
            execution.failure = process_failure(
                &replay,
                "property regression failed to replay in its killable worker",
            )
            .failure;
        }
        Err(source) => {
            execution.failure = oracle_failure(source).failure;
        }
    }
    execution
}

fn worker_args(
    root: &RepoRoot,
    id: &str,
    plan_digest: &str,
    channel_option: &str,
    channel_path: &std::path::Path,
) -> Vec<OsString> {
    vec![
        OsString::from("--root"),
        root.path.as_os_str().to_owned(),
        OsString::from("qualification"),
        OsString::from("correctness"),
        OsString::from("worker"),
        OsString::from("property"),
        OsString::from("--id"),
        OsString::from(id),
        OsString::from("--plan-sha256"),
        OsString::from(plan_digest),
        OsString::from(channel_option),
        channel_path.as_os_str().to_owned(),
    ]
}
