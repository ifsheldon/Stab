use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use super::checklist::parse as parse_checklist;
use super::model::{
    ManifestRowDisposition, MeasurementPair, PerformanceDisposition, PerformanceFeature,
    QualificationSuite, RowDecision, SCHEMA_VERSION, UpstreamPerfSource, WaiverDisposition,
    WaiverKind, WaiverPair, WaiverSourcePolicy,
};
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::error::BenchError;
use crate::manifest::{BenchmarkManifest, BenchmarkRow, Runner, ThresholdClass};
use crate::root::RepoRoot;

mod replacements;
mod rows;

use rows::{
    classify_manifest_row, row_classifications, row_decision, selected_stim_symbols, stim_mapping,
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

pub(super) fn manifest_feature(row: &BenchmarkRow) -> Result<&'static str, BenchError> {
    classify_manifest_row(row)
}

pub(super) fn inherited_runtime_group_id(row_id: &str) -> Option<String> {
    replacements::runtime_group_id(row_id)
}

pub(super) fn manifest_disposition(row: &BenchmarkRow) -> PerformanceDisposition {
    if inherited_runtime_group_id(&row.id).is_some() {
        PerformanceDisposition::CoveredByParent
    } else if row_decision(row) == RowDecision::Removed
        || row.threshold_class == ThresholdClass::BaselineMetadata
    {
        PerformanceDisposition::NotPerformanceRelevant
    } else if row_decision(row) == RowDecision::Diagnostic {
        PerformanceDisposition::Diagnostic
    } else {
        PerformanceDisposition::FutureCandidate
    }
}

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
struct BetaWaiverRow {
    id: String,
    reason: String,
    follow_up: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegressionWaiverRow {
    id: String,
    kind: WaiverKind,
    measurement_pairs: Vec<WaiverPair>,
    reason: String,
    follow_up: String,
}

pub(super) struct SourceReferences {
    pub(super) correctness_digest: String,
    pub(super) correctness_features: BTreeMap<String, Vec<String>>,
    pub(super) correctness_cases: BTreeSet<String>,
    pub(super) threshold_rows: BTreeSet<String>,
    pub(super) threshold_ratios: BTreeMap<String, Option<String>>,
    pub(super) threshold_pairs: BTreeMap<String, BTreeSet<(String, String, String)>>,
    pub(super) beta_waivers: BTreeSet<String>,
    pub(super) regression_waivers: BTreeSet<String>,
    pub(super) adapter_regression_waivers: BTreeSet<String>,
    pub(super) unstable_pair_waivers: BTreeSet<String>,
    pub(super) waiver_policies: BTreeMap<(String, String), WaiverSourcePolicy>,
    pub(super) public_api: BTreeMap<String, ApiReference>,
    pub(super) checklist_items: BTreeMap<String, ChecklistReference>,
    pub(super) checklist_children: BTreeMap<String, ChecklistChildReference>,
}

pub(super) struct ApiReference {
    pub(super) owner_case_id: String,
    pub(super) performance_groups: Vec<String>,
}

pub(super) struct ChecklistReference {
    pub(super) performance_features: Vec<String>,
    pub(super) selected_child_ids: Vec<String>,
}

pub(super) struct ChecklistChildReference {
    pub(super) item_id: String,
    pub(super) performance_features: Vec<String>,
}

pub(super) fn load_source_references(root: &RepoRoot) -> Result<SourceReferences, BenchError> {
    let correctness: CorrectnessManifest =
        read_repo_json_bounded(root, &root.correctness_manifest())?;
    let thresholds: IdRows<ThresholdRow> =
        read_repo_json_bounded(root, &root.primary_thresholds())?;
    let beta: IdRows<BetaWaiverRow> = read_repo_json_bounded(root, &root.primary_beta_waivers())?;
    let regression: IdRows<RegressionWaiverRow> =
        read_repo_json_bounded(root, &root.primary_regression_waivers())?;
    let checklist_source = read_repo_text_bounded(root, &root.feature_checklist())?;
    let checklist = parse_checklist(&checklist_source)?;
    if thresholds.schema_version != 2 || beta.schema_version != 1 || regression.schema_version != 2
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
    let adapter_regression_waivers = regression
        .rows
        .iter()
        .filter(|row| row.kind == WaiverKind::NoComparableBaseline)
        .map(|row| row.id.clone())
        .collect();
    let unstable_pair_waivers = regression
        .rows
        .iter()
        .filter(|row| row.kind == WaiverKind::UnstableFaithfulPairs)
        .map(|row| row.id.clone())
        .collect();
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
                    owner_case_id: item.owner_case_id,
                    performance_groups: item.performance_groups,
                },
            )
        })
        .collect();
    let mut checklist_items = BTreeMap::new();
    let mut checklist_children = BTreeMap::new();
    for item in checklist {
        let item_id = item.id.clone();
        if checklist_items
            .insert(
                item_id.clone(),
                ChecklistReference {
                    performance_features: item.performance_features.clone(),
                    selected_child_ids: item.selected_child_ids.clone(),
                },
            )
            .is_some()
        {
            return Err(BenchError::Qualification(format!(
                "duplicate checklist item id {item_id:?}"
            )));
        }
        for ownership in item.selected_child_ownership {
            let child_id = ownership.child_id;
            if checklist_children
                .insert(
                    child_id.clone(),
                    ChecklistChildReference {
                        item_id: item_id.clone(),
                        performance_features: ownership.performance_features,
                    },
                )
                .is_some()
            {
                return Err(BenchError::Qualification(format!(
                    "duplicate checklist child id {child_id:?}"
                )));
            }
        }
    }
    Ok(SourceReferences {
        correctness_digest: correctness.semantic_digest.clone(),
        correctness_features: correctness
            .features
            .iter()
            .map(|feature| (feature.id.clone(), feature.performance_groups.clone()))
            .collect(),
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
        adapter_regression_waivers,
        unstable_pair_waivers,
        waiver_policies: beta
            .rows
            .iter()
            .map(|row| {
                let file = "benchmarks/m12-primary-beta-waivers.json".to_string();
                (
                    (file.clone(), row.id.clone()),
                    WaiverSourcePolicy {
                        waiver_file: file,
                        kind: WaiverKind::NoComparableBaseline,
                        reason: row.reason.clone(),
                        follow_up: row.follow_up.clone(),
                        measurement_pairs: Vec::new(),
                    },
                )
            })
            .chain(regression.rows.iter().map(|row| {
                let file = "benchmarks/m12-primary-regression-waivers.json".to_string();
                (
                    (file.clone(), row.id.clone()),
                    WaiverSourcePolicy {
                        waiver_file: file,
                        kind: row.kind,
                        reason: row.reason.clone(),
                        follow_up: row.follow_up.clone(),
                        measurement_pairs: row.measurement_pairs.clone(),
                    },
                )
            }))
            .collect(),
        public_api,
        checklist_items,
        checklist_children,
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
    let beta_waivers: IdRows<BetaWaiverRow> =
        read_repo_json_bounded(root, &root.primary_beta_waivers())?;
    let regression_waivers: IdRows<RegressionWaiverRow> =
        read_repo_json_bounded(root, &root.primary_regression_waivers())?;
    if thresholds.schema_version != 2
        || beta_waivers.schema_version != 1
        || regression_waivers.schema_version != 2
    {
        return Err(BenchError::Qualification(format!(
            "qualification threshold or waiver schema version is unsupported: thresholds={} beta={} regression={}",
            thresholds.schema_version,
            beta_waivers.schema_version,
            regression_waivers.schema_version
        )));
    }
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
    let mut row_dispositions = Vec::with_capacity(benchmark_manifest.rows.len());
    for row in &benchmark_manifest.rows {
        let feature_id = classify_manifest_row(row)?;
        let threshold = threshold_by_id.get(row.id.as_str()).copied();
        let regression_waiver = regression_by_id.get(row.id.as_str()).copied();
        let adapter_waived = beta_by_id.contains_key(row.id.as_str())
            || regression_waiver.is_some_and(is_adapter_waiver);
        let selected_stim_symbols = selected_stim_symbols(row, &upstream_perf_sources);
        let classifications =
            row_classifications(row, threshold, adapter_waived, &selected_stim_symbols);
        let decision = row_decision(row);
        let runtime_group_id = replacements::runtime_group_id(&row.id);
        let disposition = manifest_disposition(row);
        let stim_mapping = stim_mapping(row, adapter_waived);
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
            performance_feature: feature_id.to_string(),
            supporting_performance_features,
            disposition,
            runtime_group_id,
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
            replacement_contracts: replacements::contracts(row),
            waiver_refs: waiver_refs(row, &beta_by_id, &regression_by_id),
        });
    }
    row_dispositions.sort_by(|left, right| left.id.cmp(&right.id));

    let performance_features = PERFORMANCE_FEATURE_IDS
        .iter()
        .map(|feature_id| {
            let mut correctness_features = correctness
                .features
                .iter()
                .filter(|feature| {
                    feature
                        .performance_groups
                        .iter()
                        .any(|group| group == feature_id)
                })
                .map(|feature| feature.id.clone())
                .collect::<Vec<_>>();
            correctness_features.sort();
            PerformanceFeature {
                id: (*feature_id).to_string(),
                correctness_features,
            }
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

fn discover_perf_sources(
    root: &RepoRoot,
    manifest: &BenchmarkManifest,
) -> Result<Vec<UpstreamPerfSource>, BenchError> {
    let list =
        read_repo_text_bounded(root, &safe_stim_source_path(root, "file_lists/perf_files")?)?;
    let mut sources = Vec::new();
    for path in list.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let source = read_repo_text_bounded(root, &safe_stim_source_path(root, path)?)?;
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
    let path = root.path.join(relative);
    crate::source_file::validate_repo_regular_file(root, &path)?;
    Ok(path)
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

fn merge_waivers(
    beta: &[BetaWaiverRow],
    regression: &[RegressionWaiverRow],
) -> Vec<WaiverDisposition> {
    let mut by_id = BTreeMap::<String, Vec<WaiverSourcePolicy>>::new();
    for row in beta {
        by_id
            .entry(row.id.clone())
            .or_default()
            .push(WaiverSourcePolicy {
                waiver_file: "benchmarks/m12-primary-beta-waivers.json".to_string(),
                kind: WaiverKind::NoComparableBaseline,
                reason: row.reason.clone(),
                follow_up: row.follow_up.clone(),
                measurement_pairs: Vec::new(),
            });
    }
    for row in regression {
        by_id
            .entry(row.id.clone())
            .or_default()
            .push(WaiverSourcePolicy {
                waiver_file: "benchmarks/m12-primary-regression-waivers.json".to_string(),
                kind: row.kind,
                reason: row.reason.clone(),
                follow_up: row.follow_up.clone(),
                measurement_pairs: row.measurement_pairs.clone(),
            });
    }
    by_id
        .into_iter()
        .map(|(id, mut policies)| {
            policies.sort_by(|left, right| left.waiver_file.cmp(&right.waiver_file));
            let retirement_mapping = policies
                .iter()
                .all(|policy| policy.kind == WaiverKind::NoComparableBaseline)
                .then(|| waiver_retirement_mapping(&id).to_string());
            WaiverDisposition {
                id,
                policies,
                qualification_disposition: PerformanceDisposition::Diagnostic,
                retirement_mapping,
            }
        })
        .collect()
}

fn is_adapter_waiver(row: &RegressionWaiverRow) -> bool {
    row.kind == WaiverKind::NoComparableBaseline
}

fn waiver_retirement_mapping(id: &str) -> &'static str {
    match id {
        "m4-circuit-canonical-print" | "m7-convert-stim-canonical" => {
            "stim_adapter::circuit::canonical_serialize"
        }
        "m7-convert-01-to-ptb64" => "stim_adapter::result::convert_01_to_ptb64",
        "m8-measure-reader-ptb64-contract" => "stim_adapter::result::read_ptb64_dense_sparse",
        _ => "UNMAPPED-WAIVER",
    }
}

fn waiver_refs(
    row: &BenchmarkRow,
    beta: &BTreeMap<&str, &BetaWaiverRow>,
    regression: &BTreeMap<&str, &RegressionWaiverRow>,
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

pub(super) use super::runtime::identity::sha256_hex;

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
    fn regression_waiver_schema_requires_a_kind_and_pair_list() {
        let missing_kind = r#"{
            "schema_version": 2,
            "rows": [{
                "id": "row",
                "measurement_pairs": [],
                "reason": "reason",
                "follow_up": "follow-up"
            }]
        }"#;
        let missing_pairs = r#"{
            "schema_version": 2,
            "rows": [{
                "id": "row",
                "kind": "no-comparable-baseline",
                "reason": "reason",
                "follow_up": "follow-up"
            }]
        }"#;

        let kind_error = serde_json::from_str::<IdRows<RegressionWaiverRow>>(missing_kind)
            .err()
            .expect("schema-v2 regression waiver kind must be required");
        let pair_error = serde_json::from_str::<IdRows<RegressionWaiverRow>>(missing_pairs)
            .err()
            .expect("schema-v2 regression waiver pairs must be required");

        assert!(kind_error.to_string().contains("missing field `kind`"));
        assert!(
            pair_error
                .to_string()
                .contains("missing field `measurement_pairs`")
        );
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
    fn pinned_stim_sources_support_a_retained_repository_root() {
        let directory = tempfile::tempdir().expect("temporary repository");
        std::fs::create_dir_all(directory.path().join("vendor/stim/file_lists"))
            .expect("create Stim source directory");
        std::fs::write(
            directory.path().join("vendor/stim/file_lists/perf_files"),
            b"src/example.perf.cc\n",
        )
        .expect("write Stim perf list");
        let root = RepoRoot::resolve(directory.path()).expect("resolve repository");
        let descriptor = crate::source_file::open_repo_directory_descriptor(&root, &root.path)
            .expect("retain repository root");
        let retained = RepoRoot::from_retained_descriptor(descriptor);

        let path = safe_stim_source_path(&retained, "file_lists/perf_files")
            .expect("validate through retained root");
        let contents = read_repo_text_bounded(&retained, &path)
            .expect("read Stim source through retained root");

        assert_eq!(contents, "src/example.perf.cc\n");
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

        assert!(error.to_string().contains("source input"));
    }
}
