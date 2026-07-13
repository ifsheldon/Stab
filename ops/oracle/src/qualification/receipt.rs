use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::artifact::QualificationOutputDir;
use super::model::{ExecutionTier, FeatureId};

const RUN_REQUEST_SCHEMA_VERSION: u32 = 3;
const EXECUTION_RECEIPT_SCHEMA_VERSION: u32 = 3;
const RUN_COMPLETION_SCHEMA_VERSION: u32 = 1;
const MAX_RUN_REQUEST_BYTES: usize = 4 << 20;
const MAX_RUN_COMPLETION_BYTES: usize = 4 << 20;
pub(super) const MAX_EXECUTION_RECEIPT_BYTES: usize = 256 << 10;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunRequestReceipt {
    pub(super) schema_version: u32,
    pub(super) qualification_manifest_digest: String,
    pub(super) stab_commit: String,
    pub(super) worktree_was_clean: bool,
    pub(super) stim_tag: String,
    pub(super) stim_commit: String,
    pub(super) tier: ExecutionTier,
    pub(super) feature_filters: Vec<FeatureId>,
    pub(super) case_filters: Vec<String>,
    pub(super) allow_deferred: bool,
    pub(super) executables: Vec<ExecutableIdentity>,
    pub(super) execution_environment_sha256: String,
    pub(super) selected_cases: Vec<RequestedCase>,
    pub(super) planned_case_ids: Vec<String>,
    pub(super) deferred_case_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RequestedCase {
    pub(super) case_id: String,
    pub(super) selector_sha256: String,
    pub(super) case_contract_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutableIdentity {
    pub(super) role: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

pub(super) struct RunRequestReceiptInput {
    pub(super) qualification_manifest_digest: String,
    pub(super) stab_commit: String,
    pub(super) worktree_was_clean: bool,
    pub(super) stim_tag: String,
    pub(super) stim_commit: String,
    pub(super) tier: ExecutionTier,
    pub(super) feature_filters: Vec<FeatureId>,
    pub(super) case_filters: Vec<String>,
    pub(super) allow_deferred: bool,
    pub(super) executables: Vec<ExecutableIdentity>,
    pub(super) execution_environment_sha256: String,
    pub(super) selected_cases: Vec<RequestedCase>,
    pub(super) planned_case_ids: Vec<String>,
    pub(super) deferred_case_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunCompletionReceipt {
    pub(super) schema_version: u32,
    pub(super) run_request_sha256: String,
    pub(super) report_sha256: String,
    pub(super) cases: Vec<CompletedCase>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompletedCase {
    pub(super) case_id: String,
    pub(super) execution_receipt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutionReceipt {
    pub(super) schema_version: u32,
    pub(super) run_request_sha256: String,
    pub(super) case_id: String,
    pub(super) selector_sha256: String,
    pub(super) executables: Vec<ExecutableIdentity>,
    pub(super) execution_environment_sha256: String,
    pub(super) verdict: ExecutionVerdict,
    pub(super) exit_status: Option<i32>,
    pub(super) exact_test_count: Option<usize>,
    pub(super) stdout: Option<StreamReceipt>,
    pub(super) stderr: Option<StreamReceipt>,
    pub(super) statistical_attempts: Vec<StatisticalAttemptReceipt>,
    pub(super) auxiliary_outputs: Vec<ReceiptArtifact>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ExecutionVerdict {
    Accepted,
    Rejected,
    InfrastructureFailure,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum StatisticalAttemptVerdict {
    Passed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StatisticalAttemptReceipt {
    pub(super) seed: u64,
    pub(super) verdict: StatisticalAttemptVerdict,
    pub(super) completed_shots: u64,
    pub(super) completed_comparisons: u32,
    pub(super) completed_batches: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StreamReceipt {
    pub(super) bytes: u64,
    pub(super) sha256: String,
    pub(super) complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReceiptArtifact {
    pub(super) path: PathBuf,
    pub(super) bytes: usize,
    pub(super) sha256: String,
}

pub(super) struct ExecutionReceiptInput {
    pub(super) run_request_sha256: String,
    pub(super) case_id: String,
    pub(super) selector_sha256: String,
    pub(super) executables: Vec<ExecutableIdentity>,
    pub(super) execution_environment_sha256: String,
    pub(super) verdict: ExecutionVerdict,
    pub(super) exit_status: Option<i32>,
    pub(super) exact_test_count: Option<usize>,
    pub(super) stdout: Option<StreamReceipt>,
    pub(super) stderr: Option<StreamReceipt>,
    pub(super) statistical_attempts: Vec<StatisticalAttemptReceipt>,
    pub(super) auxiliary_outputs: Vec<ReceiptArtifact>,
}

#[derive(Debug, Error)]
pub(crate) enum ReceiptError {
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),

    #[error("qualification receipt JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),

    #[error("qualification receipt is not in canonical generated form")]
    NonCanonical,
}

impl RunRequestReceipt {
    pub(super) fn new(input: RunRequestReceiptInput) -> Self {
        Self {
            schema_version: RUN_REQUEST_SCHEMA_VERSION,
            qualification_manifest_digest: input.qualification_manifest_digest,
            stab_commit: input.stab_commit,
            worktree_was_clean: input.worktree_was_clean,
            stim_tag: input.stim_tag,
            stim_commit: input.stim_commit,
            tier: input.tier,
            feature_filters: input.feature_filters,
            case_filters: input.case_filters,
            allow_deferred: input.allow_deferred,
            executables: input.executables,
            execution_environment_sha256: input.execution_environment_sha256,
            selected_cases: input.selected_cases,
            planned_case_ids: input.planned_case_ids,
            deferred_case_ids: input.deferred_case_ids,
        }
    }

    pub(super) fn publish(&self, output: &QualificationOutputDir) -> Result<String, ReceiptError> {
        let bytes = canonical_json(self)?;
        output.write(Path::new("request.json"), &bytes)?;
        Ok(sha256(&bytes))
    }

    pub(super) fn read(output: &QualificationOutputDir) -> Result<(Self, String), ReceiptError> {
        let bytes = output.read(Path::new("request.json"), MAX_RUN_REQUEST_BYTES)?;
        let receipt: Self = serde_json::from_slice(&bytes)?;
        if canonical_json(&receipt)? != bytes {
            return Err(ReceiptError::NonCanonical);
        }
        Ok((receipt, sha256(&bytes)))
    }

    pub(super) const fn schema_is_current(&self) -> bool {
        self.schema_version == RUN_REQUEST_SCHEMA_VERSION
    }
}

impl ExecutionReceipt {
    pub(super) fn publish(&self, output: &QualificationOutputDir) -> Result<String, ReceiptError> {
        let bytes = canonical_json(self)?;
        output.write(&execution_receipt_path(&self.case_id), &bytes)?;
        Ok(sha256(&bytes))
    }

    pub(super) fn read(
        output: &QualificationOutputDir,
        case_id: &str,
    ) -> Result<(Self, String), ReceiptError> {
        let bytes = output.read(
            &execution_receipt_path(case_id),
            MAX_EXECUTION_RECEIPT_BYTES,
        )?;
        let receipt: Self = serde_json::from_slice(&bytes)?;
        if canonical_json(&receipt)? != bytes {
            return Err(ReceiptError::NonCanonical);
        }
        Ok((receipt, sha256(&bytes)))
    }

    pub(super) const fn schema_is_current(&self) -> bool {
        self.schema_version == EXECUTION_RECEIPT_SCHEMA_VERSION
    }
}

impl RunCompletionReceipt {
    pub(super) fn new(
        run_request_sha256: String,
        report_sha256: String,
        cases: Vec<CompletedCase>,
    ) -> Self {
        Self {
            schema_version: RUN_COMPLETION_SCHEMA_VERSION,
            run_request_sha256,
            report_sha256,
            cases,
        }
    }

    pub(super) fn publish(&self, output: &QualificationOutputDir) -> Result<String, ReceiptError> {
        let bytes = canonical_json(self)?;
        output.write(Path::new("completion.json"), &bytes)?;
        Ok(sha256(&bytes))
    }

    pub(super) fn read(output: &QualificationOutputDir) -> Result<(Self, String), ReceiptError> {
        let bytes = output.read(Path::new("completion.json"), MAX_RUN_COMPLETION_BYTES)?;
        let receipt: Self = serde_json::from_slice(&bytes)?;
        if canonical_json(&receipt)? != bytes {
            return Err(ReceiptError::NonCanonical);
        }
        Ok((receipt, sha256(&bytes)))
    }

    pub(super) const fn schema_is_current(&self) -> bool {
        self.schema_version == RUN_COMPLETION_SCHEMA_VERSION
    }
}

pub(super) fn new_execution_receipt(input: ExecutionReceiptInput) -> ExecutionReceipt {
    ExecutionReceipt {
        schema_version: EXECUTION_RECEIPT_SCHEMA_VERSION,
        run_request_sha256: input.run_request_sha256,
        case_id: input.case_id,
        selector_sha256: input.selector_sha256,
        executables: input.executables,
        execution_environment_sha256: input.execution_environment_sha256,
        verdict: input.verdict,
        exit_status: input.exit_status,
        exact_test_count: input.exact_test_count,
        stdout: input.stdout,
        stderr: input.stderr,
        statistical_attempts: input.statistical_attempts,
        auxiliary_outputs: input.auxiliary_outputs,
    }
}

pub(super) fn case_contract_sha256<T: Serialize>(case: &T) -> Result<String, ReceiptError> {
    let bytes = serde_json::to_vec(case)?;
    let mut hasher = Sha256::new();
    hasher.update(b"stab-cq1/case-contract/v1\0");
    hasher.update(bytes);
    Ok(render_digest(&hasher.finalize()))
}

fn execution_receipt_path(case_id: &str) -> PathBuf {
    PathBuf::from("cases")
        .join(case_id)
        .join("execution-receipt.json")
}

fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sha256(bytes: &[u8]) -> String {
    render_digest(&Sha256::digest(bytes))
}

fn render_digest(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RepoRoot;

    fn output() -> (tempfile::TempDir, QualificationOutputDir) {
        let temporary = tempfile::tempdir().expect("temporary receipt root");
        let root = RepoRoot {
            path: temporary.path().to_path_buf(),
        };
        let output = QualificationOutputDir::parse(
            &root,
            Path::new("target/qualification/correctness/receipt-test"),
        )
        .expect("qualification output");
        (temporary, output)
    }

    fn request() -> RunRequestReceipt {
        RunRequestReceipt::new(RunRequestReceiptInput {
            qualification_manifest_digest: "a".repeat(64),
            stab_commit: "b".repeat(40),
            worktree_was_clean: true,
            stim_tag: "v1.16.0".to_string(),
            stim_commit: "c".repeat(40),
            tier: ExecutionTier::Pr,
            feature_filters: Vec::new(),
            case_filters: Vec::new(),
            allow_deferred: false,
            executables: Vec::new(),
            execution_environment_sha256: "d".repeat(64),
            selected_cases: Vec::new(),
            planned_case_ids: Vec::new(),
            deferred_case_ids: Vec::new(),
        })
    }

    #[test]
    fn run_request_round_trip_is_canonical_and_digest_bound() {
        let (_temporary, output) = output();
        let request = request();
        let published = request.publish(&output).expect("publish request");
        let (read, digest) = RunRequestReceipt::read(&output).expect("read request");

        assert_eq!(read, request);
        assert_eq!(digest, published);

        output
            .write(
                Path::new("request.json"),
                &serde_json::to_vec(&request).expect("compact request"),
            )
            .expect("replace request with noncanonical bytes");
        assert!(matches!(
            RunRequestReceipt::read(&output),
            Err(ReceiptError::NonCanonical)
        ));
    }

    #[test]
    fn execution_receipt_rejects_an_unsafe_case_path() {
        let (_temporary, output) = output();
        let receipt = new_execution_receipt(ExecutionReceiptInput {
            run_request_sha256: "a".repeat(64),
            case_id: "../escape".to_string(),
            selector_sha256: "b".repeat(64),
            executables: Vec::new(),
            execution_environment_sha256: "c".repeat(64),
            verdict: ExecutionVerdict::InfrastructureFailure,
            exit_status: None,
            exact_test_count: None,
            stdout: None,
            stderr: None,
            statistical_attempts: Vec::new(),
            auxiliary_outputs: Vec::new(),
        });

        assert!(matches!(
            receipt.publish(&output),
            Err(ReceiptError::Artifact(_))
        ));
    }
}
