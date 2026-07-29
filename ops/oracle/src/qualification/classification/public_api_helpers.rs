use super::FeatureId;

pub(super) fn classify_extracted_analysis_api(
    source_path: &str,
    api_lower: &str,
) -> Option<FeatureId> {
    if is_generation_api(api_lower) {
        return Some(FeatureId::Generation);
    }
    if is_flow_api(api_lower) {
        return Some(FeatureId::FlowUtils);
    }
    if source_path.starts_with("crates/stab-analysis/src/circuit_generation") {
        return Some(FeatureId::Generation);
    }
    if source_path.starts_with("crates/stab-analysis/src/circuit_flow")
        || source_path.starts_with("crates/stab-analysis/src/sparse_rev_frame_tracker")
    {
        return Some(FeatureId::FlowUtils);
    }
    if source_path == "crates/stab-analysis/src/mbqc_decomposition.rs" {
        return Some(FeatureId::CircuitApi);
    }
    if api_lower.ends_with("::decomposed_circuit")
        || api_lower.ends_with("::simplified_circuit")
        || matches!(
            source_path,
            "crates/stab-analysis/src/circuit_simplify.rs"
                | "crates/stab-analysis/src/circuit_transforms.rs"
        )
    {
        return Some(FeatureId::CircuitApi);
    }
    if source_path == "crates/stab-analysis/src/circuit_tableau.rs" {
        return Some(FeatureId::Algebra);
    }
    None
}

fn is_generation_api(api_lower: &str) -> bool {
    [
        "codedistance",
        "roundcount",
        "repetitioncodetask",
        "surfacecodetask",
        "colorcodetask",
        "repetitioncodeparams",
        "surfacecodeparams",
        "colorcodeparams",
        "generatedcircuit",
    ]
    .iter()
    .any(|item| api_path_mentions_item(api_lower, item))
        || api_lower.ends_with("::generate_repetition_code_circuit")
        || api_lower.ends_with("::generate_surface_code_circuit")
        || api_lower.ends_with("::generate_color_code_circuit")
}

fn is_flow_api(api_lower: &str) -> bool {
    [
        "unsignedstabilizerflowcheck",
        "unsignedstabilizerflowfailure",
    ]
    .iter()
    .any(|item| api_path_mentions_item(api_lower, item))
        || api_lower.rsplit("::").next().is_some_and(|function| {
            [
                "check_if_circuit_has_unsigned_stabilizer_flows",
                "check_unsigned_stabilizer_flows_with_diagnostics",
                "circuit_flow_generators",
                "circuit_has_all_unsigned_stabilizer_flows",
                "circuit_has_unsigned_stabilizer_flow",
                "solve_for_flow_measurements",
            ]
            .contains(&function)
        })
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
            "resourcekind",
            "resourcelimiterror",
            "resourceoperation",
            "satmaterializationlimits",
        ]
        .iter()
        .any(|name| api_lower.contains(name))
        || api_path_mentions_item(api_lower, "estimate")
}
