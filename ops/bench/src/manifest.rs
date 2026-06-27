use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::error::BenchError;
use crate::root::RepoRoot;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct BenchmarkRow {
    pub(crate) id: String,
    pub(crate) milestone: Milestone,
    pub(crate) threshold_class: String,
    pub(crate) runner: Runner,
    pub(crate) upstream_source: String,
    pub(crate) stim_perf_filter: String,
    pub(crate) argv: String,
    pub(crate) stdin_path: String,
    pub(crate) phase: String,
    pub(crate) measurement: String,
    pub(crate) description: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) enum Milestone {
    #[serde(rename = "M4")]
    M4,
    #[serde(rename = "M5")]
    M5,
    #[serde(rename = "M6")]
    M6,
    #[serde(rename = "M7")]
    M7,
    #[serde(rename = "M8")]
    M8,
    #[serde(rename = "M9")]
    M9,
    #[serde(rename = "M10")]
    M10,
    #[serde(rename = "M11")]
    M11,
    #[serde(rename = "M12")]
    M12,
}

impl Milestone {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::M4 => "M4",
            Self::M5 => "M5",
            Self::M6 => "M6",
            Self::M7 => "M7",
            Self::M8 => "M8",
            Self::M9 => "M9",
            Self::M10 => "M10",
            Self::M11 => "M11",
            Self::M12 => "M12",
        }
    }

    fn all_implementation_milestones() -> &'static [Self] {
        &[
            Self::M4,
            Self::M5,
            Self::M6,
            Self::M7,
            Self::M8,
            Self::M9,
            Self::M10,
            Self::M11,
            Self::M12,
        ]
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) enum Runner {
    #[serde(rename = "contract-only")]
    ContractOnly,
    #[serde(rename = "stim-cli")]
    StimCli,
    #[serde(rename = "stim-perf")]
    StimPerf,
}

impl Runner {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ContractOnly => "contract-only",
            Self::StimCli => "stim-cli",
            Self::StimPerf => "stim-perf",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct CompatibilityRow {
    upstream_path: String,
    source_kind: CompatibilitySourceKind,
    priority: CompatibilityPriority,
    status: CompatibilityStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum CompatibilitySourceKind {
    #[serde(rename = "perf")]
    Perf,
    #[serde(rename = "future")]
    Future,
    #[serde(other)]
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum CompatibilityPriority {
    #[serde(rename = "Bench")]
    Bench,
    #[serde(other)]
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum CompatibilityStatus {
    #[serde(rename = "planned")]
    Planned,
    #[serde(other)]
    Other,
}

impl CompatibilityRow {
    fn requires_benchmark(&self) -> bool {
        matches!(
            self.source_kind,
            CompatibilitySourceKind::Perf | CompatibilitySourceKind::Future
        ) && self.priority == CompatibilityPriority::Bench
            && self.status == CompatibilityStatus::Planned
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BenchmarkManifest {
    pub(crate) rows: Vec<BenchmarkRow>,
}

impl BenchmarkManifest {
    pub(crate) fn read(root: &RepoRoot) -> Result<Self, BenchError> {
        let path = root.manifest();
        let content = std::fs::read_to_string(&path)
            .map_err(|source| BenchError::ReadManifest { path, source })?;
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(content.as_bytes());
        let rows = reader.deserialize().collect::<Result<Vec<_>, _>>()?;
        Ok(Self { rows })
    }

    pub(crate) fn check(&self, root: &RepoRoot) -> Result<(), BenchError> {
        let mut violations = Vec::new();
        let mut ids = BTreeSet::new();
        let stim_source = root.default_stim_source();
        for row in &self.rows {
            if row.id.is_empty() {
                violations.push("row with empty id".to_string());
            } else if !ids.insert(row.id.clone()) {
                violations.push(format!("duplicate benchmark id {}", row.id));
            } else if !is_safe_benchmark_id(&row.id) {
                violations.push(format!(
                    "{} has unsafe id; ids may only contain ASCII letters, digits, hyphen, and underscore",
                    row.id
                ));
            }
            for (field, value) in [
                ("threshold_class", &row.threshold_class),
                ("upstream_source", &row.upstream_source),
                ("phase", &row.phase),
                ("measurement", &row.measurement),
                ("description", &row.description),
            ] {
                if value.is_empty() {
                    violations.push(format!("{} has empty {field}", row.id));
                }
            }
            validate_vendor_source(&stim_source, row, &mut violations);
            match row.runner {
                Runner::StimPerf => {
                    if row.stim_perf_filter.is_empty() {
                        violations.push(format!("{} stim-perf row has no filter", row.id));
                    }
                    if !row.argv.is_empty() {
                        violations.push(format!("{} stim-perf row should not set argv", row.id));
                    }
                }
                Runner::StimCli => {
                    if !row.stim_perf_filter.is_empty() {
                        violations.push(format!("{} stim-cli row should not set filter", row.id));
                    }
                    if row.argv_tokens().is_empty() {
                        violations.push(format!("{} stim-cli row has no argv tokens", row.id));
                    }
                    if row.argv.split('|').any(str::is_empty) {
                        violations.push(format!("{} has an empty argv token", row.id));
                    }
                    if !row.stdin_path.is_empty() {
                        validate_repo_file(root, &row.id, "stdin_path", &row.stdin_path)
                            .unwrap_or_else(|violation| violations.push(violation));
                    }
                }
                Runner::ContractOnly => {
                    if !row.stim_perf_filter.is_empty() || !row.argv.is_empty() {
                        violations.push(format!(
                            "{} contract-only row should not set runnable fields",
                            row.id
                        ));
                    }
                }
            }
        }
        self.check_required_contracts(&mut violations);
        self.check_milestone_coverage(&mut violations);
        self.check_compatibility_coverage(root, &mut violations);
        if violations.is_empty() {
            Ok(())
        } else {
            Err(BenchError::ManifestValidation(
                violations.join("\n").into_boxed_str(),
            ))
        }
    }

    pub(crate) fn list(&self, milestone: Option<&str>) {
        let mut groups: BTreeMap<(Milestone, String, Runner), Vec<&BenchmarkRow>> = BTreeMap::new();
        for row in &self.rows {
            if milestone.is_some_and(|milestone| milestone != row.milestone.as_str()) {
                continue;
            }
            groups
                .entry((row.milestone, row.threshold_class.clone(), row.runner))
                .or_default()
                .push(row);
        }
        for ((milestone, threshold_class, runner), rows) in groups {
            println!(
                "{} / {} / {}:",
                milestone.as_str(),
                threshold_class,
                runner.as_str()
            );
            for row in rows {
                println!(
                    "- {} [{}] {} -> {}",
                    row.id, row.phase, row.measurement, row.upstream_source
                );
            }
        }
    }

    pub(crate) fn filtered<'a>(
        &'a self,
        only: &[String],
    ) -> Result<Vec<&'a BenchmarkRow>, BenchError> {
        for filter in only {
            if !self
                .rows
                .iter()
                .any(|row| row.id == *filter || row.milestone.as_str() == filter)
            {
                return Err(BenchError::UnmatchedFilter(filter.clone()));
            }
        }
        Ok(self
            .rows
            .iter()
            .filter(|row| {
                only.is_empty()
                    || only
                        .iter()
                        .any(|filter| row.id == *filter || row.milestone.as_str() == filter)
            })
            .collect())
    }

    pub(crate) fn compare_rows<'a>(
        &'a self,
        milestone: Option<&str>,
        primary: bool,
    ) -> Result<Vec<&'a BenchmarkRow>, BenchError> {
        let rows = self
            .rows
            .iter()
            .filter(|row| milestone.is_none_or(|milestone| milestone == row.milestone.as_str()))
            .filter(|row| !primary || row.is_primary())
            .collect::<Vec<_>>();
        if let Some(milestone) = milestone
            && rows.is_empty()
        {
            return Err(BenchError::UnmatchedFilter(format!(
                "milestone {milestone}"
            )));
        }
        if primary && rows.is_empty() {
            return Err(BenchError::UnmatchedFilter("primary".to_string()));
        }
        Ok(rows)
    }

    fn check_compatibility_coverage(&self, root: &RepoRoot, violations: &mut Vec<String>) {
        let benchmark_sources = self
            .rows
            .iter()
            .map(|row| row.upstream_source.as_str())
            .collect::<BTreeSet<_>>();
        let content = match std::fs::read_to_string(root.compatibility_matrix()) {
            Ok(content) => content,
            Err(error) => {
                violations.push(format!("failed to read compatibility matrix: {error}"));
                return;
            }
        };
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(content.as_bytes());
        for row in reader.deserialize::<CompatibilityRow>() {
            match row {
                Ok(row) => {
                    if row.requires_benchmark()
                        && !benchmark_sources.contains(row.upstream_path.as_str())
                    {
                        violations.push(format!("missing benchmark row for {}", row.upstream_path));
                    }
                }
                Err(error) => {
                    violations.push(format!("failed to parse compatibility matrix row: {error}"));
                }
            }
        }
    }

    fn check_required_contracts(&self, violations: &mut Vec<String>) {
        let ids = self
            .rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<BTreeSet<_>>();
        for id in REQUIRED_BENCHMARK_IDS {
            if !ids.contains(id) {
                violations.push(format!("missing required benchmark contract {id}"));
            }
        }
    }

    fn check_milestone_coverage(&self, violations: &mut Vec<String>) {
        for milestone in Milestone::all_implementation_milestones() {
            if !self.rows.iter().any(|row| row.milestone == *milestone) {
                violations.push(format!("missing benchmark rows for {}", milestone.as_str()));
            }
        }
    }
}

impl BenchmarkRow {
    pub(crate) fn is_primary(&self) -> bool {
        self.milestone != Milestone::M12 && self.threshold_class != "baseline-metadata"
    }

    pub(crate) fn argv_tokens(&self) -> Vec<String> {
        self.argv
            .split('|')
            .filter(|token| !token.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }

    pub(crate) fn stdin(&self, root: &RepoRoot) -> Result<Vec<u8>, BenchError> {
        if self.stdin_path.is_empty() {
            return Ok(Vec::new());
        }
        let path = root.path.join(&self.stdin_path);
        std::fs::read(&path).map_err(|source| BenchError::ReadStdin { path, source })
    }
}

fn validate_vendor_source(stim_source: &Path, row: &BenchmarkRow, violations: &mut Vec<String>) {
    let source = Path::new(&row.upstream_source);
    if source.components().any(unsafe_component) {
        violations.push(format!(
            "{} has unsafe upstream source {}",
            row.id, row.upstream_source
        ));
        return;
    }
    if is_future_source(source) {
        if row.runner != Runner::ContractOnly {
            violations.push(format!(
                "{} future upstream source must use contract-only runner",
                row.id
            ));
        }
        return;
    }
    let path = stim_source.join(source);
    if !path.is_file() {
        violations.push(format!(
            "{} upstream source does not exist: {}",
            row.id, row.upstream_source
        ));
    }
}

fn is_future_source(source: &Path) -> bool {
    matches!(
        source.components().next(),
        Some(Component::Normal(component)) if component == "future"
    )
}

fn validate_repo_file(
    root: &RepoRoot,
    id: &str,
    field: &str,
    relative: &str,
) -> Result<(), String> {
    let relative_path = Path::new(relative);
    if relative_path.components().any(unsafe_component) {
        return Err(format!("{id} has unsafe {field} {relative}"));
    }
    let path = root.path.join(relative_path);
    if !path.is_file() {
        return Err(format!("{id} {field} does not exist: {relative}"));
    }
    Ok(())
}

fn unsafe_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Prefix(_) | Component::RootDir | Component::ParentDir | Component::CurDir
    )
}

pub(crate) fn is_safe_benchmark_id(id: &str) -> bool {
    id.bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

const REQUIRED_BENCHMARK_IDS: &[&str] = &[
    "m4-circuit-parse",
    "m4-circuit-canonical-print",
    "m7-convert-stim-canonical",
    "m7-gen-repetition-d3-r3",
    "m7-gen-repetition-d3-r30",
    "m7-gen-repetition-d5-r5",
    "m7-gen-repetition-d5-r50",
    "m7-gen-repetition-d9-r9",
    "m7-gen-repetition-d9-r90",
    "m7-gen-repetition-d17-r17",
    "m7-gen-repetition-d17-r170",
    "m7-gen-rotated-surface-d3-r3",
    "m7-gen-rotated-surface-d3-r30",
    "m7-gen-rotated-surface-d5-r5",
    "m7-gen-rotated-surface-d5-r50",
    "m7-gen-rotated-surface-d9-r9",
    "m7-gen-rotated-surface-d9-r90",
    "m7-gen-rotated-surface-d17-r17",
    "m7-gen-rotated-surface-d17-r170",
    "m7-gen-unrotated-surface-d3-r3",
    "m7-gen-unrotated-surface-d3-r30",
    "m7-gen-unrotated-surface-d5-r5",
    "m7-gen-unrotated-surface-d5-r50",
    "m7-gen-unrotated-surface-d9-r9",
    "m7-gen-unrotated-surface-d9-r90",
    "m7-gen-color-d5-r5",
    "m8-sample-analysis-1shot",
    "m8-sample-throughput-1024",
    "m8-sample-throughput-1000000",
    "m8-probability-util",
    "m8-sample-primary-repetition-contract",
    "m8-sample-primary-rotated-surface-contract",
    "m8-sample-primary-unrotated-surface-contract",
    "m8-sample-high-repeat-contract",
    "m9-detect-text-cli",
    "m9-detect-bitpacked-cli",
    "m9-convert-measurements-dets",
    "m9-detect-primary-matrix-contract",
    "m9-m2d-text-cli",
    "m9-m2d-bitpacked-contract",
    "m9-m2d-primary-matrix-contract",
    "m10-analyze-errors-high-repeat-contract",
    "m10-dem-parse-contract",
    "m10-dem-print-contract",
    "m11-sample-dem-sparse-contract",
    "m11-sample-dem-dense-contract",
    "m11-sample-dem-repeated-contract",
    "m11-sample-dem-high-detector-contract",
    "m12-primary-performance-matrix",
];

#[cfg(test)]
mod tests {
    use super::{BenchmarkManifest, Runner};

    const MANIFEST_CSV: &str = include_str!("../../../benchmarks/manifest.csv");

    #[test]
    fn repository_benchmark_manifest_passes_validation() {
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(MANIFEST_CSV.as_bytes());
        let rows = reader
            .deserialize()
            .collect::<Result<Vec<_>, _>>()
            .expect("parse manifest");
        let manifest = BenchmarkManifest { rows };
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repo root");
        let root = crate::root::RepoRoot::resolve(root).expect("resolve repo root");

        manifest.check(&root).expect("manifest validation");
    }

    #[test]
    fn primary_compare_rows_freeze_m4_through_m11_without_metadata_or_m12_placeholders() {
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(MANIFEST_CSV.as_bytes());
        let rows = reader
            .deserialize()
            .collect::<Result<Vec<_>, _>>()
            .expect("parse manifest");
        let manifest = BenchmarkManifest { rows };

        let primary = manifest
            .compare_rows(None, true)
            .expect("primary compare rows");

        assert!(!primary.is_empty());
        assert!(primary.iter().all(|row| row.is_primary()));
        assert!(primary.iter().any(|row| row.id == "m4-circuit-parse"));
        assert!(
            primary
                .iter()
                .any(|row| row.id == "m11-sample-dem-high-detector-contract")
        );
        assert!(
            primary
                .iter()
                .all(|row| row.id != "m7-perf-harness" && row.milestone != super::Milestone::M12)
        );
    }

    #[test]
    fn legacy_contract_named_rows_can_use_public_stim_cli_baselines() {
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(MANIFEST_CSV.as_bytes());
        let rows = reader
            .deserialize()
            .collect::<Result<Vec<super::BenchmarkRow>, _>>()
            .expect("parse manifest");

        for (id, expected_argv, expected_stdin) in [
            (
                "m9-m2d-bitpacked-contract",
                "m2d|--in_format=01|--out_format=b8|--circuit=oracle/fixtures/inputs/m2d_basic.stim",
                "oracle/fixtures/inputs/m2d_basic_measurements.01",
            ),
            (
                "m10-analyze-errors-high-repeat-contract",
                "analyze_errors|--fold_loops",
                "oracle/fixtures/inputs/analyze_errors_fold_repeat.stim",
            ),
            (
                "m11-sample-dem-sparse-contract",
                "sample_dem|--shots|64|--out_format=b8|--seed|5",
                "benchmarks/fixtures/m11_sample_dem_sparse_contract.dem",
            ),
            (
                "m11-sample-dem-dense-contract",
                "sample_dem|--shots|64|--out_format=b8|--seed|5",
                "benchmarks/fixtures/m11_sample_dem_dense_contract.dem",
            ),
            (
                "m11-sample-dem-repeated-contract",
                "sample_dem|--shots|64|--out_format=b8|--seed|5",
                "benchmarks/fixtures/m11_sample_dem_repeated_contract.dem",
            ),
            (
                "m11-sample-dem-high-detector-contract",
                "sample_dem|--shots|64|--out_format=b8|--seed|5",
                "benchmarks/fixtures/m11_sample_dem_high_detector_contract.dem",
            ),
        ] {
            let row = rows
                .iter()
                .find(|row| row.id == id)
                .expect("benchmark row should exist");

            assert_eq!(
                row.runner,
                Runner::StimCli,
                "{id} should use pinned Stim CLI"
            );
            assert_eq!(row.argv, expected_argv);
            assert_eq!(row.stdin_path, expected_stdin);
        }
    }

    #[test]
    fn benchmark_ids_are_filename_safe_for_report_artifacts() {
        assert!(super::is_safe_benchmark_id("m11-sample_dem"));
        assert!(!super::is_safe_benchmark_id("m11/sample-dem"));
        assert!(!super::is_safe_benchmark_id("../m11-sample-dem"));
    }
}
