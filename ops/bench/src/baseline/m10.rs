use std::hint::black_box;
use std::time::{Duration, Instant};

use stab_core::{
    Circuit, CodeDistance, DetectorErrorModel, ErrorAnalyzerOptions, Probability, RoundCount,
    SurfaceCodeParams, SurfaceCodeTask, circuit_to_detector_error_model,
    generate_surface_code_circuit, independent_to_disjoint_xyz_errors,
    shortest_graphlike_undetectable_logical_error, try_disjoint_to_independent_xyz_errors,
};

use crate::allocations::measure_tracked_memory;
use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{
    STAB_COMPARE_ITERATIONS, TINY_DIRECT_COMPARE_REPETITIONS, duration_variance_seconds,
    measure_stab_iterations, stab_runner_error,
};

const DEM_PARSE_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/sample_dem_deterministic.dem");
const ANALYZE_BASIC_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/analyze_errors_basic.stim");
const ANALYZE_FOLD_REPEAT_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/analyze_errors_fold_repeat.stim");
const ERROR_ANALYZER_COMPARE_ITERATIONS: usize = 16;
const ERROR_ANALYZER_ROUNDS: u32 = 3;
const ERROR_ANALYZER_DISTANCE: u32 = 3;
const GRAPHLIKE_SEARCH_DETECTORS: u64 = 128;
const GRAPHLIKE_SEARCH_GRAPH_EDGES: f64 = (GRAPHLIKE_SEARCH_DETECTORS * 2) as f64;

pub(super) fn run_dem_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "m10-graphlike-search" => run_graphlike_search_row(row).map(Some),
        "m10-error-analyzer" => run_error_analyzer_row(row).map(Some),
        "m10-error-decomp" => run_error_decomp_row(row).map(Some),
        "m10-dem-parse-contract" => run_dem_parse_row(row).map(Some),
        "m10-dem-print-contract" => run_dem_print_row(row).map(Some),
        "m10-analyze-errors-decompose-cli" => run_analyze_decompose_row(row).map(Some),
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
        ("m10-analyze-errors-decompose-cli", "stab_analyze_errors_decompose_basic") => {
            Some((1.0, "circuits/s"))
        }
        ("m10-error-analyzer", "stab_error_analyzer_surface_code") => {
            Some((error_analyzer_detector_count(), "detectors/s"))
        }
        ("m10-error-decomp", "stab_independent_to_disjoint_xyz_errors")
        | ("m10-error-decomp", "stab_disjoint_to_independent_xyz_errors_approx_exact")
        | ("m10-error-decomp", "stab_disjoint_to_independent_xyz_errors_approx_p10")
        | ("m10-error-decomp", "stab_disjoint_to_independent_xyz_errors_approx_p100") => {
            Some((1.0, "conversions/s"))
        }
        ("m10-graphlike-search", "stab_graphlike_search_chain") => {
            Some((GRAPHLIKE_SEARCH_GRAPH_EDGES, "graphlike-edges/s"))
        }
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m10-dem-parse-contract" => Some(
            "contract-representative: Stab measures in-process .dem parse/print on the current M10 deterministic fixture",
        ),
        "m10-dem-print-contract" => Some(
            "contract-only: Stab measures in-process .dem printing on the current M10 deterministic fixture; pinned Stim has no matching DEM canonical-print CLI or stim_perf row",
        ),
        "m10-analyze-errors-fold-cli" => Some(
            "contract-representative: Stab measures in-process analyze_errors --fold_loops on the current high-repeat fixture",
        ),
        "m10-analyze-errors-high-repeat-contract" => Some(
            "cli-baseline: Stab measures in-process analyze_errors --fold_loops against pinned Stim analyze_errors on the same high-repeat fixture",
        ),
        "m10-analyze-errors-decompose-cli" => Some(
            "contract-representative: Stab measures in-process analyze_errors --decompose_errors on the pinned basic CLI fixture; deeper decomposition stress remains covered by the m10-error-decomp contract",
        ),
        "m10-error-analyzer" => Some(
            "contract-representative: Stab measures in-process generated rotated-memory-z surface-code analysis at d3/r3; the upstream Stim perf row uses d11/r100 and remains the eventual scale target",
        ),
        "m10-error-decomp" => Some(
            "direct-match: Stab measures independent/disjoint XYZ probability conversion families against pinned Stim util_bot error_decomp perf filters; exact and independent paths use case-diverse in-process batches to reduce timer noise",
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

fn run_analyze_decompose_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = Circuit::from_stim_str(ANALYZE_BASIC_FIXTURE)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![measure_stab_iterations(
        "stab_analyze_errors_decompose_basic",
        STAB_COMPARE_ITERATIONS,
        || {
            let dem = circuit_to_detector_error_model(
                &circuit,
                ErrorAnalyzerOptions {
                    decompose_errors: true,
                    ..ErrorAnalyzerOptions::default()
                },
            )
            .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(dem.items().len());
            Ok(())
        },
    )?])
}

fn run_error_analyzer_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = error_analyzer_surface_code(&row.id)?;
    Ok(vec![measure_stab_iterations(
        "stab_error_analyzer_surface_code",
        ERROR_ANALYZER_COMPARE_ITERATIONS,
        || {
            let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
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

fn run_error_decomp_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let p01 = benchmark_probability(&row.id, 0.01)?;
    let p02 = benchmark_probability(&row.id, 0.02)?;
    let p10 = benchmark_probability(&row.id, 0.1)?;
    let p15 = benchmark_probability(&row.id, 0.15)?;
    let p20 = benchmark_probability(&row.id, 0.2)?;
    let p30 = benchmark_probability(&row.id, 0.3)?;
    let zero = benchmark_probability(&row.id, 0.0)?;
    let independent_cases = [
        [p10, p20, p30],
        [p10, p30, p20],
        [p20, p10, p30],
        [p20, p30, p10],
        [p30, p10, p20],
        [p30, p20, p10],
    ];
    let exact_cases = [
        [p10, p20, p15],
        [p10, p15, p20],
        [p20, p10, p15],
        [p20, p15, p10],
        [p15, p10, p20],
        [p15, p20, p10],
    ];
    Ok(vec![
        measure_error_decomp_cases(
            "stab_independent_to_disjoint_xyz_errors",
            &independent_cases,
            |[x, y, z]| {
                black_box(independent_to_disjoint_xyz_errors(
                    black_box(x),
                    black_box(y),
                    black_box(z),
                ))
                .map_err(|error| stab_runner_error(&row.id, error))?;
                Ok(())
            },
        )?,
        measure_error_decomp_cases(
            "stab_disjoint_to_independent_xyz_errors_approx_exact",
            &exact_cases,
            |[x, y, z]| {
                black_box(try_disjoint_to_independent_xyz_errors(
                    black_box(x),
                    black_box(y),
                    black_box(z),
                ))
                .map_err(|error| stab_runner_error(&row.id, error))?;
                Ok(())
            },
        )?,
        measure_error_decomp_cases(
            "stab_disjoint_to_independent_xyz_errors_approx_p10",
            &[[p10, p20, zero]],
            |[x, y, z]| {
                black_box(try_disjoint_to_independent_xyz_errors(
                    black_box(x),
                    black_box(y),
                    black_box(z),
                ))
                .map_err(|error| stab_runner_error(&row.id, error))?;
                Ok(())
            },
        )?,
        measure_error_decomp_cases(
            "stab_disjoint_to_independent_xyz_errors_approx_p100",
            &[[p01, p02, zero]],
            |[x, y, z]| {
                black_box(try_disjoint_to_independent_xyz_errors(
                    black_box(x),
                    black_box(y),
                    black_box(z),
                ))
                .map_err(|error| stab_runner_error(&row.id, error))?;
                Ok(())
            },
        )?,
    ])
}

fn benchmark_probability(row_id: &str, value: f64) -> Result<Probability, BenchError> {
    Probability::try_new(value).map_err(|error| stab_runner_error(row_id, error))
}

fn measure_error_decomp_cases(
    name: &str,
    cases: &[[Probability; 3]],
    mut operation: impl FnMut([Probability; 3]) -> Result<(), BenchError>,
) -> Result<Measurement, BenchError> {
    let operation_count = TINY_DIRECT_COMPARE_REPETITIONS
        .checked_mul(cases.len())
        .filter(|count| *count > 0)
        .ok_or_else(|| BenchError::StabRunner {
            row_id: "m10-error-decomp".to_string(),
            message: "error-decomposition benchmark requires at least one case".to_string(),
        })?;
    let mut timings = Vec::with_capacity(STAB_COMPARE_ITERATIONS);
    for _ in 0..STAB_COMPARE_ITERATIONS {
        let start = Instant::now();
        run_error_decomp_case_batch(cases, &mut operation)?;
        timings.push(start.elapsed().div_f64(operation_count as f64));
    }
    let variance_seconds = duration_variance_seconds(&timings);
    timings.sort();
    let seconds = timings
        .get(timings.len() / 2)
        .map(Duration::as_secs_f64)
        .unwrap_or_default();
    let tracked_memory =
        measure_tracked_memory(|| run_error_decomp_case_batch(cases, &mut operation))?;
    Ok(Measurement {
        name: name.to_string(),
        seconds,
        variance_seconds,
        allocation: tracked_memory.allocation,
        resident_bytes: tracked_memory.resident_bytes_max,
        iterations: Some(STAB_COMPARE_ITERATIONS),
    })
}

fn run_error_decomp_case_batch(
    cases: &[[Probability; 3]],
    operation: &mut impl FnMut([Probability; 3]) -> Result<(), BenchError>,
) -> Result<(), BenchError> {
    for _ in 0..TINY_DIRECT_COMPARE_REPETITIONS {
        for case in cases {
            operation(black_box(*case))?;
        }
    }
    Ok(())
}

fn error_analyzer_surface_code(row_id: &str) -> Result<Circuit, BenchError> {
    let probability =
        Probability::try_new(0.001).map_err(|error| stab_runner_error(row_id, error))?;
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(u64::from(ERROR_ANALYZER_ROUNDS))
            .map_err(|error| stab_runner_error(row_id, error))?,
        CodeDistance::try_new(ERROR_ANALYZER_DISTANCE)
            .map_err(|error| stab_runner_error(row_id, error))?,
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .map_err(|error| stab_runner_error(row_id, error))?
    .with_before_measure_flip_probability(probability)
    .with_after_reset_flip_probability(probability)
    .with_after_clifford_depolarization(probability);
    let generated =
        generate_surface_code_circuit(&params).map_err(|error| stab_runner_error(row_id, error))?;
    Ok(generated.circuit().clone())
}

fn error_analyzer_detector_count() -> f64 {
    let distance = ERROR_ANALYZER_DISTANCE as f64;
    let rounds = ERROR_ANALYZER_ROUNDS as f64;
    (distance * distance - 1.0) * rounds
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
