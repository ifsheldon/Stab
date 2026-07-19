use std::collections::BTreeSet;

use super::{
    MAX_INPUT_BYTES, ThresholdRow, UpstreamPerfSource, sha256_hex, waiver_retirement_mapping,
};
use crate::comparability::ComparabilityClass;
use crate::error::BenchError;
use crate::manifest::{BenchmarkRow, Runner, ThresholdClass};
use crate::qualification::model::{
    FixtureLocator, InputByteCount, Phase, RowClassification, RowDecision, RunnerFidelity,
    ScalePoint, StimMapping, ThresholdPolicy, WorkloadFamily,
};
use crate::root::RepoRoot;

pub(super) fn classify_manifest_row(row: &BenchmarkRow) -> Result<&'static str, BenchError> {
    let id = row.id.as_str();
    let feature = if id == "m12-primary-performance-matrix" {
        "PERF-RESOURCE-BOUNDARIES"
    } else if matches!(
        id,
        "m7-perf-harness" | "m7-cli-dispatch" | "pf7-cli-legacy-dispatch-startup"
    ) {
        "PERF-CLI-STARTUP-AND-ERRORS"
    } else if id.starts_with("m8-measure-reader-") {
        "PERF-RESULT-IO"
    } else if id.starts_with("m7-convert-") || id == "m9-convert-measurements-dets" {
        "PERF-CONVERT-CLI"
    } else if id.starts_with("m7-gen-") {
        "PERF-GENERATION"
    } else if id.starts_with("m5-") || id == "m8-probability-util" {
        "PERF-BIT-KERNELS"
    } else if id.starts_with("m6-") {
        "PERF-STABILIZER-ALGEBRA"
    } else if matches!(
        id,
        "m4-gate-lookup" | "pf1-gate-metadata-lookup" | "pf3-gate-semantic-wide"
    ) {
        "PERF-GATE-CONTRACT"
    } else if id.starts_with("m11-") || id == "pf4-dem-sampler-folded-repeat" {
        "PERF-DEM-SAMPLING"
    } else if id.starts_with("m9-detect-")
        || id.starts_with("m9-m2d-")
        || id.starts_with("pf3-m2d-")
        || id == "pf3-detect-sweep-sampling"
        || id.starts_with("pf7-cli-m2d-")
    {
        "PERF-DETECTION"
    } else if id.starts_with("m8-") {
        "PERF-SAMPLING"
    } else if matches!(
        id,
        "m10-error-analyzer"
            | "m10-error-decomp"
            | "m10-analyze-errors-decompose-cli"
            | "m10-analyze-errors-fold-cli"
            | "m10-analyze-errors-high-repeat-contract"
            | "pf3-analyze-errors-sweep"
            | "pf6-error-decomp-loop-folded"
            | "pfm-b5-analyzer-cycle-folding"
            | "pfm-b5-analyzer-generated-qec"
            | "pf7-cli-analyze-errors-generated"
            | "pf7-cli-analyze-errors-decompose"
    ) {
        "PERF-ERROR-ANALYSIS"
    } else if id == "m10-graphlike-search"
        || matches!(
            id,
            "pf4-dem-folded-graphlike-traversal"
                | "pf4-dem-hypergraph-logical-repeat"
                | "pf4-dem-hypergraph-no-target-repeat"
                | "pf4-dem-search-zero-shift-repeat"
                | "pf4-dem-search-annotation-repeat"
                | "pf4-dem-search-mixed-zero-probability-repeat"
                | "pf4-dem-search-nested-repeat"
                | "pf4-dem-sat-flat-repeat-fold"
                | "pf4-error-matcher-filter-flat-repeat"
                | "pf4-error-matcher-filter-nested-repeat"
                | "pf4-error-matcher-filter-logical-repeat"
                | "pf4-error-matcher-filter-annotation-repeat"
                | "pf6-sparse-rev-frame-loop"
        )
        || id.starts_with("pfm-b5-graphlike-")
        || id.starts_with("pfm-b5-hypergraph-")
        || id.starts_with("pfm-b5-wcnf-")
    {
        "PERF-SEARCH-AND-MATCHING"
    } else if matches!(
        id,
        "m9-detecting-regions-basic-batch"
            | "m9-missing-detectors-basic-batch"
            | "pf2-time-reverse-flow"
            | "pf2-time-reverse-flow-measurement"
    ) || id.starts_with("pfm-b1-time-reverse-")
        || id.starts_with("pf5-")
        || id == "pfm-b4-flow-solve-matrix-sizes"
    {
        "PERF-FLOWS-AND-DETECTOR-UTILITIES"
    } else if matches!(
        id,
        "m10-dem-parse-contract"
            | "m10-dem-print-contract"
            | "pf1-dem-counts-repeat"
            | "pf1-dem-without-tags"
            | "pf4-dem-flatten-repeat"
            | "pf4-dem-rounded"
            | "pf4-dem-coordinate-map"
            | "pf4-dem-folded-traversal"
            | "pfm-b3-dem-traversal-core"
    ) {
        "PERF-DEM-MODEL"
    } else if id.starts_with("m4-circuit-")
        || id == "pf1-circuit-coordinate-query"
        || id.starts_with("pf2-circuit-")
        || matches!(
            id,
            "pf2-feedback-inline-batch" | "m9-feedback-inline-mpp-batch"
        )
    {
        "PERF-CIRCUIT-MODEL"
    } else {
        return Err(BenchError::Qualification(format!(
            "benchmark manifest row {id:?} has no primary performance owner"
        )));
    };
    Ok(feature)
}
pub(super) fn classify_phase(row: &BenchmarkRow) -> Phase {
    let id = row.id.as_str();
    if row.runner == Runner::StimCli {
        Phase::EndToEnd
    } else if row.phase == "startup" {
        Phase::Startup
    } else if id.contains("parse") || id.contains("reader") {
        Phase::Parse
    } else if id.contains("print") || id.contains("write") {
        Phase::Serialize
    } else if id.contains("convert") || id.contains("m2d") {
        Phase::Convert
    } else if id.contains("search")
        || id.contains("graphlike")
        || id.contains("hypergraph")
        || id.contains("wcnf")
    {
        Phase::Search
    } else if id.contains("flatten")
        || id.contains("rounded")
        || id.contains("without")
        || id.contains("time-reverse")
        || id.contains("decompose")
        || id.contains("feedback")
    {
        Phase::Transform
    } else if id.contains("reference-sample-tree") {
        Phase::Compile
    } else {
        Phase::Execute
    }
}

pub(super) fn row_decision(row: &BenchmarkRow) -> RowDecision {
    const REWORKED: [&str; 3] = ["m4-circuit-parse", "m5-simd-bits", "m5-simd-word"];
    const REMOVED: [&str; 2] = ["m7-perf-harness", "m12-primary-performance-matrix"];
    const SUPERSEDED: [&str; 11] = [
        "m10-analyze-errors-fold-cli",
        "m4-circuit-canonical-print",
        "m5-simd-bit-table",
        "m5-sparse-xor",
        "m6-clifford-string",
        "m6-pauli-iter",
        "m6-pauli-string",
        "m9-feedback-inline-mpp-batch",
        "pf3-m2d-sweep-b8",
        "pf7-cli-m2d-sweep-b8",
        "pf7-cli-m2d-feedback-inline",
    ];
    const DIAGNOSTIC: [&str; 4] = [
        "m7-cli-dispatch",
        "m7-convert-stim-canonical",
        "m7-convert-01-to-ptb64",
        "pf3-gate-semantic-wide",
    ];
    if REWORKED.contains(&row.id.as_str()) {
        RowDecision::Reworked
    } else if REMOVED.contains(&row.id.as_str()) {
        RowDecision::Removed
    } else if SUPERSEDED.contains(&row.id.as_str()) {
        RowDecision::Superseded
    } else if DIAGNOSTIC.contains(&row.id.as_str()) {
        RowDecision::Diagnostic
    } else if row.runner == Runner::StimPerf && row.comparability == ComparabilityClass::DirectMatch
    {
        RowDecision::Retained
    } else {
        RowDecision::Reworked
    }
}

pub(super) fn row_classifications(
    row: &BenchmarkRow,
    threshold: Option<&ThresholdRow>,
    waived: bool,
    selected_stim_symbols: &[String],
) -> Vec<RowClassification> {
    if row.id == "m4-circuit-parse" {
        return vec![RowClassification::Faithful];
    }
    let mut values = BTreeSet::new();
    let decision = row_decision(row);
    let active = !matches!(decision, RowDecision::Removed);
    if matches!(decision, RowDecision::Removed) {
        values.insert(RowClassification::Stale);
    }
    if matches!(decision, RowDecision::Superseded) {
        values.insert(RowClassification::Duplicate);
    }
    if waived {
        values.insert(RowClassification::Diagnostic);
        values.insert(RowClassification::AdapterCandidate);
    } else if active {
        match row.comparability {
            ComparabilityClass::DirectMatch if row.runner == Runner::StimPerf => {
                values.insert(RowClassification::Faithful);
            }
            ComparabilityClass::PartialMatch
            | ComparabilityClass::ContractRepresentative
            | ComparabilityClass::ContractSmoke
            | ComparabilityClass::ContractProxy => {
                values.insert(RowClassification::Proxy);
            }
            _ => {
                values.insert(RowClassification::Diagnostic);
            }
        }
    }
    if active && !scale_complete_or_inapplicable(row) {
        values.insert(RowClassification::MissingScale);
    }
    if active {
        values.insert(RowClassification::MissingOutputDigest);
    }
    if active {
        values.insert(RowClassification::MissingCorrectnessPreflight);
    }
    if active && row.runner == Runner::StimCli {
        values.insert(RowClassification::InProcessProcessMismatch);
    }
    if active && row.runner == Runner::ContractOnly {
        values.insert(RowClassification::MissingComparator);
        values.insert(RowClassification::AdapterCandidate);
    }
    if row.runner == Runner::StimPerf && selected_stim_symbols.len() > 1 {
        values.insert(RowClassification::HeterogeneousMeasurements);
    }
    let paired = threshold
        .into_iter()
        .flat_map(|threshold| &threshold.measurement_thresholds)
        .map(|pair| pair.stim_name.as_str())
        .collect::<BTreeSet<_>>();
    if row.runner == Runner::StimPerf
        && selected_stim_symbols.len() > 1
        && selected_stim_symbols
            .iter()
            .any(|symbol| !paired.contains(symbol.as_str()))
    {
        values.insert(RowClassification::UnmatchedSubmeasurement);
    }
    values.into_iter().collect()
}

fn scale_complete_or_inapplicable(row: &BenchmarkRow) -> bool {
    const INAPPLICABLE: [&str; 6] = [
        "m4-gate-lookup",
        "pf1-gate-metadata-lookup",
        "m10-error-decomp",
        "m7-cli-dispatch",
        "pf7-cli-legacy-dispatch-startup",
        "pf3-gate-semantic-wide",
    ];
    let id = row.id.as_str();
    INAPPLICABLE.contains(&id)
        || id.starts_with("m7-gen-") && !id.contains("-color-")
        || matches!(
            id,
            "m8-sample-analysis-1shot"
                | "m8-sample-throughput-1024"
                | "m8-sample-throughput-1000000"
        )
        || matches!(
            id,
            "pfm-b1-time-reverse-generated-surface"
                | "pfm-b1-time-reverse-mpad-matrix"
                | "pfm-b1-time-reverse-large-unitary-repeat"
                | "pfm-b4-flow-solve-matrix-sizes"
        )
}

pub(super) fn stim_mapping(row: &BenchmarkRow, waived: bool) -> StimMapping {
    if waived {
        StimMapping::PlannedAdapter {
            symbol: waiver_retirement_mapping(&row.id).to_string(),
            source: row.upstream_source.clone(),
        }
    } else {
        match row.runner {
            Runner::StimPerf => StimMapping::StimPerf {
                source: row.upstream_source.clone(),
                filter: row.stim_perf_filter.clone(),
            },
            Runner::StimCli => StimMapping::ProcessCli {
                argv: row.argv.clone(),
                stdin_path: row.stdin_path.clone(),
            },
            Runner::ContractOnly => StimMapping::PlannedAdapter {
                symbol: format!("stim_adapter::{}", row.id.replace('-', "_")),
                source: row.upstream_source.clone(),
            },
        }
    }
}

pub(super) fn runner_fidelity(row: &BenchmarkRow, waived: bool) -> RunnerFidelity {
    if waived {
        RunnerFidelity::AdapterLibrary
    } else {
        match row.runner {
            Runner::StimPerf => RunnerFidelity::StimPerf,
            Runner::StimCli => RunnerFidelity::ProcessCli,
            Runner::ContractOnly => RunnerFidelity::AdapterLibrary,
        }
    }
}

pub(super) fn threshold_policy(
    row: &BenchmarkRow,
    has_threshold: bool,
    waived: bool,
    has_unmatched_submeasurement: bool,
) -> ThresholdPolicy {
    if waived || has_unmatched_submeasurement {
        ThresholdPolicy::ReportOnly
    } else if has_threshold && row.runner == Runner::StimPerf {
        ThresholdPolicy::Primary1_25
    } else if row.threshold_class == ThresholdClass::BaselineMetadata {
        ThresholdPolicy::NotApplicable
    } else if row.threshold_class == ThresholdClass::PerformanceGate {
        ThresholdPolicy::RegressionOnly
    } else {
        ThresholdPolicy::ReportOnly
    }
}

pub(super) fn selected_stim_symbols(
    row: &BenchmarkRow,
    sources: &[UpstreamPerfSource],
) -> Vec<String> {
    if row.runner != Runner::StimPerf {
        return Vec::new();
    }
    sources
        .iter()
        .filter(|source| source.path == row.upstream_source)
        .flat_map(|source| &source.symbols)
        .filter(|symbol| {
            row.stim_perf_filter.split(',').any(|filter| {
                let filter = filter.trim();
                filter.strip_suffix('*').map_or_else(
                    || symbol.as_str() == filter,
                    |prefix| symbol.starts_with(prefix),
                )
            })
        })
        .cloned()
        .collect()
}

pub(super) fn workload_family(
    root: &RepoRoot,
    row: &BenchmarkRow,
) -> Result<WorkloadFamily, BenchError> {
    let (input_bytes, fixture, deterministic_seed) = if row.stdin_path.is_empty() {
        (
            InputByteCount::NotApplicable,
            FixtureLocator::Inline { id: row.id.clone() },
            format!("source-owned-inline:{}", row.id),
        )
    } else {
        let path = root.path.join(&row.stdin_path);
        let bytes =
            crate::source_file::read_repo_regular_file_bounded(root, &path, MAX_INPUT_BYTES)?;
        let length = u64::try_from(bytes.len()).map_err(|_| {
            BenchError::Qualification(format!(
                "benchmark fixture {} length does not fit in u64",
                row.stdin_path
            ))
        })?;
        (
            InputByteCount::Exact { bytes: length },
            FixtureLocator::RepositoryFile {
                path: row.stdin_path.clone(),
                sha256: sha256_hex(&bytes),
            },
            "corpus-digest-owned".to_string(),
        )
    };
    Ok(WorkloadFamily {
        fixture,
        source: row.upstream_source.clone(),
        deterministic_seed,
        scales: vec![ScalePoint {
            id: "inherited".to_string(),
            parameters: row.description.clone(),
            input_bytes,
            semantic_work: None,
            input_digest: None,
        }],
    })
}
