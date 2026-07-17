use std::collections::BTreeMap;

use super::{
    CorrectnessApi, CorrectnessManifest, default_timing_policy, owner, sha256_hex, work_unit,
};
use crate::qualification::model::{
    ApiDisposition, EvidenceState, FixtureLocator, InputByteCount, MemoryMethod, MemoryPolicy,
    OutputContract, PerformanceDisposition, Phase, QualificationGroup, QualificationStatus,
    RowOrigin, RunnerFidelity, ScalePoint, ThresholdPolicy, WorkloadFamily,
};

pub(super) const BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID: &str =
    "PERFQ-M5-BIT-MATRIX-TRANSPOSE-ALLOCATING";
pub(super) const BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID: &str =
    "PERFQ-M5-BIT-MATRIX-TRANSPOSE-IN-PLACE";
pub(super) const PAULI_STRING_MULTIPLY_GROUP_ID: &str = "PERFQ-M6-PAULI-STRING";

pub(super) fn make_disposition(item: &CorrectnessApi) -> ApiDisposition {
    let performance_feature = item
        .performance_groups
        .first()
        .cloned()
        .unwrap_or_else(|| "PERF-RESOURCE-BOUNDARIES".to_string());
    let behavioral = is_behavioral(item);
    let supporting_performance_features = item.performance_groups.iter().skip(1).cloned().collect();
    let parent_group_ids = if behavioral {
        item.performance_groups
            .iter()
            .map(|feature| qualification_group_id(item, feature))
            .collect()
    } else {
        Vec::new()
    };
    ApiDisposition {
        id: item.id.clone(),
        path: item.path.clone(),
        kind: item.kind.clone(),
        performance_feature,
        supporting_performance_features,
        correctness_case_id: item.owner_case_id.clone(),
        disposition: if behavioral {
            PerformanceDisposition::CoveredByParent
        } else {
            PerformanceDisposition::NotPerformanceRelevant
        },
        parent_group_ids,
        reason: if behavioral {
            "Behavioral operation is assigned to its exact planned measured parent; PQ2 through PQ5 must implement or explicitly reclassify that workload."
                .to_string()
        } else {
            "Declaration-only, marker, or diagnostic trait shape has no independent runtime workload; behavioral operations are inventoried separately."
                .to_string()
        },
    }
}

pub(super) fn is_behavioral(item: &CorrectnessApi) -> bool {
    matches!(item.kind.as_str(), "function" | "method")
        || item.kind == "trait-impl" && behavioral_trait_impl(&item.path)
}

fn qualification_group_id(item: &CorrectnessApi, performance_feature: &str) -> String {
    if performance_feature == "PERF-BIT-KERNELS" {
        match item.path.as_str() {
            "stab_core::BitMatrix::transpose" | "stab_core::bits::BitMatrix::transpose" => {
                return BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID.to_string();
            }
            "stab_core::BitMatrix::transpose_square_in_place"
            | "stab_core::bits::BitMatrix::transpose_square_in_place" => {
                return BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID.to_string();
            }
            _ => {}
        }
    }
    if performance_feature == "PERF-STABILIZER-ALGEBRA"
        && matches!(
            item.path.as_str(),
            "stab_core::PauliString::right_multiply_in_place_returning_log_i_scalar"
                | "stab_core::stabilizers::PauliString::right_multiply_in_place_returning_log_i_scalar"
        )
    {
        return PAULI_STRING_MULTIPLY_GROUP_ID.to_string();
    }
    let phase = phase(&item.path);
    let key = format!(
        "{performance_feature}\0{}\0{}",
        api_parent(&item.path, &item.kind),
        phase_name(phase)
    );
    let digest = sha256_hex(key.as_bytes());
    let Some(suffix) = digest.get(..16) else {
        unreachable!("SHA-256 hexadecimal output is at least sixteen bytes");
    };
    format!("PERFQ-API-{suffix}")
}

pub(super) fn qualification_groups(correctness: &CorrectnessManifest) -> Vec<QualificationGroup> {
    let mut grouped = BTreeMap::<String, (String, Vec<&CorrectnessApi>)>::new();
    for item in correctness
        .public_api_items
        .iter()
        .filter(|item| is_behavioral(item))
    {
        for feature in &item.performance_groups {
            grouped
                .entry(qualification_group_id(item, feature))
                .or_insert_with(|| (feature.clone(), Vec::new()))
                .1
                .push(item);
        }
    }
    grouped
        .into_iter()
        .map(|(id, (feature, items))| qualification_group(id, &feature, &items))
        .collect()
}

fn qualification_group(
    id: String,
    performance_feature: &str,
    items: &[&CorrectnessApi],
) -> QualificationGroup {
    let Some(item) = items.first().copied() else {
        unreachable!("API qualification groups are constructed from nonempty entries");
    };
    let fixture_item_id = items
        .iter()
        .map(|item| item.id.as_str())
        .min()
        .unwrap_or(item.id.as_str());
    let mut correctness_cases = items
        .iter()
        .map(|item| item.owner_case_id.clone())
        .collect::<Vec<_>>();
    correctness_cases.sort();
    correctness_cases.dedup();
    QualificationGroup {
        id: id.clone(),
        manifest_row: id.to_ascii_lowercase(),
        row_origin: RowOrigin::Planned,
        performance_feature: performance_feature.to_string(),
        checklist_anchors: Vec::new(),
        checklist_child_ids: Vec::new(),
        public_api_items: Vec::new(),
        disposition: PerformanceDisposition::Measured,
        phase: phase(&item.path),
        runner_fidelity: RunnerFidelity::AdapterLibrary,
        correctness_cases,
        correctness_binding: crate::qualification::model::CorrectnessBinding::ExactApiOwners,
        planned_correctness_case_id: None,
        workload_family: WorkloadFamily {
            fixture: FixtureLocator::Generated {
                id: format!("api-small-medium-large-17-{id}"),
            },
            source: "oracle/qualification-manifest.json".to_string(),
            deterministic_seed: "17".to_string(),
            scales: [("small", 1_u64), ("medium", 64), ("large", 4_096)]
                .into_iter()
                .map(|(id, semantic_items)| ScalePoint {
                    id: id.to_string(),
                    parameters: format!(
                        "generator=api-owner-phase-v1; semantic_items={semantic_items}; seed=17; fixture_group={}",
                        fixture_item_id
                    ),
                    input_bytes: if work_unit(performance_feature) == "bytes" {
                        InputByteCount::Exact {
                            bytes: semantic_items,
                        }
                    } else {
                        InputByteCount::NotApplicable
                    },
                    semantic_work: None,
                    input_digest: None,
                })
                .collect(),
        },
        work_unit: work_unit(performance_feature).to_string(),
        output_contract: OutputContract {
            expected_shape: format!(
                "One exact named submeasurement per public_api_items path under {} with equal work count and semantic output digest; unlike API paths must never be aggregated into one ratio.",
                api_parent(&item.path, &item.kind)
            ),
            digest_state: EvidenceState::Planned,
            sink_policy: "The Stim adapter and Stab worker fully consume equivalent output outside the timed digest preflight."
                .to_string(),
            comparator_sources: Vec::new(),
        },
        timing_policy: default_timing_policy(),
        memory_policy: MemoryPolicy {
            method: MemoryMethod::StabAllocations,
            scale_ids: vec!["small".to_string(), "medium".to_string(), "large".to_string()],
            expected_growth: expected_growth(performance_feature).to_string(),
        },
        threshold_policy: ThresholdPolicy::ReportOnly,
        reason: format!(
            "Behavioral API paths owned by {} in the same phase have no truthful inherited parent workload; PQ2 through PQ5 must implement every exact named submeasurement or explicitly reclassify the path.",
            api_parent(&item.path, &item.kind)
        ),
        owner: owner(performance_feature).to_string(),
        status: QualificationStatus::Planned,
    }
}

fn expected_growth(feature: &str) -> &'static str {
    match feature {
        "PERF-SEARCH-AND-MATCHING" => "bounded search state with explicit explored-node counter",
        "PERF-SAMPLING" | "PERF-DETECTION" | "PERF-DEM-SAMPLING" => {
            "linear in active state and output record width"
        }
        "PERF-RESULT-IO" | "PERF-BIT-KERNELS" => "linear in record or bit width",
        "PERF-CLI-STARTUP-AND-ERRORS" => "constant startup plus linear accepted input",
        _ => {
            "linear in semantic_items unless the exact API contract declares bounded materialization"
        }
    }
}

fn api_parent<'a>(path: &'a str, kind: &str) -> &'a str {
    if kind == "trait-impl" {
        path.split_once(" as ").map_or(path, |(parent, _)| parent)
    } else {
        path.rsplit_once("::").map_or(path, |(parent, _)| parent)
    }
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Startup => "startup",
        Phase::Parse => "parse",
        Phase::Compile => "compile",
        Phase::Execute => "execute",
        Phase::Convert => "convert",
        Phase::Serialize => "serialize",
        Phase::Search => "search",
        Phase::Transform => "transform",
        Phase::EndToEnd => "end-to-end",
    }
}

fn phase(path: &str) -> Phase {
    let normalized = path.to_ascii_lowercase();
    if normalized.contains("parse") || normalized.contains("from_str") {
        Phase::Parse
    } else if normalized.contains("write")
        || normalized.contains("display")
        || normalized.contains("to_stim")
        || normalized.contains("to_dem")
    {
        Phase::Serialize
    } else if normalized.contains("compile") {
        Phase::Compile
    } else if normalized.contains("convert") {
        Phase::Convert
    } else if normalized.contains("search")
        || normalized.contains("find_")
        || normalized.contains("match")
    {
        Phase::Search
    } else if normalized.contains("flatten")
        || normalized.contains("without_")
        || normalized.contains("rounded")
        || normalized.contains("inverse")
        || normalized.contains("time_reversed")
        || normalized.contains("inlined")
    {
        Phase::Transform
    } else {
        Phase::Execute
    }
}

fn behavioral_trait_impl(path: &str) -> bool {
    let Some((_, rest)) = path.split_once(" as ") else {
        return false;
    };
    let trait_name = rest
        .split_once(" for@")
        .or_else(|| rest.split_once(" for "))
        .map_or(rest, |(name, _)| name)
        .split('@')
        .next()
        .unwrap_or(rest);
    matches!(
        trait_name,
        "Clone"
            | "Default"
            | "Display"
            | "From"
            | "FromStr"
            | "Hash"
            | "Iterator"
            | "Ord"
            | "PartialEq"
            | "PartialOrd"
            | "TryFrom"
    )
}
