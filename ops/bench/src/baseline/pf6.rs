use std::hint::black_box;

use stab_core::{
    __circuit_to_detector_error_model_with_diagnostics, Circuit, CodeDistance, DetectorErrorModel,
    ErrorAnalyzerDiagnostics, ErrorAnalyzerOptions, Probability, RoundCount, SurfaceCodeParams,
    SurfaceCodeTask, find_undetectable_logical_error, generate_surface_code_circuit,
    likeliest_error_sat_problem, shortest_error_sat_problem,
    shortest_graphlike_undetectable_logical_error,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::{Measurement, MeasurementObservation};

use super::{measure_stab_iterations, stab_runner_error};

const NESTED_ANALYZER_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/pfm_b5_analyzer_nested_loop.stim");
const COORDINATE_ANALYZER_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/pfm_b5_analyzer_coordinate_loop.stim");
const GAUGE_ANALYZER_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/pfm_b5_analyzer_gauge_loop.stim");
const REPETITION_ANALYZER_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/pfm_b5_analyzer_repetition_code.stim");

#[cfg(not(test))]
const PF6_MEASUREMENT_ITERATIONS: usize = 8;
#[cfg(test)]
const PF6_MEASUREMENT_ITERATIONS: usize = 1;
#[cfg(not(test))]
const PF6_SEARCH_ITERATIONS: usize = 3;
#[cfg(test)]
const PF6_SEARCH_ITERATIONS: usize = 1;

#[cfg(not(test))]
const TRANSIENT_REPEAT_COUNT: u64 = 1_000_001;
#[cfg(test)]
const TRANSIENT_REPEAT_COUNT: u64 = 17;
#[cfg(not(test))]
const SHORT_PERIOD_REPEAT_COUNT: u64 = 1_000_001;
#[cfg(test)]
const SHORT_PERIOD_REPEAT_COUNT: u64 = 81;
#[cfg(not(test))]
const LONG_PERIOD_REPEAT_COUNT: u64 = 1_000_082;
#[cfg(test)]
const LONG_PERIOD_REPEAT_COUNT: u64 = 1_024;

const NESTED_REPRESENTED_ITERATIONS: u64 = 1_001_000;
const GAUGE_REPRESENTED_ITERATIONS: u64 = 1_000_000_000_000_000;
const REPETITION_CODE_ROUNDS: u64 = 100_000;

#[cfg(not(test))]
const ANALYZER_SURFACE_DISTANCE: u32 = 11;
#[cfg(test)]
const ANALYZER_SURFACE_DISTANCE: u32 = 3;
#[cfg(not(test))]
const ANALYZER_SURFACE_ROUNDS: u64 = 100_000_000;
#[cfg(test)]
const ANALYZER_SURFACE_ROUNDS: u64 = 1_000;

#[cfg(not(test))]
const DIRECT_GRAPHLIKE_NODES: u64 = 512;
#[cfg(test)]
const DIRECT_GRAPHLIKE_NODES: u64 = 32;
const DIRECT_GRAPHLIKE_ID_STEP: u64 = 1_024;

#[cfg(not(test))]
const DIRECT_HYPERGRAPH_SEGMENTS: u64 = 64;
#[cfg(test)]
const DIRECT_HYPERGRAPH_SEGMENTS: u64 = 2;
const DIRECT_HYPERGRAPH_BASE_NODES: u64 = 10;

#[cfg(not(test))]
const DIRECT_WCNF_DETECTORS: u64 = 512;
#[cfg(test)]
const DIRECT_WCNF_DETECTORS: u64 = 16;

#[cfg(not(test))]
const GENERATED_REPORT_DISTANCE: u32 = 5;
#[cfg(test)]
const GENERATED_REPORT_DISTANCE: u32 = 3;
#[cfg(not(test))]
const GENERATED_REPORT_ROUNDS: u64 = 5;
#[cfg(test)]
const GENERATED_REPORT_ROUNDS: u64 = 3;

#[cfg(not(test))]
const GENERATED_D25_DISTANCE: u32 = 25;
#[cfg(test)]
const GENERATED_D25_DISTANCE: u32 = 3;
#[cfg(not(test))]
const GENERATED_D25_ROUNDS: u64 = 25;
#[cfg(test)]
const GENERATED_D25_ROUNDS: u64 = 3;

#[cfg(not(test))]
const GENERATED_D11_DISTANCE: u32 = 11;
#[cfg(test)]
const GENERATED_D11_DISTANCE: u32 = 3;
#[cfg(not(test))]
const GENERATED_D11_ROUNDS: u64 = 1_000;
#[cfg(test)]
const GENERATED_D11_ROUNDS: u64 = 3;

pub(super) fn run_compare_row(row: &BenchmarkRow) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "pfm-b5-analyzer-cycle-folding" => run_analyzer_cycle_row(row).map(Some),
        "pfm-b5-analyzer-generated-qec" => run_analyzer_generated_qec_row(row).map(Some),
        "pfm-b5-graphlike-search-direct-dem" => run_graphlike_direct_row(row).map(Some),
        "pfm-b5-graphlike-generated-d25" => run_graphlike_generated_row(
            row,
            GENERATED_D25_ROUNDS,
            GENERATED_D25_DISTANCE,
            "stab_pfm_b5_graphlike_generated_d25",
        )
        .map(Some),
        "pfm-b5-graphlike-generated-d11-r1000" => run_graphlike_generated_row(
            row,
            GENERATED_D11_ROUNDS,
            GENERATED_D11_DISTANCE,
            "stab_pfm_b5_graphlike_generated_d11_r1000",
        )
        .map(Some),
        "pfm-b5-hypergraph-search-direct-dem" => run_hypergraph_direct_row(row).map(Some),
        "pfm-b5-hypergraph-search-generated-qec" => run_hypergraph_generated_row(row).map(Some),
        "pfm-b5-wcnf-direct-dem" => run_wcnf_direct_row(row).map(Some),
        "pfm-b5-wcnf-generated-qec" => run_wcnf_generated_row(row).map(Some),
        _ => Ok(None),
    }
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("pfm-b5-analyzer-cycle-folding", "stab_pfm_b5_analyzer_transient") => {
            Some((TRANSIENT_REPEAT_COUNT as f64, "represented-iterations/s"))
        }
        ("pfm-b5-analyzer-cycle-folding", "stab_pfm_b5_analyzer_short_period") => {
            Some((SHORT_PERIOD_REPEAT_COUNT as f64, "represented-iterations/s"))
        }
        ("pfm-b5-analyzer-cycle-folding", "stab_pfm_b5_analyzer_long_period") => {
            Some((LONG_PERIOD_REPEAT_COUNT as f64, "represented-iterations/s"))
        }
        ("pfm-b5-analyzer-cycle-folding", "stab_pfm_b5_analyzer_nested")
        | ("pfm-b5-analyzer-cycle-folding", "stab_pfm_b5_analyzer_coordinate") => Some((
            NESTED_REPRESENTED_ITERATIONS as f64,
            "represented-iterations/s",
        )),
        ("pfm-b5-analyzer-cycle-folding", "stab_pfm_b5_analyzer_gauge") => Some((
            GAUGE_REPRESENTED_ITERATIONS as f64,
            "represented-iterations/s",
        )),
        ("pfm-b5-analyzer-generated-qec", "stab_pfm_b5_analyzer_repetition_qec") => {
            Some((REPETITION_CODE_ROUNDS as f64, "represented-rounds/s"))
        }
        ("pfm-b5-analyzer-generated-qec", "stab_pfm_b5_analyzer_surface_qec") => {
            Some((ANALYZER_SURFACE_ROUNDS as f64, "represented-rounds/s"))
        }
        ("pfm-b5-graphlike-search-direct-dem", "stab_pfm_b5_graphlike_direct_dem") => {
            Some((DIRECT_GRAPHLIKE_NODES as f64, "detector-nodes/s"))
        }
        ("pfm-b5-graphlike-generated-d25", "stab_pfm_b5_graphlike_generated_d25") => Some((
            generated_detector_count(GENERATED_D25_DISTANCE, GENERATED_D25_ROUNDS) as f64,
            "detector-nodes/s",
        )),
        ("pfm-b5-graphlike-generated-d11-r1000", "stab_pfm_b5_graphlike_generated_d11_r1000") => {
            Some((
                generated_detector_count(GENERATED_D11_DISTANCE, GENERATED_D11_ROUNDS) as f64,
                "detector-nodes/s",
            ))
        }
        ("pfm-b5-hypergraph-search-direct-dem", "stab_pfm_b5_hypergraph_direct_dem") => {
            Some((direct_hypergraph_nodes() as f64, "detector-nodes/s"))
        }
        ("pfm-b5-hypergraph-search-generated-qec", "stab_pfm_b5_hypergraph_generated_qec") => {
            Some((
                generated_detector_count(GENERATED_REPORT_DISTANCE, GENERATED_REPORT_ROUNDS) as f64,
                "detector-nodes/s",
            ))
        }
        ("pfm-b5-wcnf-direct-dem", "stab_pfm_b5_wcnf_shortest_direct")
        | ("pfm-b5-wcnf-direct-dem", "stab_pfm_b5_wcnf_likeliest_direct") => {
            Some((direct_wcnf_clauses() as f64, "clauses/s"))
        }
        ("pfm-b5-wcnf-generated-qec", "stab_pfm_b5_wcnf_shortest_generated")
        | ("pfm-b5-wcnf-generated-qec", "stab_pfm_b5_wcnf_likeliest_generated") => None,
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "pfm-b5-analyzer-cycle-folding" => Some(
            "report-only: generic reverse-state analyzer cycle discovery across transient short-period long-period nested gauge and coordinate-shift workloads; observations come from the feature-gated production diagnostics seam",
        ),
        "pfm-b5-analyzer-generated-qec" => Some(
            "report-only: generic folded analysis of source-owned repetition r100000 and rotated-surface d11/r100000000 workloads; pinned Stim has related perf filters but no faithful aggregate two-case baseline for this row",
        ),
        "pfm-b5-graphlike-search-direct-dem" => Some(
            "report-only: direct sparse-ID DEM graphlike search with graph allocation proportional to touched detectors; no faithful pinned Stim perf filter uses this exact direct model",
        ),
        "pfm-b5-graphlike-generated-d25" => Some(
            "direct-match: generated rotated-memory-X d25/r25 graphlike search matches pinned Stim find_graphlike_logical_error_surface_code_d25 setup and excludes circuit generation and DEM analysis from timing",
        ),
        "pfm-b5-graphlike-generated-d11-r1000" => Some(
            "direct-match: generated rotated-memory-X d11/r1000 graphlike search matches pinned Stim find_graphlike_logical_error_surface_code_d11_r1000 setup and excludes circuit generation and DEM analysis from timing",
        ),
        "pfm-b5-hypergraph-search-direct-dem" => Some(
            "report-only: direct bounded-degree hypergraph search includes four-detector mechanisms and sparse irrelevant components without a pinned Stim perf filter",
        ),
        "pfm-b5-hypergraph-search-generated-qec" => Some(
            "report-only: generated rotated-memory-X hypergraph search has pinned semantic tests but no faithful pinned Stim performance baseline",
        ),
        "pfm-b5-wcnf-direct-dem" => Some(
            "report-only: shortest and weighted WCNF generation over a deterministic direct chain DEM records exact variables and clauses without a pinned Stim perf filter",
        ),
        "pfm-b5-wcnf-generated-qec" => Some(
            "report-only: shortest and weighted WCNF generation over a generated QEC DEM has pinned semantic tests but no faithful pinned Stim performance baseline",
        ),
        _ => None,
    }
}

fn run_analyzer_cycle_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let transient = parse_circuit(row, &transient_circuit(TRANSIENT_REPEAT_COUNT))?;
    let short_period = parse_circuit(row, &short_period_circuit(SHORT_PERIOD_REPEAT_COUNT))?;
    let long_period = parse_circuit(row, &long_period_circuit(LONG_PERIOD_REPEAT_COUNT))?;
    let nested = parse_circuit(row, NESTED_ANALYZER_FIXTURE)?;
    let coordinate = parse_circuit(row, COORDINATE_ANALYZER_FIXTURE)?;
    let gauge = parse_circuit(row, GAUGE_ANALYZER_FIXTURE)?;
    let folded = ErrorAnalyzerOptions {
        fold_loops: true,
        ..ErrorAnalyzerOptions::default()
    };
    let gauge_options = ErrorAnalyzerOptions {
        allow_gauge_detectors: true,
        ..folded
    };

    Ok(vec![
        measure_analyzer_case(row, "stab_pfm_b5_analyzer_transient", &transient, folded)?,
        measure_analyzer_case(
            row,
            "stab_pfm_b5_analyzer_short_period",
            &short_period,
            folded,
        )?,
        measure_analyzer_case(
            row,
            "stab_pfm_b5_analyzer_long_period",
            &long_period,
            folded,
        )?,
        measure_analyzer_case(row, "stab_pfm_b5_analyzer_nested", &nested, folded)?,
        measure_analyzer_case(row, "stab_pfm_b5_analyzer_gauge", &gauge, gauge_options)?,
        measure_analyzer_case(row, "stab_pfm_b5_analyzer_coordinate", &coordinate, folded)?,
    ])
}

fn run_analyzer_generated_qec_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let repetition = parse_circuit(row, REPETITION_ANALYZER_FIXTURE)?;
    let surface = generated_surface_circuit(
        row,
        ANALYZER_SURFACE_ROUNDS,
        ANALYZER_SURFACE_DISTANCE,
        SurfaceCodeTask::RotatedMemoryZ,
        false,
    )?;
    let options = ErrorAnalyzerOptions {
        fold_loops: true,
        ..ErrorAnalyzerOptions::default()
    };
    Ok(vec![
        measure_analyzer_case(
            row,
            "stab_pfm_b5_analyzer_repetition_qec",
            &repetition,
            options,
        )?,
        measure_analyzer_case(row, "stab_pfm_b5_analyzer_surface_qec", &surface, options)?,
    ])
}

fn measure_analyzer_case(
    row: &BenchmarkRow,
    name: &str,
    circuit: &Circuit,
    options: ErrorAnalyzerOptions,
) -> Result<Measurement, BenchError> {
    let (_, diagnostics) = __circuit_to_detector_error_model_with_diagnostics(circuit, options)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    validate_fold_diagnostics(row, name, &diagnostics)?;
    let mut measurement = measure_stab_iterations(name, PF6_MEASUREMENT_ITERATIONS, || {
        let (model, diagnostics) =
            __circuit_to_detector_error_model_with_diagnostics(circuit, options)
                .map_err(|error| stab_runner_error(&row.id, error))?;
        black_box((model.items().len(), diagnostics.folded_repeat_iterations));
        Ok(())
    })?;
    measurement.observations = analyzer_observations(&diagnostics);
    Ok(measurement)
}

fn validate_fold_diagnostics(
    row: &BenchmarkRow,
    name: &str,
    diagnostics: &ErrorAnalyzerDiagnostics,
) -> Result<(), BenchError> {
    if diagnostics.used_reverse_fold
        && !diagnostics.used_bounded_fallback
        && diagnostics.recurrences_found > 0
        && diagnostics.folded_repeat_iterations > 0
        && diagnostics.emitted_compact_dem_items > 0
    {
        return Ok(());
    }
    Err(BenchError::StabRunner {
        row_id: row.id.clone(),
        message: format!("{name} did not exercise generic compact loop folding: {diagnostics:?}"),
    })
}

fn analyzer_observations(diagnostics: &ErrorAnalyzerDiagnostics) -> Vec<MeasurementObservation> {
    [
        (
            "recurrence_search_steps",
            diagnostics.recurrence_search_steps,
        ),
        ("recurrences_found", diagnostics.recurrences_found),
        ("max_recurrence_period", diagnostics.max_recurrence_period),
        (
            "represented_repeat_iterations",
            diagnostics.represented_repeat_iterations,
        ),
        (
            "folded_repeat_iterations",
            diagnostics.folded_repeat_iterations,
        ),
        ("max_boundary_entries", diagnostics.max_boundary_entries),
        (
            "emitted_compact_dem_items",
            diagnostics.emitted_compact_dem_items,
        ),
    ]
    .into_iter()
    .map(|(name, value)| MeasurementObservation {
        name: name.to_string(),
        value,
    })
    .collect()
}

fn run_graphlike_direct_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let model = direct_graphlike_model(row)?;
    let expected_items =
        usize::try_from(DIRECT_GRAPHLIKE_NODES + 1).map_err(|_| BenchError::StabRunner {
            row_id: row.id.clone(),
            message: "direct graphlike result size does not fit usize".to_string(),
        })?;
    let result = shortest_graphlike_undetectable_logical_error(&model, false)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    if result.items().len() != expected_items {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "direct graphlike model expected {expected_items} errors, got {}",
                result.items().len()
            ),
        });
    }
    let mut measurement = measure_stab_iterations(
        "stab_pfm_b5_graphlike_direct_dem",
        PF6_SEARCH_ITERATIONS,
        || {
            let result = shortest_graphlike_undetectable_logical_error(&model, false)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(result.items().len());
            Ok(())
        },
    )?;
    measurement.observations = vec![observation("detector_nodes", DIRECT_GRAPHLIKE_NODES)];
    Ok(vec![measurement])
}

fn run_graphlike_generated_row(
    row: &BenchmarkRow,
    rounds: u64,
    distance: u32,
    name: &str,
) -> Result<Vec<Measurement>, BenchError> {
    let model = generated_search_model(row, rounds, distance)?;
    let expected_distance = usize::try_from(distance).map_err(|_| BenchError::StabRunner {
        row_id: row.id.clone(),
        message: "generated graphlike distance does not fit usize".to_string(),
    })?;
    let result = shortest_graphlike_undetectable_logical_error(&model, false)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    if result.items().len() != expected_distance {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "generated graphlike model expected distance {expected_distance}, got {}",
                result.items().len()
            ),
        });
    }
    let mut measurement = measure_stab_iterations(name, PF6_SEARCH_ITERATIONS, || {
        let result = shortest_graphlike_undetectable_logical_error(&model, false)
            .map_err(|error| stab_runner_error(&row.id, error))?;
        black_box(result.items().len());
        Ok(())
    })?;
    measurement.observations = vec![
        observation("detector_nodes", generated_detector_count(distance, rounds)),
        observation("compact_dem_items", compact_dem_items(row, &model)?),
    ];
    Ok(vec![measurement])
}

fn run_hypergraph_direct_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let model = direct_hypergraph_model(row)?;
    let result = find_undetectable_logical_error(&model, 4, 4, true)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    if result.items().len() != 7 {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "direct hypergraph model expected seven errors, got {}",
                result.items().len()
            ),
        });
    }
    let mut measurement = measure_stab_iterations(
        "stab_pfm_b5_hypergraph_direct_dem",
        PF6_SEARCH_ITERATIONS,
        || {
            let result = find_undetectable_logical_error(&model, 4, 4, true)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(result.items().len());
            Ok(())
        },
    )?;
    measurement.observations = vec![observation("detector_nodes", direct_hypergraph_nodes())];
    Ok(vec![measurement])
}

fn run_hypergraph_generated_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let model = generated_search_model(row, GENERATED_REPORT_ROUNDS, GENERATED_REPORT_DISTANCE)?;
    let result = find_undetectable_logical_error(&model, 4, 4, true)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    if result.items().is_empty() {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: "generated hypergraph model produced an empty logical error".to_string(),
        });
    }
    let mut measurement = measure_stab_iterations(
        "stab_pfm_b5_hypergraph_generated_qec",
        PF6_SEARCH_ITERATIONS,
        || {
            let result = find_undetectable_logical_error(&model, 4, 4, true)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(result.items().len());
            Ok(())
        },
    )?;
    measurement.observations = vec![
        observation(
            "detector_nodes",
            generated_detector_count(GENERATED_REPORT_DISTANCE, GENERATED_REPORT_ROUNDS),
        ),
        observation("compact_dem_items", compact_dem_items(row, &model)?),
    ];
    Ok(vec![measurement])
}

fn run_wcnf_direct_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let model = direct_wcnf_model(row)?;
    let shortest =
        shortest_error_sat_problem(&model).map_err(|error| stab_runner_error(&row.id, error))?;
    let likeliest = likeliest_error_sat_problem(&model, 100)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let shortest_stats = wcnf_stats(row, &shortest)?;
    let likeliest_stats = wcnf_stats(row, &likeliest)?;
    let shortest_measurement = measure_wcnf(
        row,
        "stab_pfm_b5_wcnf_shortest_direct",
        || shortest_error_sat_problem(&model),
        shortest_stats,
    )?;
    let likeliest_measurement = measure_wcnf(
        row,
        "stab_pfm_b5_wcnf_likeliest_direct",
        || likeliest_error_sat_problem(&model, 100),
        likeliest_stats,
    )?;
    Ok(vec![shortest_measurement, likeliest_measurement])
}

fn run_wcnf_generated_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let model = generated_search_model(row, GENERATED_REPORT_ROUNDS, GENERATED_REPORT_DISTANCE)?;
    let shortest =
        shortest_error_sat_problem(&model).map_err(|error| stab_runner_error(&row.id, error))?;
    let likeliest = likeliest_error_sat_problem(&model, 100)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![
        measure_wcnf(
            row,
            "stab_pfm_b5_wcnf_shortest_generated",
            || shortest_error_sat_problem(&model),
            wcnf_stats(row, &shortest)?,
        )?,
        measure_wcnf(
            row,
            "stab_pfm_b5_wcnf_likeliest_generated",
            || likeliest_error_sat_problem(&model, 100),
            wcnf_stats(row, &likeliest)?,
        )?,
    ])
}

fn measure_wcnf(
    row: &BenchmarkRow,
    name: &str,
    mut encode: impl FnMut() -> stab_core::CircuitResult<String>,
    (variables, clauses): (u64, u64),
) -> Result<Measurement, BenchError> {
    let mut measurement = measure_stab_iterations(name, PF6_MEASUREMENT_ITERATIONS, || {
        let output = encode().map_err(|error| stab_runner_error(&row.id, error))?;
        black_box(output.len());
        Ok(())
    })?;
    measurement.observations = vec![
        observation("variables", variables),
        observation("clauses", clauses),
    ];
    Ok(measurement)
}

fn direct_graphlike_model(row: &BenchmarkRow) -> Result<DetectorErrorModel, BenchError> {
    let mut text = String::new();
    text.push_str("error(0.001) D0\n");
    for node in 0..DIRECT_GRAPHLIKE_NODES.saturating_sub(1) {
        let left = node.saturating_mul(DIRECT_GRAPHLIKE_ID_STEP);
        let right = (node + 1).saturating_mul(DIRECT_GRAPHLIKE_ID_STEP);
        text.push_str(&format!("error(0.001) D{left} D{right}\n"));
    }
    let last = DIRECT_GRAPHLIKE_NODES
        .saturating_sub(1)
        .saturating_mul(DIRECT_GRAPHLIKE_ID_STEP);
    text.push_str(&format!("error(0.001) D{last} L0\n"));
    DetectorErrorModel::from_dem_str(&text).map_err(|error| stab_runner_error(&row.id, error))
}

fn direct_hypergraph_model(row: &BenchmarkRow) -> Result<DetectorErrorModel, BenchError> {
    let mut text = String::from(
        "error(0.1) D0 D1\n\
         error(0.1) D0 D1 D2 D3\n\
         error(0.1) D2 D3 D4 D5 L0\n\
         error(0.1) D4 D5 D6 D7\n\
         error(0.1) D6 D7 D8 D9\n\
         error(0.1) D8\n\
         error(0.1) D9\n",
    );
    for segment in 0..DIRECT_HYPERGRAPH_SEGMENTS {
        let base = 100 + segment.saturating_mul(8);
        text.push_str(&format!(
            "error(0.001) D{base} D{} D{} D{}\n",
            base + 1,
            base + 2,
            base + 3
        ));
    }
    DetectorErrorModel::from_dem_str(&text).map_err(|error| stab_runner_error(&row.id, error))
}

fn direct_wcnf_model(row: &BenchmarkRow) -> Result<DetectorErrorModel, BenchError> {
    let mut text = String::new();
    text.push_str("error(0.1) D0\n");
    for detector in 0..DIRECT_WCNF_DETECTORS.saturating_sub(1) {
        text.push_str(&format!("error(0.1) D{detector} D{}\n", detector + 1));
    }
    text.push_str(&format!(
        "error(0.1) D{} L0\n",
        DIRECT_WCNF_DETECTORS.saturating_sub(1)
    ));
    DetectorErrorModel::from_dem_str(&text).map_err(|error| stab_runner_error(&row.id, error))
}

fn generated_search_model(
    row: &BenchmarkRow,
    rounds: u64,
    distance: u32,
) -> Result<DetectorErrorModel, BenchError> {
    let circuit =
        generated_surface_circuit(row, rounds, distance, SurfaceCodeTask::RotatedMemoryX, true)?;
    stab_core::circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            decompose_errors: true,
            block_decomposition_from_introducing_remnant_edges: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .map_err(|error| stab_runner_error(&row.id, error))
}

fn generated_surface_circuit(
    row: &BenchmarkRow,
    rounds: u64,
    distance: u32,
    task: SurfaceCodeTask,
    include_before_round_noise: bool,
) -> Result<Circuit, BenchError> {
    let probability =
        Probability::try_new(0.001).map_err(|error| stab_runner_error(&row.id, error))?;
    let mut params = SurfaceCodeParams::new(
        RoundCount::try_new(rounds).map_err(|error| stab_runner_error(&row.id, error))?,
        CodeDistance::try_new(distance).map_err(|error| stab_runner_error(&row.id, error))?,
        task,
    )
    .map_err(|error| stab_runner_error(&row.id, error))?
    .with_after_clifford_depolarization(probability)
    .with_before_measure_flip_probability(probability)
    .with_after_reset_flip_probability(probability);
    if include_before_round_noise {
        params = params.with_before_round_data_depolarization(probability);
    }
    generate_surface_code_circuit(&params)
        .map(|generated| generated.circuit().clone())
        .map_err(|error| stab_runner_error(&row.id, error))
}

fn parse_circuit(row: &BenchmarkRow, text: &str) -> Result<Circuit, BenchError> {
    Circuit::from_stim_str(text).map_err(|error| stab_runner_error(&row.id, error))
}

fn transient_circuit(repeat_count: u64) -> String {
    format!(
        "MR 1\nREPEAT {repeat_count} {{\n    X_ERROR(0.25) 0\n    CX 0 1\n    MR 1\n    DETECTOR rec[-2] rec[-1]\n}}\nM 0\nOBSERVABLE_INCLUDE(9) rec[-1]\n"
    )
}

fn short_period_circuit(repeat_count: u64) -> String {
    format!(
        "R 0 1 2 3 4\nREPEAT {repeat_count} {{\n    CNOT 0 1 1 2 2 3 3 4\n    DETECTOR\n}}\nM 4\nOBSERVABLE_INCLUDE(9) rec[-1]\n"
    )
}

fn long_period_circuit(repeat_count: u64) -> String {
    format!(
        "R 0 1 2 3 4 5 6\nREPEAT {repeat_count} {{\n    CNOT 0 1 1 2 2 3 3 4 4 5 5 6 6 0\n    DETECTOR\n}}\nM 6\nOBSERVABLE_INCLUDE(9) rec[-1]\nR 7\nX_ERROR(1) 7\nM 7\nDETECTOR rec[-1]\n"
    )
}

fn generated_detector_count(distance: u32, rounds: u64) -> u64 {
    u64::from(distance)
        .saturating_mul(u64::from(distance))
        .saturating_sub(1)
        .saturating_mul(rounds)
}

fn direct_hypergraph_nodes() -> u64 {
    DIRECT_HYPERGRAPH_BASE_NODES.saturating_add(DIRECT_HYPERGRAPH_SEGMENTS.saturating_mul(4))
}

fn direct_wcnf_clauses() -> u64 {
    DIRECT_WCNF_DETECTORS.saturating_mul(6).saturating_add(2)
}

fn compact_dem_items(row: &BenchmarkRow, model: &DetectorErrorModel) -> Result<u64, BenchError> {
    fn count(model: &DetectorErrorModel) -> Option<u64> {
        let mut total = 0_u64;
        for item in model.items() {
            total = total.checked_add(1)?;
            if let stab_core::DemItem::RepeatBlock(repeat) = item {
                total = total.checked_add(count(repeat.body())?)?;
            }
        }
        Some(total)
    }
    count(model).ok_or_else(|| BenchError::StabRunner {
        row_id: row.id.clone(),
        message: "compact DEM item count overflowed".to_string(),
    })
}

fn wcnf_stats(row: &BenchmarkRow, output: &str) -> Result<(u64, u64), BenchError> {
    let header = output
        .lines()
        .next()
        .ok_or_else(|| BenchError::StabRunner {
            row_id: row.id.clone(),
            message: "WCNF output has no header".to_string(),
        })?;
    let fields = header.split_whitespace().collect::<Vec<_>>();
    let variables = fields
        .get(2)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!("invalid WCNF variable count in {header:?}"),
        })?;
    let clauses = fields
        .get(3)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!("invalid WCNF clause count in {header:?}"),
        })?;
    Ok((variables, clauses))
}

fn observation(name: &str, value: u64) -> MeasurementObservation {
    MeasurementObservation {
        name: name.to_string(),
        value,
    }
}
