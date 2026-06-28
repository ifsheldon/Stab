use std::hint::black_box;

use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use stab_core::{
    Circuit, CompiledSampler, Probability, ReferenceSampleTree, SampleFormat,
    biased_randomize_bits,
    result_formats::{write_ptb64_records_checked, write_records},
    result_streaming::{for_each_ptb64_record_all, for_each_record},
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{
    TINY_DIRECT_COMPARE_REPETITIONS, measure_stab, measure_stab_batched, measure_stab_iterations,
    stab_runner_error,
};

const SAMPLE_NOISY_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/sample_noisy.stim");
const PRIMARY_REPETITION_FIXTURE: &str =
    include_str!("../../../../benchmarks/fixtures/m8_sample_primary_repetition_d3_r3.stim");
const PRIMARY_ROTATED_SURFACE_FIXTURE: &str =
    include_str!("../../../../benchmarks/fixtures/m8_sample_primary_rotated_surface_d3_r3.stim");
const PRIMARY_UNROTATED_SURFACE_FIXTURE: &str =
    include_str!("../../../../benchmarks/fixtures/m8_sample_primary_unrotated_surface_d3_r3.stim");
const HIGH_REPEAT_CONTRACT_FIXTURE: &str =
    include_str!("../../../../benchmarks/fixtures/m8_sample_high_repeat_contract.stim");
const MEASURE_READER_BITS: usize = 10_000;
const PROBABILITY_UTIL_BITS: usize = 1024;
const PROBABILITY_UTIL_WORDS: usize = PROBABILITY_UTIL_BITS / u64::BITS as usize;
const PROBABILITY_UTIL_CASES: [(&str, f64); 7] = [
    ("stab_biased_random_1024_0point1percent", 0.001),
    ("stab_biased_random_1024_0point01percent", 0.0001),
    ("stab_biased_random_1024_1percent", 0.01),
    ("stab_biased_random_1024_40percent", 0.4),
    ("stab_biased_random_1024_50percent", 0.5),
    ("stab_biased_random_1024_90percent", 0.9),
    ("stab_biased_random_1024_99percent", 0.99),
];
const FRAME_SIMULATOR_QUBITS: usize = 32;
#[cfg(not(test))]
const FRAME_SIMULATOR_SHOTS: usize = 4;
#[cfg(test)]
const FRAME_SIMULATOR_SHOTS: usize = 2;
const TABLEAU_SIMULATOR_QUBITS: usize = 16;
#[cfg(not(test))]
const PRIMARY_MATRIX_SHOTS: usize = 64;
#[cfg(test)]
const PRIMARY_MATRIX_SHOTS: usize = 2;
const HIGH_REPEAT_CONTRACT_REPS: u64 = 512;
#[cfg(not(test))]
const REFERENCE_SAMPLE_OUTER_REPS: usize = 20;
#[cfg(test)]
const REFERENCE_SAMPLE_OUTER_REPS: usize = 4;
#[cfg(not(test))]
const REFERENCE_SAMPLE_INNER_REPS: usize = 20;
#[cfg(test)]
const REFERENCE_SAMPLE_INNER_REPS: usize = 4;
#[cfg(not(test))]
const SIMULATOR_COMPARE_ITERATIONS: usize = 3;
#[cfg(test)]
const SIMULATOR_COMPARE_ITERATIONS: usize = 1;
#[cfg(not(test))]
const MILLION_SHOT_COMPARE_ITERATIONS: usize = 8;
#[cfg(test)]
const MILLION_SHOT_COMPARE_ITERATIONS: usize = 1;

pub(super) fn run_sample_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "m8-measure-reader-01" => {
            run_measure_reader_format_row(row, "stab_measure_reader_01_10k", SampleFormat::ZeroOne)
                .map(Some)
        }
        "m8-measure-reader-b8" => {
            run_measure_reader_format_row(row, "stab_measure_reader_b8_10k", SampleFormat::B8)
                .map(Some)
        }
        "m8-measure-reader-r8" => {
            run_measure_reader_format_row(row, "stab_measure_reader_r8_10k", SampleFormat::R8)
                .map(Some)
        }
        "m8-measure-reader-hits" => {
            run_measure_reader_format_row(row, "stab_measure_reader_hits_10k", SampleFormat::Hits)
                .map(Some)
        }
        "m8-measure-reader-dets" => {
            run_measure_reader_format_row(row, "stab_measure_reader_dets_10k", SampleFormat::Dets)
                .map(Some)
        }
        "m8-measure-reader-ptb64-contract" => run_measure_reader_ptb64_row(row).map(Some),
        "m8-frame-simulator" => run_frame_simulator_row(row).map(Some),
        "m8-tableau-simulator" => run_tableau_simulator_row(row).map(Some),
        "m8-reference-sample-tree" => run_reference_sample_tree_row(row).map(Some),
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
        "m8-probability-util" => run_probability_util_row(row).map(Some),
        "m8-sample-primary-repetition-contract" => run_primary_repetition_row(row).map(Some),
        "m8-sample-primary-rotated-surface-contract" => run_primary_surface_row(
            row,
            "stab_sample_primary_rotated_surface_d3_r3",
            PRIMARY_ROTATED_SURFACE_FIXTURE,
        )
        .map(Some),
        "m8-sample-primary-unrotated-surface-contract" => run_primary_surface_row(
            row,
            "stab_sample_primary_unrotated_surface_d3_r3",
            PRIMARY_UNROTATED_SURFACE_FIXTURE,
        )
        .map(Some),
        "m8-sample-high-repeat-contract" => run_high_repeat_contract_row(row).map(Some),
        _ => Ok(None),
    }
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m8-measure-reader-ptb64-contract", "stab_measure_reader_ptb64_64x10k_contract") => {
            Some((64.0 * 10_000.0, "bits/s"))
        }
        (row_id, name)
            if matches!(
                row_id,
                "m8-measure-reader-01"
                    | "m8-measure-reader-b8"
                    | "m8-measure-reader-r8"
                    | "m8-measure-reader-hits"
                    | "m8-measure-reader-dets"
            ) && name.starts_with("stab_measure_reader_") =>
        {
            Some((10_000.0, "bits/s"))
        }
        ("m8-frame-simulator", "stab_frame_compile_depolarize1") => Some((32.0, "qubits/s")),
        ("m8-frame-simulator", "stab_frame_sample_depolarize1_b8") => {
            Some((32.0 * 4.0, "op-qubits/s"))
        }
        ("m8-tableau-simulator", "stab_tableau_sample_cx_1shot") => Some((16.0, "op-qubits/s")),
        ("m8-reference-sample-tree", "stab_reference_sample_tree_nested") => {
            Some((422.0, "measurements/s"))
        }
        ("m8-sample-analysis-1shot", "stab_sample_compile_noisy_1q") => {
            Some((1.0, "compilations/s"))
        }
        ("m8-sample-analysis-1shot", "stab_sample_1shot_zero_one") => Some((1.0, "shots/s")),
        ("m8-sample-throughput-1024", "stab_sample_1024_zero_one") => Some((1024.0, "shots/s")),
        ("m8-sample-throughput-1000000", "stab_sample_1000000_zero_one") => {
            Some((1_000_000.0, "shots/s"))
        }
        ("m8-probability-util", name) if name.starts_with("stab_biased_random_1024_") => {
            Some((1024.0, "probability-draws/s"))
        }
        ("m8-sample-primary-repetition-contract", "stab_sample_primary_repetition_d3_r3") => {
            Some((PRIMARY_MATRIX_SHOTS as f64, "shots/s"))
        }
        (
            "m8-sample-primary-rotated-surface-contract",
            "stab_sample_primary_rotated_surface_d3_r3",
        ) => Some((PRIMARY_MATRIX_SHOTS as f64, "shots/s")),
        (
            "m8-sample-primary-unrotated-surface-contract",
            "stab_sample_primary_unrotated_surface_d3_r3",
        ) => Some((PRIMARY_MATRIX_SHOTS as f64, "shots/s")),
        ("m8-sample-high-repeat-contract", "stab_sample_high_repeat_contract") => {
            Some((HIGH_REPEAT_CONTRACT_REPS as f64, "repeat-body-executions/s"))
        }
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m8-measure-reader-01" => Some(
            "partial-match: Stab measures the public 01 reusable-record reader against pinned Stim read_01 dense and sparse reader filters",
        ),
        "m8-measure-reader-b8" => Some(
            "partial-match: Stab measures the public b8 reusable-record reader against pinned Stim read_b8 dense and sparse reader filters",
        ),
        "m8-measure-reader-r8" => Some(
            "partial-match: Stab measures the public r8 reusable-record reader against pinned Stim read_r8 dense and sparse reader filters",
        ),
        "m8-measure-reader-hits" => Some(
            "partial-match: Stab measures the public hits reusable-record reader against pinned Stim read_hits dense and sparse reader filters",
        ),
        "m8-measure-reader-dets" => Some(
            "partial-match: Stab measures the public dets reusable-record reader against pinned Stim read_dets dense and sparse reader filters",
        ),
        "m8-measure-reader-ptb64-contract" => Some(
            "contract-only: Stab measures ptb64 reader throughput against upstream ptb64 reader tests because pinned Stim has no ptb64 reader perf filter",
        ),
        "m8-frame-simulator" => Some(
            "report-only: Stab measures the current public sampler frame path for a bounded depolarizing workload; upstream baseline is an internal bit-parallel frame simulator",
        ),
        "m8-tableau-simulator" => Some(
            "report-only: Stab measures one-shot public sampler execution through Clifford tableau operations; upstream baseline is an internal 10K-qubit tableau simulator primitive",
        ),
        "m8-reference-sample-tree" => Some(
            "partial-match: Stab measures the basic reference-sample-tree helper; optimized loop-folded construction remains a logged M8 spec gap",
        ),
        "m8-sample-analysis-1shot" => Some(
            "report-only: Stab splits core sampler compilation and one-shot sampling; pinned Stim baseline is end-to-end CLI sample",
        ),
        "m8-sample-throughput-1024" | "m8-sample-throughput-1000000" => Some(
            "report-only: Stab measures in-process core sampler throughput with default 01 output; pinned Stim baseline includes CLI process, parse, and output costs",
        ),
        "m8-probability-util" => Some(
            "direct-match: Stab measures the biased random bit utility against the pinned Stim probability_util perf filters",
        ),
        "m8-sample-primary-repetition-contract" => Some(
            "cli-baseline: Stab samples the source-owned generated repetition-code d3/r3 fixture with b8 output against pinned Stim sample on the same fixture",
        ),
        "m8-sample-primary-rotated-surface-contract" => Some(
            "cli-baseline: Stab samples the source-owned generated rotated-surface d3/r3 fixture with b8 output against pinned Stim sample on the same fixture",
        ),
        "m8-sample-primary-unrotated-surface-contract" => Some(
            "cli-baseline: Stab samples the source-owned generated unrotated-surface d3/r3 fixture with b8 output against pinned Stim sample on the same fixture",
        ),
        "m8-sample-high-repeat-contract" => Some(
            "cli-baseline: Stab samples the source-owned repeat-heavy fixture with b8 output against pinned Stim sample on the same fixture; optimized loop folding remains a logged M8 spec gap",
        ),
        _ => None,
    }
}

fn run_measure_reader_format_row(
    row: &BenchmarkRow,
    name: &'static str,
    format: SampleFormat,
) -> Result<Vec<Measurement>, BenchError> {
    let source_record = deterministic_measure_reader_record();
    let input = write_records(std::slice::from_ref(&source_record), format);
    Ok(vec![measure_stab(name, || {
        let mut set_bits = 0usize;
        for_each_record(&input, format, MEASURE_READER_BITS, |record| {
            set_bits += record.iter().filter(|bit| **bit).count();
            Ok(())
        })
        .map_err(|error| stab_runner_error(&row.id, error))?;
        black_box(set_bits);
        Ok(())
    })?])
}

fn run_measure_reader_ptb64_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let source_record = deterministic_measure_reader_record();
    let ptb64_records = (0..64).map(|_| source_record.clone()).collect::<Vec<_>>();
    let ptb64_input = write_ptb64_records_checked(&ptb64_records)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![measure_stab(
        "stab_measure_reader_ptb64_64x10k_contract",
        || {
            let mut set_bits = 0usize;
            for_each_ptb64_record_all(&ptb64_input, MEASURE_READER_BITS, |record| {
                set_bits += record.iter().filter(|bit| **bit).count();
                Ok(())
            })
            .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(set_bits);
            Ok(())
        },
    )?])
}

fn run_probability_util_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    PROBABILITY_UTIL_CASES
        .iter()
        .map(|(name, probability)| {
            let probability = Probability::try_new(*probability)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let mut rng = SmallRng::seed_from_u64(0);
            let mut words = [0u64; PROBABILITY_UTIL_WORDS];
            measure_stab_batched(name, TINY_DIRECT_COMPARE_REPETITIONS, || {
                biased_randomize_bits(probability, &mut words, &mut rng);
                black_box(&words);
                Ok(())
            })
        })
        .collect()
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

fn run_frame_simulator_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let fixture = frame_simulator_fixture();
    let circuit = sample_circuit(&row.id, &fixture)?;
    let sampler = compile_sampler(&row.id, &circuit)?;
    Ok(vec![
        measure_stab_iterations(
            "stab_frame_compile_depolarize1",
            SIMULATOR_COMPARE_ITERATIONS,
            || {
                let compiled = CompiledSampler::compile(&circuit)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(compiled);
                Ok(())
            },
        )?,
        measure_stab_iterations(
            "stab_frame_sample_depolarize1_b8",
            SIMULATOR_COMPARE_ITERATIONS,
            || {
                let output = sampler.sample_bytes_with_seed(
                    FRAME_SIMULATOR_SHOTS,
                    SampleFormat::B8,
                    Some(5),
                );
                black_box(output.len());
                Ok(())
            },
        )?,
    ])
}

fn run_tableau_simulator_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let fixture = tableau_simulator_fixture();
    let circuit = sample_circuit(&row.id, &fixture)?;
    let sampler = compile_sampler(&row.id, &circuit)?;
    Ok(vec![measure_stab_iterations(
        "stab_tableau_sample_cx_1shot",
        SIMULATOR_COMPARE_ITERATIONS,
        || {
            let output = sampler.sample_bytes_with_seed(1, SampleFormat::B8, Some(5));
            black_box(output.len());
            Ok(())
        },
    )?])
}

fn run_reference_sample_tree_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let fixture = reference_sample_tree_fixture();
    let circuit = sample_circuit(&row.id, &fixture)?;
    Ok(vec![measure_stab_iterations(
        "stab_reference_sample_tree_nested",
        SIMULATOR_COMPARE_ITERATIONS,
        || {
            let tree = ReferenceSampleTree::from_circuit_reference_sample(&circuit)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(tree.size());
            Ok(())
        },
    )?])
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

fn run_primary_repetition_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    run_primary_generated_sample_row(
        row,
        "stab_sample_primary_repetition_d3_r3",
        PRIMARY_REPETITION_FIXTURE,
    )
}

fn run_primary_surface_row(
    row: &BenchmarkRow,
    measurement_name: &'static str,
    fixture: &str,
) -> Result<Vec<Measurement>, BenchError> {
    run_primary_generated_sample_row(row, measurement_name, fixture)
}

fn run_primary_generated_sample_row(
    row: &BenchmarkRow,
    measurement_name: &'static str,
    fixture: &str,
) -> Result<Vec<Measurement>, BenchError> {
    let circuit = sample_circuit(&row.id, fixture)?;
    let sampler = compile_sampler(&row.id, &circuit)?;
    Ok(vec![measure_stab_iterations(
        measurement_name,
        SIMULATOR_COMPARE_ITERATIONS,
        || {
            let output =
                sampler.sample_bytes_with_seed(PRIMARY_MATRIX_SHOTS, SampleFormat::B8, Some(5));
            black_box(output.len());
            Ok(())
        },
    )?])
}

fn run_high_repeat_contract_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = sample_circuit(&row.id, HIGH_REPEAT_CONTRACT_FIXTURE)?;
    let sampler = compile_sampler(&row.id, &circuit)?;
    Ok(vec![measure_stab_iterations(
        "stab_sample_high_repeat_contract",
        SIMULATOR_COMPARE_ITERATIONS,
        || {
            let output = sampler.sample_bytes_with_seed(1, SampleFormat::B8, Some(5));
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

fn deterministic_measure_reader_record() -> Vec<bool> {
    (0..MEASURE_READER_BITS)
        .map(|index| (index * 17 + 3) % 10 == 0)
        .collect()
}

fn frame_simulator_fixture() -> String {
    let targets = (0..FRAME_SIMULATOR_QUBITS)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    format!("DEPOLARIZE1(0.001) {targets}\nM {targets}\n")
}

fn tableau_simulator_fixture() -> String {
    let mut text = String::new();
    for index in 0..TABLEAU_SIMULATOR_QUBITS {
        text.push_str("H ");
        text.push_str(&index.to_string());
        text.push('\n');
    }
    text.push_str("CX");
    for index in 0..TABLEAU_SIMULATOR_QUBITS.saturating_sub(1) {
        text.push(' ');
        text.push_str(&index.to_string());
        text.push(' ');
        text.push_str(&(index + 1).to_string());
    }
    text.push('\n');
    text.push('M');
    for index in 0..TABLEAU_SIMULATOR_QUBITS {
        text.push(' ');
        text.push_str(&index.to_string());
    }
    text.push('\n');
    text
}

fn reference_sample_tree_fixture() -> String {
    format!(
        "M 0\nREPEAT {REFERENCE_SAMPLE_OUTER_REPS} {{\n    REPEAT {REFERENCE_SAMPLE_INNER_REPS} {{\n        X 0\n        M 0\n    }}\n    X 0\n    M 0\n}}\nX 0\nM 0\n"
    )
}

#[cfg(test)]
mod tests {
    use crate::manifest::{BenchmarkRow, Milestone, Runner};

    use super::{compare_note, measurement_work, run_sample_compare_row};

    #[test]
    fn m8_benchmark_rows_have_stab_compare_runners() {
        for (id, expected_measurements) in [
            ("m8-measure-reader-01", &["stab_measure_reader_01_10k"][..]),
            ("m8-measure-reader-b8", &["stab_measure_reader_b8_10k"][..]),
            ("m8-measure-reader-r8", &["stab_measure_reader_r8_10k"][..]),
            (
                "m8-measure-reader-hits",
                &["stab_measure_reader_hits_10k"][..],
            ),
            (
                "m8-measure-reader-dets",
                &["stab_measure_reader_dets_10k"][..],
            ),
            (
                "m8-measure-reader-ptb64-contract",
                &["stab_measure_reader_ptb64_64x10k_contract"][..],
            ),
            (
                "m8-frame-simulator",
                &[
                    "stab_frame_compile_depolarize1",
                    "stab_frame_sample_depolarize1_b8",
                ][..],
            ),
            (
                "m8-tableau-simulator",
                &["stab_tableau_sample_cx_1shot"][..],
            ),
            (
                "m8-reference-sample-tree",
                &["stab_reference_sample_tree_nested"][..],
            ),
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
                &[
                    "stab_biased_random_1024_0point1percent",
                    "stab_biased_random_1024_0point01percent",
                    "stab_biased_random_1024_1percent",
                    "stab_biased_random_1024_40percent",
                    "stab_biased_random_1024_50percent",
                    "stab_biased_random_1024_90percent",
                    "stab_biased_random_1024_99percent",
                ][..],
            ),
            (
                "m8-sample-primary-repetition-contract",
                &["stab_sample_primary_repetition_d3_r3"][..],
            ),
            (
                "m8-sample-primary-rotated-surface-contract",
                &["stab_sample_primary_rotated_surface_d3_r3"][..],
            ),
            (
                "m8-sample-primary-unrotated-surface-contract",
                &["stab_sample_primary_unrotated_surface_d3_r3"][..],
            ),
            (
                "m8-sample-high-repeat-contract",
                &["stab_sample_high_repeat_contract"][..],
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
