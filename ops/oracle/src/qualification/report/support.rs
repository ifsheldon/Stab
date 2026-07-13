use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use sha2::{Digest as _, Sha256};

use crate::qualification::artifact::{MAX_REPORT_BYTES, QualificationOutputDir};
use crate::qualification::model::{Comparator, EvidenceSelector, FeatureId};
use crate::qualification::receipt::{CompletedCase, RunCompletionReceipt};

use super::{
    CaseOutcome, CorrectnessPreflight, CorrectnessPreflightCase, DomainComparatorCount,
    PreflightExpectation, QualificationReport, ReportError, ReportExpectation,
};

pub(in crate::qualification) fn regenerate(
    output: &QualificationOutputDir,
    expectation: &ReportExpectation,
) -> Result<(), ReportError> {
    let report_bytes = output.read(Path::new("report.json"), MAX_REPORT_BYTES)?;
    let report: QualificationReport = serde_json::from_slice(&report_bytes)?;
    report.validate(output, expectation)?;
    if canonical_json(&report)? != report_bytes {
        return Err(ReportError::Validation(
            "report.json is not in canonical generated form".into(),
        ));
    }
    let completion_sha256 = validate_completion(output, &report, &sha256(&report_bytes), None)?;
    output.write(Path::new("report.md"), report.markdown().as_bytes())?;
    let preflight =
        CorrectnessPreflight::from_report(&report, sha256(&report_bytes), completion_sha256);
    output.write(Path::new("preflight.json"), &canonical_json(&preflight)?)?;
    Ok(())
}

pub(in crate::qualification) fn validate_preflight(
    output: &QualificationOutputDir,
    report_expectation: &ReportExpectation,
    expectation: &PreflightExpectation,
    expected_completion_sha256: &str,
) -> Result<(), ReportError> {
    let report_bytes = output.read(Path::new("report.json"), MAX_REPORT_BYTES)?;
    let report: QualificationReport = serde_json::from_slice(&report_bytes)?;
    report.validate(output, report_expectation)?;
    if canonical_json(&report)? != report_bytes {
        return Err(ReportError::Validation(
            "report.json is not in canonical generated form".into(),
        ));
    }
    let completion_sha256 = validate_completion(
        output,
        &report,
        &sha256(&report_bytes),
        Some(expected_completion_sha256),
    )?;
    let preflight_bytes = output.read(Path::new("preflight.json"), MAX_REPORT_BYTES)?;
    let preflight: CorrectnessPreflight = serde_json::from_slice(&preflight_bytes)?;
    if canonical_json(&preflight)? != preflight_bytes {
        return Err(ReportError::Validation(
            "preflight.json is not in canonical generated form".into(),
        ));
    }
    let expected_report_sha256 = sha256(&report_bytes);
    let report_cases = report
        .results
        .iter()
        .map(|result| {
            (
                result.case_id.clone(),
                CorrectnessPreflightCase {
                    outcome: result.outcome,
                    selector_sha256: result.selector_sha256.clone(),
                    execution_receipt_sha256: result.execution_receipt_sha256.clone(),
                    stdout_sha256: result.stdout_sha256.clone(),
                    stderr_sha256: result.stderr_sha256.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    if preflight.cases != report_cases
        || preflight.local_modifications != report.local_modifications
        || preflight.selection_complete != report.selection_complete
        || preflight.deferred_count != report.deferred_count
        || preflight.tier != report.tier
        || preflight.allow_deferred != report.allow_deferred
        || preflight.qualification_manifest_digest != report.qualification_manifest_digest
        || preflight.completion_sha256 != completion_sha256
        || preflight.run_request_sha256 != report.run_request_sha256
        || preflight.stab_commit != report.stab_commit
        || preflight.stim_commit != report.stim_commit
    {
        return Err(ReportError::Preflight(
            "preflight case or run metadata disagrees with report.json".into(),
        ));
    }
    preflight.validate_cases(expectation, &expected_report_sha256)
}

pub(super) fn completed_cases(report: &QualificationReport) -> Vec<CompletedCase> {
    report
        .results
        .iter()
        .map(|result| CompletedCase {
            case_id: result.case_id.clone(),
            execution_receipt_sha256: result.execution_receipt_sha256.clone(),
        })
        .collect()
}

pub(super) fn validate_completion(
    output: &QualificationOutputDir,
    report: &QualificationReport,
    report_sha256: &str,
    expected_completion_sha256: Option<&str>,
) -> Result<String, ReportError> {
    let (completion, actual_completion_sha256) = RunCompletionReceipt::read(output)
        .map_err(|source| ReportError::Validation(source.to_string().into_boxed_str()))?;
    if !completion.schema_is_current()
        || completion.run_request_sha256 != report.run_request_sha256
        || completion.report_sha256 != report_sha256
        || completion.cases != completed_cases(report)
        || expected_completion_sha256.is_some_and(|expected| expected != actual_completion_sha256)
    {
        return Err(ReportError::Preflight(
            "completion receipt is stale or disagrees with its controller-approved digest".into(),
        ));
    }
    Ok(actual_completion_sha256)
}

pub(super) fn expected_case_counts(
    report: &QualificationReport,
    expectation: &ReportExpectation,
) -> Vec<DomainComparatorCount> {
    let mut counts = BTreeMap::<(FeatureId, Comparator), [usize; 4]>::new();
    for result in &report.results {
        let values = counts
            .entry((result.feature_id, result.comparator))
            .or_default();
        match result.outcome {
            CaseOutcome::Passed => values[0] += 1,
            CaseOutcome::Failed => values[1] += 1,
        }
    }
    for case in &expectation.planned_cases {
        counts
            .entry((case.feature_id, case.comparator))
            .or_default()[2] += 1;
    }
    for case in &expectation.deferred_cases {
        counts
            .entry((case.feature_id, case.comparator))
            .or_default()[3] += 1;
    }
    counts
        .into_iter()
        .map(
            |((feature_id, comparator), [passed, failed, planned, deferred])| {
                DomainComparatorCount {
                    feature_id,
                    comparator,
                    passed,
                    failed,
                    planned,
                    deferred,
                }
            },
        )
        .collect()
}

pub(super) fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, ReportError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(in crate::qualification) fn selector_sha256(
    selector: &EvidenceSelector,
) -> Result<String, ReportError> {
    let bytes = serde_json::to_vec(selector)?;
    Ok(sha256(&bytes))
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
