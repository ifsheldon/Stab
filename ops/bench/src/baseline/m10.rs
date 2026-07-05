use std::ffi::OsString;
use std::hint::black_box;
use std::io::{self, Write};
use std::str::FromStr;
use std::time::{Duration, Instant};

use stab_core::{
    Circuit, CodeDistance, DetectorErrorModel, ErrorAnalyzerOptions, Flow, Probability, RoundCount,
    SurfaceCodeParams, SurfaceCodeTask, check_if_circuit_has_unsigned_stabilizer_flows,
    circuit_to_detector_error_model, find_undetectable_logical_error,
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
const ANALYZE_SWEEP_CONTROL_FIXTURE: &str = "X_ERROR(0.25) 0\n\
                                            CX sweep[0] 0\n\
                                            CY sweep[1] 0\n\
                                            CZ sweep[2] 0\n\
                                            CZ 0 sweep[3]\n\
                                            CZ sweep[4] sweep[5]\n\
                                            XCZ 0 sweep[6]\n\
                                            YCZ 0 sweep[7]\n\
                                            M 1\n\
                                            CZ rec[-1] sweep[8]\n\
                                            CZ sweep[9] rec[-1]\n\
                                            M 0\n\
                                            DETECTOR rec[-1]\n";
const ERROR_ANALYZER_COMPARE_ITERATIONS: usize = 16;
const ERROR_ANALYZER_ROUNDS: u32 = 3;
const ERROR_ANALYZER_DISTANCE: u32 = 3;
const GRAPHLIKE_SEARCH_DETECTORS: u64 = 128;
const GRAPHLIKE_SEARCH_GRAPH_EDGES: f64 = (GRAPHLIKE_SEARCH_DETECTORS * 2) as f64;
const ERROR_DECOMP_DIRECT_COMPARE_REPETITIONS: usize = TINY_DIRECT_COMPARE_REPETITIONS * 16;
#[cfg(not(test))]
const ERROR_DECOMP_LOOP_REPEAT_COUNT: u64 = 4096;
#[cfg(test)]
const ERROR_DECOMP_LOOP_REPEAT_COUNT: u64 = 5;
#[cfg(not(test))]
const SPARSE_REVERSE_UNITARY_REPEAT_COUNT: u64 = 1_000_001;
#[cfg(test)]
const SPARSE_REVERSE_UNITARY_REPEAT_COUNT: u64 = 17;

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
        "pf3-analyze-errors-sweep" => run_analyze_sweep_row(row).map(Some),
        "pf6-analyze-errors-generated-surface" => run_analyze_generated_core_row(row).map(Some),
        "pf6-error-decomp-loop-folded" => run_error_decomp_loop_folded_row(row).map(Some),
        "pf6-graphlike-search-generated" => run_generated_graphlike_search_row(row).map(Some),
        "pf6-hypergraph-search-generated" => run_generated_hypergraph_search_row(row).map(Some),
        "pf6-sparse-rev-frame-loop" => run_sparse_reverse_frame_loop_row(row).map(Some),
        "pf7-cli-analyze-errors-generated" => run_analyze_generated_cli_row(row).map(Some),
        "pf7-cli-analyze-errors-decompose" => run_analyze_decompose_cli_row(row).map(Some),
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
        ("pf7-cli-analyze-errors-decompose", "stab_pf7_cli_analyze_errors_decompose") => {
            Some((1.0, "circuits/s"))
        }
        ("pf7-cli-analyze-errors-generated", "stab_pf7_cli_analyze_errors_generated") => {
            Some((error_analyzer_detector_count(), "detectors/s"))
        }
        ("pf6-analyze-errors-generated-surface", "stab_pf6_analyze_errors_generated_surface") => {
            Some((error_analyzer_detector_count(), "detectors/s"))
        }
        ("pf6-error-decomp-loop-folded", "stab_pf6_error_decomp_loop_folded") => {
            Some((ERROR_DECOMP_LOOP_REPEAT_COUNT as f64, "folded-rounds/s"))
        }
        ("pf6-graphlike-search-generated", "stab_pf6_graphlike_search_generated_surface")
        | ("pf6-hypergraph-search-generated", "stab_pf6_hypergraph_search_generated_surface") => {
            Some((error_analyzer_detector_count(), "detectors/s"))
        }
        ("pf6-sparse-rev-frame-loop", "stab_pf6_sparse_rev_unitary_repeat_flow") => Some((
            SPARSE_REVERSE_UNITARY_REPEAT_COUNT as f64,
            "folded-rounds/s",
        )),
        ("pf3-analyze-errors-sweep", "stab_analyze_errors_sweep_control") => {
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
        "pf7-cli-analyze-errors-decompose" => Some(
            "report-only: Stab measures the public CLI analyze_errors --decompose_errors path for PF7 visible CLI parity using the source-owned M10 basic fixture",
        ),
        "pf7-cli-analyze-errors-generated" => Some(
            "report-only: Stab measures the public CLI analyze_errors path on the source-owned generated d3/r3 rotated-memory-z surface-code analyzer workload",
        ),
        "pf6-analyze-errors-generated-surface" => Some(
            "report-only: Stab measures the Rust generated d3/r3 rotated-memory-z surface-code analyzer workload without a faithful pinned Stim CLI timing ratio",
        ),
        "pf6-error-decomp-loop-folded" => Some(
            "report-only: Stab measures Rust analyze_errors with fold_loops plus decompose_errors over a repeated composite-error fixture; pinned Stim exposes equivalent analyzer behavior but not a faithful Rust direct baseline in this harness",
        ),
        "pf6-graphlike-search-generated" => Some(
            "report-only: Stab measures generated rotated-surface-code DEM graphlike search after source-owned Rust analysis and decomposition; pinned Stim exposes this as C++ API/perf behavior, not a faithful public CLI baseline",
        ),
        "pf6-hypergraph-search-generated" => Some(
            "report-only: Stab measures generated rotated-surface-code DEM hypergraph search after source-owned Rust analysis and decomposition; pinned Stim exposes this as C++ API behavior, not a faithful public CLI baseline",
        ),
        "pf6-sparse-rev-frame-loop" => Some(
            "report-only: Stab measures public unsigned-flow checking over a measurement-dependent fixed two-qubit Clifford unitary repeat so the sparse reverse frame tracker must use loop folding; broader sparse tracker parity and provenance remain outside this row",
        ),
        "pf3-analyze-errors-sweep" => Some(
            "report-only: Stab measures in-process analyzer handling for selected sweep-controlled Clifford gates and CZ bit-bit no-op groups that are semantically ignored by the error analyzer",
        ),
        "m10-error-analyzer" => Some(
            "contract-representative: Stab measures in-process generated rotated-memory-z surface-code analysis at d3/r3; the upstream Stim perf row uses d11/r100 and remains the eventual scale target",
        ),
        "m10-error-decomp" => Some(
            "direct-match: Stab measures independent/disjoint XYZ probability conversion families against pinned Stim util_bot error_decomp perf filters using enlarged pinned-case in-process batches; impossible zero-component disjoint cases use a semantic fast reject",
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

fn run_analyze_decompose_cli_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let args = vec![
        OsString::from("stab"),
        OsString::from("analyze_errors"),
        OsString::from("--decompose_errors"),
    ];
    Ok(vec![measure_stab_iterations(
        "stab_pf7_cli_analyze_errors_decompose",
        STAB_COMPARE_ITERATIONS,
        || {
            let mut stdout = CountingWriter::default();
            let mut stderr = Vec::new();
            let status = stab_cli::run_from(
                args.clone(),
                ANALYZE_BASIC_FIXTURE.as_bytes(),
                &mut stdout,
                &mut stderr,
            );
            if status != 0 {
                return Err(BenchError::StabRunner {
                    row_id: row.id.clone(),
                    message: format!(
                        "stab-cli analyze_errors failed with status {status}: {}",
                        String::from_utf8_lossy(&stderr)
                    ),
                });
            }
            black_box(stdout.len());
            Ok(())
        },
    )?])
}

fn run_analyze_generated_cli_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = error_analyzer_surface_code(&row.id)?;
    let circuit_text = circuit.to_stim_string();
    let args = vec![OsString::from("stab"), OsString::from("analyze_errors")];
    Ok(vec![measure_stab_iterations(
        "stab_pf7_cli_analyze_errors_generated",
        ERROR_ANALYZER_COMPARE_ITERATIONS,
        || {
            let mut stdout = CountingWriter::default();
            let mut stderr = Vec::new();
            let status = stab_cli::run_from(
                args.clone(),
                circuit_text.as_bytes(),
                &mut stdout,
                &mut stderr,
            );
            if status != 0 {
                return Err(BenchError::StabRunner {
                    row_id: row.id.clone(),
                    message: format!(
                        "stab-cli analyze_errors generated fixture failed with status {status}: {}",
                        String::from_utf8_lossy(&stderr)
                    ),
                });
            }
            black_box(stdout.len());
            Ok(())
        },
    )?])
}

fn run_analyze_generated_core_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = error_analyzer_surface_code(&row.id)?;
    Ok(vec![measure_stab_iterations(
        "stab_pf6_analyze_errors_generated_surface",
        ERROR_ANALYZER_COMPARE_ITERATIONS,
        || {
            let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(dem.items().len());
            Ok(())
        },
    )?])
}

fn run_error_decomp_loop_folded_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = Circuit::from_stim_str(&error_decomp_loop_folded_fixture())
        .map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![measure_stab_iterations(
        "stab_pf6_error_decomp_loop_folded",
        ERROR_ANALYZER_COMPARE_ITERATIONS,
        || {
            let dem = circuit_to_detector_error_model(
                &circuit,
                ErrorAnalyzerOptions {
                    fold_loops: true,
                    decompose_errors: true,
                    block_decomposition_from_introducing_remnant_edges: true,
                    ..ErrorAnalyzerOptions::default()
                },
            )
            .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(dem.items().len());
            Ok(())
        },
    )?])
}

fn run_generated_graphlike_search_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let model = generated_search_surface_code_dem(&row.id)?;
    Ok(vec![measure_stab_iterations(
        "stab_pf6_graphlike_search_generated_surface",
        ERROR_ANALYZER_COMPARE_ITERATIONS,
        || {
            let shortest = shortest_graphlike_undetectable_logical_error(&model, false)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(shortest.items().len());
            Ok(())
        },
    )?])
}

fn run_generated_hypergraph_search_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let model = generated_search_surface_code_dem(&row.id)?;
    Ok(vec![measure_stab_iterations(
        "stab_pf6_hypergraph_search_generated_surface",
        ERROR_ANALYZER_COMPARE_ITERATIONS,
        || {
            let shortest = find_undetectable_logical_error(&model, 4, 4, true)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(shortest.items().len());
            Ok(())
        },
    )?])
}

fn run_analyze_sweep_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = Circuit::from_stim_str(ANALYZE_SWEEP_CONTROL_FIXTURE)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![measure_stab_iterations(
        "stab_analyze_errors_sweep_control",
        STAB_COMPARE_ITERATIONS,
        || {
            let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(dem.items().len());
            Ok(())
        },
    )?])
}

fn run_sparse_reverse_frame_loop_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = Circuit::from_stim_str(&sparse_reverse_frame_loop_fixture())
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let flows = [
        Flow::from_str("Z_ -> rec[-1]").map_err(|error| BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!("failed to parse sparse reverse flow benchmark fixture: {error}"),
        })?,
    ];
    Ok(vec![measure_stab_iterations(
        "stab_pf6_sparse_rev_unitary_repeat_flow",
        ERROR_ANALYZER_COMPARE_ITERATIONS,
        || {
            let result = check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows);
            if result != [true] {
                return Err(BenchError::StabRunner {
                    row_id: row.id.clone(),
                    message: format!(
                        "sparse reverse flow benchmark expected [true], got {result:?}"
                    ),
                });
            }
            black_box(result);
            Ok(())
        },
    )?])
}

#[derive(Default)]
struct CountingWriter {
    bytes: usize,
}

impl CountingWriter {
    fn len(&self) -> usize {
        self.bytes
    }
}

impl Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes = self.bytes.checked_add(buf.len()).ok_or_else(|| {
            io::Error::other("analyze_errors benchmark output byte count overflowed")
        })?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
    let independent_cases = [[p10, p20, p30]];
    let exact_cases = [[p10, p20, p15]];
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
    let operation_count = ERROR_DECOMP_DIRECT_COMPARE_REPETITIONS
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
        resident_delta_bytes: tracked_memory.resident_delta_bytes_max,
        iterations: Some(STAB_COMPARE_ITERATIONS),
    })
}

fn run_error_decomp_case_batch(
    cases: &[[Probability; 3]],
    operation: &mut impl FnMut([Probability; 3]) -> Result<(), BenchError>,
) -> Result<(), BenchError> {
    for _ in 0..ERROR_DECOMP_DIRECT_COMPARE_REPETITIONS {
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

fn generated_search_surface_code_dem(row_id: &str) -> Result<DetectorErrorModel, BenchError> {
    let circuit = error_analyzer_surface_code(row_id)?;
    circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            decompose_errors: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .map_err(|error| stab_runner_error(row_id, error))
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

fn sparse_reverse_frame_loop_fixture() -> String {
    format!("REPEAT {SPARSE_REVERSE_UNITARY_REPEAT_COUNT} {{\n    SWAP 0 1\n}}\nM 1\n")
}

fn error_decomp_loop_folded_fixture() -> String {
    format!(
        "\
REPEAT {ERROR_DECOMP_LOOP_REPEAT_COUNT} {{
    R 0 1 2
    X_ERROR(0.125) 0
    X_ERROR(0.25) 1
    X_ERROR(0.375) 2
    M 0 1 2
    DETECTOR rec[-3] rec[-1]
    DETECTOR rec[-2] rec[-1]
    DETECTOR rec[-3] rec[-1]
}}
"
    )
}
