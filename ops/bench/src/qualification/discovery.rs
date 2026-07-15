use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::checklist::{RawChecklistItem, parse as parse_checklist};
use super::model::{
    ChecklistItem, ChecklistScope, CorrectnessBinding, EvidenceState, FixtureLocator,
    InputByteCount, ManifestRowDisposition, MeasurementPair, MemoryMethod, MemoryPolicy,
    OutputContract, PerformanceDisposition, PerformanceFeature, Phase, QualificationGroup,
    QualificationStatus, QualificationSuite, RowClassification, RowDecision, RowOrigin,
    RunnerFidelity, SCHEMA_VERSION, ScalePoint, ThresholdPolicy, TimingPolicy, UpstreamPerfSource,
    WaiverDisposition, WaiverReason, WorkloadFamily,
};
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::error::BenchError;
use crate::manifest::{BenchmarkManifest, BenchmarkRow, Runner, ThresholdClass};
use crate::root::RepoRoot;

mod api;
mod graduation;
mod rows;

use rows::{
    classify_manifest_row, classify_phase, row_classifications, row_decision, runner_fidelity,
    selected_stim_symbols, stim_mapping, threshold_policy, workload_family,
};

pub(super) const PERFORMANCE_FEATURE_IDS: [&str; 16] = [
    "PERF-CIRCUIT-MODEL",
    "PERF-DEM-MODEL",
    "PERF-RESULT-IO",
    "PERF-GATE-CONTRACT",
    "PERF-BIT-KERNELS",
    "PERF-STABILIZER-ALGEBRA",
    "PERF-GENERATION",
    "PERF-CONVERT-CLI",
    "PERF-SAMPLING",
    "PERF-DETECTION",
    "PERF-DEM-SAMPLING",
    "PERF-ERROR-ANALYSIS",
    "PERF-SEARCH-AND-MATCHING",
    "PERF-FLOWS-AND-DETECTOR-UTILITIES",
    "PERF-CLI-STARTUP-AND-ERRORS",
    "PERF-RESOURCE-BOUNDARIES",
];

const MAX_INPUT_BYTES: usize = 16 << 20;
const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Deserialize)]
struct CorrectnessManifest {
    semantic_digest: String,
    features: Vec<CorrectnessFeature>,
    public_api_items: Vec<CorrectnessApi>,
    evidence_cases: Vec<CorrectnessEvidence>,
}

#[derive(Deserialize)]
struct CorrectnessFeature {
    id: String,
    performance_groups: Vec<String>,
}

#[derive(Deserialize)]
struct CorrectnessApi {
    id: String,
    path: String,
    kind: String,
    owner_case_id: String,
    performance_groups: Vec<String>,
}

#[derive(Deserialize)]
struct CorrectnessEvidence {
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdRows<T> {
    schema_version: u32,
    rows: Vec<T>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThresholdRow {
    id: String,
    #[serde(default)]
    max_relative_ratio: Option<serde_json::Number>,
    #[serde(default)]
    measurement_thresholds: Vec<MeasurementThreshold>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasurementThreshold {
    stim_name: String,
    stab_name: String,
    max_relative_ratio: serde_json::Number,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WaiverRow {
    id: String,
    reason: String,
    follow_up: String,
}

pub(super) struct SourceReferences {
    pub(super) correctness_cases: BTreeSet<String>,
    pub(super) threshold_rows: BTreeSet<String>,
    pub(super) threshold_ratios: BTreeMap<String, Option<String>>,
    pub(super) threshold_pairs: BTreeMap<String, BTreeSet<(String, String, String)>>,
    pub(super) beta_waivers: BTreeSet<String>,
    pub(super) regression_waivers: BTreeSet<String>,
    pub(super) waiver_reasons: BTreeMap<(String, String), String>,
    pub(super) public_api: BTreeMap<String, ApiReference>,
}

pub(super) struct ApiReference {
    pub(super) path: String,
    pub(super) kind: String,
    pub(super) owner_case_id: String,
    pub(super) performance_groups: Vec<String>,
}

pub(super) fn load_source_references(root: &RepoRoot) -> Result<SourceReferences, BenchError> {
    let correctness: CorrectnessManifest =
        read_repo_json_bounded(root, &root.correctness_manifest())?;
    let thresholds: IdRows<ThresholdRow> =
        read_repo_json_bounded(root, &root.primary_thresholds())?;
    let beta: IdRows<WaiverRow> = read_repo_json_bounded(root, &root.primary_beta_waivers())?;
    let regression: IdRows<WaiverRow> =
        read_repo_json_bounded(root, &root.primary_regression_waivers())?;
    if thresholds.schema_version != 2 || beta.schema_version != 1 || regression.schema_version != 1
    {
        return Err(BenchError::Qualification(format!(
            "qualification threshold or waiver schema version is unsupported: thresholds={} beta={} regression={}",
            thresholds.schema_version, beta.schema_version, regression.schema_version
        )));
    }
    let correctness_cases = unique_ids(
        "correctness case",
        correctness
            .evidence_cases
            .iter()
            .map(|case| case.id.as_str()),
    )?;
    let threshold_rows = unique_ids(
        "threshold row",
        thresholds.rows.iter().map(|row| row.id.as_str()),
    )?;
    let beta_waivers = unique_ids("beta waiver", beta.rows.iter().map(|row| row.id.as_str()))?;
    let regression_waivers = unique_ids(
        "regression waiver",
        regression.rows.iter().map(|row| row.id.as_str()),
    )?;
    unique_ids(
        "public API",
        correctness
            .public_api_items
            .iter()
            .map(|item| item.id.as_str()),
    )?;
    let public_api = correctness
        .public_api_items
        .into_iter()
        .map(|item| {
            (
                item.id,
                ApiReference {
                    path: item.path,
                    kind: item.kind,
                    owner_case_id: item.owner_case_id,
                    performance_groups: item.performance_groups,
                },
            )
        })
        .collect();
    Ok(SourceReferences {
        correctness_cases,
        threshold_rows,
        threshold_ratios: thresholds
            .rows
            .iter()
            .map(|row| {
                (
                    row.id.clone(),
                    row.max_relative_ratio.as_ref().map(ToString::to_string),
                )
            })
            .collect(),
        threshold_pairs: thresholds
            .rows
            .into_iter()
            .map(|row| {
                (
                    row.id,
                    row.measurement_thresholds
                        .into_iter()
                        .map(|pair| {
                            (
                                pair.stim_name,
                                pair.stab_name,
                                pair.max_relative_ratio.to_string(),
                            )
                        })
                        .collect(),
                )
            })
            .collect(),
        beta_waivers,
        regression_waivers,
        waiver_reasons: [
            ("benchmarks/m12-primary-beta-waivers.json", &beta.rows),
            (
                "benchmarks/m12-primary-regression-waivers.json",
                &regression.rows,
            ),
        ]
        .into_iter()
        .flat_map(|(file, rows)| {
            rows.iter()
                .map(move |row| ((file.to_string(), row.id.clone()), row.reason.clone()))
        })
        .collect(),
        public_api,
    })
}

fn unique_ids<'a>(
    label: &str,
    values: impl IntoIterator<Item = &'a str>,
) -> Result<BTreeSet<String>, BenchError> {
    let mut ids = BTreeSet::new();
    for value in values {
        if !ids.insert(value.to_string()) {
            return Err(BenchError::Qualification(format!(
                "duplicate {label} id {value:?}"
            )));
        }
    }
    Ok(ids)
}

pub(super) fn generate(
    root: &RepoRoot,
    benchmark_manifest: &BenchmarkManifest,
) -> Result<QualificationSuite, BenchError> {
    let correctness: CorrectnessManifest =
        read_repo_json_bounded(root, &root.correctness_manifest())?;
    let thresholds: IdRows<ThresholdRow> =
        read_repo_json_bounded(root, &root.primary_thresholds())?;
    let beta_waivers: IdRows<WaiverRow> =
        read_repo_json_bounded(root, &root.primary_beta_waivers())?;
    let regression_waivers: IdRows<WaiverRow> =
        read_repo_json_bounded(root, &root.primary_regression_waivers())?;
    if thresholds.schema_version != 2
        || beta_waivers.schema_version != 1
        || regression_waivers.schema_version != 1
    {
        return Err(BenchError::Qualification(format!(
            "qualification threshold or waiver schema version is unsupported: thresholds={} beta={} regression={}",
            thresholds.schema_version,
            beta_waivers.schema_version,
            regression_waivers.schema_version
        )));
    }
    let raw_checklist = parse_checklist(&read_repo_text_bounded(root, &root.feature_checklist())?)?;
    let upstream_perf_sources = discover_perf_sources(root, benchmark_manifest)?;

    let threshold_by_id = thresholds
        .rows
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let beta_by_id = beta_waivers
        .rows
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let regression_by_id = regression_waivers
        .rows
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let mut groups = Vec::with_capacity(benchmark_manifest.rows.len());
    let mut row_dispositions = Vec::with_capacity(benchmark_manifest.rows.len());
    for row in &benchmark_manifest.rows {
        let feature_id = classify_manifest_row(row)?;
        let group_id = format!("PERFQ-{}", row.id.to_ascii_uppercase());
        let threshold = threshold_by_id.get(row.id.as_str()).copied();
        let waived = beta_by_id.contains_key(row.id.as_str())
            || regression_by_id.contains_key(row.id.as_str());
        let selected_stim_symbols = selected_stim_symbols(row, &upstream_perf_sources);
        let correctness_cases = Vec::new();
        let correctness_binding = CorrectnessBinding::Unresolved;
        let classifications = row_classifications(row, threshold, waived, &selected_stim_symbols);
        let decision = row_decision(row);
        let disposition = if decision == RowDecision::Removed
            || row.threshold_class == ThresholdClass::BaselineMetadata
        {
            PerformanceDisposition::NotPerformanceRelevant
        } else {
            PerformanceDisposition::Measured
        };
        let stim_mapping = stim_mapping(row, waived);
        let threshold_policy = threshold_policy(
            row,
            threshold.is_some(),
            waived,
            classifications.contains(&RowClassification::UnmatchedSubmeasurement),
        );
        let supporting_performance_features = if (row.runner == Runner::StimCli
            || row.id.starts_with("pf7-cli-"))
            && feature_id != "PERF-CLI-STARTUP-AND-ERRORS"
        {
            vec!["PERF-CLI-STARTUP-AND-ERRORS".to_string()]
        } else {
            Vec::new()
        };
        row_dispositions.push(ManifestRowDisposition {
            id: row.id.clone(),
            primary_group_id: group_id.clone(),
            supporting_performance_features,
            decision,
            classifications,
            stim_mapping,
            threshold_refs: threshold
                .is_some()
                .then(|| "benchmarks/m12-primary-thresholds.json".to_string())
                .into_iter()
                .collect(),
            threshold_max_relative_ratio: threshold
                .and_then(|row| row.max_relative_ratio.as_ref())
                .map(ToString::to_string),
            threshold_measurement_pairs: threshold
                .into_iter()
                .flat_map(|row| &row.measurement_thresholds)
                .map(|measurement| MeasurementPair {
                    stim_name: measurement.stim_name.clone(),
                    stab_name: measurement.stab_name.clone(),
                    max_relative_ratio: measurement.max_relative_ratio.to_string(),
                })
                .collect(),
            waiver_refs: waiver_refs(row, &beta_by_id, &regression_by_id),
        });
        let mut group = QualificationGroup {
            id: group_id.clone(),
            manifest_row: row.id.clone(),
            row_origin: RowOrigin::Inherited,
            performance_feature: feature_id.to_string(),
            checklist_anchors: Vec::new(),
            checklist_child_ids: Vec::new(),
            public_api_items: Vec::new(),
            disposition,
            phase: classify_phase(row),
            runner_fidelity: runner_fidelity(row, waived),
            correctness_cases,
            correctness_binding,
            planned_correctness_case_id: Some(format!("CQPLANNED-{group_id}")),
            workload_family: workload_family(root, row)?,
            work_unit: work_unit(feature_id).to_string(),
            output_contract: OutputContract {
                expected_shape: format!(
                    "{} output count, width, and semantic digest from the declared correctness preflight",
                    row.measurement
                ),
                digest_state: EvidenceState::Planned,
                sink_policy: if row.runner == Runner::StimCli {
                    "Both public processes consume equivalent complete output sinks after an untimed exact-output preflight."
                        .to_string()
                } else {
                    "Both workers black-box equal semantic work and an untimed output digest."
                        .to_string()
                },
            },
            timing_policy: default_timing_policy(),
            memory_policy: MemoryPolicy {
                method: if row.runner == Runner::StimCli {
                    MemoryMethod::ProcessRss
                } else if row.threshold_class == ThresholdClass::BaselineMetadata {
                    MemoryMethod::NotApplicable
                } else {
                    MemoryMethod::StabAllocations
                },
                scale_ids: vec!["inherited".to_string()],
                expected_growth: "unclassified until PQ2 through PQ5 provide three-scale evidence"
                    .to_string(),
            },
            threshold_policy,
            reason: group_reason(decision).to_string(),
            owner: owner(feature_id).to_string(),
            status: QualificationStatus::Planned,
        };
        graduation::apply(&mut group);
        groups.push(group);
    }
    groups.push(resource_boundary_group());
    groups.extend(api::qualification_groups(&correctness));
    groups.extend(checklist_qualification_groups(&raw_checklist));
    groups.sort_by(|left, right| left.id.cmp(&right.id));
    row_dispositions.sort_by(|left, right| left.id.cmp(&right.id));

    let mut checklist_items = raw_checklist
        .into_iter()
        .map(make_checklist_item)
        .collect::<Vec<_>>();
    checklist_items.sort_by(|left, right| left.id.cmp(&right.id));

    let mut public_api_items = correctness
        .public_api_items
        .iter()
        .map(api::make_disposition)
        .collect::<Vec<_>>();
    public_api_items.sort_by(|left, right| left.path.cmp(&right.path));
    let group_index = groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    for item in &public_api_items {
        for parent in &item.parent_group_ids {
            if let Some(index) = group_index.get(parent).copied()
                && let Some(group) = groups.get_mut(index)
            {
                group.public_api_items.push(item.path.clone());
            }
        }
    }

    let performance_features = PERFORMANCE_FEATURE_IDS
        .iter()
        .map(|feature_id| PerformanceFeature {
            id: (*feature_id).to_string(),
            correctness_features: correctness
                .features
                .iter()
                .filter(|feature| {
                    feature
                        .performance_groups
                        .iter()
                        .any(|group| group == feature_id)
                })
                .map(|feature| feature.id.clone())
                .collect(),
            disposition: PerformanceDisposition::Measured,
            group_ids: groups
                .iter()
                .filter(|group| {
                    group.performance_feature == *feature_id
                        && group.disposition == PerformanceDisposition::Measured
                })
                .map(|group| group.id.clone())
                .collect(),
            reason: "Implemented selected operations have variable-size or public latency work in this qualification domain."
                .to_string(),
        })
        .collect();

    let waiver_rows = merge_waivers(&beta_waivers.rows, &regression_waivers.rows);
    let mut suite = QualificationSuite {
        schema_version: SCHEMA_VERSION,
        stim_version: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        correctness_digest: correctness.semantic_digest,
        semantic_digest: ZERO_DIGEST.to_string(),
        performance_features,
        checklist_items,
        public_api_items,
        qualification_groups: groups,
        manifest_rows: row_dispositions,
        upstream_perf_sources,
        waiver_rows,
    };
    suite.semantic_digest = semantic_digest(&suite)?;
    Ok(suite)
}

pub(super) fn semantic_digest(suite: &QualificationSuite) -> Result<String, BenchError> {
    let mut payload = suite.clone();
    payload.semantic_digest = ZERO_DIGEST.to_string();
    let bytes = serde_json::to_vec(&payload)?;
    Ok(sha256_hex(&bytes))
}

fn default_timing_policy() -> TimingPolicy {
    TimingPolicy {
        calibration_min_ms: 250,
        calibration_max_ms: 2_000,
        warmup_batches: 3,
        full_pairs: 9,
        timeout_seconds: 600,
        gate_statistic: "median paired ratio and fixed-seed bootstrap 95% upper bound".to_string(),
    }
}

fn resource_boundary_group() -> QualificationGroup {
    QualificationGroup {
        id: "PERFQ-RESOURCE-BOUNDARIES".to_string(),
        manifest_row: "pq-resource-boundaries".to_string(),
        row_origin: RowOrigin::Planned,
        performance_feature: "PERF-RESOURCE-BOUNDARIES".to_string(),
        checklist_anchors: Vec::new(),
        checklist_child_ids: Vec::new(),
        public_api_items: Vec::new(),
        disposition: PerformanceDisposition::Measured,
        phase: Phase::EndToEnd,
        runner_fidelity: RunnerFidelity::AdapterLibrary,
        correctness_cases: Vec::new(),
        correctness_binding: CorrectnessBinding::Unresolved,
        planned_correctness_case_id: Some("CQPLANNED-PERFQ-RESOURCE-BOUNDARIES".to_string()),
        workload_family: WorkloadFamily {
            fixture: FixtureLocator::Generated {
                id: "resource-boundary-matrix".to_string(),
            },
            source: "benchmarks/stim-qualification-suite.json".to_string(),
            deterministic_seed: "resource-boundary-v1".to_string(),
            scales: [
                (
                    "small",
                    "generator=resource-boundary-matrix-v1; seed=resource-boundary-v1; input_bytes=1024; records=64; search_states=1024; boundary_probe=declared-cap-and-cap-plus-one-outside-timing",
                    1_024,
                ),
                (
                    "medium",
                    "generator=resource-boundary-matrix-v1; seed=resource-boundary-v1; input_bytes=1048576; records=4096; search_states=100000; boundary_probe=declared-cap-and-cap-plus-one-outside-timing",
                    1_048_576,
                ),
                (
                    "large",
                    "generator=resource-boundary-matrix-v1; seed=resource-boundary-v1; input_bytes=67108864; records=1000000; search_states=1000000; boundary_probe=declared-cap-and-cap-plus-one-outside-timing",
                    67_108_864,
                ),
            ]
            .into_iter()
            .map(|(id, parameters, input_bytes)| ScalePoint {
                id: id.to_string(),
                parameters: parameters.to_string(),
                input_bytes: InputByteCount::Exact { bytes: input_bytes },
                semantic_work: None,
                input_digest: None,
            })
                .collect(),
        },
        work_unit: "admission-checks".to_string(),
        output_contract: OutputContract {
            expected_shape: "Exact accept/reject status, bounded-failure latency class, peak RSS, and allocation or active-state growth for each named resource contract."
                .to_string(),
            digest_state: EvidenceState::Planned,
            sink_policy: "Resource evidence is reported per contract and scale; heterogeneous contracts never produce an aggregate timing ratio."
                .to_string(),
        },
        timing_policy: default_timing_policy(),
        memory_policy: MemoryPolicy {
            method: MemoryMethod::ProcessRss,
            scale_ids: vec!["small".to_string(), "medium".to_string(), "large".to_string()],
            expected_growth: "contract-specific constant, linear-width, linear-active-state, bounded-materialization, or capped-search class"
                .to_string(),
        },
        threshold_policy: ThresholdPolicy::ReportOnly,
        reason: "The inherited M12 row is policy metadata, so PQ6 owns a new measured resource-boundary matrix instead of timing the metadata row."
            .to_string(),
        owner: "ops/bench".to_string(),
        status: QualificationStatus::Planned,
    }
}

fn checklist_qualification_groups(items: &[RawChecklistItem]) -> Vec<QualificationGroup> {
    items
        .iter()
        .filter(|item| item.scope == ChecklistScope::Selected)
        .flat_map(|item| {
            item.performance_features
                .iter()
                .map(move |feature| checklist_qualification_group(item, feature))
        })
        .collect()
}

fn checklist_qualification_group(item: &RawChecklistItem, feature: &str) -> QualificationGroup {
    let group_id = checklist_group_id(item, feature);
    let public_cli = item.section.starts_with("11.");
    let child_ids = item
        .selected_child_ownership
        .iter()
        .filter(|ownership| {
            ownership
                .performance_features
                .iter()
                .any(|candidate| candidate == feature)
        })
        .map(|ownership| ownership.child_id.clone())
        .collect::<Vec<_>>();
    QualificationGroup {
        id: group_id.clone(),
        manifest_row: group_id.to_ascii_lowercase(),
        row_origin: RowOrigin::Planned,
        performance_feature: feature.to_string(),
        checklist_anchors: vec![item.id.clone()],
        checklist_child_ids: child_ids.clone(),
        public_api_items: Vec::new(),
        disposition: PerformanceDisposition::Measured,
        phase: Phase::EndToEnd,
        runner_fidelity: if public_cli {
            RunnerFidelity::ProcessCli
        } else {
            RunnerFidelity::AdapterLibrary
        },
        correctness_cases: Vec::new(),
        correctness_binding: CorrectnessBinding::Unresolved,
        planned_correctness_case_id: Some(format!("CQPLANNED-{group_id}")),
        workload_family: WorkloadFamily {
            fixture: FixtureLocator::Generated {
                id: format!("checklist-small-medium-large-17-{group_id}"),
            },
            source: "docs/stab-feature-checklist.md".to_string(),
            deterministic_seed: "17".to_string(),
            scales: [("small", 1_u64), ("medium", 64), ("large", 4_096)]
                .into_iter()
                .map(|(id, semantic_items)| ScalePoint {
                    id: id.to_string(),
                    parameters: format!(
                        "generator=checklist-child-v1; {}={semantic_items}; seed=17; fixture_group={group_id}",
                        work_unit(feature)
                    ),
                    input_bytes: if work_unit(feature) == "bytes" {
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
        work_unit: work_unit(feature).to_string(),
        output_contract: OutputContract {
            expected_shape: format!(
                "Exact named submeasurements for selected checklist child ids [{}] ({:?}) in {feature}; unlike phases or operations must never be aggregated into one ratio.",
                child_ids.join(","),
                item.selected_child.as_deref().unwrap_or(&item.feature),
            ),
            digest_state: EvidenceState::Planned,
            sink_policy: if public_cli {
                "Built Stim and Stab processes fully consume equivalent output after an untimed exact-output preflight."
                    .to_string()
            } else {
                "The Stim adapter and Stab worker fully consume equivalent output outside the timed digest preflight."
                    .to_string()
            },
        },
        timing_policy: default_timing_policy(),
        memory_policy: MemoryPolicy {
            method: if public_cli {
                MemoryMethod::ProcessRss
            } else {
                MemoryMethod::StabAllocations
            },
            scale_ids: vec!["small".to_string(), "medium".to_string(), "large".to_string()],
            expected_growth: checklist_expected_growth(feature).to_string(),
        },
        threshold_policy: ThresholdPolicy::ReportOnly,
        reason: "This exact selected checklist child has no truthful inherited parent for every promised operation; the owning PQ milestone must implement every named submeasurement or explicitly narrow the child."
            .to_string(),
        owner: owner(feature).to_string(),
        status: QualificationStatus::Planned,
    }
}

fn checklist_expected_growth(feature: &str) -> &'static str {
    match feature {
        "PERF-SEARCH-AND-MATCHING" => "bounded search state with explicit explored-node counter",
        "PERF-SAMPLING" | "PERF-DETECTION" | "PERF-DEM-SAMPLING" => {
            "linear in active state and output record width"
        }
        "PERF-RESULT-IO" | "PERF-BIT-KERNELS" => "linear in record or bit width",
        "PERF-CLI-STARTUP-AND-ERRORS" | "PERF-CONVERT-CLI" | "PERF-GENERATION" => {
            "constant startup plus linear accepted input or generated output"
        }
        _ => {
            "linear in semantic work unless the selected checklist child declares bounded materialization"
        }
    }
}

fn checklist_group_id(item: &RawChecklistItem, feature: &str) -> String {
    format!(
        "PERFQ-CHECKLIST-{}-{}",
        item.id.trim_start_matches("PERFC-"),
        feature.trim_start_matches("PERF-")
    )
}

fn group_reason(decision: RowDecision) -> &'static str {
    match decision {
        RowDecision::Retained => {
            "The inherited operation shape is retained, but PQ1 must add correctness preflight, exact output digest, scales, and paired statistics before qualification."
        }
        RowDecision::Reworked => {
            "The inherited workload requires a faithful runner, exact phase split, scale family, or output contract before it can produce qualification evidence."
        }
        RowDecision::Diagnostic => {
            "The inherited row remains visible as diagnostic evidence and cannot produce a comprehensive ratio in its current shape."
        }
        RowDecision::Superseded => {
            "A more specific row owns this behavior; retain the old identity only until manifest migration removes the duplicate workload."
        }
        RowDecision::Removed => {
            "This row is metadata rather than a timed product workload and must be removed from the executable benchmark manifest."
        }
    }
}

fn make_checklist_item(item: RawChecklistItem) -> ChecklistItem {
    let parent_group_ids = item
        .performance_features
        .iter()
        .map(|feature| checklist_group_id(&item, feature))
        .collect::<Vec<_>>();
    let disposition =
        if item.scope == ChecklistScope::Deferred || item.performance_features.is_empty() {
            PerformanceDisposition::NotPerformanceRelevant
        } else {
            PerformanceDisposition::CoveredByParent
        };
    let reason = match (item.scope, disposition) {
        (ChecklistScope::Deferred, _) => {
            "This product row is explicitly deferred and cannot contribute a passing performance claim."
                .to_string()
        }
        (_, PerformanceDisposition::NotPerformanceRelevant) => {
            "This row describes packaging, documentation, or evidence infrastructure instead of product runtime work."
                .to_string()
        }
        _ => "The implemented child is assigned to the listed measured workload families; any deferred remainder stays outside executable qualification."
            .to_string(),
    };
    ChecklistItem {
        id: item.id,
        source_line: item.source_line,
        anchor_digest: item.anchor_digest,
        section: item.section,
        feature: item.feature,
        raw_status: item.raw_status,
        scope: item.scope,
        deferred_remainder: item.deferred_remainder,
        selected_child: item.selected_child,
        deferred_child: item.deferred_child,
        selected_child_ids: item.selected_child_ids,
        deferred_child_ids: item.deferred_child_ids,
        selected_child_ownership: item.selected_child_ownership,
        performance_features: item.performance_features,
        disposition,
        parent_group_ids,
        reason,
    }
}

fn discover_perf_sources(
    root: &RepoRoot,
    manifest: &BenchmarkManifest,
) -> Result<Vec<UpstreamPerfSource>, BenchError> {
    let list = read_text_bounded(&safe_stim_source_path(root, "file_lists/perf_files")?)?;
    let mut sources = Vec::new();
    for path in list.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let source = read_text_bounded(&safe_stim_source_path(root, path)?)?;
        let mut symbols = source
            .lines()
            .filter_map(extract_benchmark_symbol)
            .collect::<Vec<_>>();
        symbols.sort();
        symbols.dedup();
        let mut manifest_rows = manifest
            .rows
            .iter()
            .filter(|row| row.upstream_source == path)
            .map(|row| row.id.clone())
            .collect::<Vec<_>>();
        manifest_rows.sort();
        sources.push(UpstreamPerfSource {
            path: path.to_string(),
            symbols,
            manifest_rows,
        });
    }
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(sources)
}

fn safe_stim_source_path(root: &RepoRoot, requested: &str) -> Result<PathBuf, BenchError> {
    let requested = Path::new(requested);
    if requested.is_absolute()
        || requested
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(BenchError::Qualification(format!(
            "unsafe pinned-Stim source path {requested:?}"
        )));
    }
    let stim_relative = root
        .default_stim_source()
        .strip_prefix(&root.path)
        .map_err(|_| {
            BenchError::Qualification(
                "pinned-Stim source directory is outside the repository root".to_string(),
            )
        })?
        .to_path_buf();
    let relative = stim_relative.join(requested);
    let component_count = relative.components().count();
    let mut current = root.path.clone();
    for (index, component) in relative.components().enumerate() {
        current.push(component.as_os_str());
        let metadata =
            std::fs::symlink_metadata(&current).map_err(|source| BenchError::QualificationIo {
                path: current.clone(),
                source,
            })?;
        let final_component = index + 1 == component_count;
        if metadata.file_type().is_symlink()
            || final_component && !metadata.is_file()
            || !final_component && !metadata.is_dir()
        {
            return Err(BenchError::Qualification(format!(
                "pinned-Stim source component {} has an unsafe file type",
                current.display()
            )));
        }
    }
    Ok(current)
}

fn extract_benchmark_symbol(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix("BENCHMARK(")?;
    let end = rest.find(')')?;
    let symbol = rest.get(..end)?;
    (!symbol.is_empty()
        && symbol
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
    .then(|| symbol.to_string())
}

fn merge_waivers(beta: &[WaiverRow], regression: &[WaiverRow]) -> Vec<WaiverDisposition> {
    let mut by_id = BTreeMap::<String, (BTreeMap<String, String>, String)>::new();
    for (name, rows) in [
        ("benchmarks/m12-primary-beta-waivers.json", beta),
        ("benchmarks/m12-primary-regression-waivers.json", regression),
    ] {
        for row in rows {
            let entry = by_id
                .entry(row.id.clone())
                .or_insert_with(|| (BTreeMap::new(), row.follow_up.clone()));
            entry.0.insert(name.to_string(), row.reason.clone());
            if entry.1 != row.follow_up
                || row.reason.trim().is_empty()
                || row.follow_up.trim().is_empty()
            {
                entry.1 = "SOURCE-WAIVER-MISMATCH".to_string();
            }
        }
    }
    by_id
        .into_iter()
        .map(|(id, (reasons, follow_up))| WaiverDisposition {
            retirement_mapping: waiver_retirement_mapping(&id).to_string(),
            id,
            waiver_files: reasons.keys().cloned().collect(),
            reasons: reasons
                .into_iter()
                .map(|(waiver_file, reason)| WaiverReason {
                    waiver_file,
                    reason,
                })
                .collect(),
            qualification_disposition: PerformanceDisposition::Measured,
            follow_up,
        })
        .collect()
}

fn waiver_retirement_mapping(id: &str) -> &'static str {
    match id {
        "m4-circuit-canonical-print" | "m7-convert-stim-canonical" => {
            "stim_adapter::circuit::canonical_serialize"
        }
        "m7-convert-01-to-ptb64" => "stim_adapter::result::convert_01_to_ptb64",
        "m8-measure-reader-ptb64-contract" => "stim_adapter::result::read_ptb64_dense_sparse",
        "m10-dem-print-contract" => "stim_adapter::dem::canonical_serialize",
        _ => "UNMAPPED-WAIVER",
    }
}

fn waiver_refs(
    row: &BenchmarkRow,
    beta: &BTreeMap<&str, &WaiverRow>,
    regression: &BTreeMap<&str, &WaiverRow>,
) -> Vec<String> {
    let mut refs = Vec::new();
    if beta.contains_key(row.id.as_str()) {
        refs.push("benchmarks/m12-primary-beta-waivers.json".to_string());
    }
    if regression.contains_key(row.id.as_str()) {
        refs.push("benchmarks/m12-primary-regression-waivers.json".to_string());
    }
    refs
}

fn work_unit(feature: &str) -> &'static str {
    match feature {
        "PERF-BIT-KERNELS" | "PERF-RESULT-IO" => "bits",
        "PERF-SAMPLING" | "PERF-DEM-SAMPLING" => "shots",
        "PERF-DETECTION" => "detector-events",
        "PERF-GATE-CONTRACT" => "gates",
        "PERF-ERROR-ANALYSIS" | "PERF-DEM-MODEL" => "instructions",
        "PERF-SEARCH-AND-MATCHING" => "search-nodes",
        "PERF-FLOWS-AND-DETECTOR-UTILITIES" => "flows",
        "PERF-CIRCUIT-MODEL" | "PERF-GENERATION" => "instructions",
        "PERF-CONVERT-CLI" | "PERF-CLI-STARTUP-AND-ERRORS" => "bytes",
        "PERF-STABILIZER-ALGEBRA" => "qubits",
        "PERF-RESOURCE-BOUNDARIES" => "admission-checks",
        _ => "operations",
    }
}

fn owner(feature: &str) -> &'static str {
    match feature {
        "PERF-RESULT-IO" => "stab-core/result-formats",
        "PERF-BIT-KERNELS" => "stab-core/bits",
        "PERF-GENERATION" => "stab-core/generation",
        "PERF-CONVERT-CLI" | "PERF-CLI-STARTUP-AND-ERRORS" => "stab-cli",
        "PERF-SAMPLING" => "stab-core/sampling",
        "PERF-DETECTION" => "stab-core/detection",
        "PERF-DEM-SAMPLING" => "stab-core/dem-sampler",
        "PERF-ERROR-ANALYSIS" => "stab-core/analyzer",
        "PERF-SEARCH-AND-MATCHING" => "stab-core/search",
        "PERF-FLOWS-AND-DETECTOR-UTILITIES" => "stab-core/flow-utils",
        "PERF-RESOURCE-BOUNDARIES" => "ops/bench",
        _ => "stab-core",
    }
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let Some(high) = HEX.get(usize::from(byte >> 4)).copied() else {
            unreachable!("a four-bit hexadecimal digit is within the lookup table");
        };
        let Some(low) = HEX.get(usize::from(byte & 0x0f)).copied() else {
            unreachable!("a four-bit hexadecimal digit is within the lookup table");
        };
        encoded.push(char::from(high));
        encoded.push(char::from(low));
    }
    encoded
}

fn read_text_bounded(path: &Path) -> Result<String, BenchError> {
    let bytes = super::io::read_regular_file_bounded(path, MAX_INPUT_BYTES)?;
    String::from_utf8(bytes).map_err(|_| {
        BenchError::Qualification(format!(
            "qualification input {} is not UTF-8",
            path.display()
        ))
    })
}

fn read_repo_text_bounded(root: &RepoRoot, path: &Path) -> Result<String, BenchError> {
    let bytes = crate::source_file::read_repo_regular_file_bounded(root, path, MAX_INPUT_BYTES)?;
    String::from_utf8(bytes).map_err(|error| {
        BenchError::Qualification(format!(
            "qualification input {} is not UTF-8: {error}",
            path.display()
        ))
    })
}

fn read_repo_json_bounded<T: for<'de> Deserialize<'de>>(
    root: &RepoRoot,
    path: &Path,
) -> Result<T, BenchError> {
    let text = read_repo_text_bounded(root, path)?;
    super::io::preflight_json_shape(text.as_bytes())?;
    serde_json::from_str(&text).map_err(BenchError::Json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comparability::ComparabilityClass;

    #[test]
    fn benchmark_symbol_extraction_is_exact() {
        assert_eq!(
            extract_benchmark_symbol("BENCHMARK(read_01) {"),
            Some("read_01".to_string())
        );
        assert_eq!(extract_benchmark_symbol("// BENCHMARK(fake)"), None);
        assert_eq!(extract_benchmark_symbol("BENCHMARK(bad-name)"), None);
    }

    #[test]
    fn threshold_source_schema_rejects_unknown_fields() {
        let source = r#"{
            "schema_version": 2,
            "rows": [{
                "id": "row",
                "measurement_thresholds": [{
                    "stim_name": "stim",
                    "stab_name": "stab",
                    "max_relative_ratio": 1.25,
                    "unexpected": true
                }]
            }]
        }"#;

        let result = serde_json::from_str::<IdRows<ThresholdRow>>(source);
        assert!(result.is_err(), "unknown threshold field must fail");
        let error = result.err().expect("threshold parse error");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn row_classification_separates_major_domains() {
        let make = |id: &str| BenchmarkRow {
            id: id.to_string(),
            milestone: crate::manifest::Milestone::M4,
            threshold_class: ThresholdClass::ReportOnly,
            runner: Runner::ContractOnly,
            upstream_source: "src/stim.test.cc".to_string(),
            stim_perf_filter: String::new(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "analysis".to_string(),
            measurement: "test".to_string(),
            description: "test".to_string(),
            comparability: ComparabilityClass::ContractOnly,
        };
        assert_eq!(
            classify_manifest_row(&make("m8-measure-reader-01")).expect("owned row"),
            "PERF-RESULT-IO"
        );
        assert_eq!(
            classify_manifest_row(&make("pfm-b5-wcnf-direct-dem")).expect("owned row"),
            "PERF-SEARCH-AND-MATCHING"
        );
        assert_eq!(
            classify_manifest_row(&make("pfm-b1-time-reverse-generated-surface"))
                .expect("owned row"),
            "PERF-FLOWS-AND-DETECTOR-UTILITIES"
        );
    }

    #[test]
    fn pinned_stim_source_paths_reject_absolute_and_parent_components() {
        let directory = tempfile::tempdir().expect("temporary repository");
        std::fs::create_dir_all(directory.path().join("vendor/stim/file_lists"))
            .expect("create Stim source tree");
        std::fs::write(
            directory.path().join("vendor/stim/file_lists/perf_files"),
            b"",
        )
        .expect("write perf list");
        let root = RepoRoot::resolve(directory.path()).expect("resolve root");

        assert!(safe_stim_source_path(&root, "../outside").is_err());
        assert!(safe_stim_source_path(&root, "/tmp/outside").is_err());
        assert!(safe_stim_source_path(&root, "file_lists/perf_files").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn pinned_stim_source_paths_reject_symlinked_ancestors() {
        let directory = tempfile::tempdir().expect("temporary repository");
        let outside = tempfile::tempdir().expect("outside directory");
        std::fs::create_dir_all(directory.path().join("vendor/stim"))
            .expect("create Stim source tree");
        std::fs::write(
            outside.path().join("case.perf.cc"),
            b"BENCHMARK(outside) {}",
        )
        .expect("write outside source");
        std::os::unix::fs::symlink(outside.path(), directory.path().join("vendor/stim/src"))
            .expect("create source symlink");
        let root = RepoRoot::resolve(directory.path()).expect("resolve root");

        let error = safe_stim_source_path(&root, "src/case.perf.cc")
            .expect_err("symlinked source ancestor must fail");

        assert!(error.to_string().contains("unsafe file type"));
    }

    #[cfg(unix)]
    #[test]
    fn same_length_fixture_mutation_changes_corpus_digest() {
        let directory = tempfile::tempdir().expect("temporary repository");
        let fixture = directory.path().join("benchmarks/fixtures/input.stim");
        std::fs::create_dir_all(fixture.parent().expect("fixture parent"))
            .expect("create fixture directory");
        std::fs::write(&fixture, b"M 0").expect("write first fixture");
        let root = RepoRoot::resolve(directory.path()).expect("resolve root");
        let row = BenchmarkRow {
            id: "fixture-digest-test".to_string(),
            milestone: crate::manifest::Milestone::M4,
            threshold_class: ThresholdClass::ReportOnly,
            runner: Runner::ContractOnly,
            upstream_source: "src/stim.test.cc".to_string(),
            stim_perf_filter: String::new(),
            argv: String::new(),
            stdin_path: "benchmarks/fixtures/input.stim".to_string(),
            phase: "parse".to_string(),
            measurement: "test".to_string(),
            description: "test fixture digest".to_string(),
            comparability: ComparabilityClass::ContractOnly,
        };
        let first = workload_family(&root, &row).expect("first workload family");

        std::fs::write(&fixture, b"H 0").expect("write same-length fixture mutation");
        let second = workload_family(&root, &row).expect("second workload family");

        assert_eq!(
            first.scales.first().expect("first scale").input_bytes,
            second.scales.first().expect("second scale").input_bytes
        );
        let first_digest = match &first.fixture {
            FixtureLocator::RepositoryFile { sha256, .. } => Some(sha256),
            _ => None,
        }
        .expect("repository fixture");
        let second_digest = match &second.fixture {
            FixtureLocator::RepositoryFile { sha256, .. } => Some(sha256),
            _ => None,
        }
        .expect("repository fixture");
        assert_ne!(first_digest, second_digest);
    }
}
