use std::collections::BTreeMap;

use super::Issues;
use crate::qualification::discovery::SourceReferences;
use crate::qualification::model::{
    EvidenceState, FixtureLocator, InputByteCount, QualificationGroup, ScalePoint, ThresholdPolicy,
};

pub(super) fn validate_planned_workload(
    group: &QualificationGroup,
    references: &SourceReferences,
    issues: &mut Issues,
) {
    let expected_scale_ids = ["small", "medium", "large"];
    let actual_scale_ids = group
        .workload_family
        .scales
        .iter()
        .map(|scale| scale.id.as_str())
        .collect::<Vec<_>>();
    let expected_generator = if group.id.starts_with("PERFQ-API-") {
        "api-owner-phase-v1"
    } else if group.id.starts_with("PERFQ-CHECKLIST-") {
        "checklist-child-v1"
    } else if group.id == "PERFQ-RESOURCE-BOUNDARIES" {
        "resource-boundary-matrix-v1"
    } else {
        issues.push(format!(
            "planned group {} has no registered workload generator",
            group.id
        ));
        return;
    };
    let expected_fixture_id = if group.id.starts_with("PERFQ-API-") {
        format!("api-small-medium-large-17-{}", group.id)
    } else if group.id.starts_with("PERFQ-CHECKLIST-") {
        format!("checklist-small-medium-large-17-{}", group.id)
    } else {
        "resource-boundary-matrix".to_string()
    };
    if actual_scale_ids != expected_scale_ids
        || is_placeholder(&group.workload_family.deterministic_seed)
        || !matches!(
            &group.workload_family.fixture,
            FixtureLocator::Generated { id } if id == &expected_fixture_id
        )
    {
        issues.push(format!(
            "planned group {} lacks exact small/medium/large scales, a concrete seed, or generated-corpus semantics",
            group.id
        ));
    }
    let expected_sizes = [1_u64, 64, 4_096];
    for (index, scale) in group.workload_family.scales.iter().enumerate() {
        let Some(parameters) = parameter_map(&group.id, &scale.parameters, issues) else {
            continue;
        };
        if parameters.get("generator") != Some(&expected_generator)
            || parameters.get("seed") != Some(&group.workload_family.deterministic_seed.as_str())
            || parameters.values().any(|value| is_placeholder(value))
        {
            issues.push(format!(
                "planned group {} scale {} has an unregistered generator, mismatched seed, or placeholder value",
                group.id, scale.id
            ));
        }
        if group.id.starts_with("PERFQ-API-") {
            validate_parameter_keys(
                group,
                scale,
                &parameters,
                &["fixture_group", "generator", "seed", "semantic_items"],
                issues,
            );
            let expected_size = expected_sizes.get(index).copied();
            validate_scale_value(
                group,
                scale,
                &parameters,
                "semantic_items",
                expected_size,
                issues,
            );
            validate_input_byte_count(group, scale, expected_size, issues);
            let expected_fixture_group = references
                .public_api
                .iter()
                .filter(|(_, api)| {
                    group.public_api_items.contains(&api.path)
                        && api.performance_groups.contains(&group.performance_feature)
                        && group.correctness_cases.contains(&api.owner_case_id)
                })
                .map(|(id, _)| id.as_str())
                .min();
            if parameters.get("fixture_group").copied() != expected_fixture_group {
                issues.push(format!(
                    "planned API group {} scale {} lacks an exact CQ API fixture group",
                    group.id, scale.id
                ));
            }
        } else if group.id.starts_with("PERFQ-CHECKLIST-") {
            let mut expected_keys = vec!["fixture_group", "generator", "seed"];
            expected_keys.push(group.work_unit.as_str());
            expected_keys.sort_unstable();
            validate_parameter_keys(group, scale, &parameters, &expected_keys, issues);
            let expected_size = expected_sizes.get(index).copied();
            validate_scale_value(
                group,
                scale,
                &parameters,
                &group.work_unit,
                expected_size,
                issues,
            );
            validate_input_byte_count(group, scale, expected_size, issues);
            if parameters.get("fixture_group") != Some(&group.id.as_str()) {
                issues.push(format!(
                    "planned checklist group {} scale {} has the wrong fixture group",
                    group.id, scale.id
                ));
            }
        } else {
            validate_parameter_keys(
                group,
                scale,
                &parameters,
                &[
                    "boundary_probe",
                    "generator",
                    "input_bytes",
                    "records",
                    "search_states",
                    "seed",
                ],
                issues,
            );
            validate_resource_scale(group, scale, index, &parameters, issues);
        }
    }
    if group.output_contract.digest_state != EvidenceState::Planned
        || group.timing_policy.calibration_min_ms != 250
        || group.timing_policy.calibration_max_ms != 2_000
        || group.timing_policy.warmup_batches != 3
        || group.timing_policy.full_pairs != 9
        || group.timing_policy.timeout_seconds != 600
        || group.timing_policy.gate_statistic
            != "median paired ratio and fixed-seed bootstrap 95% upper bound"
        || group.memory_policy.scale_ids != expected_scale_ids
        || group.threshold_policy != ThresholdPolicy::ReportOnly
    {
        issues.push(format!(
            "planned group {} has an incomplete output, timing, memory, or threshold contract",
            group.id
        ));
    }
}

fn validate_resource_scale(
    group: &QualificationGroup,
    scale: &ScalePoint,
    index: usize,
    parameters: &BTreeMap<&str, &str>,
    issues: &mut Issues,
) {
    let expected = [
        (1_024_u64, 64_u64, 1_024_u64),
        (1_048_576, 4_096, 100_000),
        (67_108_864, 1_000_000, 1_000_000),
    ];
    if let Some((input_bytes, records, search_states)) = expected.get(index).copied() {
        for (key, value) in [
            ("input_bytes", input_bytes),
            ("records", records),
            ("search_states", search_states),
        ] {
            validate_scale_value(group, scale, parameters, key, Some(value), issues);
        }
        if scale.input_bytes != (InputByteCount::Exact { bytes: input_bytes }) {
            issues.push(format!(
                "resource-boundary scale {} has the wrong typed input byte count",
                scale.id
            ));
        }
    }
    if parameters.get("boundary_probe") != Some(&"declared-cap-and-cap-plus-one-outside-timing") {
        issues.push(format!(
            "resource-boundary scale {} lacks the frozen cap probe",
            scale.id
        ));
    }
}

fn validate_input_byte_count(
    group: &QualificationGroup,
    scale: &ScalePoint,
    expected_size: Option<u64>,
    issues: &mut Issues,
) {
    let expected = if group.work_unit == "bytes" {
        expected_size.map(|bytes| InputByteCount::Exact { bytes })
    } else {
        Some(InputByteCount::NotApplicable)
    };
    if Some(scale.input_bytes) != expected {
        issues.push(format!(
            "planned group {} scale {} has the wrong typed input byte count",
            group.id, scale.id
        ));
    }
}

fn parameter_map<'a>(
    group_id: &str,
    value: &'a str,
    issues: &mut Issues,
) -> Option<BTreeMap<&'a str, &'a str>> {
    let mut parameters = BTreeMap::new();
    for part in value
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let Some((key, value)) = part.split_once('=') else {
            issues.push(format!(
                "planned group {group_id} has malformed parameter {part:?}"
            ));
            return None;
        };
        if key.is_empty()
            || value.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || parameters.insert(key, value).is_some()
        {
            issues.push(format!(
                "planned group {group_id} has invalid or duplicate parameter {key:?}"
            ));
            return None;
        }
    }
    Some(parameters)
}

fn validate_parameter_keys(
    group: &QualificationGroup,
    scale: &ScalePoint,
    parameters: &BTreeMap<&str, &str>,
    expected: &[&str],
    issues: &mut Issues,
) {
    let actual = parameters.keys().copied().collect::<Vec<_>>();
    if actual != expected {
        issues.push(format!(
            "planned group {} scale {} has parameter keys {actual:?}, expected {expected:?}",
            group.id, scale.id
        ));
    }
}

fn validate_scale_value(
    group: &QualificationGroup,
    scale: &ScalePoint,
    parameters: &BTreeMap<&str, &str>,
    key: &str,
    expected: Option<u64>,
    issues: &mut Issues,
) {
    let actual = parameters
        .get(key)
        .and_then(|value| value.parse::<u64>().ok());
    if actual != expected {
        issues.push(format!(
            "planned group {} scale {} has {key}={actual:?}, expected {expected:?}",
            group.id, scale.id
        ));
    }
}

fn is_placeholder(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.trim().is_empty()
        || [
            "todo",
            "tbd",
            "decide",
            "later",
            "must bind",
            "placeholder",
            "unknown",
            "unset",
            "fixme",
        ]
        .iter()
        .any(|marker| value.contains(marker))
}
