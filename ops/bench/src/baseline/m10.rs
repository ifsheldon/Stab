use std::hint::black_box;

use stab_core::{
    Circuit, DetectorErrorModel, ErrorAnalyzerOptions, circuit_to_detector_error_model,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{STAB_COMPARE_ITERATIONS, measure_stab_iterations, stab_runner_error};

const DEM_PARSE_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/sample_dem_deterministic.dem");
const ANALYZE_FOLD_REPEAT_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/analyze_errors_fold_repeat.stim");

pub(super) fn run_dem_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
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
