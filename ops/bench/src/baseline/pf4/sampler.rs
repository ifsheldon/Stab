use std::hint::black_box;

use stab_core::{CompiledDemSampler, DetectionConversionOutput, DetectorErrorModel};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{TRANSFORM_REPETITIONS, measure_stab_batched, stab_runner_error};

#[cfg(not(test))]
const SAMPLER_REPEAT_COUNT: u64 = 4096;
#[cfg(test)]
const SAMPLER_REPEAT_COUNT: u64 = 2;
#[cfg(not(test))]
const SAMPLER_NO_OP_REPEAT_COUNT: u64 = 1_000_000;
#[cfg(test)]
const SAMPLER_NO_OP_REPEAT_COUNT: u64 = 64;
#[cfg(not(test))]
const SAMPLER_DETERMINISTIC_REPEAT_COUNT: u64 = 64_000_001;
#[cfg(test)]
const SAMPLER_DETERMINISTIC_REPEAT_COUNT: u64 = 65;
#[cfg(not(test))]
const SAMPLER_SINGLE_STOCHASTIC_REPEAT_COUNT: u64 = 64_000_001;
#[cfg(test)]
const SAMPLER_SINGLE_STOCHASTIC_REPEAT_COUNT: u64 = 65;
#[cfg(not(test))]
const SAMPLER_FLAT_STOCHASTIC_REPEAT_COUNT: u64 = 64_000_001;
#[cfg(test)]
const SAMPLER_FLAT_STOCHASTIC_REPEAT_COUNT: u64 = 65;
#[cfg(not(test))]
const SAMPLER_NESTED_STOCHASTIC_REPEAT_COUNT: u64 = 64_000_001;
#[cfg(test)]
const SAMPLER_NESTED_STOCHASTIC_REPEAT_COUNT: u64 = 65;
#[cfg(not(test))]
const SAMPLER_SHOTS: usize = 64;
#[cfg(test)]
const SAMPLER_SHOTS: usize = 2;

const SAMPLER_NESTED_STOCHASTIC_ERRORS_PER_REPETITION: u64 = 7;

pub(super) fn run_dem_sampler_repeat_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
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
    let deterministic_fixture = sampler_deterministic_repeat_fixture();
    let deterministic_model = DetectorErrorModel::from_dem_str(&deterministic_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let deterministic_sampler = CompiledDemSampler::compile(&deterministic_model)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let single_stochastic_fixture = sampler_single_stochastic_repeat_fixture();
    let single_stochastic_model = DetectorErrorModel::from_dem_str(&single_stochastic_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let single_stochastic_sampler = CompiledDemSampler::compile(&single_stochastic_model)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let flat_stochastic_fixture = sampler_flat_stochastic_repeat_fixture();
    let flat_stochastic_model = DetectorErrorModel::from_dem_str(&flat_stochastic_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let flat_stochastic_sampler = CompiledDemSampler::compile(&flat_stochastic_model)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let nested_stochastic_fixture = sampler_nested_stochastic_repeat_fixture();
    let nested_stochastic_model = DetectorErrorModel::from_dem_str(&nested_stochastic_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let nested_stochastic_sampler = CompiledDemSampler::compile(&nested_stochastic_model)
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
                black_box(detection_output_checksum(&output));
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
                black_box(detection_output_checksum(&output));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sampler_sample_deterministic_parity_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let output = deterministic_sampler
                    .sample_detection_events_with_seed(SAMPLER_SHOTS, Some(5))
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(detection_output_checksum(&output));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sampler_sample_single_stochastic_parity_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let output = single_stochastic_sampler
                    .sample_detection_events_with_seed(SAMPLER_SHOTS, Some(5))
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(detection_output_checksum(&output));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sampler_sample_flat_stochastic_parity_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let output = flat_stochastic_sampler
                    .sample_detection_events_with_seed(SAMPLER_SHOTS, Some(5))
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(detection_output_checksum(&output));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sampler_sample_nested_stochastic_parity_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let output = nested_stochastic_sampler
                    .sample_detection_events_with_seed(SAMPLER_SHOTS, Some(5))
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(detection_output_checksum(&output));
                Ok(())
            },
        )?,
    ])
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
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
        (
            "pf4-dem-sampler-folded-repeat",
            "stab_pf4_dem_sampler_sample_deterministic_parity_repeat",
        ) => Some((
            (SAMPLER_DETERMINISTIC_REPEAT_COUNT as f64) * (SAMPLER_SHOTS as f64),
            "folded-deterministic-error-occurrences/s",
        )),
        (
            "pf4-dem-sampler-folded-repeat",
            "stab_pf4_dem_sampler_sample_single_stochastic_parity_repeat",
        ) => Some((
            (SAMPLER_SINGLE_STOCHASTIC_REPEAT_COUNT as f64) * (SAMPLER_SHOTS as f64),
            "folded-stochastic-error-occurrences/s",
        )),
        (
            "pf4-dem-sampler-folded-repeat",
            "stab_pf4_dem_sampler_sample_flat_stochastic_parity_repeat",
        ) => Some((
            (SAMPLER_FLAT_STOCHASTIC_REPEAT_COUNT as f64) * 3.0 * (SAMPLER_SHOTS as f64),
            "folded-flat-stochastic-error-occurrences/s",
        )),
        (
            "pf4-dem-sampler-folded-repeat",
            "stab_pf4_dem_sampler_sample_nested_stochastic_parity_repeat",
        ) => Some((
            (SAMPLER_NESTED_STOCHASTIC_REPEAT_COUNT as f64)
                * (SAMPLER_NESTED_STOCHASTIC_ERRORS_PER_REPETITION as f64)
                * (SAMPLER_SHOTS as f64),
            "folded-nested-stochastic-error-occurrences/s",
        )),
        _ => None,
    }
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

fn sampler_deterministic_repeat_fixture() -> String {
    format!(
        "\
repeat {SAMPLER_DETERMINISTIC_REPEAT_COUNT} {{
    error(1) D0 L0
}}
"
    )
}

fn sampler_single_stochastic_repeat_fixture() -> String {
    format!(
        "\
repeat {SAMPLER_SINGLE_STOCHASTIC_REPEAT_COUNT} {{
    error(0.25) D0 L0
}}
"
    )
}

fn sampler_flat_stochastic_repeat_fixture() -> String {
    format!(
        "\
repeat {SAMPLER_FLAT_STOCHASTIC_REPEAT_COUNT} {{
    error(0.25) D0 L0
    error(0.125) D0
    error(1) L1
}}
"
    )
}

fn sampler_nested_stochastic_repeat_fixture() -> String {
    format!(
        "\
repeat {SAMPLER_NESTED_STOCHASTIC_REPEAT_COUNT} {{
    repeat 3 {{
        error(0.25) D0 L0
        error(0.125) D1
    }}
    error(1) L1
}}
"
    )
}

fn detection_output_checksum(output: &DetectionConversionOutput) -> u64 {
    let mut checksum = output.records.len() as u64;
    checksum ^= (output.detector_count as u64).rotate_left(7);
    checksum ^= (output.observable_count as u64).rotate_left(13);
    for record in &output.records {
        for (index, bit) in record.detectors.iter().enumerate() {
            checksum = checksum.rotate_left(3) ^ ((index as u64) << 1) ^ u64::from(*bit);
        }
        for (index, bit) in record.observables.iter().enumerate() {
            checksum = checksum.rotate_left(5) ^ ((index as u64) << 1) ^ u64::from(*bit);
        }
    }
    checksum
}
