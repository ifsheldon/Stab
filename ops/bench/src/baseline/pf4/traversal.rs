use std::hint::black_box;

use stab_core::{DemDetectorId, DemRepeatBlock, DemRepeatCount, DetectorErrorModel};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::super::{measure_stab_batched, stab_runner_error};
#[cfg(not(test))]
const FLAT_INSTRUCTION_COUNT: u64 = 4096;
#[cfg(test)]
const FLAT_INSTRUCTION_COUNT: u64 = 8;
#[cfg(not(test))]
const NESTED_OUTER_REPEAT_COUNT: u64 = 1_000_000_000;
#[cfg(test)]
const NESTED_OUTER_REPEAT_COUNT: u64 = 4;
#[cfg(not(test))]
const NESTED_INNER_REPEAT_COUNT: u64 = 1_000_000_000;
#[cfg(test)]
const NESTED_INNER_REPEAT_COUNT: u64 = 3;
#[cfg(not(test))]
const SPARSE_REPEAT_COUNT: u64 = 4_000_000;
#[cfg(test)]
const SPARSE_REPEAT_COUNT: u64 = 8;
#[cfg(not(test))]
const SPARSE_QUERY_DETECTOR: u64 = 1_500_000;
#[cfg(test)]
const SPARSE_QUERY_DETECTOR: u64 = 3;

const NESTED_BODY_INSTRUCTION_COUNT: u64 = 3;
#[cfg(not(test))]
const WIDE_COORDINATE_DIMENSIONS: usize = 4096;
#[cfg(test)]
const WIDE_COORDINATE_DIMENSIONS: usize = 4;
#[cfg(not(test))]
const WIDE_COORDINATE_REPEAT_DEPTH: usize = 128;
#[cfg(test)]
const WIDE_COORDINATE_REPEAT_DEPTH: usize = 2;

#[cfg(not(test))]
const FLAT_BATCH_REPETITIONS: usize = 64;
#[cfg(test)]
const FLAT_BATCH_REPETITIONS: usize = 1;
#[cfg(not(test))]
const NESTED_BATCH_REPETITIONS: usize = 8192;
#[cfg(test)]
const NESTED_BATCH_REPETITIONS: usize = 1;
#[cfg(not(test))]
const SPARSE_BATCH_REPETITIONS: usize = 4096;
#[cfg(test)]
const SPARSE_BATCH_REPETITIONS: usize = 1;
#[cfg(not(test))]
const WIDE_COORDINATE_BATCH_REPETITIONS: usize = 128;
#[cfg(test)]
const WIDE_COORDINATE_BATCH_REPETITIONS: usize = 1;

pub(super) fn run_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let flat = DetectorErrorModel::from_dem_str(&flat_fixture())
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let nested = DetectorErrorModel::from_dem_str(&nested_fixture())
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let sparse = DetectorErrorModel::from_dem_str(&sparse_fixture())
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let wide_coordinate =
        wide_coordinate_fixture().map_err(|error| stab_runner_error(&row.id, error))?;
    let sparse_detector = DemDetectorId::try_new(SPARSE_QUERY_DETECTOR)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![
        measure_stab_batched(
            "stab_pfm_b3_dem_traversal_flat_equivalent",
            FLAT_BATCH_REPETITIONS,
            || {
                black_box(
                    flat.count_detectors()
                        .map_err(|error| stab_runner_error(&row.id, error))?,
                );
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pfm_b3_dem_traversal_nested_large_repeat",
            NESTED_BATCH_REPETITIONS,
            || {
                black_box(
                    nested
                        .count_errors()
                        .map_err(|error| stab_runner_error(&row.id, error))?,
                );
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pfm_b3_dem_traversal_sparse_selected_coordinate",
            SPARSE_BATCH_REPETITIONS,
            || {
                let coordinates = sparse
                    .detector_coordinates_for([sparse_detector])
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(coordinates.len());
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pfm_b3_dem_traversal_wide_coordinate_irrelevant",
            WIDE_COORDINATE_BATCH_REPETITIONS,
            || {
                black_box(
                    wide_coordinate
                        .count_detectors()
                        .map_err(|error| stab_runner_error(&row.id, error))?,
                );
                Ok(())
            },
        )?,
    ])
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("pfm-b3-dem-traversal-core", "stab_pfm_b3_dem_traversal_flat_equivalent") => {
            Some((FLAT_INSTRUCTION_COUNT as f64, "compact-instructions/s"))
        }
        ("pfm-b3-dem-traversal-core", "stab_pfm_b3_dem_traversal_nested_large_repeat") => Some((
            nested_represented_instructions(),
            "represented-instructions/s",
        )),
        ("pfm-b3-dem-traversal-core", "stab_pfm_b3_dem_traversal_sparse_selected_coordinate") => {
            Some((1.0, "selected-detectors/s"))
        }
        ("pfm-b3-dem-traversal-core", "stab_pfm_b3_dem_traversal_wide_coordinate_irrelevant") => {
            Some((
                (WIDE_COORDINATE_REPEAT_DEPTH + 1) as f64,
                "compact-instructions/s",
            ))
        }
        _ => None,
    }
}

fn nested_represented_instructions() -> f64 {
    (NESTED_OUTER_REPEAT_COUNT as f64)
        * (NESTED_INNER_REPEAT_COUNT as f64)
        * (NESTED_BODY_INSTRUCTION_COUNT as f64)
}

fn flat_fixture() -> String {
    (0..FLAT_INSTRUCTION_COUNT)
        .map(|index| format!("error(0.125) D{} L0\n", index % 64))
        .collect()
}

fn nested_fixture() -> String {
    format!(
        "repeat {NESTED_OUTER_REPEAT_COUNT} {{\n    repeat {NESTED_INNER_REPEAT_COUNT} {{\n        error(0.125) D0 L0\n        detector(1, 2) D0\n        shift_detectors 1\n    }}\n}}\n"
    )
}

fn sparse_fixture() -> String {
    let sparse_declaration = if cfg!(test) { 4 } else { 2_000_000 };
    format!(
        "repeat {SPARSE_REPEAT_COUNT} {{\n    repeat 1 {{\n        detector(7) D0\n    }}\n    detector(99) D{sparse_declaration}\n    shift_detectors(1) 1\n}}\n"
    )
}

fn wide_coordinate_fixture() -> Result<DetectorErrorModel, stab_core::CircuitError> {
    let coordinates = std::iter::repeat_n("1", WIDE_COORDINATE_DIMENSIONS)
        .collect::<Vec<_>>()
        .join(",");
    let mut model =
        DetectorErrorModel::from_dem_str(&format!("shift_detectors({coordinates}) 0\n"))?;
    for _ in 0..WIDE_COORDINATE_REPEAT_DEPTH {
        let mut outer = DetectorErrorModel::new();
        outer.push_repeat_block(DemRepeatBlock::new(DemRepeatCount::new(1), model, None));
        model = outer;
    }
    Ok(model)
}
