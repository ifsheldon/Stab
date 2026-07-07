use std::hint::black_box;

use stab_core::{
    DetectorErrorModel, find_undetectable_logical_error,
    shortest_graphlike_undetectable_logical_error,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::super::{measure_stab_batched, stab_runner_error};
use super::{
    SEARCH_FLAT_REPEAT_COUNT, TRANSFORM_REPETITIONS, dem_model_checksum,
    search_no_target_flat_repeat_fixture,
};

pub(super) fn run_dem_hypergraph_no_target_repeat_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let fixture = search_no_target_flat_repeat_fixture(SEARCH_FLAT_REPEAT_COUNT);
    let model = DetectorErrorModel::from_dem_str(&fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![measure_stab_batched(
        "stab_pf4_dem_hyper_no_target_repeat_skip",
        TRANSFORM_REPETITIONS,
        || {
            let logical_error =
                find_undetectable_logical_error(&model, usize::MAX, usize::MAX, false)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(dem_model_checksum(&logical_error));
            Ok(())
        },
    )?])
}

pub(super) fn run_dem_search_zero_shift_repeat_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let fixture = search_zero_shift_repeat_fixture(SEARCH_FLAT_REPEAT_COUNT);
    let model = DetectorErrorModel::from_dem_str(&fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![
        measure_stab_batched(
            "stab_pf4_dem_graphlike_zero_shift_repeat_fold",
            TRANSFORM_REPETITIONS,
            || {
                let logical_error = shortest_graphlike_undetectable_logical_error(&model, false)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&logical_error));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_hyper_zero_shift_repeat_fold",
            TRANSFORM_REPETITIONS,
            || {
                let logical_error =
                    find_undetectable_logical_error(&model, usize::MAX, usize::MAX, false)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&logical_error));
                Ok(())
            },
        )?,
    ])
}

pub(super) fn run_dem_search_annotation_repeat_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let fixture = search_annotation_repeat_fixture(SEARCH_FLAT_REPEAT_COUNT);
    let model = DetectorErrorModel::from_dem_str(&fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![
        measure_stab_batched(
            "stab_pf4_dem_graphlike_annotation_repeat_fold",
            TRANSFORM_REPETITIONS,
            || {
                let logical_error = shortest_graphlike_undetectable_logical_error(&model, false)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&logical_error));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_hyper_annotation_repeat_fold",
            TRANSFORM_REPETITIONS,
            || {
                let logical_error =
                    find_undetectable_logical_error(&model, usize::MAX, usize::MAX, false)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&logical_error));
                Ok(())
            },
        )?,
    ])
}

pub(super) fn run_dem_search_mixed_zero_probability_repeat_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let fixture = search_mixed_zero_probability_repeat_fixture(SEARCH_FLAT_REPEAT_COUNT);
    let model = DetectorErrorModel::from_dem_str(&fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![
        measure_stab_batched(
            "stab_pf4_dem_graphlike_mixed_zero_probability_repeat_fold",
            TRANSFORM_REPETITIONS,
            || {
                let logical_error = shortest_graphlike_undetectable_logical_error(&model, false)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&logical_error));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_hyper_mixed_zero_probability_repeat_fold",
            TRANSFORM_REPETITIONS,
            || {
                let logical_error =
                    find_undetectable_logical_error(&model, usize::MAX, usize::MAX, false)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&logical_error));
                Ok(())
            },
        )?,
    ])
}

pub(super) fn run_dem_search_nested_repeat_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let fixture = search_nested_repeat_fixture(SEARCH_FLAT_REPEAT_COUNT);
    let model = DetectorErrorModel::from_dem_str(&fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![
        measure_stab_batched(
            "stab_pf4_dem_graphlike_nested_repeat_fold",
            TRANSFORM_REPETITIONS,
            || {
                let logical_error = shortest_graphlike_undetectable_logical_error(&model, false)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&logical_error));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_hyper_nested_repeat_fold",
            TRANSFORM_REPETITIONS,
            || {
                let logical_error =
                    find_undetectable_logical_error(&model, usize::MAX, usize::MAX, false)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem_model_checksum(&logical_error));
                Ok(())
            },
        )?,
    ])
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("pf4-dem-hypergraph-no-target-repeat", "stab_pf4_dem_hyper_no_target_repeat_skip") => {
            Some((
                SEARCH_FLAT_REPEAT_COUNT as f64,
                "skipped-no-target-errors/s",
            ))
        }
        ("pf4-dem-search-zero-shift-repeat", "stab_pf4_dem_graphlike_zero_shift_repeat_fold")
        | ("pf4-dem-search-zero-shift-repeat", "stab_pf4_dem_hyper_zero_shift_repeat_fold") => {
            Some((
                SEARCH_FLAT_REPEAT_COUNT as f64,
                "folded-zero-shift-target-errors/s",
            ))
        }
        ("pf4-dem-search-annotation-repeat", "stab_pf4_dem_graphlike_annotation_repeat_fold")
        | ("pf4-dem-search-annotation-repeat", "stab_pf4_dem_hyper_annotation_repeat_fold") => {
            Some((
                (SEARCH_FLAT_REPEAT_COUNT as f64) * 2.0,
                "folded-annotated-target-errors/s",
            ))
        }
        (
            "pf4-dem-search-mixed-zero-probability-repeat",
            "stab_pf4_dem_graphlike_mixed_zero_probability_repeat_fold",
        )
        | (
            "pf4-dem-search-mixed-zero-probability-repeat",
            "stab_pf4_dem_hyper_mixed_zero_probability_repeat_fold",
        ) => Some((
            (SEARCH_FLAT_REPEAT_COUNT as f64) * 2.0,
            "folded-active-target-errors/s",
        )),
        ("pf4-dem-search-nested-repeat", "stab_pf4_dem_graphlike_nested_repeat_fold")
        | ("pf4-dem-search-nested-repeat", "stab_pf4_dem_hyper_nested_repeat_fold") => Some((
            (SEARCH_FLAT_REPEAT_COUNT as f64) * (SEARCH_FLAT_REPEAT_COUNT as f64) * 2.0,
            "folded-nested-target-errors/s",
        )),
        _ => None,
    }
}

fn search_zero_shift_repeat_fixture(repeat_count: u64) -> String {
    format!(
        "\
repeat {repeat_count} {{
    error(0.1)
    shift_detectors 0
    error(0.2) L0
}}
"
    )
}

fn search_nested_repeat_fixture(repeat_count: u64) -> String {
    format!(
        "\
repeat {repeat_count} {{
    detector(1, 2) D0
    repeat {repeat_count} {{
        error(0.1) D0
        shift_detectors 0
        error(0.2) D0 L0
    }}
}}
"
    )
}

fn search_annotation_repeat_fixture(repeat_count: u64) -> String {
    format!(
        "\
repeat {repeat_count} {{
    detector(1, 2) D0
    logical_observable L2
    error(0.1) D0
    error(0.2) D0 L0
}}
"
    )
}

fn search_mixed_zero_probability_repeat_fixture(repeat_count: u64) -> String {
    format!(
        "\
repeat {repeat_count} {{
    error(0) D1000000 L1000
    error(0.1) D0
    shift_detectors 0
    error(0.1) D0 L0
}}
"
    )
}
