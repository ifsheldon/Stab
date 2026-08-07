use super::super::identity::Sha256Digest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{
    CompletionError, CompletionScope, QualificationTier, RollupReplayEvidence, find_rollup,
};
use crate::qualification::runtime::correctness::{
    CorrectnessPreflightEvidence, CorrectnessPreflightStatus,
};

const MAX_DISTINCT_CORRECTNESS_ARTIFACTS: usize = 11;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompletionCorrectness {
    pub(super) group_id: String,
    pub(super) evidence: CorrectnessPreflightEvidence,
}

pub(super) fn collect(
    rollups: &[RollupReplayEvidence],
    scope: &CompletionScope,
    expected_correctness_inventory_sha256: &str,
) -> Result<Vec<CompletionCorrectness>, CompletionError> {
    let collected = scope
        .group_ids
        .iter()
        .map(|group_id| {
            let full = find_rollup(rollups, group_id, QualificationTier::Full)?;
            let soak = find_rollup(rollups, group_id, QualificationTier::Soak)?;
            let expected_case_ids = scope
                .correctness_case_ids
                .get(group_id)
                .ok_or_else(|| CompletionError::GroupCorrectness(group_id.clone()))?;
            if full.correctness_preflight != soak.correctness_preflight
                || !valid_evidence(
                    &full.correctness_preflight,
                    expected_case_ids,
                    expected_correctness_inventory_sha256,
                )
            {
                return Err(CompletionError::GroupCorrectness(group_id.clone()));
            }
            Ok(CompletionCorrectness {
                group_id: group_id.clone(),
                evidence: full.correctness_preflight.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !shared_prerequisites_are_consistent(&collected) {
        return Err(CompletionError::CorrectnessArtifactCount);
    }
    Ok(collected)
}

pub(super) fn valid_manifest(
    correctness_preflights: &[CompletionCorrectness],
    scope: &CompletionScope,
    correctness_inventory_sha256: &str,
) -> bool {
    correctness_preflights.len() == scope.group_ids.len()
        && scope.correctness_case_ids.len() == scope.group_ids.len()
        && correctness_preflights.iter().zip(&scope.group_ids).all(
            |(correctness, expected_group_id)| {
                correctness.group_id == *expected_group_id
                    && scope
                        .correctness_case_ids
                        .get(expected_group_id)
                        .is_some_and(|expected_case_ids| {
                            valid_evidence(
                                &correctness.evidence,
                                expected_case_ids,
                                correctness_inventory_sha256,
                            )
                        })
            },
        )
        && shared_prerequisites_are_consistent(correctness_preflights)
}

fn shared_prerequisites_are_consistent(correctness_preflights: &[CompletionCorrectness]) -> bool {
    let mut by_case_set = BTreeMap::<&[String], &CorrectnessPreflightEvidence>::new();
    for correctness in correctness_preflights {
        match by_case_set.get(correctness.evidence.case_ids.as_slice()) {
            Some(existing) if **existing != correctness.evidence => return false,
            Some(_) => {}
            None => {
                by_case_set.insert(
                    correctness.evidence.case_ids.as_slice(),
                    &correctness.evidence,
                );
            }
        }
    }
    by_case_set.len() <= MAX_DISTINCT_CORRECTNESS_ARTIFACTS
}

fn valid_evidence(
    evidence: &CorrectnessPreflightEvidence,
    expected_case_ids: &[String],
    expected_correctness_inventory_sha256: &str,
) -> bool {
    evidence.status == CorrectnessPreflightStatus::Passed
        && evidence.case_ids == expected_case_ids
        && !evidence.reason.is_empty()
        && evidence
            .source_directory
            .as_deref()
            .is_some_and(|path| !path.is_empty())
        && evidence.qualification_manifest_sha256.as_deref()
            == Some(expected_correctness_inventory_sha256)
        && [
            evidence.request_sha256.as_deref(),
            evidence.completion_sha256.as_deref(),
            evidence.report_sha256.as_deref(),
            evidence.preflight_sha256.as_deref(),
        ]
        .into_iter()
        .all(|digest| digest.is_some_and(Sha256Digest::is_valid_str))
}
