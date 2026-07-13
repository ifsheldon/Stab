use std::collections::{BTreeMap, BTreeSet};

use super::{
    RunError, current_environment_evidence, deferred_product_counts, expected_selector_digests,
    parse_case_ids, select_cases, statistical_run_plan, upstream_disposition_counts,
    validate_case_feature_filters, validate_selection_policy,
};
use crate::RepoRoot;
use crate::qualification::model::{Comparator, EvidenceCase, QualificationManifest};
use crate::qualification::receipt::RunRequestReceipt;
use crate::qualification::report::{
    ExpectedCase, ExpectedDispositionCase, ReportExpectation, ResourceExecution,
};

pub(in crate::qualification) fn report_expectation(
    root: &RepoRoot,
    manifest: &QualificationManifest,
    request: &RunRequestReceipt,
    request_sha256: &str,
) -> Result<ReportExpectation, RunError> {
    let (metadata, current_metadata_executables) = current_environment_evidence(root, manifest)?;
    if !request.schema_is_current()
        || request.qualification_manifest_digest != manifest.semantic_digest.to_string()
        || request.stab_commit != metadata.stab_commit
        || request.worktree_was_clean != !metadata.local_modifications
        || request.stim_tag != metadata.stim_tag
        || request.stim_commit != metadata.stim_commit
    {
        return Err(RunError::ReportContract(
            "run-request receipt metadata is stale".into(),
        ));
    }
    super::super::executables::validate_identities(&request.executables)
        .map_err(|source| RunError::ReportContract(source.to_string().into_boxed_str()))?;
    let request_metadata_executables = request
        .executables
        .iter()
        .filter(|identity| {
            current_metadata_executables
                .iter()
                .any(|current| current.role == identity.role)
        })
        .cloned()
        .collect::<Vec<_>>();
    if request_metadata_executables != current_metadata_executables {
        return Err(RunError::ReportContract(
            "run-request metadata executables differ from the currently resolved sealed tools"
                .into(),
        ));
    }
    if !request
        .execution_environment_sha256
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || request.execution_environment_sha256.len() != 64
    {
        return Err(RunError::ReportContract(
            "run-request execution environment digest is malformed".into(),
        ));
    }
    let features = request
        .feature_filters
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if features.len() != request.feature_filters.len()
        || features.iter().copied().collect::<Vec<_>>() != request.feature_filters
    {
        return Err(RunError::ReportContract(
            "feature filters are duplicated or not in canonical order".into(),
        ));
    }
    let case_ids = parse_case_ids(manifest, &request.case_filters)?;
    if case_ids.len() != request.case_filters.len()
        || case_ids.iter().cloned().collect::<Vec<_>>() != request.case_filters
    {
        return Err(RunError::ReportContract(
            "case filters are duplicated or not in canonical order".into(),
        ));
    }
    validate_case_feature_filters(manifest, &features, &case_ids)?;
    let borrowed_ids = case_ids.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let selection = select_cases(
        &manifest.evidence_cases,
        request.tier,
        &features,
        &borrowed_ids,
    );
    validate_selection_policy(
        &selection,
        !request.case_filters.is_empty(),
        request.allow_deferred,
    )?;
    let statistical = statistical_run_plan(root, &selection.selected, request.tier)?;
    let selector_ids = selection
        .selected
        .iter()
        .map(|case| case.id.to_string())
        .collect::<Vec<_>>();
    let selector_digests = expected_selector_digests(root, manifest, &selector_ids)?;
    let fixture_expected_statuses = crate::fixtures::qualification_expected_statuses(root)
        .map_err(|source| RunError::Environment(source.to_string().into_boxed_str()))?;
    let expected_requested_cases = selection
        .selected
        .iter()
        .map(|case| {
            Ok(crate::qualification::receipt::RequestedCase {
                case_id: case.id.to_string(),
                selector_sha256: selector_digests.get(case.id.as_str()).cloned().ok_or_else(
                    || {
                        RunError::ReportContract(
                            format!("selected case {} has no resolved selector digest", case.id)
                                .into_boxed_str(),
                        )
                    },
                )?,
                case_contract_sha256: crate::qualification::receipt::case_contract_sha256(*case)
                    .map_err(|source| {
                        RunError::ReportContract(source.to_string().into_boxed_str())
                    })?,
            })
        })
        .collect::<Result<Vec<_>, RunError>>()?;
    let planned_ids = selection
        .planned
        .iter()
        .map(|case| case.id.to_string())
        .collect::<Vec<_>>();
    let deferred_ids = selection
        .deferred
        .iter()
        .map(|case| case.id.to_string())
        .collect::<Vec<_>>();
    if request.selected_cases != expected_requested_cases
        || request.planned_case_ids != planned_ids
        || request.deferred_case_ids != deferred_ids
    {
        return Err(RunError::ReportContract(
            "run-request receipt selection or case contracts are stale".into(),
        ));
    }
    let selected_cases = selection
        .selected
        .iter()
        .map(|case| {
            let receipt_attempt_count = statistical.seeds.get(case.id.as_str()).map_or(1, Vec::len);
            let receipt_limit = |label: &str, per_attempt: usize| -> Result<u64, RunError> {
                let total = per_attempt
                    .checked_mul(receipt_attempt_count)
                    .ok_or_else(|| {
                        RunError::ReportContract(
                            format!("selected case {} {label} receipt limit overflowed", case.id)
                                .into_boxed_str(),
                        )
                    })?;
                u64::try_from(total).map_err(|_| {
                    RunError::ReportContract(
                        format!(
                            "selected case {} {label} receipt limit exceeds u64",
                            case.id
                        )
                        .into_boxed_str(),
                    )
                })
            };
            let selector_sha256 =
                selector_digests
                    .get(case.id.as_str())
                    .cloned()
                    .ok_or_else(|| {
                        RunError::ReportContract(
                            format!("selected case {} has no resolved selector digest", case.id)
                                .into_boxed_str(),
                        )
                    })?;
            let expected_exit_status = match case.primary_selector.kind {
                crate::qualification::model::SelectorKind::OracleFixture => {
                    let [fixture_id] = case.primary_selector.value.as_slice() else {
                        return Err(RunError::ReportContract(
                            format!("selected case {} has an invalid fixture selector", case.id)
                                .into_boxed_str(),
                        ));
                    };
                    fixture_expected_statuses
                        .get(fixture_id)
                        .copied()
                        .ok_or_else(|| {
                            RunError::ReportContract(
                                format!(
                                    "selected case {} references fixture {fixture_id:?} without an expected status",
                                    case.id
                                )
                                .into_boxed_str(),
                            )
                        })?
                }
                crate::qualification::model::SelectorKind::CargoTest
                | crate::qualification::model::SelectorKind::OpsCheck
                | crate::qualification::model::SelectorKind::PropertyTarget => 0,
            };
            Ok((
                case.id.to_string(),
                ExpectedCase {
                    feature_id: case.feature_id,
                    comparator: case.comparator,
                    selector: case.primary_selector.clone(),
                    selector_sha256,
                    expected_exit_status,
                    artifact_limit_bytes: case.execution.artifact_limit_bytes,
                    stdout_receipt_limit_bytes: receipt_limit(
                        "stdout",
                        case.execution.stdout_limit_bytes,
                    )?,
                    stderr_receipt_limit_bytes: receipt_limit(
                        "stderr",
                        case.execution.stderr_limit_bytes,
                    )?,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, RunError>>()?;
    let disposition = |case: &&EvidenceCase| ExpectedDispositionCase {
        feature_id: case.feature_id,
        comparator: case.comparator,
        deferred_product: case.deferred_product,
    };
    let resource_contracts = selection
        .selected
        .iter()
        .copied()
        .filter(|case| case.comparator == Comparator::Resource)
        .map(|case| ResourceExecution {
            case_id: case.id.to_string(),
            kind: case.resource_contract.kind.as_str().to_string(),
            detail: case.resource_contract.detail.clone(),
            negative_axes: case.negative_axes.clone(),
            timeout_ms: case.execution.timeout_ms,
            stdout_limit_bytes: case.execution.stdout_limit_bytes,
            stderr_limit_bytes: case.execution.stderr_limit_bytes,
            artifact_limit_bytes: case.execution.artifact_limit_bytes,
        })
        .collect();
    Ok(ReportExpectation {
        metadata,
        run_request_sha256: request_sha256.to_string(),
        executables: request.executables.clone(),
        execution_environment_sha256: request.execution_environment_sha256.clone(),
        tier: request.tier,
        feature_filters: features.iter().copied().collect(),
        case_filters: case_ids.iter().cloned().collect(),
        allow_deferred: request.allow_deferred,
        selected_cases,
        planned_cases: selection.planned.iter().map(disposition).collect(),
        deferred_cases: selection.deferred.iter().map(disposition).collect(),
        statistical_declared_budget: statistical.declared_bound,
        statistical_planned_shots: statistical.shots,
        statistical_planned_seeds: statistical.seeds,
        statistical_shots_per_batch: statistical
            .attempt_contracts
            .iter()
            .map(|(id, contract)| (id.clone(), contract.shots_per_batch))
            .collect(),
        statistical_comparisons_per_attempt: statistical
            .attempt_contracts
            .iter()
            .map(|(id, contract)| (id.clone(), contract.comparisons_per_attempt))
            .collect(),
        statistical_batches_per_attempt: statistical
            .attempt_contracts
            .iter()
            .map(|(id, contract)| (id.clone(), contract.batches_per_attempt))
            .collect(),
        statistical_shots_per_attempt: statistical.shots_per_attempt,
        statistical_exact_bound_per_attempt: statistical.exact_bound_per_attempt,
        property_corpus_ids: selection
            .selected
            .iter()
            .filter(|case| case.comparator == Comparator::Property)
            .map(|case| case.source_id.clone())
            .collect(),
        resource_contracts,
        upstream_dispositions: upstream_disposition_counts(manifest, &features),
        deferred_products: deferred_product_counts(&selection.deferred),
    })
}
