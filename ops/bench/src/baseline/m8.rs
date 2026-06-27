use std::hint::black_box;

use stab_core::{Circuit, CompiledSampler, SampleFormat};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{measure_stab, measure_stab_iterations, stab_runner_error};

const SAMPLE_NOISY_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/sample_noisy.stim");
const SAMPLE_BIASED_PROBABILITY_FIXTURE: &str = "X_ERROR(0.125) 0\nM 0\n";
const MILLION_SHOT_COMPARE_ITERATIONS: usize = 8;

pub(super) fn run_sample_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "m8-sample-analysis-1shot" => run_sample_analysis_row(row).map(Some),
        "m8-sample-throughput-1024" => run_sample_throughput_row(
            row,
            "stab_sample_1024_zero_one",
            SAMPLE_NOISY_FIXTURE,
            1024,
            super::STAB_COMPARE_ITERATIONS,
        )
        .map(Some),
        "m8-sample-throughput-1000000" => run_sample_throughput_row(
            row,
            "stab_sample_1000000_zero_one",
            SAMPLE_NOISY_FIXTURE,
            1_000_000,
            MILLION_SHOT_COMPARE_ITERATIONS,
        )
        .map(Some),
        "m8-probability-util" => run_sample_throughput_row(
            row,
            "stab_sample_biased_probability_1024",
            SAMPLE_BIASED_PROBABILITY_FIXTURE,
            1024,
            super::STAB_COMPARE_ITERATIONS,
        )
        .map(Some),
        _ => Ok(None),
    }
}

fn run_sample_analysis_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = sample_circuit(&row.id, SAMPLE_NOISY_FIXTURE)?;
    let sampler = compile_sampler(&row.id, &circuit)?;
    Ok(vec![
        measure_stab("stab_sample_compile_noisy_1q", || {
            let compiled = CompiledSampler::compile(&circuit)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(compiled);
            Ok(())
        })?,
        measure_stab("stab_sample_1shot_zero_one", || {
            let output = sampler.sample_bytes_with_seed(1, SampleFormat::ZeroOne, Some(5));
            black_box(output.len());
            Ok(())
        })?,
    ])
}

fn run_sample_throughput_row(
    row: &BenchmarkRow,
    measurement_name: &str,
    fixture: &str,
    shots: usize,
    iterations: usize,
) -> Result<Vec<Measurement>, BenchError> {
    let circuit = sample_circuit(&row.id, fixture)?;
    let sampler = compile_sampler(&row.id, &circuit)?;
    Ok(vec![measure_stab_iterations(
        measurement_name,
        iterations,
        || {
            let output = sampler.sample_bytes_with_seed(shots, SampleFormat::ZeroOne, Some(5));
            black_box(output.len());
            Ok(())
        },
    )?])
}

fn sample_circuit(row_id: &str, fixture: &str) -> Result<Circuit, BenchError> {
    Circuit::from_stim_str(fixture).map_err(|error| stab_runner_error(row_id, error))
}

fn compile_sampler(row_id: &str, circuit: &Circuit) -> Result<CompiledSampler, BenchError> {
    CompiledSampler::compile(circuit).map_err(|error| stab_runner_error(row_id, error))
}

#[cfg(test)]
mod tests {
    use crate::manifest::{BenchmarkRow, Milestone, Runner};

    use super::run_sample_compare_row;
    use crate::baseline::{compare_note, measurement_work};

    #[test]
    fn m8_benchmark_rows_have_stab_compare_runners() {
        for (id, expected_measurements) in [
            (
                "m8-sample-analysis-1shot",
                &["stab_sample_compile_noisy_1q", "stab_sample_1shot_zero_one"][..],
            ),
            (
                "m8-sample-throughput-1024",
                &["stab_sample_1024_zero_one"][..],
            ),
            (
                "m8-sample-throughput-1000000",
                &["stab_sample_1000000_zero_one"][..],
            ),
            (
                "m8-probability-util",
                &["stab_sample_biased_probability_1024"][..],
            ),
        ] {
            let row = BenchmarkRow {
                id: id.to_string(),
                milestone: Milestone::M8,
                threshold_class: "report-only".to_string(),
                runner: Runner::StimCli,
                upstream_source: "src/stim/cmd/command_sample.test.cc".to_string(),
                stim_perf_filter: String::new(),
                argv: "sample|--shots|1".to_string(),
                stdin_path: "oracle/fixtures/inputs/sample_noisy.stim".to_string(),
                phase: "throughput".to_string(),
                measurement: "sample".to_string(),
                description: "test row".to_string(),
            };

            let measurements = run_sample_compare_row(&row)
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
