use std::hint::black_box;

use stab_core::{
    Circuit, DetectorErrorModel, ErrorAnalyzerOptions, circuit_to_detector_error_model,
    shortest_graphlike_undetectable_logical_error,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{STAB_COMPARE_ITERATIONS, measure_stab_iterations, stab_runner_error};

const DEM_PARSE_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/sample_dem_deterministic.dem");
const ANALYZE_FOLD_REPEAT_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/analyze_errors_fold_repeat.stim");
const GRAPHLIKE_SEARCH_DETECTORS: u64 = 128;
const GRAPHLIKE_SEARCH_GRAPH_EDGES: f64 = (GRAPHLIKE_SEARCH_DETECTORS * 2) as f64;

pub(super) fn run_dem_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "m10-graphlike-search" => run_graphlike_search_row(row).map(Some),
        "m10-dem-parse-contract" => run_dem_parse_row(row).map(Some),
        "m10-dem-print-contract" => run_dem_print_row(row).map(Some),
        "m10-analyze-errors-fold-cli" => run_analyze_fold_row(row).map(Some),
        "m10-analyze-errors-high-repeat-contract" => run_analyze_fold_row(row).map(Some),
        _ => Ok(None),
    }
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m10-dem-parse-contract", "stab_dem_parse_sample") => {
            Some((DEM_PARSE_FIXTURE.len() as f64, "bytes/s"))
        }
        ("m10-dem-print-contract", "stab_dem_print_sample") => {
            Some((DEM_PARSE_FIXTURE.len() as f64, "bytes/s"))
        }
        ("m10-analyze-errors-fold-cli", "stab_analyze_errors_fold_repeat")
        | ("m10-analyze-errors-high-repeat-contract", "stab_analyze_errors_fold_repeat") => {
            Some((1000.0, "folded-rounds/s"))
        }
        ("m10-graphlike-search", "stab_graphlike_search_chain") => {
            Some((GRAPHLIKE_SEARCH_GRAPH_EDGES, "graphlike-edges/s"))
        }
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m10-dem-parse-contract" | "m10-dem-print-contract" => Some(
            "contract-representative: Stab measures in-process .dem parse/print on the current M10 deterministic fixture",
        ),
        "m10-analyze-errors-fold-cli" | "m10-analyze-errors-high-repeat-contract" => Some(
            "contract-representative: Stab measures in-process analyze_errors --fold_loops on the current high-repeat fixture",
        ),
        "m10-graphlike-search" => Some(
            "contract-representative: Stab measures in-process shortest graphlike logical-error search on a deterministic chain DEM",
        ),
        _ => None,
    }
}

fn run_dem_parse_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![measure_stab_iterations(
        "stab_dem_parse_sample",
        STAB_COMPARE_ITERATIONS,
        || {
            let dem = DetectorErrorModel::from_dem_str(DEM_PARSE_FIXTURE)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(dem.items().len());
            Ok(())
        },
    )?])
}

fn run_dem_print_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let dem = DetectorErrorModel::from_dem_str(DEM_PARSE_FIXTURE)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![measure_stab_iterations(
        "stab_dem_print_sample",
        STAB_COMPARE_ITERATIONS,
        || {
            let text = dem.to_dem_string();
            black_box(text.len());
            Ok(())
        },
    )?])
}

fn run_analyze_fold_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = Circuit::from_stim_str(ANALYZE_FOLD_REPEAT_FIXTURE)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![measure_stab_iterations(
        "stab_analyze_errors_fold_repeat",
        STAB_COMPARE_ITERATIONS,
        || {
            let dem = circuit_to_detector_error_model(
                &circuit,
                ErrorAnalyzerOptions {
                    fold_loops: true,
                    ..ErrorAnalyzerOptions::default()
                },
            )
            .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(dem.items().len());
            Ok(())
        },
    )?])
}

fn run_graphlike_search_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let model = graphlike_search_model(&row.id)?;
    Ok(vec![measure_stab_iterations(
        "stab_graphlike_search_chain",
        STAB_COMPARE_ITERATIONS,
        || {
            let shortest = shortest_graphlike_undetectable_logical_error(&model, false)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(shortest.items().len());
            Ok(())
        },
    )?])
}

fn graphlike_search_model(row_id: &str) -> Result<DetectorErrorModel, BenchError> {
    let mut text = String::new();
    text.push_str("error(0.001) D0\n");
    for detector in 0..GRAPHLIKE_SEARCH_DETECTORS.saturating_sub(1) {
        text.push_str("error(0.001) D");
        text.push_str(&detector.to_string());
        text.push_str(" D");
        text.push_str(&(detector + 1).to_string());
        text.push('\n');
    }
    text.push_str("error(0.001) D");
    text.push_str(&(GRAPHLIKE_SEARCH_DETECTORS - 1).to_string());
    text.push_str(" L0\n");
    DetectorErrorModel::from_dem_str(&text).map_err(|error| stab_runner_error(row_id, error))
}
