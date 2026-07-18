use std::collections::{BTreeMap, BTreeSet};

use super::super::model::{
    CorrectnessBinding, EvidenceState, ManifestRowDisposition, PerformanceDisposition,
    QualificationGroup, QualificationStatus, RowDecision, RunnerFidelity, ThresholdPolicy,
};
use super::Issues;
use super::values::{validate_identifier, validate_text};

pub(super) fn validate(
    row: &ManifestRowDisposition,
    primary_feature: Option<&str>,
    groups: &BTreeMap<&str, &QualificationGroup>,
    measurement_pairs: &BTreeSet<(&String, &String, &String)>,
    issues: &mut Issues,
) {
    let mut replacement_sources = BTreeSet::new();
    let mut replacement_targets = BTreeSet::new();
    for replacement in &row.replacement_contracts {
        validate_text(
            "legacy replacement Stim measurement",
            &replacement.legacy_stim_name,
            issues,
        );
        validate_text(
            "legacy replacement Stab measurement",
            &replacement.legacy_stab_name,
            issues,
        );
        validate_identifier(
            "legacy replacement runtime group",
            &replacement.runtime_group_id,
            issues,
        );
        validate_identifier(
            "legacy replacement runtime measurement",
            &replacement.runtime_measurement_id,
            issues,
        );
        if let Some(scale) = &replacement.runtime_scale_id {
            validate_identifier("legacy replacement runtime scale", scale, issues);
        }
        let source = (
            replacement.legacy_stim_name.as_str(),
            replacement.legacy_stab_name.as_str(),
        );
        let target = (
            replacement.runtime_group_id.as_str(),
            replacement.runtime_measurement_id.as_str(),
            replacement.runtime_scale_id.as_deref(),
        );
        if !replacement_sources.insert(source) {
            issues.push(format!(
                "manifest row {} repeats replacement source {:?}/{:?}",
                row.id, replacement.legacy_stim_name, replacement.legacy_stab_name
            ));
        }
        if !replacement_targets.insert(target) {
            issues.push(format!(
                "manifest row {} repeats replacement target {:?}/{:?}/{:?}",
                row.id,
                replacement.runtime_group_id,
                replacement.runtime_measurement_id,
                replacement.runtime_scale_id
            ));
        }
        if !measurement_pairs.iter().any(|(stim, stab, ratio)| {
            stim.as_str() == replacement.legacy_stim_name
                && stab.as_str() == replacement.legacy_stab_name
                && ratio.as_str() == "1.25"
        }) {
            issues.push(format!(
                "manifest row {} replacement source {:?}/{:?} is not an exact legacy threshold pair",
                row.id, replacement.legacy_stim_name, replacement.legacy_stab_name
            ));
        }
        match groups.get(replacement.runtime_group_id.as_str()) {
            Some(group)
                if group.performance_feature == primary_feature.unwrap_or_default()
                    && group.disposition == PerformanceDisposition::Measured
                    && group.runner_fidelity == RunnerFidelity::AdapterLibrary
                    && group.correctness_binding == CorrectnessBinding::ExactCases
                    && !group.correctness_cases.is_empty()
                    && group.output_contract.digest_state == EvidenceState::Existing
                    && group.threshold_policy == ThresholdPolicy::Primary1_25
                    && group.status != QualificationStatus::Planned => {
                if let Some(scale) = &replacement.runtime_scale_id
                    && !group
                        .workload_family
                        .scales
                        .iter()
                        .any(|candidate| candidate.id == *scale)
                {
                    issues.push(format!(
                        "manifest row {} replacement target {}/{}/{} references an unknown runtime scale",
                        row.id,
                        replacement.runtime_group_id,
                        replacement.runtime_measurement_id,
                        scale
                    ));
                }
            }
            Some(_) => issues.push(format!(
                "manifest row {} replacement target {} is not an exact implemented primary contract in the same performance feature",
                row.id, replacement.runtime_group_id
            )),
            None => issues.push(format!(
                "manifest row {} replacement target references unknown group {}",
                row.id, replacement.runtime_group_id
            )),
        }
    }
    if !row.replacement_contracts.is_empty() && row.decision != RowDecision::Reworked {
        issues.push(format!(
            "manifest row {} declares replacement contracts without a reworked decision",
            row.id
        ));
    }
    if row.id == "m6-clifford-string"
        && !matches!(
            row.replacement_contracts.as_slice(),
            [replacement]
                if replacement.legacy_stim_name == "CliffordString_multiplication_10K"
                    && replacement.legacy_stab_name
                        == "stab_clifford_string_multiplication_10K"
                    && replacement.runtime_group_id == "PERFQ-M6-CLIFFORD-STRING"
                    && replacement.runtime_measurement_id == "right-multiply-identity"
                    && replacement.runtime_scale_id.as_deref() == Some("small")
        )
    {
        issues.push(
            "manifest row m6-clifford-string must map its exact legacy pair only to PERFQ-M6-CLIFFORD-STRING/right-multiply-identity/small"
                .to_string(),
        );
    }
}
