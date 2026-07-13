use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::STIM_COMMIT;
use crate::root::RepoRoot;

const CORRECTNESS_PREFLIGHT_SCHEMA_VERSION: u32 = 6;
const MAX_CORRECTNESS_ARTIFACT_BYTES: usize = 64 << 20;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CorrectnessPreflightStatus {
    NotApplicable,
    Passed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CorrectnessPreflightEvidence {
    pub(super) status: CorrectnessPreflightStatus,
    pub(super) case_ids: Vec<String>,
    pub(super) reason: String,
    pub(super) source_directory: Option<String>,
    pub(super) qualification_manifest_sha256: Option<String>,
    pub(super) request_sha256: Option<String>,
    pub(super) completion_sha256: Option<String>,
    pub(super) report_sha256: Option<String>,
    pub(super) preflight_sha256: Option<String>,
}

pub(super) enum CorrectnessRequirement<'a> {
    NotApplicable {
        reason: &'a str,
    },
    Required {
        output: &'a Path,
        case_ids: &'a [String],
        expected_manifest_sha256: &'a str,
        expected_stab_commit: &'a str,
    },
}

pub(super) fn validate(
    root: &RepoRoot,
    requirement: CorrectnessRequirement<'_>,
) -> Result<CorrectnessPreflightEvidence, CorrectnessError> {
    match requirement {
        CorrectnessRequirement::NotApplicable { reason } => {
            if reason.trim().is_empty() {
                return Err(CorrectnessError::MissingReason);
            }
            Ok(CorrectnessPreflightEvidence {
                status: CorrectnessPreflightStatus::NotApplicable,
                case_ids: Vec::new(),
                reason: reason.to_string(),
                source_directory: None,
                qualification_manifest_sha256: None,
                request_sha256: None,
                completion_sha256: None,
                report_sha256: None,
                preflight_sha256: None,
            })
        }
        CorrectnessRequirement::Required {
            output,
            case_ids,
            expected_manifest_sha256,
            expected_stab_commit,
        } => validate_required(
            root,
            output,
            case_ids,
            expected_manifest_sha256,
            expected_stab_commit,
        ),
    }
}

fn validate_required(
    root: &RepoRoot,
    output: &Path,
    case_ids: &[String],
    expected_manifest_sha256: &str,
    expected_stab_commit: &str,
) -> Result<CorrectnessPreflightEvidence, CorrectnessError> {
    validate_output_path(output)?;
    if !valid_sha256(expected_manifest_sha256) || !valid_git_commit(expected_stab_commit) {
        return Err(CorrectnessError::InvalidExpectation);
    }
    let required = case_ids.iter().collect::<BTreeSet<_>>();
    if required.is_empty() || required.len() != case_ids.len() {
        return Err(CorrectnessError::InvalidCases);
    }
    let absolute = root.path.join(output);
    let request = read_artifact(root, &absolute.join("request.json"))?;
    let completion = read_artifact(root, &absolute.join("completion.json"))?;
    let report = read_artifact(root, &absolute.join("report.json"))?;
    let preflight_bytes = read_artifact(root, &absolute.join("preflight.json"))?;
    let preflight: CorrectnessPreflight =
        serde_json::from_slice(&preflight_bytes).map_err(CorrectnessError::Json)?;
    let request_sha256 = super::run::sha256_hex(&request);
    let completion_sha256 = super::run::sha256_hex(&completion);
    let report_sha256 = super::run::sha256_hex(&report);
    if preflight.schema_version != CORRECTNESS_PREFLIGHT_SCHEMA_VERSION
        || preflight.qualification_manifest_digest != expected_manifest_sha256
        || preflight.run_request_sha256 != request_sha256
        || preflight.completion_sha256 != completion_sha256
        || preflight.report_sha256 != report_sha256
        || preflight.stab_commit != expected_stab_commit
        || preflight.stim_commit != STIM_COMMIT
        || preflight.local_modifications
        || preflight.allow_deferred
        || !preflight.selection_complete
        || preflight.deferred_count != 0
        || !matches!(preflight.tier.as_str(), "full" | "soak")
    {
        return Err(CorrectnessError::StalePreflight);
    }
    for case_id in &required {
        let receipt = preflight
            .cases
            .get(*case_id)
            .ok_or_else(|| CorrectnessError::MissingCase((*case_id).clone()))?;
        if receipt.outcome != "passed"
            || !valid_sha256(&receipt.selector_sha256)
            || !valid_sha256(&receipt.execution_receipt_sha256)
            || !valid_sha256(&receipt.stdout_sha256)
            || !valid_sha256(&receipt.stderr_sha256)
        {
            return Err(CorrectnessError::FailedCase((*case_id).clone()));
        }
    }
    let mut case_ids = case_ids.to_vec();
    case_ids.sort();
    Ok(CorrectnessPreflightEvidence {
        status: CorrectnessPreflightStatus::Passed,
        case_ids,
        reason: "exact CQ1 artifacts and passing case receipts validated before timing".to_string(),
        source_directory: Some(output.to_string_lossy().into_owned()),
        qualification_manifest_sha256: Some(expected_manifest_sha256.to_string()),
        request_sha256: Some(request_sha256),
        completion_sha256: Some(completion_sha256),
        report_sha256: Some(report_sha256),
        preflight_sha256: Some(super::run::sha256_hex(&preflight_bytes)),
    })
}

fn read_artifact(root: &RepoRoot, path: &Path) -> Result<Vec<u8>, CorrectnessError> {
    crate::source_file::read_repo_regular_file_bounded(root, path, MAX_CORRECTNESS_ARTIFACT_BYTES)
        .map_err(|error| CorrectnessError::Read(error.to_string()))
}

fn validate_output_path(path: &Path) -> Result<(), CorrectnessError> {
    if path.is_absolute()
        || path.to_str().is_none()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CorrectnessError::InvalidOutput(path.to_path_buf()));
    }
    let components = path.components().collect::<Vec<_>>();
    if components.len() < 4
        || components.first() != Some(&Component::Normal("target".as_ref()))
        || components.get(1) != Some(&Component::Normal("qualification".as_ref()))
        || components.get(2) != Some(&Component::Normal("correctness".as_ref()))
    {
        return Err(CorrectnessError::InvalidOutput(path.to_path_buf()));
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_git_commit(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorrectnessPreflight {
    schema_version: u32,
    report_sha256: String,
    completion_sha256: String,
    qualification_manifest_digest: String,
    run_request_sha256: String,
    stab_commit: String,
    local_modifications: bool,
    stim_commit: String,
    tier: String,
    allow_deferred: bool,
    selection_complete: bool,
    deferred_count: u64,
    cases: BTreeMap<String, CorrectnessCaseReceipt>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorrectnessCaseReceipt {
    outcome: String,
    selector_sha256: String,
    execution_receipt_sha256: String,
    stdout_sha256: String,
    stderr_sha256: String,
}

#[derive(Debug, Error)]
pub(super) enum CorrectnessError {
    #[error("diagnostic correctness preflight requires a reason")]
    MissingReason,
    #[error(
        "correctness evidence path must be a normal directory below target/qualification/correctness: {0}"
    )]
    InvalidOutput(PathBuf),
    #[error("correctness preflight expectation has an invalid manifest digest or Stab commit")]
    InvalidExpectation,
    #[error("correctness preflight requires unique nonempty case ids")]
    InvalidCases,
    #[error("failed to read correctness evidence: {0}")]
    Read(String),
    #[error("correctness preflight JSON is invalid: {0}")]
    Json(serde_json::Error),
    #[error("correctness preflight is stale, dirty, deferred, incomplete, or artifact-unbound")]
    StalePreflight,
    #[error("correctness preflight omits required case {0}")]
    MissingCase(String),
    #[error("correctness preflight case {0} did not pass with complete receipt digests")]
    FailedCase(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_evidence_is_explicitly_nonapplicable() {
        let repository = tempfile::tempdir().expect("temporary repository");
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        let evidence = validate(
            &root,
            CorrectnessRequirement::NotApplicable {
                reason: "infrastructure-only workload",
            },
        )
        .expect("diagnostic preflight");
        assert_eq!(evidence.status, CorrectnessPreflightStatus::NotApplicable);
        assert!(evidence.case_ids.is_empty());
        assert!(evidence.request_sha256.is_none());
    }

    #[test]
    fn required_preflight_binds_all_artifacts_and_selected_case_receipts() {
        let repository = tempfile::tempdir().expect("temporary repository");
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        let relative = Path::new("target/qualification/correctness/full");
        let output = repository.path().join(relative);
        std::fs::create_dir_all(&output).expect("create correctness output");
        let request = b"request\n";
        let completion = b"completion\n";
        let report = b"report\n";
        std::fs::write(output.join("request.json"), request).expect("write request");
        std::fs::write(output.join("completion.json"), completion).expect("write completion");
        std::fs::write(output.join("report.json"), report).expect("write report");
        let manifest = "a".repeat(64);
        let commit = "b".repeat(40);
        let receipt_digest = "c".repeat(64);
        let preflight = serde_json::json!({
            "schema_version": CORRECTNESS_PREFLIGHT_SCHEMA_VERSION,
            "report_sha256": crate::qualification::runtime::run::sha256_hex(report),
            "completion_sha256": crate::qualification::runtime::run::sha256_hex(completion),
            "qualification_manifest_digest": manifest,
            "run_request_sha256": crate::qualification::runtime::run::sha256_hex(request),
            "stab_commit": commit,
            "local_modifications": false,
            "stim_commit": STIM_COMMIT,
            "tier": "full",
            "allow_deferred": false,
            "selection_complete": true,
            "deferred_count": 0,
            "cases": {
                "cq-case": {
                    "outcome": "passed",
                    "selector_sha256": receipt_digest,
                    "execution_receipt_sha256": "d".repeat(64),
                    "stdout_sha256": "e".repeat(64),
                    "stderr_sha256": "f".repeat(64)
                }
            }
        });
        std::fs::write(
            output.join("preflight.json"),
            serde_json::to_vec(&preflight).expect("serialize preflight"),
        )
        .expect("write preflight");

        let evidence = validate(
            &root,
            CorrectnessRequirement::Required {
                output: relative,
                case_ids: &["cq-case".to_string()],
                expected_manifest_sha256: &"a".repeat(64),
                expected_stab_commit: &"b".repeat(40),
            },
        )
        .expect("bound correctness preflight");
        assert_eq!(evidence.status, CorrectnessPreflightStatus::Passed);

        let mut stale = preflight;
        stale
            .as_object_mut()
            .expect("preflight fixture is an object")
            .insert(
                "run_request_sha256".to_string(),
                serde_json::Value::String("0".repeat(64)),
            );
        std::fs::write(
            output.join("preflight.json"),
            serde_json::to_vec(&stale).expect("serialize stale preflight"),
        )
        .expect("write stale preflight");
        assert!(
            validate(
                &root,
                CorrectnessRequirement::Required {
                    output: relative,
                    case_ids: &["cq-case".to_string()],
                    expected_manifest_sha256: &"a".repeat(64),
                    expected_stab_commit: &"b".repeat(40),
                },
            )
            .is_err()
        );
    }
}
