use std::hint::black_box;

use stab_core::{
    Circuit, CompiledDemSampler, DemDetectorId, DemInstructionKind, DemItem, DemTarget,
    DetectorErrorModel, ErrorAnalyzerOptions, circuit_to_detector_error_model,
    explain_errors_from_circuit, find_undetectable_logical_error, shortest_error_sat_problem,
    shortest_graphlike_undetectable_logical_error,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{measure_stab_batched, stab_runner_error};

#[cfg(not(test))]
const TRANSFORM_REPETITIONS: usize = 8;
#[cfg(test)]
const TRANSFORM_REPETITIONS: usize = 1;
#[cfg(not(test))]
const FLATTEN_REPETITIONS: u64 = 4096;
#[cfg(test)]
const FLATTEN_REPETITIONS: u64 = 2;
#[cfg(not(test))]
const ROUNDED_ERROR_COUNT: usize = 4096;
#[cfg(test)]
const ROUNDED_ERROR_COUNT: usize = 4;
#[cfg(not(test))]
const COORDINATE_MAP_DETECTORS: u64 = 4096;
#[cfg(test)]
const COORDINATE_MAP_DETECTORS: u64 = 4;
#[cfg(not(test))]
const SAMPLER_REPEAT_COUNT: u64 = 4096;
#[cfg(test)]
const SAMPLER_REPEAT_COUNT: u64 = 2;
#[cfg(not(test))]
const SAMPLER_NO_OP_REPEAT_COUNT: u64 = 1_000_000;
#[cfg(test)]
const SAMPLER_NO_OP_REPEAT_COUNT: u64 = 64;
#[cfg(not(test))]
const SAMPLER_SHOTS: usize = 64;
#[cfg(test)]
const SAMPLER_SHOTS: usize = 2;
#[cfg(not(test))]
const SEARCH_REPEAT_COUNT: u64 = 2048;
#[cfg(test)]
const SEARCH_REPEAT_COUNT: u64 = 2;
#[cfg(not(test))]
const SEARCH_ZERO_REPEAT_COUNT: u64 = 1_000_000;
#[cfg(test)]
const SEARCH_ZERO_REPEAT_COUNT: u64 = 64;
#[cfg(not(test))]
const SAT_REPEAT_COUNT: u64 = 512;
#[cfg(test)]
const SAT_REPEAT_COUNT: u64 = 2;
#[cfg(not(test))]
const ANALYZER_REPEAT_COUNT: u64 = 1024;
#[cfg(test)]
const ANALYZER_REPEAT_COUNT: u64 = 2;
#[cfg(not(test))]
const MATCHER_REPEAT_COUNT: u64 = 2048;
#[cfg(test)]
const MATCHER_REPEAT_COUNT: u64 = 2;
#[cfg(not(test))]
const FLAT_OVERLAP_REPEAT_COUNT: u64 = 4096;
#[cfg(test)]
const FLAT_OVERLAP_REPEAT_COUNT: u64 = 4;

const FLATTEN_FIXED_INSTRUCTIONS: u64 = 2;
const FLATTEN_SOURCE_INSTRUCTIONS_PER_REPETITION: u64 = 4;
const ROUNDED_REPEAT_ERROR_COUNT: usize = 2;
const SELECTED_COORDINATE_DETECTORS: usize = 2;
const SPARSE_OVERLAP_COORDINATE_DETECTORS: usize = 1;
const NESTED_SPARSE_COORDINATE_DETECTORS: usize = 1;
const SEARCH_FIXED_ERRORS: u64 = 2;
const ANALYZER_INSTRUCTIONS_PER_REPETITION: u64 = 3;
const MATCHER_INSTRUCTIONS_PER_REPETITION: u64 = 1;

pub(super) fn run_dem_transform_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "pf4-dem-flatten-repeat" => Ok(Some(run_dem_flatten_repeat_row(row)?)),
        "pf4-dem-rounded" => Ok(Some(run_dem_rounded_row(row)?)),
        "pf4-dem-coordinate-map" => Ok(Some(run_dem_coordinate_map_row(row)?)),
        "pf4-dem-folded-traversal" => Ok(Some(run_dem_search_sat_repeat_row(row)?)),
        "pf4-dem-folded-graphlike-traversal" => Ok(Some(run_dem_graphlike_repeat_row(row)?)),
        "pf4-dem-sampler-folded-repeat" => Ok(Some(run_dem_sampler_repeat_row(row)?)),
        _ => Ok(None),
    }
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("pf4-dem-flatten-repeat", "stab_pf4_dem_flatten_repeat") => Some((
            flatten_expanded_source_instructions() as f64,
            "expanded-instructions/s",
        )),
        ("pf4-dem-rounded", "stab_pf4_dem_rounded") => {
            Some((rounded_probability_args() as f64, "probability-args/s"))
        }
        ("pf4-dem-coordinate-map", "stab_pf4_dem_coordinate_map_all_bounded") => {
            Some((COORDINATE_MAP_DETECTORS as f64, "detectors/s"))
        }
        ("pf4-dem-coordinate-map", "stab_pf4_dem_coordinate_map_selected_huge_repeat") => {
            Some((SELECTED_COORDINATE_DETECTORS as f64, "selected-detectors/s"))
        }
        ("pf4-dem-coordinate-map", "stab_pf4_dem_coordinate_map_sparse_overlap") => Some((
            SPARSE_OVERLAP_COORDINATE_DETECTORS as f64,
            "selected-detectors/s",
        )),
        ("pf4-dem-coordinate-map", "stab_pf4_dem_coordinate_map_nested_sparse_overlap") => Some((
            NESTED_SPARSE_COORDINATE_DETECTORS as f64,
            "selected-detectors/s",
        )),
        ("pf4-dem-coordinate-map", "stab_pf4_dem_coordinate_map_flat_overlap_all") => {
            Some((flat_overlap_coordinate_detectors() as f64, "detectors/s"))
        }
        ("pf4-dem-sampler-folded-repeat", "stab_pf4_dem_sampler_compile_folded_repeat") => {
            Some((SAMPLER_REPEAT_COUNT as f64, "logical-error-occurrences/s"))
        }
        ("pf4-dem-sampler-folded-repeat", "stab_pf4_dem_sampler_sample_folded_repeat") => Some((
            (SAMPLER_REPEAT_COUNT as f64) * (SAMPLER_SHOTS as f64),
            "error-applications/s",
        )),
        (
            "pf4-dem-sampler-folded-repeat",
            "stab_pf4_dem_sampler_sample_zero_probability_folded_repeat",
        ) => Some((
            (SAMPLER_NO_OP_REPEAT_COUNT as f64) * (SAMPLER_SHOTS as f64),
            "skipped-detector-error-occurrences/s",
        )),
        ("pf4-dem-folded-traversal", "stab_pf4_dem_hyper_capped_repeat") => Some((
            search_expanded_errors(SEARCH_REPEAT_COUNT) as f64,
            "expanded-errors/s",
        )),
        ("pf4-dem-folded-traversal", "stab_pf4_dem_hyper_zero_probability_repeat_skip") => Some((
            SEARCH_ZERO_REPEAT_COUNT as f64,
            "skipped-zero-probability-errors/s",
        )),
        ("pf4-dem-folded-traversal", "stab_pf4_dem_sat_capped_repeat") => Some((
            search_expanded_errors(SAT_REPEAT_COUNT) as f64,
            "expanded-errors/s",
        )),
        ("pf4-dem-folded-traversal", "stab_pf4_dem_analyzer_capped_repeat") => Some((
            analyzer_expanded_instructions() as f64,
            "expanded-instructions/s",
        )),
        ("pf4-dem-folded-traversal", "stab_pf4_error_matcher_capped_repeat") => Some((
            matcher_expanded_instructions() as f64,
            "expanded-instructions/s",
        )),
        ("pf4-dem-folded-graphlike-traversal", "stab_pf4_dem_graphlike_capped_repeat") => Some((
            search_expanded_errors(SEARCH_REPEAT_COUNT) as f64,
            "expanded-errors/s",
        )),
        (
            "pf4-dem-folded-graphlike-traversal",
            "stab_pf4_dem_graphlike_zero_probability_repeat_skip",
        ) => Some((
            SEARCH_ZERO_REPEAT_COUNT as f64,
            "skipped-zero-probability-errors/s",
        )),
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "pf4-dem-flatten-repeat" => Some(
            "contract-only: Stab measures the Rust DetectorErrorModel::flattened public API over repeat, tag, detector-shift, coordinate-shift, separator, and observable cases; pinned Stim exposes equivalent behavior but not a faithful Rust direct baseline",
        ),
        "pf4-dem-rounded" => Some(
            "contract-only: Stab measures the Rust DetectorErrorModel::rounded public API over top-level and nested error probabilities while preserving non-error coordinate args; pinned Stim exposes equivalent behavior but not a faithful Rust direct baseline",
        ),
        "pf4-dem-coordinate-map" => Some(
            "contract-only: Stab measures bounded all-detector DEM coordinate maps, selected detector coordinate lookup through a huge-repeat model, sparse flat and nested overlapping selected-coordinate lookups, and many-selected flat-overlap coordinate lookup; pinned Stim exposes equivalent behavior but not a faithful Rust direct baseline",
        ),
        "pf4-dem-sampler-folded-repeat" => Some(
            "contract-only: Stab measures folded CompiledDemSampler compile, stochastic direct sample behavior, and zero-probability repeat skipping; sampled-error materialization and excessive stochastic repeated-error work remain capped and broader PF4 traversal consumers remain explicit follow-up work",
        ),
        "pf4-dem-folded-traversal" => Some(
            "contract-only: Stab measures current capped-repeat hypergraph search, zero-probability repeat skipping for hypergraph search, SAT problem generation, analyzer, and ErrorMatcher traversal behavior; true folded traversal remains an explicit RPF4 follow-up",
        ),
        "pf4-dem-folded-graphlike-traversal" => Some(
            "contract-only: Stab measures current capped-repeat graphlike search behavior plus zero-probability repeat skipping; true folded graphlike traversal remains an explicit RPF4 follow-up",
        ),
        _ => None,
    }
}

fn run_dem_flatten_repeat_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let dem = DetectorErrorModel::from_dem_str(&flatten_repeat_fixture())
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![measure_stab_batched(
        "stab_pf4_dem_flatten_repeat",
        TRANSFORM_REPETITIONS,
        || {
            let flattened = dem
                .flattened()
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(dem_model_checksum(&flattened));
            Ok(())
        },
    )?])
}

fn run_dem_rounded_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let dem = DetectorErrorModel::from_dem_str(&rounded_fixture())
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![measure_stab_batched(
        "stab_pf4_dem_rounded",
        TRANSFORM_REPETITIONS,
        || {
            let rounded = dem
                .rounded(3)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(dem_model_checksum(&rounded));
            Ok(())
        },
    )?])
}

fn run_dem_coordinate_map_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let bounded_dem = DetectorErrorModel::from_dem_str(&coordinate_map_fixture())
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let selected_dem = DetectorErrorModel::from_dem_str(COORDINATE_SELECTED_FIXTURE)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let selected_detectors = [
        DemDetectorId::try_new(0).map_err(|error| stab_runner_error(&row.id, error))?,
        DemDetectorId::try_new(1_000_000).map_err(|error| stab_runner_error(&row.id, error))?,
    ];
    let sparse_overlap_dem = DetectorErrorModel::from_dem_str(COORDINATE_SPARSE_OVERLAP_FIXTURE)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let sparse_overlap_detectors =
        [DemDetectorId::try_new(1_500_001).map_err(|error| stab_runner_error(&row.id, error))?];
    let nested_sparse_overlap_dem =
        DetectorErrorModel::from_dem_str(COORDINATE_NESTED_SPARSE_OVERLAP_FIXTURE)
            .map_err(|error| stab_runner_error(&row.id, error))?;
    let nested_sparse_overlap_detectors =
        [DemDetectorId::try_new(1_500_000).map_err(|error| stab_runner_error(&row.id, error))?];
    let flat_overlap_dem = DetectorErrorModel::from_dem_str(&coordinate_flat_overlap_fixture())
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![
        measure_stab_batched(
            "stab_pf4_dem_coordinate_map_all_bounded",
            TRANSFORM_REPETITIONS,
            || {
                let coordinates = bounded_dem
                    .detector_coordinates()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(coordinate_map_checksum(&coordinates));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_coordinate_map_selected_huge_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let coordinates = selected_dem
                    .detector_coordinates_for(selected_detectors)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(coordinate_map_checksum(&coordinates));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_coordinate_map_sparse_overlap",
            TRANSFORM_REPETITIONS,
            || {
                let coordinates = sparse_overlap_dem
                    .detector_coordinates_for(sparse_overlap_detectors)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(coordinate_map_checksum(&coordinates));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_coordinate_map_nested_sparse_overlap",
            TRANSFORM_REPETITIONS,
            || {
                let coordinates = nested_sparse_overlap_dem
                    .detector_coordinates_for(nested_sparse_overlap_detectors)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(coordinate_map_checksum(&coordinates));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_coordinate_map_flat_overlap_all",
            TRANSFORM_REPETITIONS,
            || {
                let coordinates = flat_overlap_dem
                    .detector_coordinates()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(coordinate_map_checksum(&coordinates));
                Ok(())
            },
        )?,
    ])
}

fn run_dem_sampler_repeat_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let fixture = sampler_repeat_fixture();
    let model = DetectorErrorModel::from_dem_str(&fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let sampler =
        CompiledDemSampler::compile(&model).map_err(|error| stab_runner_error(&row.id, error))?;
    let no_op_fixture = sampler_no_op_repeat_fixture();
    let no_op_model = DetectorErrorModel::from_dem_str(&no_op_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let no_op_sampler = CompiledDemSampler::compile(&no_op_model)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![
        measure_stab_batched(
            "stab_pf4_dem_sampler_compile_folded_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let compiled = CompiledDemSampler::compile(&model)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(compiled.error_count());
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sampler_sample_folded_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let output = sampler
                    .sample_detection_events_with_seed(SAMPLER_SHOTS, Some(5))
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(output.records.len());
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sampler_sample_zero_probability_folded_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let output = no_op_sampler
                    .sample_detection_events_with_seed(SAMPLER_SHOTS, Some(5))
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(output.records.len());
                Ok(())
            },
        )?,
    ])
}

fn run_dem_search_sat_repeat_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let hyper_fixture = search_repeat_fixture(SEARCH_REPEAT_COUNT);
    let hyper_model = DetectorErrorModel::from_dem_str(&hyper_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let hyper_zero_fixture = search_zero_probability_repeat_fixture(SEARCH_ZERO_REPEAT_COUNT);
    let hyper_zero_model = DetectorErrorModel::from_dem_str(&hyper_zero_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let sat_fixture = search_repeat_fixture(SAT_REPEAT_COUNT);
    let sat_model = DetectorErrorModel::from_dem_str(&sat_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let analyzer_fixture = analyzer_repeat_fixture(ANALYZER_REPEAT_COUNT);
    let analyzer_circuit = Circuit::from_stim_str(&analyzer_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let matcher_fixture = matcher_repeat_fixture(MATCHER_REPEAT_COUNT);
    let matcher_circuit = Circuit::from_stim_str(&matcher_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![
        measure_stab_batched(
            "stab_pf4_dem_hyper_capped_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let logical_error =
                    find_undetectable_logical_error(&hyper_model, usize::MAX, usize::MAX, false)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&logical_error));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_hyper_zero_probability_repeat_skip",
            TRANSFORM_REPETITIONS,
            || {
                let logical_error = find_undetectable_logical_error(
                    &hyper_zero_model,
                    usize::MAX,
                    usize::MAX,
                    false,
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&logical_error));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sat_capped_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let problem = shortest_error_sat_problem(&sat_model)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(problem.len());
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_analyzer_capped_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let model = circuit_to_detector_error_model(
                    &analyzer_circuit,
                    ErrorAnalyzerOptions::default(),
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&model));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_error_matcher_capped_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let explained = explain_errors_from_circuit(&matcher_circuit, None, false)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(explained.len());
                Ok(())
            },
        )?,
    ])
}

fn run_dem_graphlike_repeat_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let fixture = search_repeat_fixture(SEARCH_REPEAT_COUNT);
    let model = DetectorErrorModel::from_dem_str(&fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let zero_fixture = search_zero_probability_repeat_fixture(SEARCH_ZERO_REPEAT_COUNT);
    let zero_model = DetectorErrorModel::from_dem_str(&zero_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![
        measure_stab_batched(
            "stab_pf4_dem_graphlike_capped_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let logical_error = shortest_graphlike_undetectable_logical_error(&model, false)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&logical_error));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_graphlike_zero_probability_repeat_skip",
            TRANSFORM_REPETITIONS,
            || {
                let logical_error =
                    shortest_graphlike_undetectable_logical_error(&zero_model, false)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&logical_error));
                Ok(())
            },
        )?,
    ])
}

fn flatten_repeat_fixture() -> String {
    format!(
        "\
error[root](0.125) D0 L0
repeat[outer] {FLATTEN_REPETITIONS} {{
    error[body](0.000123456) D0 D1 ^ D2 L1
    detector[det](1, 2) D0
    logical_observable[obs] L2
    shift_detectors[step](0.5, 1, 0.25) 2
}}
detector[end](3, 4) D0
"
    )
}

fn search_repeat_fixture(repeat_count: u64) -> String {
    format!(
        "\
error(0.1) D0
repeat {repeat_count} {{
    error(0.1) D0 D1
    shift_detectors 1
}}
error(0.1) D0 L0
"
    )
}

fn search_zero_probability_repeat_fixture(repeat_count: u64) -> String {
    format!(
        "\
repeat {repeat_count} {{
    error(0) D1000000 L1000
}}
error(0.1) D0
error(0.1) D0 L0
"
    )
}

fn analyzer_repeat_fixture(repeat_count: u64) -> String {
    format!(
        "\
REPEAT {repeat_count} {{
    X_ERROR(0.125) 0
    M 0
    DETECTOR rec[-1]
}}
"
    )
}

fn matcher_repeat_fixture(repeat_count: u64) -> String {
    format!(
        "\
R 0
REPEAT {repeat_count} {{
    TICK
}}
X_ERROR(0.125) 0
M 0
DETECTOR rec[-1]
"
    )
}

fn coordinate_map_fixture() -> String {
    format!(
        "\
repeat {COORDINATE_MAP_DETECTORS} {{
    detector(1, 2, 3) D0
    shift_detectors(0.5, 1.5, 2.5) 1
}}
"
    )
}

const COORDINATE_SELECTED_FIXTURE: &str = "\
repeat 1000001 {
    detector(1, 2) D0
    shift_detectors(3, 4) 1
}
";

const COORDINATE_SPARSE_OVERLAP_FIXTURE: &str = "\
repeat 2000001 {
    detector(10) D2000000
    shift_detectors(1) 1
    detector(20) D0
}
";

const COORDINATE_NESTED_SPARSE_OVERLAP_FIXTURE: &str = "\
repeat 4000000 {
    repeat 1 {
        detector(7) D0
    }
    detector(99) D2000000
    shift_detectors(1) 1
}
";

fn coordinate_flat_overlap_fixture() -> String {
    format!(
        "\
repeat {FLAT_OVERLAP_REPEAT_COUNT} {{
    detector(100) D2
    detector(0) D0
    shift_detectors(1) 1
}}
"
    )
}

fn sampler_repeat_fixture() -> String {
    format!(
        "\
repeat {SAMPLER_REPEAT_COUNT} {{
    error(0.25) D0 L0
    shift_detectors 1
}}
"
    )
}

fn sampler_no_op_repeat_fixture() -> String {
    format!(
        "\
repeat {SAMPLER_NO_OP_REPEAT_COUNT} {{
    error(0) D0
}}
"
    )
}

fn rounded_fixture() -> String {
    let mut text = String::new();
    for index in 0..ROUNDED_ERROR_COUNT {
        let probability = 0.000001 + (index as f64) / 10_000_000.0;
        text.push_str(&format!("error[p{index}]({probability}) D0 D1 L0\n"));
    }
    text.push_str(
        "\
repeat[nested] 128 {
    error[a](0.123456789) D1 D2 L3
    error[b](0.987654321) D3 ^ D4 L5
    detector(0.0200000334, 0.12345) D0
    shift_detectors(5.0300004, 0.12345) 3
}
",
    );
    text
}

fn flatten_expanded_source_instructions() -> u64 {
    FLATTEN_FIXED_INSTRUCTIONS + FLATTEN_REPETITIONS * FLATTEN_SOURCE_INSTRUCTIONS_PER_REPETITION
}

fn flat_overlap_coordinate_detectors() -> u64 {
    FLAT_OVERLAP_REPEAT_COUNT + 2
}

fn rounded_probability_args() -> usize {
    ROUNDED_ERROR_COUNT + ROUNDED_REPEAT_ERROR_COUNT
}

fn search_expanded_errors(repeat_count: u64) -> u64 {
    SEARCH_FIXED_ERRORS + repeat_count
}

fn analyzer_expanded_instructions() -> u64 {
    ANALYZER_REPEAT_COUNT * ANALYZER_INSTRUCTIONS_PER_REPETITION
}

fn matcher_expanded_instructions() -> u64 {
    MATCHER_REPEAT_COUNT * MATCHER_INSTRUCTIONS_PER_REPETITION
}

fn dem_model_checksum(model: &DetectorErrorModel) -> u64 {
    model
        .items()
        .iter()
        .fold(model.items().len() as u64, |checksum, item| {
            checksum.rotate_left(5) ^ dem_item_checksum(item)
        })
}

fn dem_item_checksum(item: &DemItem) -> u64 {
    match item {
        DemItem::Instruction(instruction) => {
            let mut checksum = dem_instruction_kind_checksum(instruction.kind());
            checksum ^= instruction
                .tag()
                .map_or(0, |tag| tag.len() as u64)
                .rotate_left(3);
            for arg in instruction.args() {
                checksum = checksum.rotate_left(7) ^ arg.to_bits();
            }
            for target in instruction.targets() {
                checksum = checksum.rotate_left(11) ^ dem_target_checksum(target);
            }
            checksum
        }
        DemItem::RepeatBlock(repeat) => {
            repeat.repeat_count().get()
                ^ repeat
                    .tag()
                    .map_or(0, |tag| tag.len() as u64)
                    .rotate_left(13)
                ^ dem_model_checksum(repeat.body()).rotate_left(17)
        }
    }
}

fn dem_instruction_kind_checksum(kind: DemInstructionKind) -> u64 {
    match kind {
        DemInstructionKind::Error => 1,
        DemInstructionKind::Detector => 2,
        DemInstructionKind::LogicalObservable => 3,
        DemInstructionKind::ShiftDetectors => 4,
    }
}

fn dem_target_checksum(target: &DemTarget) -> u64 {
    match target {
        DemTarget::RelativeDetector(id) => 0x10 ^ id.get(),
        DemTarget::LogicalObservable(id) => 0x20 ^ id.get(),
        DemTarget::Separator => 0x30,
        DemTarget::Numeric(value) => 0x40 ^ *value,
    }
}

fn coordinate_map_checksum(
    coordinates: &std::collections::BTreeMap<DemDetectorId, Vec<f64>>,
) -> u64 {
    coordinates
        .iter()
        .fold(coordinates.len() as u64, |checksum, (detector, values)| {
            checksum.rotate_left(5) ^ detector.get() ^ float_slice_checksum(values).rotate_left(11)
        })
}

fn float_slice_checksum(values: &[f64]) -> u64 {
    values.iter().fold(values.len() as u64, |checksum, value| {
        checksum.rotate_left(7) ^ value.to_bits()
    })
}
