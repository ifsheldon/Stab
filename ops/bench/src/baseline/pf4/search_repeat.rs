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
