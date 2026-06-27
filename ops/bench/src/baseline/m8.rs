use std::hint::black_box;

use stab_core::{
    Circuit, CodeDistance, CompiledSampler, Probability, ReferenceSampleTree, RepetitionCodeParams,
    RepetitionCodeTask, RoundCount, SampleFormat, SurfaceCodeParams, SurfaceCodeTask,
    generate_repetition_code_circuit, generate_surface_code_circuit,
    result_formats::{read_records, write_records},
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{measure_stab, measure_stab_iterations, stab_runner_error};

const SAMPLE_NOISY_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/sample_noisy.stim");
const SAMPLE_BIASED_PROBABILITY_FIXTURE: &str = "X_ERROR(0.125) 0\nM 0\n";
const MEASURE_READER_BITS: usize = 10_000;
const FRAME_SIMULATOR_QUBITS: usize = 32;
#[cfg(not(test))]
const FRAME_SIMULATOR_SHOTS: usize = 4;
#[cfg(test)]
const FRAME_SIMULATOR_SHOTS: usize = 2;
const TABLEAU_SIMULATOR_QUBITS: usize = 16;
const PRIMARY_MATRIX_DISTANCE: u32 = 3;
const PRIMARY_MATRIX_ROUNDS: u64 = 3;
#[cfg(not(test))]
const PRIMARY_MATRIX_SHOTS: usize = 64;
#[cfg(test)]
const PRIMARY_MATRIX_SHOTS: usize = 2;
#[cfg(not(test))]
const HIGH_REPEAT_CONTRACT_REPS: u64 = 512;
#[cfg(test)]
const HIGH_REPEAT_CONTRACT_REPS: u64 = 4;
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
        "m8-measure-reader" => run_measure_reader_row(row).map(Some),
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
        "m8-probability-util" => run_sample_throughput_row(
            row,
            "stab_sample_biased_probability_1024",
            SAMPLE_BIASED_PROBABILITY_FIXTURE,
            1024,
            super::STAB_COMPARE_ITERATIONS,
        )
        .map(Some),
        "m8-sample-primary-repetition-contract" => run_primary_repetition_row(row).map(Some),
        "m8-sample-primary-rotated-surface-contract" => run_primary_surface_row(
            row,
            "stab_sample_primary_rotated_surface_d3_r3",
            SurfaceCodeTask::RotatedMemoryZ,
        )
        .map(Some),
        "m8-sample-primary-unrotated-surface-contract" => run_primary_surface_row(
            row,
            "stab_sample_primary_unrotated_surface_d3_r3",
            SurfaceCodeTask::UnrotatedMemoryZ,
        )
        .map(Some),
        "m8-sample-high-repeat-contract" => run_high_repeat_contract_row(row).map(Some),
        _ => Ok(None),
    }
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m8-measure-reader", name) if name.starts_with("stab_measure_reader_") => {
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
        ("m8-probability-util", "stab_sample_biased_probability_1024") => {
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
        "m8-measure-reader" => Some(
            "partial-match: Stab reports supported 01/b8/r8/hits/dets readers; ptb64 reader parity is not implemented yet",
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
            "contract-proxy: Stab exercises the sampler probability path because there is no standalone probability-util public API yet",
        ),
        "m8-sample-primary-repetition-contract" => Some(
            "contract-representative: Stab samples a generated repetition-code d3/r3 circuit; full primary matrix thresholds remain M12 work",
        ),
        "m8-sample-primary-rotated-surface-contract" => Some(
            "contract-representative: Stab samples a generated rotated-surface d3/r3 circuit; full primary matrix thresholds remain M12 work",
        ),
        "m8-sample-primary-unrotated-surface-contract" => Some(
            "contract-representative: Stab samples a generated unrotated-surface d3/r3 circuit; full primary matrix thresholds remain M12 work",
        ),
        "m8-sample-high-repeat-contract" => Some(
            "contract-representative: Stab samples a repeat-heavy circuit without flattening during compilation; optimized loop folding remains a logged M8 spec gap",
        ),
        _ => None,
    }
}

fn run_measure_reader_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let source_record = deterministic_measure_reader_record();
    let encoded = [
        (
            "stab_measure_reader_01_10k",
            SampleFormat::ZeroOne,
            write_records(std::slice::from_ref(&source_record), SampleFormat::ZeroOne),
        ),
        (
            "stab_measure_reader_b8_10k",
            SampleFormat::B8,
            write_records(std::slice::from_ref(&source_record), SampleFormat::B8),
        ),
        (
            "stab_measure_reader_r8_10k",
            SampleFormat::R8,
            write_records(std::slice::from_ref(&source_record), SampleFormat::R8),
        ),
        (
            "stab_measure_reader_hits_10k",
            SampleFormat::Hits,
            write_records(std::slice::from_ref(&source_record), SampleFormat::Hits),
        ),
        (
            "stab_measure_reader_dets_10k",
            SampleFormat::Dets,
            write_records(std::slice::from_ref(&source_record), SampleFormat::Dets),
        ),
    ];
    encoded
        .iter()
        .map(|(name, format, input)| {
            measure_stab(name, || {
                let records = read_records(input, *format, MEASURE_READER_BITS)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(records.iter().flatten().filter(|bit| **bit).count());
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
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(PRIMARY_MATRIX_ROUNDS)
            .map_err(|error| stab_runner_error(&row.id, error))?,
        CodeDistance::try_new(PRIMARY_MATRIX_DISTANCE)
            .map_err(|error| stab_runner_error(&row.id, error))?,
        RepetitionCodeTask::Memory,
    )
    .map_err(|error| stab_runner_error(&row.id, error))?
    .with_before_measure_flip_probability(
        Probability::try_new(0.001).map_err(|error| stab_runner_error(&row.id, error))?,
    );
    let generated = generate_repetition_code_circuit(&params)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    run_primary_generated_sample_row(
        row,
        "stab_sample_primary_repetition_d3_r3",
        generated.circuit(),
    )
}

fn run_primary_surface_row(
    row: &BenchmarkRow,
    measurement_name: &'static str,
    task: SurfaceCodeTask,
) -> Result<Vec<Measurement>, BenchError> {
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(PRIMARY_MATRIX_ROUNDS)
            .map_err(|error| stab_runner_error(&row.id, error))?,
        CodeDistance::try_new(PRIMARY_MATRIX_DISTANCE)
            .map_err(|error| stab_runner_error(&row.id, error))?,
        task,
    )
    .map_err(|error| stab_runner_error(&row.id, error))?
    .with_after_clifford_depolarization(
        Probability::try_new(0.001).map_err(|error| stab_runner_error(&row.id, error))?,
    );
    let generated = generate_surface_code_circuit(&params)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    run_primary_generated_sample_row(row, measurement_name, generated.circuit())
}

fn run_primary_generated_sample_row(
    row: &BenchmarkRow,
    measurement_name: &'static str,
    circuit: &Circuit,
) -> Result<Vec<Measurement>, BenchError> {
    let sampler = compile_sampler(&row.id, circuit)?;
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
    let fixture = high_repeat_contract_fixture();
    let circuit = sample_circuit(&row.id, &fixture)?;
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

fn high_repeat_contract_fixture() -> String {
    format!("REPEAT {HIGH_REPEAT_CONTRACT_REPS} {{\n    H 0\n    M 0\n    R 0\n}}\n")
}

#[cfg(test)]
mod tests {
    use crate::manifest::{BenchmarkRow, Milestone, Runner};

    use super::{compare_note, measurement_work, run_sample_compare_row};

    #[test]
    fn m8_benchmark_rows_have_stab_compare_runners() {
        for (id, expected_measurements) in [
            (
                "m8-measure-reader",
                &[
                    "stab_measure_reader_01_10k",
                    "stab_measure_reader_b8_10k",
                    "stab_measure_reader_r8_10k",
                    "stab_measure_reader_hits_10k",
                    "stab_measure_reader_dets_10k",
                ][..],
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
                &["stab_sample_biased_probability_1024"][..],
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
