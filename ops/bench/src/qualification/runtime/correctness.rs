use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::root::RepoRoot;

mod binding;

pub(super) use binding::CorrectnessArtifactBinding;

const RUN_REQUEST_SCHEMA_VERSION: u32 = 3;
const RUN_COMPLETION_SCHEMA_VERSION: u32 = 1;
const MAX_CORRECTNESS_ARTIFACT_BYTES: usize = 64 << 20;
const MAX_EXECUTION_RECEIPT_BYTES: usize = 256 << 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CorrectnessSchemaFamily {
    V6,
    V7,
}

impl CorrectnessSchemaFamily {
    fn from_report_schema(schema_version: u32) -> Option<Self> {
        match schema_version {
            6 => Some(Self::V6),
            7 => Some(Self::V7),
            _ => None,
        }
    }

    const fn report_and_preflight_version(self) -> u32 {
        match self {
            Self::V6 => 6,
            Self::V7 => 7,
        }
    }

    const fn execution_receipt_version(self) -> u32 {
        match self {
            Self::V6 => 3,
            Self::V7 => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CorrectnessPreflightStatus {
    NotApplicable,
    Passed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
        expected_request_sha256: &'a str,
        expected_completion_sha256: &'a str,
    },
}

pub(super) fn validate(
    root: &RepoRoot,
    requirement: CorrectnessRequirement<'_>,
) -> Result<CorrectnessPreflightEvidence, CorrectnessError> {
    Ok(validate_bound(root, requirement)?.0)
}

pub(super) fn validate_bound(
    root: &RepoRoot,
    requirement: CorrectnessRequirement<'_>,
) -> Result<(CorrectnessPreflightEvidence, CorrectnessArtifactBinding), CorrectnessError> {
    match requirement {
        CorrectnessRequirement::NotApplicable { reason } => {
            if reason.trim().is_empty() {
                return Err(CorrectnessError::MissingReason);
            }
            Ok((
                CorrectnessPreflightEvidence {
                    status: CorrectnessPreflightStatus::NotApplicable,
                    case_ids: Vec::new(),
                    reason: reason.to_string(),
                    source_directory: None,
                    qualification_manifest_sha256: None,
                    request_sha256: None,
                    completion_sha256: None,
                    report_sha256: None,
                    preflight_sha256: None,
                },
                CorrectnessArtifactBinding::default(),
            ))
        }
        CorrectnessRequirement::Required {
            output,
            case_ids,
            expected_manifest_sha256,
            expected_stab_commit,
            expected_request_sha256,
            expected_completion_sha256,
        } => validate_required(
            root,
            RequiredValidation {
                output,
                case_ids,
                expected_manifest_sha256,
                expected_stab_commit,
                expected_request_sha256,
                expected_completion_sha256,
            },
        ),
    }
}

#[cfg(test)]
pub(in crate::qualification::runtime) fn bind_test_artifact_tree(
    root: &RepoRoot,
    output: &Path,
    case_ids: &[&str],
) -> Result<CorrectnessArtifactBinding, CorrectnessError> {
    let mut binding = CorrectnessArtifactBinding::open(root, output)?;
    for name in [
        "completion.json",
        "preflight.json",
        "report.json",
        "report.md",
        "request.json",
    ] {
        binding.read_top_and_bind(name, MAX_CORRECTNESS_ARTIFACT_BYTES)?;
    }
    binding.bind_case_directories(case_ids.iter().copied())?;
    for case_id in case_ids {
        binding.read_case_receipt_and_bind(case_id, MAX_EXECUTION_RECEIPT_BYTES)?;
    }
    Ok(binding)
}

struct RequiredValidation<'a> {
    output: &'a Path,
    case_ids: &'a [String],
    expected_manifest_sha256: &'a str,
    expected_stab_commit: &'a str,
    expected_request_sha256: &'a str,
    expected_completion_sha256: &'a str,
}

fn validate_required(
    root: &RepoRoot,
    validation: RequiredValidation<'_>,
) -> Result<(CorrectnessPreflightEvidence, CorrectnessArtifactBinding), CorrectnessError> {
    let RequiredValidation {
        output,
        case_ids,
        expected_manifest_sha256,
        expected_stab_commit,
        expected_request_sha256,
        expected_completion_sha256,
    } = validation;
    validate_output_path(output)?;
    if !valid_sha256(expected_manifest_sha256)
        || !valid_git_commit(expected_stab_commit)
        || !valid_sha256(expected_request_sha256)
        || !valid_sha256(expected_completion_sha256)
    {
        return Err(CorrectnessError::InvalidExpectation);
    }
    let required = case_ids.iter().collect::<BTreeSet<_>>();
    if required.is_empty()
        || required.len() != case_ids.len()
        || required.iter().any(|case_id| !valid_case_id(case_id))
    {
        return Err(CorrectnessError::InvalidCases);
    }

    let mut binding = CorrectnessArtifactBinding::open(root, output)?;
    let request_bytes =
        binding.read_top_and_bind("request.json", MAX_CORRECTNESS_ARTIFACT_BYTES)?;
    let completion_bytes =
        binding.read_top_and_bind("completion.json", MAX_CORRECTNESS_ARTIFACT_BYTES)?;
    let report_bytes = binding.read_top_and_bind("report.json", MAX_CORRECTNESS_ARTIFACT_BYTES)?;
    let preflight_bytes =
        binding.read_top_and_bind("preflight.json", MAX_CORRECTNESS_ARTIFACT_BYTES)?;
    binding.read_top_and_bind("report.md", MAX_CORRECTNESS_ARTIFACT_BYTES)?;
    let request: RunRequest = parse_canonical("request.json", &request_bytes)?;
    let completion: RunCompletion = parse_canonical("completion.json", &completion_bytes)?;
    let report: CorrectnessReport = parse_canonical("report.json", &report_bytes)?;
    let preflight: CorrectnessPreflight = parse_canonical("preflight.json", &preflight_bytes)?;

    let request_sha256 = super::run::sha256_hex(&request_bytes);
    let completion_sha256 = super::run::sha256_hex(&completion_bytes);
    let report_sha256 = super::run::sha256_hex(&report_bytes);
    if request_sha256 != expected_request_sha256 || completion_sha256 != expected_completion_sha256
    {
        return Err(CorrectnessError::ControllerBinding);
    }

    let requested = validate_request(&request, expected_manifest_sha256, expected_stab_commit)?;
    let (schema_family, results) = validate_report(&report, &request, &request_sha256, &requested)?;
    let statistical_attempts = validate_statistical_ledger(&report, &results)?;
    validate_completion(&completion, &request_sha256, &report_sha256, &results)?;
    validate_execution_receipts(
        &request,
        &request_sha256,
        schema_family,
        &statistical_attempts,
        &results,
        &mut binding,
    )?;
    validate_preflight(
        &preflight,
        &report,
        &request_sha256,
        &report_sha256,
        &completion_sha256,
        schema_family,
        &results,
    )?;
    for case_id in &required {
        if !results.contains_key(case_id.as_str()) {
            return Err(CorrectnessError::MissingCase((*case_id).clone()));
        }
    }

    let mut case_ids = case_ids.to_vec();
    case_ids.sort();
    Ok((
        CorrectnessPreflightEvidence {
            status: CorrectnessPreflightStatus::Passed,
            case_ids,
            reason: "canonical CQ request, report, completion, preflight, and passing execution receipts independently reconstructed before timing".to_string(),
            source_directory: Some(output.to_string_lossy().into_owned()),
            qualification_manifest_sha256: Some(expected_manifest_sha256.to_string()),
            request_sha256: Some(request_sha256),
            completion_sha256: Some(completion_sha256),
            report_sha256: Some(report_sha256),
            preflight_sha256: Some(super::run::sha256_hex(&preflight_bytes)),
        },
        binding,
    ))
}

fn validate_request<'a>(
    request: &'a RunRequest,
    expected_manifest_sha256: &str,
    expected_stab_commit: &str,
) -> Result<BTreeMap<&'a str, &'a RequestedCase>, CorrectnessError> {
    if request.schema_version != RUN_REQUEST_SCHEMA_VERSION
        || request.qualification_manifest_digest != expected_manifest_sha256
        || request.stab_commit != expected_stab_commit
        || !request.worktree_was_clean
        || request.stim_tag != STIM_TAG
        || request.stim_commit != STIM_COMMIT
        || !matches!(request.tier.as_str(), "full" | "soak")
        || request.allow_deferred
        || !request.planned_case_ids.is_empty()
        || !request.deferred_case_ids.is_empty()
        || !valid_sha256(&request.execution_environment_sha256)
        || request.executables.is_empty()
    {
        return Err(CorrectnessError::RequestContract);
    }
    if !unique_valid_case_ids(&request.planned_case_ids)
        || !unique_valid_case_ids(&request.deferred_case_ids)
    {
        return Err(CorrectnessError::RequestContract);
    }
    for executable in &request.executables {
        if executable.role.is_empty() || executable.bytes == 0 || !valid_sha256(&executable.sha256)
        {
            return Err(CorrectnessError::RequestContract);
        }
    }
    let mut requested = BTreeMap::new();
    for case in &request.selected_cases {
        if !valid_case_id(&case.case_id)
            || !valid_sha256(&case.selector_sha256)
            || !valid_sha256(&case.case_contract_sha256)
            || requested.insert(case.case_id.as_str(), case).is_some()
        {
            return Err(CorrectnessError::RequestContract);
        }
    }
    if requested.is_empty() {
        return Err(CorrectnessError::RequestContract);
    }
    Ok(requested)
}

fn validate_report<'a>(
    report: &'a CorrectnessReport,
    request: &RunRequest,
    request_sha256: &str,
    requested: &BTreeMap<&str, &RequestedCase>,
) -> Result<(CorrectnessSchemaFamily, BTreeMap<&'a str, &'a CaseResult>), CorrectnessError> {
    let Some(schema_family) = CorrectnessSchemaFamily::from_report_schema(report.schema_version)
    else {
        return Err(CorrectnessError::ReportContract);
    };
    if report.qualification_manifest_digest != request.qualification_manifest_digest
        || report.run_request_sha256 != request_sha256
        || report.stab_commit != request.stab_commit
        || report.local_modifications
        || report.stim_tag != request.stim_tag
        || report.stim_commit != request.stim_commit
        || report.tier != request.tier
        || report.feature_filters != request.feature_filters
        || report.case_filters != request.case_filters
        || report.allow_deferred
        || report.selected_count != request.selected_cases.len()
        || report.selected_count != report.results.len()
        || report.planned_count != request.planned_case_ids.len()
        || report.deferred_count != request.deferred_case_ids.len()
        || report.passed_count != report.results.len()
        || report.failed_count != 0
        || !report.selection_complete
    {
        return Err(CorrectnessError::ReportContract);
    }
    let mut results = BTreeMap::new();
    for result in &report.results {
        let Some(expected) = requested.get(result.case_id.as_str()) else {
            return Err(CorrectnessError::ReportContract);
        };
        let selector_bytes =
            serde_json::to_vec(&result.selector).map_err(CorrectnessError::Json)?;
        if result.outcome != "passed"
            || result.selector_sha256 != expected.selector_sha256
            || !selector_digest_matches_contract(result, &selector_bytes)
            || !valid_sha256(&result.execution_receipt_sha256)
            || !result.stdout_sha256.as_deref().is_some_and(valid_sha256)
            || !result.stderr_sha256.as_deref().is_some_and(valid_sha256)
            || !result.artifacts.is_empty()
            || results.insert(result.case_id.as_str(), result).is_some()
        {
            return Err(CorrectnessError::ReportContract);
        }
    }
    if results.len() != requested.len() {
        return Err(CorrectnessError::ReportContract);
    }
    Ok((schema_family, results))
}

fn validate_statistical_ledger<'a>(
    report: &'a CorrectnessReport,
    results: &BTreeMap<&str, &CaseResult>,
) -> Result<BTreeMap<&'a str, Vec<&'a StatisticalAttempt>>, CorrectnessError> {
    let statistical_case_ids = results
        .values()
        .filter(|result| result.comparator == "statistical")
        .map(|result| result.case_id.as_str())
        .collect::<BTreeSet<_>>();
    if !valid_seed_panels(&report.statistical_planned_seeds, &statistical_case_ids)
        || !valid_seed_panels(&report.statistical_seeds, &statistical_case_ids)
        || report.statistical_planned_seeds != report.statistical_seeds
        || report.statistical_planned_shots != report.statistical_shots
    {
        return Err(CorrectnessError::ReportContract);
    }

    let Some(declared_bound) = canonical_probability_bound(&report.statistical_declared_budget)
    else {
        return Err(CorrectnessError::ReportContract);
    };
    let Some(consumed_bound) = canonical_probability_bound(&report.statistical_consumed_bound)
    else {
        return Err(CorrectnessError::ReportContract);
    };
    if consumed_bound > declared_bound {
        return Err(CorrectnessError::ReportContract);
    }

    let mut attempts_by_case = BTreeMap::<&str, Vec<&StatisticalAttempt>>::new();
    let mut attempted_seeds = BTreeMap::<String, Vec<u64>>::new();
    let mut attempted_shots = 0_u64;
    let mut attempt_keys = BTreeSet::new();
    for attempt in &report.statistical_attempts {
        let Some(result) = results.get(attempt.case_id.as_str()) else {
            return Err(CorrectnessError::ReportContract);
        };
        if result.comparator != "statistical"
            || attempt.outcome != "passed"
            || attempt.completed_shots == 0
            || attempt.completed_comparisons == 0
            || attempt.completed_batches == 0
            || !attempt_keys.insert((attempt.case_id.as_str(), attempt.seed))
        {
            return Err(CorrectnessError::ReportContract);
        }
        attempted_shots = attempted_shots
            .checked_add(attempt.completed_shots)
            .ok_or(CorrectnessError::ReportContract)?;
        attempted_seeds
            .entry(attempt.case_id.clone())
            .or_default()
            .push(attempt.seed);
        attempts_by_case
            .entry(attempt.case_id.as_str())
            .or_default()
            .push(attempt);
    }
    if attempted_shots != report.statistical_shots || attempted_seeds != report.statistical_seeds {
        return Err(CorrectnessError::ReportContract);
    }
    Ok(attempts_by_case)
}

fn valid_seed_panels(
    panels: &BTreeMap<String, Vec<u64>>,
    statistical_case_ids: &BTreeSet<&str>,
) -> bool {
    panels.keys().map(String::as_str).collect::<BTreeSet<_>>() == *statistical_case_ids
        && panels.values().all(|seeds| {
            !seeds.is_empty() && seeds.iter().collect::<BTreeSet<_>>().len() == seeds.len()
        })
}

fn canonical_probability_bound(value: &str) -> Option<f64> {
    if value.len() > 32 {
        return None;
    }
    let parsed = value.parse::<f64>().ok()?;
    (parsed.is_finite()
        && !parsed.is_sign_negative()
        && parsed <= 1e-4
        && format!("{parsed:.17e}") == value)
        .then_some(parsed)
}

fn selector_digest_matches_contract(result: &CaseResult, selector_bytes: &[u8]) -> bool {
    match result.selector.kind.as_str() {
        "cargo-test" | "property-target" => {
            result.selector_sha256 == super::run::sha256_hex(selector_bytes)
        }
        // CQ resolves these selectors against source-owned blocker or fixture contracts before
        // freezing the controller-approved request digest. Their display selector is not the
        // hashed execution contract, so the performance consumer binds the request, report, and
        // execution receipt instead of hashing the display value.
        "oracle-fixture" | "ops-check" => true,
        _ => false,
    }
}

fn validate_completion(
    completion: &RunCompletion,
    request_sha256: &str,
    report_sha256: &str,
    results: &BTreeMap<&str, &CaseResult>,
) -> Result<(), CorrectnessError> {
    let expected_cases = results
        .values()
        .map(|result| {
            (
                result.case_id.clone(),
                result.execution_receipt_sha256.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut actual_cases = BTreeMap::new();
    for case in &completion.cases {
        if !valid_case_id(&case.case_id)
            || !valid_sha256(&case.execution_receipt_sha256)
            || actual_cases
                .insert(case.case_id.clone(), case.execution_receipt_sha256.clone())
                .is_some()
        {
            return Err(CorrectnessError::CompletionContract);
        }
    }
    if completion.schema_version != RUN_COMPLETION_SCHEMA_VERSION
        || completion.run_request_sha256 != request_sha256
        || completion.report_sha256 != report_sha256
        || actual_cases != expected_cases
    {
        return Err(CorrectnessError::CompletionContract);
    }
    Ok(())
}

fn validate_execution_receipts(
    request: &RunRequest,
    request_sha256: &str,
    schema_family: CorrectnessSchemaFamily,
    report_attempts: &BTreeMap<&str, Vec<&StatisticalAttempt>>,
    results: &BTreeMap<&str, &CaseResult>,
    binding: &mut CorrectnessArtifactBinding,
) -> Result<(), CorrectnessError> {
    binding.bind_case_directories(results.keys().copied())?;
    for result in results.values() {
        let bytes =
            binding.read_case_receipt_and_bind(&result.case_id, MAX_EXECUTION_RECEIPT_BYTES)?;
        if super::run::sha256_hex(&bytes) != result.execution_receipt_sha256 {
            return Err(CorrectnessError::ExecutionReceipt(result.case_id.clone()));
        }
        let receipt: ExecutionReceipt = parse_canonical("execution-receipt.json", &bytes)?;
        let stdout_sha256 = receipt.stdout.as_ref().map(|stream| stream.sha256.as_str());
        let stderr_sha256 = receipt.stderr.as_ref().map(|stream| stream.sha256.as_str());
        if receipt.schema_version != schema_family.execution_receipt_version()
            || receipt.run_request_sha256 != request_sha256
            || receipt.case_id != result.case_id
            || receipt.selector_sha256 != result.selector_sha256
            || receipt.executables != request.executables
            || receipt.execution_environment_sha256 != request.execution_environment_sha256
            || receipt.verdict != "accepted"
            || receipt.exit_status.is_none()
            || receipt.exact_test_count != result.exact_test_count
            || (result.selector.kind == "cargo-test" && receipt.exact_test_count != Some(1))
            || stdout_sha256 != result.stdout_sha256.as_deref()
            || stderr_sha256 != result.stderr_sha256.as_deref()
            || receipt.auxiliary_outputs != result.artifacts
            || !receipt.auxiliary_outputs.is_empty()
            || !complete_stream(receipt.stdout.as_ref())
            || !complete_stream(receipt.stderr.as_ref())
            || !statistical_attempts_match(
                &result.case_id,
                report_attempts,
                &receipt.statistical_attempts,
            )
        {
            return Err(CorrectnessError::ExecutionReceipt(result.case_id.clone()));
        }
    }
    Ok(())
}

fn complete_stream(stream: Option<&StreamReceipt>) -> bool {
    stream.is_some_and(|stream| stream.complete && valid_sha256(&stream.sha256))
}

fn statistical_attempts_match(
    case_id: &str,
    report_attempts: &BTreeMap<&str, Vec<&StatisticalAttempt>>,
    receipt_attempts: &[StatisticalAttemptReceipt],
) -> bool {
    let expected = report_attempts.get(case_id).map_or(&[][..], Vec::as_slice);
    if expected.len() != receipt_attempts.len() {
        return false;
    }
    for (report, receipt) in expected.iter().zip(receipt_attempts) {
        if report.outcome != "passed"
            || receipt.verdict != "passed"
            || receipt.seed != report.seed
            || receipt.completed_shots != report.completed_shots
            || receipt.completed_comparisons != report.completed_comparisons
            || receipt.completed_batches != report.completed_batches
        {
            return false;
        }
    }
    true
}

fn validate_preflight(
    preflight: &CorrectnessPreflight,
    report: &CorrectnessReport,
    request_sha256: &str,
    report_sha256: &str,
    completion_sha256: &str,
    schema_family: CorrectnessSchemaFamily,
    results: &BTreeMap<&str, &CaseResult>,
) -> Result<(), CorrectnessError> {
    let expected_cases = results
        .values()
        .map(|result| {
            (
                result.case_id.clone(),
                CorrectnessCaseReceipt {
                    outcome: result.outcome.clone(),
                    selector_sha256: result.selector_sha256.clone(),
                    execution_receipt_sha256: result.execution_receipt_sha256.clone(),
                    stdout_sha256: result.stdout_sha256.clone(),
                    stderr_sha256: result.stderr_sha256.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let expected = CorrectnessPreflight {
        schema_version: schema_family.report_and_preflight_version(),
        report_sha256: report_sha256.to_string(),
        completion_sha256: completion_sha256.to_string(),
        qualification_manifest_digest: report.qualification_manifest_digest.clone(),
        run_request_sha256: request_sha256.to_string(),
        stab_commit: report.stab_commit.clone(),
        local_modifications: report.local_modifications,
        stim_commit: report.stim_commit.clone(),
        tier: report.tier.clone(),
        allow_deferred: report.allow_deferred,
        selection_complete: report.selection_complete,
        deferred_count: report.deferred_count,
        cases: expected_cases,
    };
    if *preflight != expected {
        return Err(CorrectnessError::PreflightContract);
    }
    Ok(())
}

fn parse_canonical<T>(name: &'static str, bytes: &[u8]) -> Result<T, CorrectnessError>
where
    T: DeserializeOwned + Serialize,
{
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(CorrectnessError::ArtifactBoundary(name));
    }
    let value: T = serde_json::from_slice(bytes).map_err(CorrectnessError::Json)?;
    let mut canonical = serde_json::to_vec_pretty(&value).map_err(CorrectnessError::Json)?;
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(CorrectnessError::NonCanonical(name));
    }
    Ok(value)
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
    if components.len() < 3
        || components.first() != Some(&Component::Normal("target".as_ref()))
        || components.get(1) != Some(&Component::Normal("qualification".as_ref()))
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

fn valid_case_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn unique_valid_case_ids(values: &[String]) -> bool {
    values.iter().all(|value| valid_case_id(value))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RunRequest {
    schema_version: u32,
    qualification_manifest_digest: String,
    stab_commit: String,
    worktree_was_clean: bool,
    stim_tag: String,
    stim_commit: String,
    tier: String,
    feature_filters: Vec<String>,
    case_filters: Vec<String>,
    allow_deferred: bool,
    executables: Vec<ExecutableIdentity>,
    execution_environment_sha256: String,
    selected_cases: Vec<RequestedCase>,
    planned_case_ids: Vec<String>,
    deferred_case_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RequestedCase {
    case_id: String,
    selector_sha256: String,
    case_contract_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ExecutableIdentity {
    role: String,
    bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CorrectnessReport {
    schema_version: u32,
    qualification_manifest_digest: String,
    run_request_sha256: String,
    stab_commit: String,
    local_modifications: bool,
    stim_tag: String,
    stim_commit: String,
    rust_toolchain: String,
    target_triple: String,
    operating_system: String,
    architecture: String,
    tier: String,
    feature_filters: Vec<String>,
    case_filters: Vec<String>,
    allow_deferred: bool,
    selected_count: usize,
    planned_count: usize,
    deferred_count: usize,
    passed_count: usize,
    failed_count: usize,
    selection_complete: bool,
    statistical_declared_budget: String,
    statistical_consumed_bound: String,
    statistical_planned_shots: u64,
    statistical_planned_seeds: BTreeMap<String, Vec<u64>>,
    statistical_shots: u64,
    statistical_seeds: BTreeMap<String, Vec<u64>>,
    statistical_attempts: Vec<StatisticalAttempt>,
    property_corpus_ids: Vec<String>,
    resource_case_count: usize,
    upstream_dispositions: Vec<DomainDispositionCount>,
    deferred_products: BTreeMap<String, usize>,
    case_counts: Vec<DomainComparatorCount>,
    resource_contracts: Vec<ResourceExecution>,
    results: Vec<CaseResult>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StatisticalAttempt {
    case_id: String,
    seed: u64,
    completed_shots: u64,
    completed_comparisons: u32,
    completed_batches: u32,
    outcome: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DomainDispositionCount {
    feature_id: String,
    disposition: String,
    count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DomainComparatorCount {
    feature_id: String,
    comparator: String,
    passed: usize,
    failed: usize,
    planned: usize,
    deferred: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ResourceExecution {
    case_id: String,
    kind: String,
    detail: String,
    negative_axes: Vec<String>,
    timeout_ms: u64,
    stdout_limit_bytes: usize,
    stderr_limit_bytes: usize,
    artifact_limit_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CaseResult {
    case_id: String,
    feature_id: String,
    comparator: String,
    selector: EvidenceSelector,
    selector_sha256: String,
    execution_receipt_sha256: String,
    outcome: String,
    exact_test_count: Option<usize>,
    stdout_sha256: Option<String>,
    stderr_sha256: Option<String>,
    artifacts: Vec<ReportArtifact>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceSelector {
    state: String,
    kind: String,
    value: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReportArtifact {
    path: PathBuf,
    bytes: usize,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RunCompletion {
    schema_version: u32,
    run_request_sha256: String,
    report_sha256: String,
    cases: Vec<CompletedCase>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletedCase {
    case_id: String,
    execution_receipt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ExecutionReceipt {
    schema_version: u32,
    run_request_sha256: String,
    case_id: String,
    selector_sha256: String,
    executables: Vec<ExecutableIdentity>,
    execution_environment_sha256: String,
    verdict: String,
    exit_status: Option<i32>,
    exact_test_count: Option<usize>,
    stdout: Option<StreamReceipt>,
    stderr: Option<StreamReceipt>,
    statistical_attempts: Vec<StatisticalAttemptReceipt>,
    auxiliary_outputs: Vec<ReportArtifact>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StreamReceipt {
    bytes: u64,
    sha256: String,
    complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StatisticalAttemptReceipt {
    seed: u64,
    verdict: String,
    completed_shots: u64,
    completed_comparisons: u32,
    completed_batches: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
    deferred_count: usize,
    cases: BTreeMap<String, CorrectnessCaseReceipt>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CorrectnessCaseReceipt {
    outcome: String,
    selector_sha256: String,
    execution_receipt_sha256: String,
    stdout_sha256: Option<String>,
    stderr_sha256: Option<String>,
}

#[derive(Debug, Error)]
pub(super) enum CorrectnessError {
    #[error("diagnostic correctness preflight requires a reason")]
    MissingReason,
    #[error("correctness evidence path must be a normal directory below target/qualification: {0}")]
    InvalidOutput(PathBuf),
    #[error("correctness preflight expectation has an invalid manifest digest or Stab commit")]
    InvalidExpectation,
    #[error("correctness preflight requires unique nonempty case ids")]
    InvalidCases,
    #[error("failed to read correctness evidence: {0}")]
    Read(String),
    #[error("correctness evidence artifact changed before performance publication: {0}")]
    ArtifactChanged(PathBuf),
    #[error("correctness evidence artifact size cannot be represented on this host")]
    SizeOverflow,
    #[cfg(not(unix))]
    #[error("correctness evidence artifact binding requires a Unix host")]
    UnsupportedBindingHost,
    #[error("correctness artifact {0} must be nonempty and newline terminated")]
    ArtifactBoundary(&'static str),
    #[error("correctness artifact {0} is not in canonical generated form")]
    NonCanonical(&'static str),
    #[error("correctness artifact JSON is invalid: {0}")]
    Json(serde_json::Error),
    #[error("correctness request or completion differs from its controller-approved digest")]
    ControllerBinding,
    #[error("correctness request violates the required clean full-or-soak contract")]
    RequestContract,
    #[error("correctness report does not reconstruct from request.json and passing results")]
    ReportContract,
    #[error("correctness completion does not reconstruct from report.json")]
    CompletionContract,
    #[error("correctness preflight does not reconstruct from report.json and completion.json")]
    PreflightContract,
    #[error("correctness execution receipt for case {0} is stale or inconsistent")]
    ExecutionReceipt(String),
    #[error("correctness preflight omits required case {0}")]
    MissingCase(String),
}

#[cfg(test)]
mod tests;
