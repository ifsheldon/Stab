use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use super::{
    ExecutionEvidence, MAX_FAILURE_REASON_ARTIFACT_BYTES, RunError, StatisticalAttemptEvidence,
};
use crate::qualification::artifact::QualificationOutputDir;
use crate::qualification::model::EvidenceCase;
use crate::qualification::receipt::{
    ExecutableIdentity, ExecutionReceiptInput, ExecutionVerdict, ReceiptArtifact,
    StatisticalAttemptReceipt, StatisticalAttemptVerdict, StreamReceipt, new_execution_receipt,
};
use crate::qualification::report::{ArtifactRecord, CaseOutcome, CaseResult};

pub(super) fn success(
    output: crate::ProcessOutput,
    exact_test_count: Option<usize>,
) -> ExecutionEvidence {
    let stdout_bytes = output.stdout.bytes.len();
    let stderr_bytes = output.stderr.bytes.len();
    ExecutionEvidence {
        outcome: CaseOutcome::Passed,
        completed_execution: true,
        exit_status: output.status,
        exact_test_count,
        stdout: Some(output.stdout.bytes),
        stderr: Some(output.stderr.bytes),
        stdout_bytes: Some(stdout_bytes),
        stderr_bytes: Some(stderr_bytes),
        stdout_digest: None,
        stderr_digest: None,
        failure: None,
        property_regression: None,
        statistical_attempts: Vec::new(),
    }
}

pub(super) fn success_with_digests(
    stdout_digest: String,
    stderr_digest: String,
    stdout_bytes: usize,
    stderr_bytes: usize,
    exit_status: Option<i32>,
) -> ExecutionEvidence {
    ExecutionEvidence {
        outcome: CaseOutcome::Passed,
        completed_execution: true,
        exit_status,
        exact_test_count: None,
        stdout: None,
        stderr: None,
        stdout_bytes: Some(stdout_bytes),
        stderr_bytes: Some(stderr_bytes),
        stdout_digest: Some(stdout_digest),
        stderr_digest: Some(stderr_digest),
        failure: None,
        property_regression: None,
        statistical_attempts: Vec::new(),
    }
}

pub(super) fn failure(reason: &str) -> ExecutionEvidence {
    let bytes = reason
        .as_bytes()
        .get(..reason.len().min(MAX_FAILURE_REASON_ARTIFACT_BYTES))
        .unwrap_or_default()
        .to_vec();
    ExecutionEvidence {
        outcome: CaseOutcome::Failed,
        completed_execution: false,
        exit_status: None,
        exact_test_count: None,
        stdout: None,
        stderr: None,
        stdout_bytes: None,
        stderr_bytes: None,
        stdout_digest: None,
        stderr_digest: None,
        failure: Some(bytes),
        property_regression: None,
        statistical_attempts: Vec::new(),
    }
}

pub(super) fn failure_with_parts(
    reason: impl AsRef<str>,
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
    statistical_attempts: Vec<StatisticalAttemptEvidence>,
) -> ExecutionEvidence {
    let mut execution = failure(reason.as_ref());
    execution.stdout_bytes = stdout.as_ref().map(Vec::len);
    execution.stderr_bytes = stderr.as_ref().map(Vec::len);
    execution.stdout = stdout;
    execution.stderr = stderr;
    execution.statistical_attempts = statistical_attempts;
    execution
}

#[expect(
    clippy::too_many_arguments,
    reason = "the aggregate statistical receipt carries both bounded streams and terminal process state"
)]
pub(super) fn failure_with_digests(
    reason: impl AsRef<str>,
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
    stdout_digest: String,
    stderr_digest: String,
    stdout_bytes: usize,
    stderr_bytes: usize,
    exit_status: Option<i32>,
    completed: bool,
    statistical_attempts: Vec<StatisticalAttemptEvidence>,
) -> ExecutionEvidence {
    let mut execution = failure_with_parts(reason, stdout, stderr, statistical_attempts);
    execution.completed_execution = completed;
    execution.exit_status = exit_status;
    execution.stdout_bytes = Some(stdout_bytes);
    execution.stderr_bytes = Some(stderr_bytes);
    execution.stdout_digest = Some(stdout_digest);
    execution.stderr_digest = Some(stderr_digest);
    execution
}

pub(super) fn fixture_failure(
    source: crate::fixtures::QualificationFixtureFailure,
) -> ExecutionEvidence {
    let parts = source.into_parts();
    let mut execution = failure_with_parts(parts.reason, parts.stdout, parts.stderr, Vec::new());
    execution.completed_execution = parts.completed;
    execution.exit_status = parts.status;
    execution
}

pub(super) fn oracle_failure(source: crate::OracleError) -> ExecutionEvidence {
    let (stdout, stderr) = source
        .captured_streams()
        .map(|(stdout, stderr)| (Some(stdout.bytes.clone()), Some(stderr.bytes.clone())))
        .unwrap_or((None, None));
    failure_with_parts(source.to_string(), stdout, stderr, Vec::new())
}

pub(super) fn capture_limit_failure(
    mut execution: ExecutionEvidence,
    reason: &str,
) -> ExecutionEvidence {
    execution.outcome = CaseOutcome::Failed;
    execution.exact_test_count = None;
    execution.failure = failure(reason).failure;
    execution
}

pub(super) fn process_failure(output: &crate::ProcessOutput, reason: &str) -> ExecutionEvidence {
    let mut detail = Vec::new();
    detail.extend_from_slice(reason.as_bytes());
    detail.extend_from_slice(b"\nstdout:\n");
    detail.extend_from_slice(&output.stdout.bytes);
    detail.extend_from_slice(b"\nstderr:\n");
    detail.extend_from_slice(&output.stderr.bytes);
    detail.truncate(MAX_FAILURE_REASON_ARTIFACT_BYTES);
    ExecutionEvidence {
        outcome: CaseOutcome::Failed,
        completed_execution: true,
        exit_status: output.status,
        exact_test_count: None,
        stdout: Some(output.stdout.bytes.clone()),
        stderr: Some(output.stderr.bytes.clone()),
        stdout_bytes: Some(output.stdout.bytes.len()),
        stderr_bytes: Some(output.stderr.bytes.len()),
        stdout_digest: None,
        stderr_digest: None,
        failure: Some(detail),
        property_regression: None,
        statistical_attempts: Vec::new(),
    }
}

pub(super) fn case_result(
    case: &EvidenceCase,
    execution: ExecutionEvidence,
    selector_sha256: String,
    run_request_sha256: &str,
    output: &QualificationOutputDir,
    executables: &[ExecutableIdentity],
    execution_environment_sha256: &str,
) -> Result<CaseResult, RunError> {
    let ExecutionEvidence {
        outcome,
        completed_execution,
        exit_status,
        exact_test_count,
        stdout,
        stderr,
        stdout_bytes,
        stderr_bytes,
        stdout_digest,
        stderr_digest,
        failure,
        property_regression,
        statistical_attempts,
    } = execution;
    let case_dir = PathBuf::from("cases").join(case.id.as_str());
    let mut artifacts = Vec::new();
    if let Some(failure) = failure {
        let kept = failure
            .get(..failure.len().min(MAX_FAILURE_REASON_ARTIFACT_BYTES))
            .unwrap_or(failure.as_slice());
        artifacts.push(write_artifact(output, &case_dir.join("failure.txt"), kept)?);
    }
    if outcome == CaseOutcome::Failed {
        if let Some(bytes) = property_regression.as_deref() {
            artifacts.push(write_artifact(
                output,
                &case_dir.join("property-regression.case"),
                bytes,
            )?);
        }
        if property_regression.is_none()
            && let Some(bytes) = stdout.as_deref()
        {
            let kept = bytes
                .get(..bytes.len().min(case.execution.stdout_limit_bytes))
                .unwrap_or(bytes);
            artifacts.push(write_artifact(output, &case_dir.join("stdout.bin"), kept)?);
        }
        if let Some(bytes) = stderr.as_deref() {
            let kept = bytes
                .get(..bytes.len().min(case.execution.stderr_limit_bytes))
                .unwrap_or(bytes);
            artifacts.push(write_artifact(output, &case_dir.join("stderr.bin"), kept)?);
        }
    }
    let artifact_bytes = artifacts.iter().try_fold(0_usize, |total, artifact| {
        total.checked_add(artifact.bytes).ok_or_else(|| {
            RunError::ReportContract(
                format!("case {} artifact byte count overflowed", case.id).into_boxed_str(),
            )
        })
    })?;
    if artifact_bytes > case.execution.artifact_limit_bytes {
        return Err(RunError::ReportContract(
            format!(
                "case {} retained {artifact_bytes} artifact bytes, exceeding its {}-byte contract",
                case.id, case.execution.artifact_limit_bytes
            )
            .into_boxed_str(),
        ));
    }
    let stdout_sha256 = stdout_digest.or_else(|| stdout.as_deref().map(sha256));
    let stderr_sha256 = stderr_digest.or_else(|| stderr.as_deref().map(sha256));
    let verdict = match (outcome, completed_execution) {
        (CaseOutcome::Passed, true) => ExecutionVerdict::Accepted,
        (CaseOutcome::Passed, false) => {
            return Err(RunError::ReportContract(
                format!("case {} passed without completing execution", case.id).into_boxed_str(),
            ));
        }
        (CaseOutcome::Failed, true) => ExecutionVerdict::Rejected,
        (CaseOutcome::Failed, false) => ExecutionVerdict::InfrastructureFailure,
    };
    let stream_receipt = |label: &str,
                          bytes: Option<usize>,
                          digest: Option<&String>|
     -> Result<Option<StreamReceipt>, RunError> {
        match (bytes, digest) {
            (Some(bytes), Some(digest)) => Ok(Some(StreamReceipt {
                bytes: u64::try_from(bytes).map_err(|_| {
                    RunError::ReportContract(
                        format!("case {} {label} byte count exceeds u64", case.id).into_boxed_str(),
                    )
                })?,
                sha256: digest.clone(),
                complete: completed_execution,
            })),
            (None, None) => Ok(None),
            _ => Err(RunError::ReportContract(
                format!("case {} {label} evidence is incomplete", case.id).into_boxed_str(),
            )),
        }
    };
    let receipt = new_execution_receipt(ExecutionReceiptInput {
        run_request_sha256: run_request_sha256.to_string(),
        case_id: case.id.to_string(),
        selector_sha256: selector_sha256.clone(),
        executables: executables.to_vec(),
        execution_environment_sha256: execution_environment_sha256.to_string(),
        verdict,
        exit_status,
        exact_test_count,
        stdout: stream_receipt("stdout", stdout_bytes, stdout_sha256.as_ref())?,
        stderr: stream_receipt("stderr", stderr_bytes, stderr_sha256.as_ref())?,
        statistical_attempts: statistical_attempts
            .iter()
            .map(|attempt| StatisticalAttemptReceipt {
                seed: attempt.seed,
                verdict: match attempt.outcome {
                    CaseOutcome::Passed => StatisticalAttemptVerdict::Passed,
                    CaseOutcome::Failed => StatisticalAttemptVerdict::Failed,
                },
                completed_shots: attempt.completed_shots,
                completed_comparisons: attempt.completed_comparisons,
                completed_batches: attempt.completed_batches,
            })
            .collect(),
        auxiliary_outputs: artifacts
            .iter()
            .map(|artifact| ReceiptArtifact {
                path: artifact.path.clone(),
                bytes: artifact.bytes,
                sha256: artifact.sha256.clone(),
            })
            .collect(),
    });
    let execution_receipt_sha256 = receipt
        .publish(output)
        .map_err(|source| RunError::ReportContract(source.to_string().into_boxed_str()))?;
    Ok(CaseResult {
        case_id: case.id.to_string(),
        feature_id: case.feature_id,
        comparator: case.comparator,
        selector: case.primary_selector.clone(),
        selector_sha256,
        execution_receipt_sha256,
        outcome,
        exact_test_count,
        stdout_sha256,
        stderr_sha256,
        artifacts,
    })
}

fn write_artifact(
    output: &QualificationOutputDir,
    relative: &Path,
    bytes: &[u8],
) -> Result<ArtifactRecord, RunError> {
    Ok(ArtifactRecord {
        path: output.write(relative, bytes)?,
        bytes: bytes.len(),
        sha256: sha256(bytes),
    })
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
