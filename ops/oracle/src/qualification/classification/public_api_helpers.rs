use super::FeatureId;

pub(super) fn classify_extracted_analysis_api(
    source_path: &str,
    api_lower: &str,
) -> Option<FeatureId> {
    if api_lower.ends_with("::decomposed_circuit")
        || api_lower.ends_with("::simplified_circuit")
        || source_path == "crates/stab-analysis/src/circuit_simplify.rs"
    {
        return Some(FeatureId::CircuitApi);
    }
    if source_path == "crates/stab-analysis/src/circuit_tableau.rs" {
        return Some(FeatureId::Algebra);
    }
    None
}

pub(super) fn api_path_mentions_item(api_path: &str, item: &str) -> bool {
    api_path.split("::").any(|segment| {
        segment == item
            || segment
                .strip_prefix(item)
                .is_some_and(|suffix| suffix.starts_with(" as "))
    })
}

pub(super) fn is_resource_policy_api(api_lower: &str) -> bool {
    api_lower.ends_with("_with_limits")
        || api_lower.ends_with("_and_limits")
        || api_lower.ends_with("::compile_with_limits")
        || api_lower.ends_with("::validate_replay_work_units")
        || api_lower.ends_with("::try_for_each_detection_event_from_error_records")
        || api_lower.contains("resource_estimate")
        || api_lower.contains("_limit_error")
        || [
            "parselimits",
            "repeatnestinglimit",
            "resourceestimate",
            "sourcelinelimit",
            "estimateclass",
            "circuitflattenlimits",
            "demflattenlimits",
            "detectionconversionlimits",
            "demsamplerlimits",
            "logicalerrorsearchlimits",
            "satmaterializationlimits",
        ]
        .iter()
        .any(|name| api_lower.contains(name))
        || api_path_mentions_item(api_lower, "estimate")
}
