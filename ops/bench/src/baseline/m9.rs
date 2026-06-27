use std::hint::black_box;

use stab_core::{
    Circuit, CodeDistance, CompiledSampler, DetectionConversionOptions,
    DetectionObservableOutputMode, Probability, RepetitionCodeParams, RepetitionCodeTask,
    RoundCount, SampleFormat, convert_measurements_to_detection_events,
    generate_repetition_code_circuit, measurement_record_count,
    result_formats::{read_records, write_records},
    sample_detection_events, write_detection_records,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{measure_stab_iterations, stab_runner_error};

const DETECT_BASIC_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/detect_basic.stim");
const M2D_BASIC_CIRCUIT: &str = include_str!("../../../../oracle/fixtures/inputs/m2d_basic.stim");
const M2D_BASIC_MEASUREMENTS: &[u8] =
    include_bytes!("../../../../oracle/fixtures/inputs/m2d_basic_measurements.01");
const PRIMARY_DISTANCE: u32 = 3;
const PRIMARY_ROUNDS: u64 = 3;
#[cfg(not(test))]
const DETECT_SHOTS: usize = 1024;
#[cfg(test)]
const DETECT_SHOTS: usize = 4;
#[cfg(not(test))]
const PRIMARY_SHOTS: usize = 64;
#[cfg(test)]
const PRIMARY_SHOTS: usize = 2;

pub(super) fn run_detection_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "m9-convert-measurements-dets" => {
            run_m2d_fixture_row(row, "stab_convert_measurements_to_dets", SampleFormat::Dets)
                .map(Some)
        }
        "m9-detect-text-cli" => {
            run_detect_fixture_row(row, "stab_detect_1024_dets", SampleFormat::Dets).map(Some)
        }
        "m9-detect-bitpacked-cli" => {
            run_detect_fixture_row(row, "stab_detect_1024_b8", SampleFormat::B8).map(Some)
        }
        "m9-m2d-text-cli" => {
            run_m2d_fixture_row(row, "stab_m2d_dets", SampleFormat::Dets).map(Some)
        }
        "m9-m2d-bitpacked-contract" => run_m2d_bitpacked_row(row).map(Some),
        "m9-detect-primary-matrix-contract" => run_primary_detect_row(row).map(Some),
        "m9-m2d-primary-matrix-contract" => run_primary_m2d_row(row).map(Some),
        _ => Ok(None),
    }
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m9-convert-measurements-dets", "stab_convert_measurements_to_dets")
        | ("m9-m2d-text-cli", "stab_m2d_dets")
        | ("m9-m2d-bitpacked-contract", "stab_m2d_b8") => Some((2.0, "shots/s")),
        ("m9-detect-text-cli", "stab_detect_1024_dets")
        | ("m9-detect-bitpacked-cli", "stab_detect_1024_b8") => {
            Some((DETECT_SHOTS as f64, "shots/s"))
        }
        ("m9-detect-primary-matrix-contract", "stab_detect_primary_repetition_d3_r3_dets")
        | ("m9-detect-primary-matrix-contract", "stab_detect_primary_repetition_d3_r3_b8")
        | ("m9-m2d-primary-matrix-contract", "stab_m2d_primary_repetition_d3_r3_dets")
        | ("m9-m2d-primary-matrix-contract", "stab_m2d_primary_repetition_d3_r3_b8") => {
            Some((PRIMARY_SHOTS as f64, "shots/s"))
        }
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m9-convert-measurements-dets" => Some(
            "contract-proxy: Stab measures the M9 measurement-to-detection conversion path because circuit-aware convert flags remain folded into m2d",
        ),
        "m9-detect-text-cli" | "m9-detect-bitpacked-cli" => Some(
            "report-only: Stab measures in-process detector sampling plus result writing for the public detect contract",
        ),
        "m9-m2d-text-cli" | "m9-m2d-bitpacked-contract" => Some(
            "report-only: Stab measures in-process measurement-to-detection conversion plus result writing",
        ),
        "m9-detect-primary-matrix-contract" | "m9-m2d-primary-matrix-contract" => Some(
            "contract-representative: Stab uses a generated repetition-code d3/r3 circuit for the M9 primary detection matrix",
        ),
        _ => None,
    }
}

fn run_detect_fixture_row(
    row: &BenchmarkRow,
    measurement_name: &'static str,
    format: SampleFormat,
) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, DETECT_BASIC_FIXTURE)?;
    Ok(vec![measure_stab_iterations(
        measurement_name,
        super::STAB_COMPARE_ITERATIONS,
        || {
            let output = sample_detection_events(&circuit, DETECT_SHOTS, Some(5))
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let bytes = write_detection_records(&output, detect_observable_mode(format), format)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(bytes.len());
            Ok(())
        },
    )?])
}

fn run_m2d_fixture_row(
    row: &BenchmarkRow,
    measurement_name: &'static str,
    format: SampleFormat,
) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, M2D_BASIC_CIRCUIT)?;
    let measurements = m2d_measurements(&row.id, &circuit, SampleFormat::ZeroOne)?;
    Ok(vec![measure_stab_iterations(
        measurement_name,
        super::STAB_COMPARE_ITERATIONS,
        || {
            let output = convert_measurements_to_detection_events(
                &circuit,
                &measurements,
                DetectionConversionOptions {
                    skip_reference_sample: false,
                },
            )
            .map_err(|error| stab_runner_error(&row.id, error))?;
            let bytes = write_detection_records(
                &output,
                DetectionObservableOutputMode::DetectorsOnly,
                format,
            )
            .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(bytes.len());
            Ok(())
        },
    )?])
}

fn run_m2d_bitpacked_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, M2D_BASIC_CIRCUIT)?;
    let measurements = m2d_measurements(&row.id, &circuit, SampleFormat::B8)?;
    Ok(vec![measure_stab_iterations(
        "stab_m2d_b8",
        super::STAB_COMPARE_ITERATIONS,
        || {
            let output = convert_measurements_to_detection_events(
                &circuit,
                &measurements,
                DetectionConversionOptions {
                    skip_reference_sample: false,
                },
            )
            .map_err(|error| stab_runner_error(&row.id, error))?;
            let bytes = write_detection_records(
                &output,
                DetectionObservableOutputMode::DetectorsOnly,
                SampleFormat::B8,
            )
            .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(bytes.len());
            Ok(())
        },
    )?])
}

fn run_primary_detect_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = primary_repetition_circuit(&row.id)?;
    Ok(vec![
        measure_primary_detect(
            row,
            &circuit,
            "stab_detect_primary_repetition_d3_r3_dets",
            SampleFormat::Dets,
        )?,
        measure_primary_detect(
            row,
            &circuit,
            "stab_detect_primary_repetition_d3_r3_b8",
            SampleFormat::B8,
        )?,
    ])
}

fn run_primary_m2d_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = primary_repetition_circuit(&row.id)?;
    let sampler =
        CompiledSampler::compile(&circuit).map_err(|error| stab_runner_error(&row.id, error))?;
    let measurements = sampler.sample_zero_one_with_seed(PRIMARY_SHOTS, Some(5));
    Ok(vec![
        measure_primary_m2d(
            row,
            &circuit,
            &measurements,
            "stab_m2d_primary_repetition_d3_r3_dets",
            SampleFormat::Dets,
        )?,
        measure_primary_m2d(
            row,
            &circuit,
            &measurements,
            "stab_m2d_primary_repetition_d3_r3_b8",
            SampleFormat::B8,
        )?,
    ])
}

fn measure_primary_detect(
    row: &BenchmarkRow,
    circuit: &Circuit,
    measurement_name: &'static str,
    format: SampleFormat,
) -> Result<Measurement, BenchError> {
    measure_stab_iterations(measurement_name, super::STAB_COMPARE_ITERATIONS, || {
        let output = sample_detection_events(circuit, PRIMARY_SHOTS, Some(5))
            .map_err(|error| stab_runner_error(&row.id, error))?;
        let bytes = write_detection_records(&output, detect_observable_mode(format), format)
            .map_err(|error| stab_runner_error(&row.id, error))?;
        black_box(bytes.len());
        Ok(())
    })
}

fn measure_primary_m2d(
    row: &BenchmarkRow,
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    measurement_name: &'static str,
    format: SampleFormat,
) -> Result<Measurement, BenchError> {
    measure_stab_iterations(measurement_name, super::STAB_COMPARE_ITERATIONS, || {
        let output = convert_measurements_to_detection_events(
            circuit,
            measurements,
            DetectionConversionOptions {
                skip_reference_sample: false,
            },
        )
        .map_err(|error| stab_runner_error(&row.id, error))?;
        let bytes = write_detection_records(
            &output,
            DetectionObservableOutputMode::DetectorsOnly,
            format,
        )
        .map_err(|error| stab_runner_error(&row.id, error))?;
        black_box(bytes.len());
        Ok(())
    })
}

fn m2d_measurements(
    row_id: &str,
    circuit: &Circuit,
    format: SampleFormat,
) -> Result<Vec<Vec<bool>>, BenchError> {
    let width =
        measurement_record_count(circuit).map_err(|error| stab_runner_error(row_id, error))?;
    if format == SampleFormat::ZeroOne {
        return read_records(M2D_BASIC_MEASUREMENTS, SampleFormat::ZeroOne, width)
            .map_err(|error| stab_runner_error(row_id, error));
    }
    let zero_one_records = read_records(M2D_BASIC_MEASUREMENTS, SampleFormat::ZeroOne, width)
        .map_err(|error| stab_runner_error(row_id, error))?;
    let encoded = write_records(&zero_one_records, format);
    read_records(&encoded, format, width).map_err(|error| stab_runner_error(row_id, error))
}

fn detect_observable_mode(format: SampleFormat) -> DetectionObservableOutputMode {
    if format == SampleFormat::Dets {
        DetectionObservableOutputMode::Prepend
    } else {
        DetectionObservableOutputMode::DetectorsOnly
    }
}

fn primary_repetition_circuit(row_id: &str) -> Result<Circuit, BenchError> {
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(PRIMARY_ROUNDS).map_err(|error| stab_runner_error(row_id, error))?,
        CodeDistance::try_new(PRIMARY_DISTANCE)
            .map_err(|error| stab_runner_error(row_id, error))?,
        RepetitionCodeTask::Memory,
    )
    .map_err(|error| stab_runner_error(row_id, error))?
    .with_before_measure_flip_probability(
        Probability::try_new(0.001).map_err(|error| stab_runner_error(row_id, error))?,
    );
    let generated = generate_repetition_code_circuit(&params)
        .map_err(|error| stab_runner_error(row_id, error))?;
    Ok(generated.circuit().clone())
}

fn parse_circuit(row_id: &str, text: &str) -> Result<Circuit, BenchError> {
    Circuit::from_stim_str(text).map_err(|error| stab_runner_error(row_id, error))
}
