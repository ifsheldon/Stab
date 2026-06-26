//! Compatibility matrix loading and validation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

const IMPLEMENTATION_MILESTONES: &[Milestone] = &[
    Milestone::M4,
    Milestone::M5,
    Milestone::M6,
    Milestone::M7,
    Milestone::M8,
    Milestone::M9,
    Milestone::M10,
    Milestone::M11,
    Milestone::M12,
];

const FUTURE_BUCKETS: &[&str] = &[
    "diagrams",
    "explain_errors",
    "repl",
    "python",
    "js_wasm",
    "crumble",
    "cirq",
    "sinter",
    "stimflow",
    "zx",
    "lattice_surgery",
    "qasm",
    "quirk",
    "gpu",
];

const REQUIRED_CORE_SURFACES: &[&str] = &[
    "stim-format",
    "dem-format",
    "result-format",
    "gate-table",
    "targets",
    "pauli-string",
    "tableau",
    "sampler",
    "circuit-generation",
    "detector-conversion",
    "dem-analysis",
];

const REQUIRED_CLI_SURFACES: &[&str] = &[
    "command-gen",
    "command-convert",
    "command-sample",
    "command-detect",
    "command-m2d",
    "command-analyze-errors",
    "command-sample-dem",
];

const EXPECTED_P0_P1_PATHS: &[&str] = &[
    "src/stim.test.cc",
    "src/stim/main_namespaced.test.cc",
    "src/stim/circuit/circuit.test.cc",
    "src/stim/circuit/circuit_instruction.test.cc",
    "src/stim/circuit/gate_decomposition.test.cc",
    "src/stim/circuit/gate_target.test.cc",
    "src/stim/cmd/command_analyze_errors.test.cc",
    "src/stim/cmd/command_convert.test.cc",
    "src/stim/cmd/command_detect.test.cc",
    "src/stim/cmd/command_gen.test.cc",
    "src/stim/cmd/command_m2d.test.cc",
    "src/stim/cmd/command_sample.test.cc",
    "src/stim/cmd/command_sample_dem.test.cc",
    "src/stim/dem/dem_instruction.test.cc",
    "src/stim/dem/detector_error_model.test.cc",
    "src/stim/gates/gates.test.cc",
    "src/stim/gen/circuit_gen_params.test.cc",
    "src/stim/gen/gen_color_code.test.cc",
    "src/stim/gen/gen_rep_code.test.cc",
    "src/stim/gen/gen_surface_code.test.cc",
    "src/stim/io/measure_record.test.cc",
    "src/stim/io/measure_record_batch.test.cc",
    "src/stim/io/measure_record_batch_writer.test.cc",
    "src/stim/io/measure_record_reader.test.cc",
    "src/stim/io/measure_record_writer.test.cc",
    "src/stim/io/sparse_shot.test.cc",
    "src/stim/mem/bit_ref.test.cc",
    "src/stim/mem/simd_bit_table.test.cc",
    "src/stim/mem/simd_bits.test.cc",
    "src/stim/mem/simd_bits_range_ref.test.cc",
    "src/stim/mem/simd_util.test.cc",
    "src/stim/mem/simd_word.test.cc",
    "src/stim/mem/sparse_xor_vec.test.cc",
    "src/stim/search/graphlike/algo.test.cc",
    "src/stim/search/graphlike/edge.test.cc",
    "src/stim/search/graphlike/graph.test.cc",
    "src/stim/search/graphlike/node.test.cc",
    "src/stim/search/graphlike/search_state.test.cc",
    "src/stim/search/hyper/algo.test.cc",
    "src/stim/search/hyper/edge.test.cc",
    "src/stim/search/hyper/graph.test.cc",
    "src/stim/search/hyper/node.test.cc",
    "src/stim/search/hyper/search_state.test.cc",
    "src/stim/search/sat/wcnf.test.cc",
    "src/stim/simulators/dem_sampler.test.cc",
    "src/stim/simulators/error_analyzer.test.cc",
    "src/stim/simulators/error_matcher.test.cc",
    "src/stim/simulators/frame_simulator.test.cc",
    "src/stim/simulators/frame_simulator_util.test.cc",
    "src/stim/simulators/graph_simulator.test.cc",
    "src/stim/simulators/matched_error.test.cc",
    "src/stim/simulators/measurements_to_detection_events.test.cc",
    "src/stim/simulators/sparse_rev_frame_tracker.test.cc",
    "src/stim/simulators/tableau_simulator.test.cc",
    "src/stim/simulators/vector_simulator.test.cc",
    "src/stim/stabilizers/clifford_string.test.cc",
    "src/stim/stabilizers/flex_pauli_string.test.cc",
    "src/stim/stabilizers/flow.test.cc",
    "src/stim/stabilizers/pauli_string.test.cc",
    "src/stim/stabilizers/pauli_string_iter.test.cc",
    "src/stim/stabilizers/pauli_string_ref.test.cc",
    "src/stim/stabilizers/tableau.test.cc",
    "src/stim/stabilizers/tableau_iter.test.cc",
    "src/stim/util_bot/arg_parse.test.cc",
    "src/stim/util_bot/error_decomp.test.cc",
    "src/stim/util_bot/probability_util.test.cc",
    "src/stim/util_bot/twiddle.test.cc",
    "src/stim/util_top/circuit_flow_generators.test.cc",
    "src/stim/util_top/circuit_inverse_qec.test.cc",
    "src/stim/util_top/circuit_inverse_unitary.test.cc",
    "src/stim/util_top/circuit_to_dem.test.cc",
    "src/stim/util_top/circuit_to_detecting_regions.test.cc",
    "src/stim/util_top/circuit_vs_tableau.test.cc",
    "src/stim/util_top/count_determined_measurements.test.cc",
    "src/stim/util_top/has_flow.test.cc",
    "src/stim/util_top/mbqc_decomposition.test.cc",
    "src/stim/util_top/missing_detectors.test.cc",
    "src/stim/util_top/reference_sample_tree.test.cc",
    "src/stim/util_top/simplified_circuit.test.cc",
    "src/stim/util_top/stabilizers_to_tableau.test.cc",
    "src/stim/util_top/transform_without_feedback.test.cc",
];

const EXPECTED_P1_PATHS: &[&str] = &[
    "src/stim/main_namespaced.test.cc",
    "src/stim/cmd/command_analyze_errors.test.cc",
    "src/stim/cmd/command_convert.test.cc",
    "src/stim/cmd/command_detect.test.cc",
    "src/stim/cmd/command_gen.test.cc",
    "src/stim/cmd/command_m2d.test.cc",
    "src/stim/cmd/command_sample.test.cc",
    "src/stim/cmd/command_sample_dem.test.cc",
    "src/stim/gen/circuit_gen_params.test.cc",
    "src/stim/gen/gen_color_code.test.cc",
    "src/stim/gen/gen_rep_code.test.cc",
    "src/stim/gen/gen_surface_code.test.cc",
    "src/stim/util_bot/arg_parse.test.cc",
    "src/stim/util_top/circuit_flow_generators.test.cc",
    "src/stim/util_top/circuit_inverse_qec.test.cc",
    "src/stim/util_top/circuit_inverse_unitary.test.cc",
    "src/stim/util_top/circuit_to_detecting_regions.test.cc",
    "src/stim/util_top/circuit_vs_tableau.test.cc",
    "src/stim/util_top/count_determined_measurements.test.cc",
    "src/stim/util_top/has_flow.test.cc",
    "src/stim/util_top/mbqc_decomposition.test.cc",
    "src/stim/util_top/missing_detectors.test.cc",
    "src/stim/util_top/reference_sample_tree.test.cc",
    "src/stim/util_top/simplified_circuit.test.cc",
    "src/stim/util_top/stabilizers_to_tableau.test.cc",
    "src/stim/util_top/transform_without_feedback.test.cc",
];

const EXPECTED_DOC_PATHS: &[&str] = &[
    "doc/circuit_data_references.md",
    "doc/file_format_dem_detector_error_model.md",
    "doc/file_format_stim_circuit.md",
    "doc/gates.md",
    "doc/result_formats.md",
    "doc/usage_command_line.md",
];

const EXPECTED_BENCH_PATHS: &[&str] = &[
    "src/stim/circuit/circuit.perf.cc",
    "src/stim/gates/gates.perf.cc",
    "src/stim/io/measure_record_reader.perf.cc",
    "src/stim/main.perf.cc",
    "src/stim/main_namespaced.perf.cc",
    "src/stim/mem/simd_bit_table.perf.cc",
    "src/stim/mem/simd_bits.perf.cc",
    "src/stim/mem/simd_word.perf.cc",
    "src/stim/mem/sparse_xor_vec.perf.cc",
    "src/stim/search/graphlike/algo.perf.cc",
    "src/stim/simulators/dem_sampler.perf.cc",
    "src/stim/simulators/error_analyzer.perf.cc",
    "src/stim/simulators/frame_simulator.perf.cc",
    "src/stim/simulators/tableau_simulator.perf.cc",
    "src/stim/stabilizers/clifford_string.perf.cc",
    "src/stim/stabilizers/pauli_string.perf.cc",
    "src/stim/stabilizers/pauli_string_iter.perf.cc",
    "src/stim/stabilizers/tableau.perf.cc",
    "src/stim/stabilizers/tableau_iter.perf.cc",
    "src/stim/util_bot/error_decomp.perf.cc",
    "src/stim/util_bot/probability_util.perf.cc",
    "src/stim/util_top/reference_sample_tree.perf.cc",
    "src/stim/util_top/stabilizers_to_tableau.perf.cc",
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct MatrixRow {
    id: String,
    upstream_path: String,
    source_kind: SourceKind,
    surface: String,
    owner_crate: String,
    milestone: Milestone,
    priority: Priority,
    parity_mode: ParityMode,
    comparator_type: ComparatorType,
    status: Status,
    future_bucket: String,
    defer_reason: String,
    acceptance_check: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
enum SourceKind {
    #[serde(rename = "cxx-test")]
    CxxTest,
    #[serde(rename = "py-test")]
    PyTest,
    #[serde(rename = "js-test")]
    JsTest,
    #[serde(rename = "perf")]
    Perf,
    #[serde(rename = "snapshot")]
    Snapshot,
    #[serde(rename = "doc")]
    Doc,
    #[serde(rename = "future")]
    Future,
}

impl SourceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::CxxTest => "cxx-test",
            Self::PyTest => "py-test",
            Self::JsTest => "js-test",
            Self::Perf => "perf",
            Self::Snapshot => "snapshot",
            Self::Doc => "doc",
            Self::Future => "future",
        }
    }

    fn requires_vendor_file(self) -> bool {
        !matches!(self, Self::Future)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
enum Milestone {
    #[serde(rename = "M0")]
    M0,
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
    #[serde(rename = "Future")]
    Future,
}

impl Milestone {
    fn as_str(self) -> &'static str {
        match self {
            Self::M0 => "M0",
            Self::M4 => "M4",
            Self::M5 => "M5",
            Self::M6 => "M6",
            Self::M7 => "M7",
            Self::M8 => "M8",
            Self::M9 => "M9",
            Self::M10 => "M10",
            Self::M11 => "M11",
            Self::M12 => "M12",
            Self::Future => "Future",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
enum Priority {
    #[serde(rename = "P0")]
    P0,
    #[serde(rename = "P1")]
    P1,
    #[serde(rename = "P2")]
    P2,
    #[serde(rename = "P3")]
    P3,
    #[serde(rename = "Bench")]
    Bench,
    #[serde(rename = "Skip")]
    Skip,
}

impl Priority {
    fn as_str(self) -> &'static str {
        match self {
            Self::P0 => "P0",
            Self::P1 => "P1",
            Self::P2 => "P2",
            Self::P3 => "P3",
            Self::Bench => "Bench",
            Self::Skip => "Skip",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum Status {
    #[serde(rename = "planned")]
    Planned,
    #[serde(rename = "deferred")]
    Deferred,
    #[serde(rename = "skipped")]
    Skipped,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum ParityMode {
    #[serde(rename = "adapted")]
    Adapted,
    #[serde(rename = "deferred")]
    Deferred,
    #[serde(rename = "exact-output")]
    ExactOutput,
    #[serde(rename = "exact-output-and-statistical")]
    ExactOutputAndStatistical,
    #[serde(rename = "performance")]
    Performance,
    #[serde(rename = "property")]
    Property,
    #[serde(rename = "semantic-mining")]
    SemanticMining,
    #[serde(rename = "skipped")]
    Skipped,
    #[serde(rename = "structural")]
    Structural,
}

impl ParityMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Adapted => "adapted",
            Self::Deferred => "deferred",
            Self::ExactOutput => "exact-output",
            Self::ExactOutputAndStatistical => "exact-output-and-statistical",
            Self::Performance => "performance",
            Self::Property => "property",
            Self::SemanticMining => "semantic-mining",
            Self::Skipped => "skipped",
            Self::Structural => "structural",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum ComparatorType {
    #[serde(rename = "benchmark")]
    Benchmark,
    #[serde(rename = "deferred")]
    Deferred,
    #[serde(rename = "direct-rust")]
    DirectRust,
    #[serde(rename = "oracle")]
    Oracle,
    #[serde(rename = "skipped")]
    Skipped,
}

impl ComparatorType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Benchmark => "benchmark",
            Self::Deferred => "deferred",
            Self::DirectRust => "direct-rust",
            Self::Oracle => "oracle",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompatibilityMatrix {
    rows: Vec<MatrixRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MatrixCheckReport {
    rows: usize,
    implementation_milestones: BTreeMap<String, usize>,
}

#[derive(Debug, Error)]
pub(crate) enum MatrixError {
    #[error("failed to read compatibility matrix {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse compatibility matrix: {0}")]
    Parse(#[from] csv::Error),

    #[error("compatibility matrix validation failed:\n{0}")]
    Validation(Box<str>),

    #[error("milestone {milestone} has no compatibility matrix rows")]
    EmptyMilestone { milestone: String },
}

impl CompatibilityMatrix {
    pub(crate) fn read_from_path(path: impl AsRef<Path>) -> Result<Self, MatrixError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|source| MatrixError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_csv(&content)
    }

    fn from_csv(content: &str) -> Result<Self, MatrixError> {
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(content.as_bytes());
        let rows = reader.deserialize().collect::<Result<Vec<_>, _>>()?;
        Ok(Self { rows })
    }

    pub(crate) fn check(&self, repo_root: &Path) -> Result<MatrixCheckReport, MatrixError> {
        let mut violations = Vec::new();
        self.check_row_basics(repo_root, &mut violations);
        self.check_required_path_coverage(&mut violations);
        self.check_full_inventory_coverage(repo_root, &mut violations);
        self.check_required_surface_coverage(&mut violations);
        self.check_future_bucket_coverage(&mut violations);
        let implementation_milestones = self.check_implementation_milestones(&mut violations);

        if !violations.is_empty() {
            return Err(MatrixError::Validation(
                violations.join("\n").into_boxed_str(),
            ));
        }
        Ok(MatrixCheckReport {
            rows: self.rows.len(),
            implementation_milestones,
        })
    }

    pub(crate) fn print_summary(&self) {
        println!("Compatibility matrix rows: {}", self.rows.len());
        for (priority, count) in self.count_by(|row| row.priority.as_str()) {
            println!("priority {priority}: {count}");
        }
    }

    pub(crate) fn print_milestone(&self, milestone: &str) -> Result<(), MatrixError> {
        let rows = self.rows_for_milestone(milestone);
        if rows.is_empty() {
            return Err(MatrixError::EmptyMilestone {
                milestone: milestone.to_string(),
            });
        }
        println!("Compatibility rows for {milestone}:");
        for row in rows {
            println!(
                "- {} [{}] {}/{} {} -> {}",
                row.id,
                row.priority.as_str(),
                row.parity_mode.as_str(),
                row.comparator_type.as_str(),
                row.upstream_path,
                row.acceptance_check
            );
        }
        Ok(())
    }

    fn rows_for_milestone(&self, milestone: &str) -> Vec<&MatrixRow> {
        self.rows
            .iter()
            .filter(|row| row.milestone.as_str() == milestone)
            .collect()
    }

    fn check_row_basics(&self, repo_root: &Path, violations: &mut Vec<String>) {
        let mut ids = BTreeSet::new();
        for row in &self.rows {
            if row.id.is_empty() {
                violations.push("row with empty id".to_string());
            } else if !ids.insert(row.id.clone()) {
                violations.push(format!("duplicate row id {}", row.id));
            }
            for (field, value) in [
                ("upstream_path", &row.upstream_path),
                ("surface", &row.surface),
                ("owner_crate", &row.owner_crate),
                ("acceptance_check", &row.acceptance_check),
            ] {
                if value.is_empty() {
                    violations.push(format!("{} has empty {field}", row.id));
                }
            }
            if row.status == Status::Deferred
                && (row.future_bucket.is_empty() || row.defer_reason.is_empty())
            {
                violations.push(format!("{} deferred without bucket and reason", row.id));
            }
            if row.status == Status::Deferred
                && (row.parity_mode != ParityMode::Deferred
                    || row.comparator_type != ComparatorType::Deferred)
            {
                violations.push(format!(
                    "{} deferred row has non-deferred comparator data",
                    row.id
                ));
            }
            if row.status == Status::Skipped
                && (row.parity_mode != ParityMode::Skipped
                    || row.comparator_type != ComparatorType::Skipped)
            {
                violations.push(format!(
                    "{} skipped row has non-skipped comparator data",
                    row.id
                ));
            }
            if row.status == Status::Planned
                && (row.parity_mode == ParityMode::Deferred
                    || row.parity_mode == ParityMode::Skipped
                    || row.comparator_type == ComparatorType::Deferred
                    || row.comparator_type == ComparatorType::Skipped)
            {
                violations.push(format!(
                    "{} planned row has deferred or skipped comparator data",
                    row.id
                ));
            }
            if row.source_kind.requires_vendor_file() {
                validate_vendor_relative_file(repo_root, row, violations);
            }
        }
    }

    fn check_required_path_coverage(&self, violations: &mut Vec<String>) {
        for path in EXPECTED_P0_P1_PATHS {
            let expected_priority = if EXPECTED_P1_PATHS.contains(path) {
                Priority::P1
            } else {
                Priority::P0
            };
            if !self.rows.iter().any(|row| {
                row.upstream_path == *path
                    && row.priority == expected_priority
                    && row.source_kind == SourceKind::CxxTest
                    && row.status == Status::Planned
            }) {
                violations.push(format!("missing P0/P1 row for {path}"));
            }
        }
        for path in EXPECTED_BENCH_PATHS {
            if !self
                .rows
                .iter()
                .any(|row| row.upstream_path == *path && row.priority == Priority::Bench)
            {
                violations.push(format!("missing Bench row for {path}"));
            }
        }
        for path in EXPECTED_DOC_PATHS {
            if !self
                .rows
                .iter()
                .any(|row| row.upstream_path == *path && row.source_kind == SourceKind::Doc)
            {
                violations.push(format!("missing doc-source row for {path}"));
            }
        }
    }

    fn check_full_inventory_coverage(&self, repo_root: &Path, violations: &mut Vec<String>) {
        for (source_kind, expected) in [
            (
                SourceKind::PyTest,
                collect_vendor_paths(repo_root, |path| {
                    path.file_name()
                        .and_then(std::ffi::OsStr::to_str)
                        .is_some_and(|name| name.ends_with("_test.py"))
                }),
            ),
            (
                SourceKind::JsTest,
                collect_vendor_paths(repo_root, |path| {
                    path.file_name()
                        .and_then(std::ffi::OsStr::to_str)
                        .is_some_and(|name| name.ends_with(".test.js"))
                }),
            ),
            (
                SourceKind::Snapshot,
                collect_vendor_paths(repo_root, |path| {
                    path.starts_with(repo_root.join("vendor").join("stim").join("testdata"))
                }),
            ),
        ] {
            match expected {
                Ok(paths) => {
                    for path in paths {
                        if !self
                            .rows
                            .iter()
                            .any(|row| row.source_kind == source_kind && row.upstream_path == path)
                        {
                            violations.push(format!(
                                "missing {} inventory row for {}",
                                source_kind.as_str(),
                                path
                            ));
                        }
                    }
                }
                Err(error) => violations.push(format!(
                    "failed to collect {} inventory: {}",
                    source_kind.as_str(),
                    error
                )),
            }
        }
    }

    fn check_required_surface_coverage(&self, violations: &mut Vec<String>) {
        let surfaces = self
            .rows
            .iter()
            .map(|row| row.surface.as_str())
            .collect::<BTreeSet<_>>();
        for surface in REQUIRED_CORE_SURFACES {
            if !surfaces.contains(surface) {
                violations.push(format!("missing core surface {surface}"));
            }
        }
        for surface in REQUIRED_CLI_SURFACES {
            if !surfaces.contains(surface) {
                violations.push(format!("missing CLI surface {surface}"));
            }
        }
    }

    fn check_future_bucket_coverage(&self, violations: &mut Vec<String>) {
        let buckets = self
            .rows
            .iter()
            .filter(|row| row.status == Status::Deferred)
            .map(|row| row.future_bucket.as_str())
            .collect::<BTreeSet<_>>();
        for bucket in FUTURE_BUCKETS {
            if !buckets.contains(bucket) {
                violations.push(format!("missing deferred future bucket {bucket}"));
            }
        }
    }

    fn check_implementation_milestones(
        &self,
        violations: &mut Vec<String>,
    ) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for milestone in IMPLEMENTATION_MILESTONES {
            let milestone_name = milestone.as_str();
            let rows = self.rows_for_milestone(milestone_name);
            if rows.is_empty() {
                violations.push(format!(
                    "no rows for implementation milestone {milestone_name}"
                ));
            }
            for row in &rows {
                if row.status != Status::Planned {
                    violations.push(format!("{} is not planned for {milestone_name}", row.id));
                }
                if row.acceptance_check.is_empty() {
                    violations.push(format!(
                        "{} has unnamed dependency data for {milestone_name}",
                        row.id
                    ));
                }
            }
            counts.insert(milestone_name.to_string(), rows.len());
        }
        counts
    }

    fn count_by<'a>(
        &'a self,
        mut key: impl FnMut(&'a MatrixRow) -> &'a str,
    ) -> BTreeMap<&'a str, usize> {
        let mut counts = BTreeMap::new();
        for row in &self.rows {
            *counts.entry(key(row)).or_insert(0) += 1;
        }
        counts
    }
}

impl MatrixCheckReport {
    pub(crate) fn print(&self) {
        println!("Compatibility matrix check passed: {} rows", self.rows);
        for milestone in IMPLEMENTATION_MILESTONES {
            let milestone_name = milestone.as_str();
            if let Some(count) = self.implementation_milestones.get(milestone_name) {
                println!("{milestone_name}: {count} row(s)");
            }
        }
    }
}

fn validate_vendor_relative_file(repo_root: &Path, row: &MatrixRow, violations: &mut Vec<String>) {
    let relative = Path::new(&row.upstream_path);
    if relative.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir | Component::CurDir
        )
    }) {
        violations.push(format!(
            "{} has unsafe upstream path {}",
            row.id, row.upstream_path
        ));
        return;
    }

    let vendor_root = repo_root.join("vendor").join("stim");
    let full_path = vendor_root.join(relative);
    let Ok(canonical_vendor_root) = std::fs::canonicalize(&vendor_root) else {
        violations.push(format!("failed to canonicalize {}", vendor_root.display()));
        return;
    };
    let Ok(canonical_path) = std::fs::canonicalize(&full_path) else {
        violations.push(format!(
            "{} upstream path does not exist: {}",
            row.id, row.upstream_path
        ));
        return;
    };
    if !canonical_path.starts_with(&canonical_vendor_root) {
        violations.push(format!(
            "{} upstream path escapes vendor/stim: {}",
            row.id, row.upstream_path
        ));
        return;
    }
    if !canonical_path.is_file() {
        violations.push(format!(
            "{} upstream path is not a file: {}",
            row.id, row.upstream_path
        ));
    }
}

fn collect_vendor_paths(
    repo_root: &Path,
    predicate: impl Fn(&Path) -> bool,
) -> Result<Vec<String>, std::io::Error> {
    let vendor_root = repo_root.join("vendor").join("stim");
    let mut paths = Vec::new();
    collect_vendor_paths_recursive(&vendor_root, &vendor_root, &predicate, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_vendor_paths_recursive(
    vendor_root: &Path,
    current: &Path,
    predicate: &impl Fn(&Path) -> bool,
    paths: &mut Vec<String>,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_vendor_paths_recursive(vendor_root, &path, predicate, paths)?;
        } else if file_type.is_file()
            && predicate(&path)
            && let Ok(relative) = path.strip_prefix(vendor_root)
        {
            paths.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CompatibilityMatrix, IMPLEMENTATION_MILESTONES};

    const MATRIX_CSV: &str = include_str!("../../../oracle/compatibility-matrix.csv");
    const HEADER: &str = "id,upstream_path,source_kind,surface,owner_crate,milestone,priority,parity_mode,comparator_type,status,future_bucket,defer_reason,acceptance_check\n";

    #[test]
    fn repository_matrix_passes_coverage_checks() {
        let matrix = CompatibilityMatrix::from_csv(MATRIX_CSV).expect("parse matrix");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repo root");

        matrix.check(root).expect("matrix coverage");
    }

    #[test]
    fn implementation_milestones_have_printable_rows() {
        let matrix = CompatibilityMatrix::from_csv(MATRIX_CSV).expect("parse matrix");

        for milestone in IMPLEMENTATION_MILESTONES {
            assert!(!matrix.rows_for_milestone(milestone.as_str()).is_empty());
        }
    }

    #[test]
    fn validation_rejects_deferred_row_without_reason() {
        let csv = format!(
            "{HEADER}bad,future/foo,future,diagrams,none,Future,P3,deferred,deferred,deferred,,,future plan\n"
        );
        let matrix = CompatibilityMatrix::from_csv(&csv).expect("parse matrix");
        let error = matrix
            .check(std::path::Path::new("."))
            .expect_err("missing deferred reason should fail");

        assert!(
            error
                .to_string()
                .contains("deferred without bucket and reason")
        );
    }

    #[test]
    fn parser_rejects_invalid_priority() {
        let csv = format!(
            "{HEADER}bad,future/foo,future,diagrams,none,Future,Nope,deferred,deferred,deferred,diagrams,reason,future plan\n"
        );

        assert!(CompatibilityMatrix::from_csv(&csv).is_err());
    }

    #[test]
    fn validation_rejects_required_row_with_wrong_priority() {
        let csv = format!(
            "{HEADER}bad,src/stim.test.cc,cxx-test,library-smoke,stab-core,M0,Skip,skipped,skipped,skipped,,,bad skip\n"
        );
        let matrix = CompatibilityMatrix::from_csv(&csv).expect("parse matrix");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repo root");
        let error = matrix.check(root).expect_err("wrong priority should fail");

        assert!(
            error
                .to_string()
                .contains("missing P0/P1 row for src/stim.test.cc")
        );
    }

    #[test]
    fn validation_rejects_vendor_path_escape() {
        let csv = format!(
            "{HEADER}bad,../Cargo.toml,cxx-test,library-smoke,stab-core,M0,P0,adapted,direct-rust,planned,,,bad path\n"
        );
        let matrix = CompatibilityMatrix::from_csv(&csv).expect("parse matrix");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repo root");
        let error = matrix.check(root).expect_err("path escape should fail");

        assert!(error.to_string().contains("unsafe upstream path"));
    }
}
