use std::hint::black_box;

use stab_core::{Circuit, DetectorErrorModel, explain_errors_from_circuit};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::super::{measure_stab_batched, stab_runner_error};
use super::TRANSFORM_REPETITIONS;

#[cfg(not(test))]
const MATCHER_FILTER_FLAT_REPEAT_COUNT: u64 = 1_000_000;
#[cfg(test)]
const MATCHER_FILTER_FLAT_REPEAT_COUNT: u64 = 100_001;
#[cfg(not(test))]
const MATCHER_FILTER_NESTED_REPEAT_COUNT: u64 = 1_000_000;
#[cfg(test)]
const MATCHER_FILTER_NESTED_REPEAT_COUNT: u64 = 100_001;
const MATCHER_FILTER_NESTED_LEFT_COUNT: u64 = 17;
const MATCHER_FILTER_NESTED_RIGHT_COUNT: u64 = 19;
#[cfg(not(test))]
const MATCHER_FILTER_LOGICAL_REPEAT_COUNT: u64 = 1_000_000;
#[cfg(test)]
const MATCHER_FILTER_LOGICAL_REPEAT_COUNT: u64 = 100_001;
const MATCHER_FILTER_LOGICAL_INNER_COUNT: u64 = 17;
#[cfg(not(test))]
const MATCHER_FILTER_ANNOTATION_REPEAT_COUNT: u64 = 1_000_000;
#[cfg(test)]
const MATCHER_FILTER_ANNOTATION_REPEAT_COUNT: u64 = 100_001;
const MATCHER_FILTER_ANNOTATION_INNER_COUNT: u64 = 17;

pub(super) fn run_row(row: &BenchmarkRow) -> Result<Option<Vec<Measurement>>, BenchError> {
    let measurements = match row.id.as_str() {
        "pf4-error-matcher-filter-flat-repeat" => run_flat_repeat_row(row)?,
        "pf4-error-matcher-filter-nested-repeat" => run_nested_repeat_row(row)?,
        "pf4-error-matcher-filter-logical-repeat" => run_logical_repeat_row(row)?,
        "pf4-error-matcher-filter-annotation-repeat" => run_annotation_repeat_row(row)?,
        _ => return Ok(None),
    };
    Ok(Some(measurements))
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        (
            "pf4-error-matcher-filter-flat-repeat",
            "stab_pf4_error_matcher_filter_flat_repeat_fold",
        ) => Some((
            MATCHER_FILTER_FLAT_REPEAT_COUNT as f64,
            "folded-filter-keys/s",
        )),
        (
            "pf4-error-matcher-filter-nested-repeat",
            "stab_pf4_error_matcher_filter_nested_repeat_fold",
        ) => Some((nested_expanded_keys(), "folded-nested-filter-keys/s")),
        (
            "pf4-error-matcher-filter-logical-repeat",
            "stab_pf4_error_matcher_filter_logical_repeat_fold",
        ) => Some((logical_expanded_keys(), "folded-logical-filter-keys/s")),
        (
            "pf4-error-matcher-filter-annotation-repeat",
            "stab_pf4_error_matcher_filter_annotation_repeat_fold",
        ) => Some((annotation_expanded_keys(), "folded-annotated-filter-keys/s")),
        _ => None,
    }
}

fn run_flat_repeat_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let matcher_filter_circuit = Circuit::from_stim_str(MATCHER_FILTER_CIRCUIT)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let matcher_filter_fixture = flat_repeat_fixture(MATCHER_FILTER_FLAT_REPEAT_COUNT);
    let matcher_filter = DetectorErrorModel::from_dem_str(&matcher_filter_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![measure_stab_batched(
        "stab_pf4_error_matcher_filter_flat_repeat_fold",
        TRANSFORM_REPETITIONS,
        || {
            let explained =
                explain_errors_from_circuit(&matcher_filter_circuit, Some(&matcher_filter), false)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(explained.len());
            Ok(())
        },
    )?])
}

fn run_nested_repeat_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let matcher_filter_circuit = Circuit::from_stim_str(MATCHER_FILTER_RICH_CIRCUIT)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let matcher_filter_fixture = nested_repeat_fixture(MATCHER_FILTER_NESTED_REPEAT_COUNT);
    let matcher_filter = DetectorErrorModel::from_dem_str(&matcher_filter_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![measure_stab_batched(
        "stab_pf4_error_matcher_filter_nested_repeat_fold",
        TRANSFORM_REPETITIONS,
        || {
            let explained =
                explain_errors_from_circuit(&matcher_filter_circuit, Some(&matcher_filter), false)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(explained.len());
            Ok(())
        },
    )?])
}

fn run_logical_repeat_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let matcher_filter_circuit = Circuit::from_stim_str(MATCHER_FILTER_LOGICAL_CIRCUIT)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let matcher_filter_fixture = logical_repeat_fixture(MATCHER_FILTER_LOGICAL_REPEAT_COUNT);
    let matcher_filter = DetectorErrorModel::from_dem_str(&matcher_filter_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![measure_stab_batched(
        "stab_pf4_error_matcher_filter_logical_repeat_fold",
        TRANSFORM_REPETITIONS,
        || {
            let explained =
                explain_errors_from_circuit(&matcher_filter_circuit, Some(&matcher_filter), false)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(explained.len());
            Ok(())
        },
    )?])
}

fn run_annotation_repeat_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let matcher_filter_circuit = Circuit::from_stim_str(MATCHER_FILTER_RICH_CIRCUIT)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let matcher_filter_fixture = annotation_repeat_fixture(MATCHER_FILTER_ANNOTATION_REPEAT_COUNT);
    let matcher_filter = DetectorErrorModel::from_dem_str(&matcher_filter_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![measure_stab_batched(
        "stab_pf4_error_matcher_filter_annotation_repeat_fold",
        TRANSFORM_REPETITIONS,
        || {
            let explained =
                explain_errors_from_circuit(&matcher_filter_circuit, Some(&matcher_filter), false)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(explained.len());
            Ok(())
        },
    )?])
}

const MATCHER_FILTER_CIRCUIT: &str = "\
M(0.125) 0
DETECTOR rec[-1]
";

const MATCHER_FILTER_RICH_CIRCUIT: &str = "\
MPAD 0
DETECTOR rec[-1]
M(0.125) 0
M(0.25) 1
DETECTOR rec[-2]
DETECTOR rec[-1]
OBSERVABLE_INCLUDE(0) rec[-1]
";

const MATCHER_FILTER_LOGICAL_CIRCUIT: &str = "\
M(0.125) 0
OBSERVABLE_INCLUDE(0) rec[-1]
M(0.25) 1
OBSERVABLE_INCLUDE(1) rec[-1]
";

fn flat_repeat_fixture(repeat_count: u64) -> String {
    format!(
        "\
repeat {repeat_count} {{
    error(0.1) D0
}}
"
    )
}

fn nested_repeat_fixture(repeat_count: u64) -> String {
    format!(
        "\
shift_detectors 1
repeat {repeat_count} {{
    shift_detectors(4, 5) 0
    repeat {MATCHER_FILTER_NESTED_LEFT_COUNT} {{
        error(0.1) D0
    }}
    repeat {MATCHER_FILTER_NESTED_RIGHT_COUNT} {{
        error(0.1) D0 D0 D1 ^ L0
        shift_detectors 0
    }}
}}
"
    )
}

fn logical_repeat_fixture(repeat_count: u64) -> String {
    format!(
        "\
repeat {repeat_count} {{
    error(0.1) L0
    repeat {MATCHER_FILTER_LOGICAL_INNER_COUNT} {{
        shift_detectors 0
        error(0.1) L1
    }}
}}
"
    )
}

fn annotation_repeat_fixture(repeat_count: u64) -> String {
    format!(
        "\
shift_detectors 1
repeat {repeat_count} {{
    detector(2, 3) D0
    logical_observable L0
    error(0.1) D0
    repeat {MATCHER_FILTER_ANNOTATION_INNER_COUNT} {{
        detector(7) D1
        logical_observable L0
        shift_detectors 0
        error(0.1) D0 D0 D1 ^ L0
    }}
}}
"
    )
}

fn nested_expanded_keys() -> f64 {
    (MATCHER_FILTER_NESTED_REPEAT_COUNT
        * (MATCHER_FILTER_NESTED_LEFT_COUNT + MATCHER_FILTER_NESTED_RIGHT_COUNT)) as f64
}

fn logical_expanded_keys() -> f64 {
    (MATCHER_FILTER_LOGICAL_REPEAT_COUNT * (1 + MATCHER_FILTER_LOGICAL_INNER_COUNT)) as f64
}

fn annotation_expanded_keys() -> f64 {
    (MATCHER_FILTER_ANNOTATION_REPEAT_COUNT * (1 + MATCHER_FILTER_ANNOTATION_INNER_COUNT)) as f64
}
