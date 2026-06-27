use std::hint::black_box;

use stab_core::{
    CompiledDemSampler, DetectionObservableOutputMode, DetectorErrorModel, SampleFormat,
    write_detection_records,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{measure_stab_iterations, stab_runner_error};

const SAMPLE_DEM_NOISY_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/sample_dem_noisy.dem");
#[cfg(not(test))]
const M11_SAMPLE_DEM_SHOTS: usize = 1024;
#[cfg(test)]
const M11_SAMPLE_DEM_SHOTS: usize = 4;
#[cfg(not(test))]
const M11_CONTRACT_SHOTS: usize = 64;
#[cfg(test)]
const M11_CONTRACT_SHOTS: usize = 2;
#[cfg(not(test))]
const M11_CONTRACT_ITERATIONS: usize = 8;
#[cfg(test)]
const M11_CONTRACT_ITERATIONS: usize = 1;
#[cfg(not(test))]
const DENSE_DETECTOR_COUNT: usize = 128;
#[cfg(test)]
const DENSE_DETECTOR_COUNT: usize = 16;
#[cfg(not(test))]
const REPEATED_DEM_REPS: usize = 128;
#[cfg(test)]
const REPEATED_DEM_REPS: usize = 4;
#[cfg(not(test))]
const HIGH_DETECTOR_COUNT: usize = 4096;
#[cfg(test)]
const HIGH_DETECTOR_COUNT: usize = 128;

pub(super) fn run_dem_sampling_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "m11-dem-sampler" => run_compiled_dem_sampler_row(row).map(Some),
        "m11-sample-dem-cli" => run_sample_dem_cli_row(row).map(Some),
        "m11-sample-dem-sparse-contract" => run_contract_row(
            row,
            "stab_sample_dem_sparse_b8",
            sparse_dem_fixture(),
            SampleFormat::B8,
        )
        .map(Some),
        "m11-sample-dem-dense-contract" => run_contract_row(
            row,
            "stab_sample_dem_dense_b8",
            dense_dem_fixture(),
            SampleFormat::B8,
        )
        .map(Some),
        "m11-sample-dem-repeated-contract" => run_contract_row(
            row,
            "stab_sample_dem_repeated_b8",
            repeated_dem_fixture(),
            SampleFormat::B8,
        )
        .map(Some),
        "m11-sample-dem-high-detector-contract" => run_contract_row(
            row,
            "stab_sample_dem_high_detector_b8",
            high_detector_dem_fixture(),
            SampleFormat::B8,
        )
        .map(Some),
        _ => Ok(None),
    }
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m11-dem-sampler", "stab_dem_sampler_sample_surface_like_1024") => {
            Some((M11_SAMPLE_DEM_SHOTS as f64, "shots/s"))
        }
        ("m11-sample-dem-cli", "stab_sample_dem_cli_1024_zero_one") => {
            Some((M11_SAMPLE_DEM_SHOTS as f64, "shots/s"))
        }
        ("m11-sample-dem-sparse-contract", "stab_sample_dem_sparse_b8")
        | ("m11-sample-dem-dense-contract", "stab_sample_dem_dense_b8")
        | ("m11-sample-dem-repeated-contract", "stab_sample_dem_repeated_b8") => {
            Some((M11_CONTRACT_SHOTS as f64, "shots/s"))
        }
        ("m11-sample-dem-high-detector-contract", "stab_sample_dem_high_detector_b8") => Some((
            (M11_CONTRACT_SHOTS * HIGH_DETECTOR_COUNT) as f64,
            "detector-bits/s",
        )),
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m11-dem-sampler" => Some(
            "contract-representative: Stab measures a precompiled surface-like DEM sampler; upstream Stim perf uses a generated d11/r100 surface-code DEM with 1024 stripes",
        ),
        "m11-sample-dem-cli" => Some(
            "report-only: Stab measures in-process sample_dem parse, compile, sample, and 01 output writing; pinned Stim baseline includes CLI process costs",
        ),
        "m11-sample-dem-sparse-contract" => Some(
            "contract-representative: Stab measures sparse detector ids with observable output routed through the DEM sampler writer",
        ),
        "m11-sample-dem-dense-contract" => Some(
            "contract-representative: Stab measures dense detector targets and bit-packed output",
        ),
        "m11-sample-dem-repeated-contract" => Some(
            "contract-representative: Stab measures repeat and detector-shift DEM sampling after the current bounded unroll compilation",
        ),
        "m11-sample-dem-high-detector-contract" => Some(
            "contract-representative: Stab measures high detector index output width with bit-packed output",
        ),
        _ => None,
    }
}

fn run_compiled_dem_sampler_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let model = parse_dem(&row.id, &surface_like_dem_fixture())?;
    let sampler =
        CompiledDemSampler::compile(&model).map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![measure_stab_iterations(
        "stab_dem_sampler_sample_surface_like_1024",
        M11_CONTRACT_ITERATIONS,
        || {
            let output = sampler
                .sample_detection_events_with_seed(M11_SAMPLE_DEM_SHOTS, Some(5))
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(output.records.len());
            Ok(())
        },
    )?])
}

fn run_sample_dem_cli_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![measure_stab_iterations(
        "stab_sample_dem_cli_1024_zero_one",
        M11_CONTRACT_ITERATIONS,
        || {
            let model = parse_dem(&row.id, SAMPLE_DEM_NOISY_FIXTURE)?;
            let sampler = CompiledDemSampler::compile(&model)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let output = sampler
                .sample_detection_events_with_seed(M11_SAMPLE_DEM_SHOTS, Some(5))
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let bytes = write_detection_records(
                &output,
                DetectionObservableOutputMode::DetectorsOnly,
                SampleFormat::ZeroOne,
            )
            .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(bytes.len());
            Ok(())
        },
    )?])
}

fn run_contract_row(
    row: &BenchmarkRow,
    measurement_name: &'static str,
    fixture: String,
    format: SampleFormat,
) -> Result<Vec<Measurement>, BenchError> {
    let model = parse_dem(&row.id, &fixture)?;
    let sampler =
        CompiledDemSampler::compile(&model).map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![measure_stab_iterations(
        measurement_name,
        M11_CONTRACT_ITERATIONS,
        || {
            let output = sampler
                .sample_detection_events_with_seed(M11_CONTRACT_SHOTS, Some(5))
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let bytes =
                write_detection_records(&output, DetectionObservableOutputMode::Append, format)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(bytes.len());
            Ok(())
        },
    )?])
}

fn parse_dem(row_id: &str, fixture: &str) -> Result<DetectorErrorModel, BenchError> {
    DetectorErrorModel::from_dem_str(fixture).map_err(|error| stab_runner_error(row_id, error))
}

fn sparse_dem_fixture() -> String {
    format!(
        "detector D{}\nlogical_observable L1\nerror(0.01) D0\nerror(0.02) D{} L1\n",
        HIGH_DETECTOR_COUNT / 2,
        HIGH_DETECTOR_COUNT / 2
    )
}

fn dense_dem_fixture() -> String {
    let mut text = String::new();
    text.push_str("error(0.001)");
    for detector in 0..DENSE_DETECTOR_COUNT {
        text.push_str(" D");
        text.push_str(&detector.to_string());
    }
    text.push_str(" L0\n");
    text
}

fn repeated_dem_fixture() -> String {
    format!(
        "repeat {REPEATED_DEM_REPS} {{\n    error(0.001) D0\n    shift_detectors 1\n}}\nerror(0.25) D0 L0\n"
    )
}

fn high_detector_dem_fixture() -> String {
    let high_detector_id = HIGH_DETECTOR_COUNT - 1;
    format!("detector D{high_detector_id}\nerror(0.001) D0\nerror(0.001) D{high_detector_id} L0\n")
}

fn surface_like_dem_fixture() -> String {
    let mut text = String::new();
    for detector in 0..DENSE_DETECTOR_COUNT {
        text.push_str("error(0.001) D");
        text.push_str(&detector.to_string());
        text.push_str(" D");
        text.push_str(&((detector + 1) % DENSE_DETECTOR_COUNT).to_string());
        if detector % 17 == 0 {
            text.push_str(" L0");
        }
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "benchmark runner tests use direct assertions for compact diagnostics"
    )]

    use crate::manifest::{BenchmarkRow, Milestone, Runner};

    use super::{compare_note, measurement_work, run_dem_sampling_compare_row};

    #[test]
    fn m11_benchmark_rows_have_stab_compare_runners() {
        for (id, runner, expected_measurements) in [
            (
                "m11-dem-sampler",
                Runner::StimPerf,
                &["stab_dem_sampler_sample_surface_like_1024"][..],
            ),
            (
                "m11-sample-dem-cli",
                Runner::StimCli,
                &["stab_sample_dem_cli_1024_zero_one"][..],
            ),
            (
                "m11-sample-dem-sparse-contract",
                Runner::ContractOnly,
                &["stab_sample_dem_sparse_b8"][..],
            ),
            (
                "m11-sample-dem-dense-contract",
                Runner::ContractOnly,
                &["stab_sample_dem_dense_b8"][..],
            ),
            (
                "m11-sample-dem-repeated-contract",
                Runner::ContractOnly,
                &["stab_sample_dem_repeated_b8"][..],
            ),
            (
                "m11-sample-dem-high-detector-contract",
                Runner::ContractOnly,
                &["stab_sample_dem_high_detector_b8"][..],
            ),
        ] {
            let row = BenchmarkRow {
                id: id.to_string(),
                milestone: Milestone::M11,
                threshold_class: "report-only".to_string(),
                runner,
                upstream_source: "src/stim/cmd/command_sample_dem.test.cc".to_string(),
                stim_perf_filter: String::new(),
                argv: "sample_dem|--shots|1024".to_string(),
                stdin_path: "oracle/fixtures/inputs/sample_dem_noisy.dem".to_string(),
                phase: "throughput".to_string(),
                measurement: "sample-dem".to_string(),
                description: "test row".to_string(),
            };

            let measurements = run_dem_sampling_compare_row(&row)
                .expect("run compare row")
                .expect("Stab runner");
            let names = measurements
                .iter()
                .map(|measurement| measurement.name.as_str())
                .collect::<Vec<_>>();

            assert_eq!(names.as_slice(), expected_measurements);
            assert!(
                compare_note(id).is_some(),
                "{id} should explain benchmark comparability"
            );
            for name in names {
                assert!(
                    measurement_work(id, name).is_some(),
                    "{id}/{name} should report normalized work"
                );
            }
        }
    }
}
