use super::FeatureId;

pub(super) fn classify_component_crate_api(
    crate_name: &str,
    source_path: &str,
    api_path: &str,
) -> Option<FeatureId> {
    if crate_name == "stab_cli" {
        return Some(FeatureId::Cli);
    }
    if source_path.starts_with("crates/stab-kernels-simd/") || crate_name == "stab_kernels_simd" {
        return Some(if api_path.contains("clifford") {
            FeatureId::Algebra
        } else {
            FeatureId::BitKernels
        });
    }
    None
}

pub(super) fn classify_facade_tier_api(crate_name: &str, api_path: &str) -> Option<FeatureId> {
    if crate_name != "stab_core" {
        return None;
    }
    if api_path.starts_with("stab_core::advanced::storage::") {
        return Some(FeatureId::BitKernels);
    }
    if api_path.starts_with("stab_core::advanced::algebra::") {
        return Some(FeatureId::Algebra);
    }
    if api_path.starts_with("stab_core::advanced::records::") {
        return Some(FeatureId::ResultFormats);
    }
    if api_path.starts_with("stab_core::advanced::backend::") {
        return Some(
            if api_path_mentions_item(api_path, "samplingcompilationdescriptor")
                || api_path.ends_with("::compilation_descriptor")
            {
                FeatureId::CircuitApi
            } else {
                FeatureId::Sampling
            },
        );
    }
    if api_path.starts_with("stab_core::advanced::traversal::") {
        return Some(
            if api_path_mentions_item(api_path, "circuitflattenedinstructioniter")
                || api_path_mentions_item(api_path, "circuitflattenedinstructionreviter")
            {
                FeatureId::CircuitApi
            } else {
                FeatureId::DemFormat
            },
        );
    }
    if api_path.starts_with("stab_core::advanced::compat::") {
        return Some(if api_path.contains("compileddemsampler") {
            FeatureId::DemSampling
        } else if api_path.contains("compiledsampler") {
            FeatureId::Sampling
        } else {
            FeatureId::Detection
        });
    }
    None
}

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
    if source_path.starts_with("crates/stab-analysis/src/circuit_to_dem") {
        return Some(FeatureId::Analyzer);
    }
    if source_path.starts_with("crates/stab-analysis/src/error_matcher")
        || source_path == "crates/stab-analysis/src/matched_error.rs"
    {
        return Some(FeatureId::Analyzer);
    }
    if source_path.starts_with("crates/stab-analysis/src/dem/sat")
        || source_path.starts_with("crates/stab-analysis/src/dem/search")
    {
        return Some(FeatureId::Search);
    }
    if source_path.starts_with("crates/stab-analysis/src/dem") {
        return Some(FeatureId::DemFormat);
    }
    if source_path.starts_with("crates/stab-analysis/src/circuit_flow")
        || source_path.starts_with("crates/stab-analysis/src/circuit_feedback")
        || source_path.starts_with("crates/stab-analysis/src/circuit_detecting_regions")
        || source_path.starts_with("crates/stab-analysis/src/sparse_rev_frame_tracker")
        || source_path.starts_with("crates/stab-analysis/src/circuit_inverse")
        || source_path.starts_with("crates/stab-analysis/src/circuit_missing_detectors")
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

pub(super) fn classify_extracted_engine_api(
    source_path: &str,
    api_lower: &str,
) -> Option<FeatureId> {
    if source_path.starts_with("crates/stab-engine/src/sampled_flow")
        || api_path_mentions_item(api_lower, "sampledflowerror")
        || api_lower.ends_with("::sample_if_circuit_has_stabilizer_flows")
    {
        return Some(FeatureId::FlowUtils);
    }
    if source_path == "crates/stab-engine/src/descriptor.rs"
        || api_path_mentions_item(api_lower, "compilationdescriptor")
        || api_path_mentions_item(api_lower, "samplingcompilationdescriptor")
        || api_path_mentions_item(api_lower, "compilationoperation")
        || api_lower.ends_with("::compilation_descriptor")
        || api_lower.ends_with("::compilation_descriptors")
        || api_lower.ends_with("_compilation_descriptor")
    {
        return Some(FeatureId::CircuitApi);
    }
    if is_dem_sampling_resource_api(api_lower) {
        return Some(FeatureId::Resource);
    }
    if source_path.starts_with("crates/stab-engine/src/dem_sampling")
        || is_extracted_dem_sampling_api(api_lower)
    {
        return Some(FeatureId::DemSampling);
    }
    if api_path_mentions_item(api_lower, "measurementtodetectionsinkadapter") {
        return Some(FeatureId::ResultFormats);
    }
    if source_path.starts_with("crates/stab-engine/src/detection")
        || is_extracted_detection_api(api_lower)
    {
        return Some(FeatureId::Detection);
    }
    if source_path.starts_with("crates/stab-engine/src/fingerprint")
        || source_path.starts_with("crates/stab-engine/src/probability")
        || source_path.starts_with("crates/stab-engine/src/sampling")
        || is_extracted_sampling_api(api_lower)
        || api_path_mentions_item(api_lower, "compilationrequestfingerprint")
        || api_path_mentions_item(api_lower, "biased_randomize_bits")
    {
        return Some(FeatureId::Sampling);
    }
    None
}

fn is_dem_sampling_resource_api(api_lower: &str) -> bool {
    [
        "demresourcelimiterror",
        "demresourcekind",
        "demsamplerlimits",
    ]
    .iter()
    .any(|item| api_path_mentions_item(api_lower, item))
        || (api_path_mentions_item(api_lower, "demsamplingplan")
            && api_lower.rsplit("::").next().is_some_and(|method| {
                [
                    "materialized_bytes_per_shot",
                    "replay_work_units_per_shot",
                    "try_reusable_detection_record",
                    "try_reusable_error_record",
                ]
                .contains(&method)
            }))
}

fn is_extracted_dem_sampling_api(api_lower: &str) -> bool {
    if !api_lower.starts_with("stab_engine::") && !api_lower.starts_with("stab_core::") {
        return false;
    }
    [
        "demerror",
        "demreplaybatchstatus",
        "demreplaysession",
        "demsamplingcancellation",
        "demsamplingcompiler",
        "demsamplingexecutionerror",
        "demsamplingplan",
        "demsamplingrunerror",
        "demsamplingrunprogress",
        "demsamplingrunstatus",
        "demsamplingrunsummary",
        "demsamplingsession",
    ]
    .iter()
    .any(|item| api_path_mentions_item(api_lower, item))
}

fn is_extracted_detection_api(api_lower: &str) -> bool {
    if !api_lower.starts_with("stab_engine::") && !api_lower.starts_with("stab_core::") {
        return false;
    }
    [
        "compileddetectionconverter",
        "detectioncompileerror",
        "detectionconversionlimits",
        "detectionconversionoptions",
        "detectionerror",
        "detectioneventrecord",
        "detectionexecutionerror",
        "detectionresourcekind",
        "detectionresourcelimiterror",
        "detectionrunerror",
        "detectionrunprogress",
        "detectionrunstatus",
        "detectionrunsummary",
        "detectionsamplingcompiler",
        "detectionsamplingplan",
        "detectionsamplingsession",
        "measurementtodetectioncompiler",
        "measurementtodetectionplan",
        "measurementtodetectionsession",
    ]
    .iter()
    .any(|item| api_path_mentions_item(api_lower, item))
        || api_lower.rsplit("::").next().is_some_and(|function| {
            [
                "detection_record_width",
                "detection_record_width_with_limits",
                "measurement_record_count",
                "measurement_record_count_with_limits",
                "validate_detection_sampling_circuit",
                "validate_detection_sampling_circuit_with_limits",
            ]
            .contains(&function)
        })
}

fn is_extracted_sampling_api(api_lower: &str) -> bool {
    if !api_lower.starts_with("stab_engine::") && !api_lower.starts_with("stab_core::") {
        return false;
    }
    [
        "backendpreference",
        "planfingerprint",
        "randompolicy",
        "referencesampletree",
        "referencesampletreeerror",
        "referencesamplemode",
        "runerror",
        "samplingbackend",
        "samplingcancellation",
        "samplingcompilationdescriptor",
        "samplingcompileerror",
        "samplingcompileerrorcode",
        "samplingcompiler",
        "samplingexecutionerror",
        "samplingplan",
        "samplingrunprogress",
        "samplingrunstatus",
        "samplingrunsummary",
        "samplingsession",
        "seed",
        "shotcount",
        "sinkfailurephase",
    ]
    .iter()
    .any(|item| api_path_mentions_item(api_lower, item))
        || api_lower.ends_with("::compilation_descriptor")
        || api_lower.ends_with("::registered_backends")
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
        "detectingregionmap",
        "detectingregionoptions",
        "detectingregiontargetmap",
        "detectingregiontargetoptions",
        "inverseqecoptions",
        "missingdetectoroptions",
        "timereversedforflowsoptions",
    ]
    .iter()
    .any(|item| api_path_mentions_item(api_lower, item))
        || api_lower.rsplit("::").next().is_some_and(|function| {
            [
                "check_if_circuit_has_unsigned_stabilizer_flows",
                "check_unsigned_stabilizer_flows_with_diagnostics",
                "circuit_inverse_qec",
                "circuit_inverse_qec_with_options",
                "circuit_inverse_unitary",
                "circuit_flow_generators",
                "circuit_has_all_unsigned_stabilizer_flows",
                "circuit_has_unsigned_stabilizer_flow",
                "circuit_time_reversed_for_flows",
                "circuit_time_reversed_for_flows_with_options",
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

pub(super) fn is_analyzer_api(api_lower: &str) -> bool {
    api_lower.contains("erroranalyzeroptions")
        || api_path_mentions_item(api_lower, "disjointpauliprobabilities")
        || api_path_mentions_item(api_lower, "independentpauliprobabilities")
        || [
            "circuiterrorlocation",
            "circuiterrorlocationstackframe",
            "circuittargetsinsideinstruction",
            "demtargetwithcoords",
            "explainederror",
            "flippedmeasurement",
            "gatetargetwithcoords",
        ]
        .iter()
        .any(|item| api_path_mentions_item(api_lower, item))
        || api_lower.contains("circuit_to_detector_error_model")
        || api_lower.ends_with("::independent_to_disjoint_xyz_errors")
        || api_lower.ends_with("::try_disjoint_to_independent_xyz_errors")
        || api_lower.ends_with("::detector_error_model")
        || api_lower.contains("explain_errors")
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
            "detectionrecordlimitsubject",
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
