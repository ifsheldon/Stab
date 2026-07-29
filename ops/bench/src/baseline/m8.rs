#[cfg(not(test))]
use std::ffi::OsString;
use std::hint::black_box;
#[cfg(test)]
use std::io::{self, Write};
#[cfg(not(test))]
use std::path::{Component, Path, PathBuf};

use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use stab_core::{
    BitPlane64Batch, Circuit, MeasurementBatchView, MeasurementCodecSink, MeasurementSink,
    Probability, RecordFormat, SampleFormat,
    result_formats::{write_ptb64_records_checked, write_records},
    result_streaming::{for_each_packed_record, for_each_ptb64_record_all, for_each_sparse_record},
};
use stab_engine::{
    BackendPreference, RandomPolicy, ReferenceSampleTree, SamplingCompiler, SamplingPlan,
    SamplingSession, Seed, ShotCount, biased_randomize_bits,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
#[cfg(not(test))]
use crate::process::{check_success, run_checked_status, run_process};
use crate::report::Measurement;
use crate::root::RepoRoot;

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
const SAMPLE_CLI_PROCESS_LAUNCHES_PER_MEASUREMENT: usize = 1;
#[cfg(not(test))]
const MILLION_SHOT_COMPARE_ITERATIONS: usize = 8;
#[cfg(test)]
const MILLION_SHOT_COMPARE_ITERATIONS: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MeasureReaderMode {
    Packed,
    Sparse,
}

pub(super) fn run_sample_compare_row(
    root: &RepoRoot,
    profile: &str,
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "m8-measure-reader-01" => run_measure_reader_format_row(
            row,
            SampleFormat::ZeroOne,
            &[
                ("stab_read_01_dense_per10", MeasureReaderMode::Packed, 10),
                ("stab_read_01_sparse_per10", MeasureReaderMode::Sparse, 10),
            ],
        )
        .map(Some),
        "m8-measure-reader-b8" => run_measure_reader_format_row(
            row,
            SampleFormat::B8,
            &[
                ("stab_read_b8_dense_per10", MeasureReaderMode::Packed, 10),
                ("stab_read_b8_sparse_per10", MeasureReaderMode::Sparse, 10),
            ],
        )
        .map(Some),
        "m8-measure-reader-r8" => run_measure_reader_format_row(
            row,
            SampleFormat::R8,
            &[
                ("stab_read_r8_dense_per10", MeasureReaderMode::Packed, 10),
                ("stab_read_r8_dense_per100", MeasureReaderMode::Packed, 100),
                ("stab_read_r8_sparse_per10", MeasureReaderMode::Sparse, 10),
                ("stab_read_r8_sparse_per100", MeasureReaderMode::Sparse, 100),
            ],
        )
        .map(Some),
        "m8-measure-reader-hits" => run_measure_reader_format_row(
            row,
            SampleFormat::Hits,
            &[
                ("stab_read_hits_dense_per10", MeasureReaderMode::Packed, 10),
                (
                    "stab_read_hits_dense_per100",
                    MeasureReaderMode::Packed,
                    100,
                ),
                ("stab_read_hits_sparse_per10", MeasureReaderMode::Sparse, 10),
                (
                    "stab_read_hits_sparse_per100",
                    MeasureReaderMode::Sparse,
                    100,
                ),
            ],
        )
        .map(Some),
        "m8-measure-reader-dets" => run_measure_reader_format_row(
            row,
            SampleFormat::Dets,
            &[
                ("stab_read_dets_dense_per10", MeasureReaderMode::Packed, 10),
                (
                    "stab_read_dets_dense_per100",
                    MeasureReaderMode::Packed,
                    100,
                ),
                ("stab_read_dets_sparse_per10", MeasureReaderMode::Sparse, 10),
                (
                    "stab_read_dets_sparse_per100",
                    MeasureReaderMode::Sparse,
                    100,
                ),
            ],
        )
        .map(Some),
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
        "m8-sample-primary-repetition-contract" => {
            run_primary_repetition_row(root, profile, row).map(Some)
        }
        "m8-sample-primary-rotated-surface-contract" => run_primary_surface_row(
            root,
            profile,
            row,
            "stab_sample_primary_rotated_surface_d3_r3",
            PRIMARY_ROTATED_SURFACE_FIXTURE,
        )
        .map(Some),
        "m8-sample-primary-unrotated-surface-contract" => run_primary_surface_row(
            root,
            profile,
            row,
            "stab_sample_primary_unrotated_surface_d3_r3",
            PRIMARY_UNROTATED_SURFACE_FIXTURE,
        )
        .map(Some),
        "m8-sample-high-repeat-contract" => {
            run_high_repeat_contract_row(root, profile, row).map(Some)
        }
        _ => Ok(None),
    }
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m8-measure-reader-ptb64-contract", "stab_measure_reader_ptb64_64x10k_contract") => {
            Some((64.0 * 10_000.0, "bits/s"))
        }
        ("m8-measure-reader-01", name)
        | ("m8-measure-reader-b8", name)
        | ("m8-measure-reader-r8", name)
        | ("m8-measure-reader-hits", name)
        | ("m8-measure-reader-dets", name)
            if name.starts_with("stab_read_") =>
        {
            if name.contains("_sparse_per") {
                let denominator = measure_reader_denominator_from_name(name)?;
                Some((10_000.0 / denominator as f64, "pops/s"))
            } else {
                Some((10_000.0, "bits/s"))
            }
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
        ("m8-reference-sample-tree", "stab_reference_sample_tree_flat_20x20") => {
            Some((422.0, "measurements/s"))
        }
        (
            "m8-sample-analysis-1shot",
            "stab_sample_compile_plan_auto_noisy_1q" | "stab_sample_compile_plan_scalar_noisy_1q",
        ) => Some((1.0, "compilations/s")),
        ("m8-sample-analysis-1shot", "stab_sample_construct_session_noisy_1q") => {
            Some((1.0, "sessions/s"))
        }
        (
            "m8-sample-analysis-1shot",
            "stab_sample_execute_witness_sink_64_continuous_session"
            | "stab_sample_consume_typed_batch_64"
            | "stab_sample_encode_b8_64"
            | "stab_sample_repeated_session_16x4_continuous_session",
        ) => Some((64.0, "shots/s")),
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
            "direct-match: Stab measures packed and sparse 01 reusable-record readers against pinned Stim read_01 dense and sparse reader filters",
        ),
        "m8-measure-reader-b8" => Some(
            "direct-match: Stab measures packed and sparse b8 reusable-record readers against pinned Stim read_b8 dense and sparse reader filters",
        ),
        "m8-measure-reader-r8" => Some(
            "direct-match: Stab measures packed and sparse r8 reusable-record readers against pinned Stim read_r8 dense and sparse reader filters",
        ),
        "m8-measure-reader-hits" => Some(
            "direct-match: Stab measures packed and sparse hits reusable-record readers against pinned Stim read_hits dense and sparse reader filters",
        ),
        "m8-measure-reader-dets" => Some(
            "direct-match: Stab measures packed and sparse dets reusable-record readers against pinned Stim read_dets dense and sparse reader filters",
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
            "report-only: Stab measures bounded flat reference-sample-tree construction for a 20-by-20 nested circuit, while pinned Stim measures folded construction for a 100000-cubed nested repeat; optimized loop-folded construction remains deferred and no Stim-relative ratio is claimed",
        ),
        "m8-sample-analysis-1shot" => Some(
            "report-only: Stab phase-separates plan compilation, scalar selection, session construction, steady-state raw execution, consumption of a prebuilt typed batch, encoding of a prebuilt typed batch, and repeated-session execution; only raw and repeated measurements advance preconstructed sessions, while pinned Stim baseline is end-to-end CLI sample",
        ),
        "m8-sample-throughput-1024" | "m8-sample-throughput-1000000" => Some(
            "report-only: Stab measures in-process core sampler throughput with default 01 output; pinned Stim baseline includes CLI process, parse, and output costs",
        ),
        "m8-probability-util" => Some(
            "direct-match: Stab measures the biased random bit utility against the pinned Stim probability_util perf filters",
        ),
        "m8-sample-primary-repetition-contract" => Some(
            "cli-baseline: Stab and pinned Stim run as bounded subprocesses with identical stdin, arguments, iteration policy, and discarded stdout; an untimed Stab preflight must match the frozen pre-A4 repetition-code d3/r3 b8 witness",
        ),
        "m8-sample-primary-rotated-surface-contract" => Some(
            "cli-baseline: Stab and pinned Stim run as bounded subprocesses with identical stdin, arguments, iteration policy, and discarded stdout; an untimed Stab preflight must match the frozen pre-A4 rotated-surface d3/r3 b8 witness",
        ),
        "m8-sample-primary-unrotated-surface-contract" => Some(
            "cli-baseline: Stab and pinned Stim run as bounded subprocesses with identical stdin, arguments, iteration policy, and discarded stdout; an untimed Stab preflight must match the frozen pre-A4 unrotated-surface d3/r3 b8 witness",
        ),
        "m8-sample-high-repeat-contract" => Some(
            "cli-baseline: Stab and pinned Stim run as bounded subprocesses with identical stdin, arguments, iteration policy, and discarded stdout; an untimed Stab preflight must match the frozen pre-A4 repeat-heavy b8 witness, while optimized loop folding remains a logged M8 spec gap",
        ),
        _ => None,
    }
}

fn run_measure_reader_format_row(
    row: &BenchmarkRow,
    format: SampleFormat,
    cases: &[(&'static str, MeasureReaderMode, usize)],
) -> Result<Vec<Measurement>, BenchError> {
    cases
        .iter()
        .map(|(name, mode, denominator)| {
            let source_record = measure_reader_record(*denominator);
            let input = write_records(std::slice::from_ref(&source_record), format);
            measure_stab_batched(name, TINY_DIRECT_COMPARE_REPETITIONS, || {
                let mut set_bits = 0usize;
                match mode {
                    MeasureReaderMode::Packed => {
                        for_each_packed_record(&input, format, MEASURE_READER_BITS, |record| {
                            set_bits += record.popcount();
                            Ok(())
                        })
                    }
                    MeasureReaderMode::Sparse => {
                        for_each_sparse_record(&input, format, MEASURE_READER_BITS, |hits| {
                            set_bits += hits.len();
                            Ok(())
                        })
                    }
                }
                .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(set_bits);
                Ok(())
            })
        })
        .collect()
}

fn run_measure_reader_ptb64_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let source_record = measure_reader_record(10);
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
    let plan = compile_plan(&row.id, &circuit)?;
    let mut raw_session = sampling_session(&row.id, &plan, 5)?;
    let mut raw_sink = BoundaryWitnessSink::default();
    let mut delivery_sink = DigestMeasurementSink::default();
    let mut repeated_session = sampling_session(&row.id, &plan, 5)?;
    let mut repeated_sink = BoundaryWitnessSink::default();
    let encoding_batch = sample_encoding_batch(&row.id, plan.measurement_width().get())?;
    Ok(vec![
        measure_stab("stab_sample_compile_plan_auto_noisy_1q", || {
            let compiled = SamplingCompiler::new()
                .compile(&circuit)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(compiled);
            Ok(())
        })?,
        measure_stab("stab_sample_compile_plan_scalar_noisy_1q", || {
            let compiled = SamplingCompiler::new()
                .backend(BackendPreference::Scalar)
                .compile(&circuit)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(compiled);
            Ok(())
        })?,
        measure_stab("stab_sample_construct_session_noisy_1q", || {
            let session = plan
                .session(RandomPolicy::Seeded(Seed::new(5)))
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(session);
            Ok(())
        })?,
        measure_stab(
            "stab_sample_execute_witness_sink_64_continuous_session",
            || {
                let summary = raw_session
                    .run(ShotCount::new(64), &mut raw_sink)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box((summary, raw_sink.digest));
                Ok(())
            },
        )?,
        measure_stab("stab_sample_consume_typed_batch_64", || {
            delivery_sink
                .write_batch(MeasurementBatchView::from_bit_planes(encoding_batch.view()))
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(delivery_sink.digest);
            Ok(())
        })?,
        measure_stab("stab_sample_encode_b8_64", || {
            let mut sink =
                MeasurementCodecSink::try_new(RecordFormat::B8, plan.measurement_width())
                    .map_err(|error| stab_runner_error(&row.id, error))?;
            sink.write_batch(MeasurementBatchView::from_bit_planes(encoding_batch.view()))
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let bytes = sink
                .into_bytes()
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(bytes.len());
            Ok(())
        })?,
        measure_stab(
            "stab_sample_repeated_session_16x4_continuous_session",
            || {
                for _ in 0..16 {
                    let summary = repeated_session
                        .run(ShotCount::new(4), &mut repeated_sink)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box((summary, repeated_sink.digest));
                }
                Ok(())
            },
        )?,
    ])
}

fn run_frame_simulator_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let fixture = frame_simulator_fixture();
    let circuit = sample_circuit(&row.id, &fixture)?;
    let plan = compile_plan(&row.id, &circuit)?;
    Ok(vec![
        measure_stab_iterations(
            "stab_frame_compile_depolarize1",
            SIMULATOR_COMPARE_ITERATIONS,
            || {
                let compiled = SamplingCompiler::new()
                    .compile(&circuit)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(compiled);
                Ok(())
            },
        )?,
        measure_stab_iterations(
            "stab_frame_sample_depolarize1_b8",
            SIMULATOR_COMPARE_ITERATIONS,
            || {
                let output = sample_plan_b8(&row.id, &plan, FRAME_SIMULATOR_SHOTS, 5)?;
                black_box(output);
                Ok(())
            },
        )?,
    ])
}

fn run_tableau_simulator_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let fixture = tableau_simulator_fixture();
    let circuit = sample_circuit(&row.id, &fixture)?;
    let plan = compile_plan(&row.id, &circuit)?;
    Ok(vec![measure_stab_iterations(
        "stab_tableau_sample_cx_1shot",
        SIMULATOR_COMPARE_ITERATIONS,
        || {
            let output = sample_plan_b8(&row.id, &plan, 1, 5)?;
            black_box(output);
            Ok(())
        },
    )?])
}

fn run_reference_sample_tree_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let fixture = reference_sample_tree_fixture();
    let circuit = sample_circuit(&row.id, &fixture)?;
    Ok(vec![measure_stab_iterations(
        "stab_reference_sample_tree_flat_20x20",
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
    let plan = compile_plan(&row.id, &circuit)?;
    Ok(vec![measure_stab_iterations(
        measurement_name,
        iterations,
        || {
            let mut session = sampling_session(&row.id, &plan, 5)?;
            let mut sink =
                MeasurementCodecSink::try_new(RecordFormat::ZeroOne, plan.measurement_width())
                    .map_err(|error| stab_runner_error(&row.id, error))?;
            sink.reserve_records(shots)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let summary = session
                .run(shot_count(&row.id, shots)?, &mut sink)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let output = sink
                .into_bytes()
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box((
                summary,
                output.len(),
                output.first().copied(),
                output.last().copied(),
            ));
            Ok(())
        },
    )?])
}

fn run_primary_repetition_row(
    root: &RepoRoot,
    profile: &str,
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    run_primary_generated_sample_row(
        root,
        profile,
        row,
        "stab_sample_primary_repetition_d3_r3",
        PRIMARY_REPETITION_FIXTURE,
    )
}

fn run_primary_surface_row(
    root: &RepoRoot,
    profile: &str,
    row: &BenchmarkRow,
    measurement_name: &'static str,
    fixture: &str,
) -> Result<Vec<Measurement>, BenchError> {
    run_primary_generated_sample_row(root, profile, row, measurement_name, fixture)
}

fn run_primary_generated_sample_row(
    root: &RepoRoot,
    profile: &str,
    row: &BenchmarkRow,
    measurement_name: &'static str,
    fixture: &str,
) -> Result<Vec<Measurement>, BenchError> {
    #[cfg(not(test))]
    {
        run_sample_cli_process_row(root, profile, row, measurement_name, fixture)
    }
    #[cfg(test)]
    let _ = (root, profile);
    #[cfg(test)]
    let expected = sample_cli_witness(row, fixture, PRIMARY_MATRIX_SHOTS, "b8")?;
    #[cfg(test)]
    Ok(vec![measure_stab_iterations(
        measurement_name,
        SAMPLE_CLI_PROCESS_LAUNCHES_PER_MEASUREMENT,
        || run_sample_cli(row, fixture, PRIMARY_MATRIX_SHOTS, "b8", expected),
    )?])
}

fn run_high_repeat_contract_row(
    root: &RepoRoot,
    profile: &str,
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    #[cfg(not(test))]
    {
        run_sample_cli_process_row(
            root,
            profile,
            row,
            "stab_sample_high_repeat_contract",
            HIGH_REPEAT_CONTRACT_FIXTURE,
        )
    }
    #[cfg(test)]
    let _ = (root, profile);
    #[cfg(test)]
    let expected = sample_cli_witness(row, HIGH_REPEAT_CONTRACT_FIXTURE, 1, "b8")?;
    #[cfg(test)]
    Ok(vec![measure_stab_iterations(
        "stab_sample_high_repeat_contract",
        SAMPLE_CLI_PROCESS_LAUNCHES_PER_MEASUREMENT,
        || run_sample_cli(row, HIGH_REPEAT_CONTRACT_FIXTURE, 1, "b8", expected),
    )?])
}

#[cfg(not(test))]
fn run_sample_cli_process_row(
    root: &RepoRoot,
    profile: &str,
    row: &BenchmarkRow,
    measurement_name: &'static str,
    fixture: &str,
) -> Result<Vec<Measurement>, BenchError> {
    let stdin = row.stdin(root)?;
    if stdin != fixture.as_bytes() {
        return Err(stab_runner_error(
            &row.id,
            "manifest input no longer matches the source-owned sampling fixture",
        ));
    }
    let args = row
        .argv_tokens()
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let program = build_stab_cli_binary(root, profile, &row.id)?;
    let expected = frozen_pre_a4_cli_witness(&row.id).ok_or_else(|| {
        stab_runner_error(
            &row.id,
            "process-equivalent sampling row has no frozen pre-A4 output witness",
        )
    })?;

    let preflight = run_process(&program, &args, &stdin, &root.path, true)?;
    check_success(&program, &preflight)?;
    let actual = OutputWitness::from_bytes(&preflight.stdout);
    if actual != expected {
        return Err(stab_runner_error(
            &row.id,
            format!(
                "sample process preflight changed from the clean pre-A4 witness: expected {expected:?}, got {actual:?}"
            ),
        ));
    }

    Ok(vec![measure_stab_iterations(
        measurement_name,
        SAMPLE_CLI_PROCESS_LAUNCHES_PER_MEASUREMENT,
        || {
            let output = run_process(&program, &args, &stdin, &root.path, false)?;
            check_success(&program, &output)?;
            black_box((output.status, output.parent_observed_peak_rss_bytes));
            Ok(())
        },
    )?])
}

#[cfg(not(test))]
fn build_stab_cli_binary(
    root: &RepoRoot,
    profile: &str,
    row_id: &str,
) -> Result<PathBuf, BenchError> {
    let profile_path = Path::new(profile);
    if profile.is_empty()
        || profile_path.components().count() != 1
        || !matches!(profile_path.components().next(), Some(Component::Normal(_)))
    {
        return Err(stab_runner_error(
            row_id,
            format!("unsafe Cargo profile name {profile:?}"),
        ));
    }
    run_checked_status(
        "cargo",
        [
            "build",
            "--quiet",
            "--profile",
            profile,
            "--package",
            "stab-cli",
            "--bin",
            "stab",
        ],
        &root.path,
    )?;
    let profile_dir = if profile == "dev" { "debug" } else { profile };
    let binary = root
        .path
        .join("target")
        .join(profile_dir)
        .join(format!("stab{}", std::env::consts::EXE_SUFFIX));
    if !binary.is_file() {
        return Err(stab_runner_error(
            row_id,
            format!(
                "Cargo completed without producing the expected Stab CLI binary {}",
                binary.display()
            ),
        ));
    }
    Ok(binary)
}

fn frozen_pre_a4_cli_witness(row_id: &str) -> Option<OutputWitness> {
    match row_id {
        "m8-sample-primary-repetition-contract" => Some(OutputWitness {
            bytes: 128,
            digest: 0xc6a0_1d09_04c3_59a5,
        }),
        "m8-sample-primary-rotated-surface-contract" => Some(OutputWitness {
            bytes: 320,
            digest: 0x0c81_72cc_5f87_aa84,
        }),
        "m8-sample-primary-unrotated-surface-contract" => Some(OutputWitness {
            bytes: 448,
            digest: 0x5298_992f_11e2_32d7,
        }),
        "m8-sample-high-repeat-contract" => Some(OutputWitness {
            bytes: 64,
            digest: 0x5e27_5dae_3600_d85b,
        }),
        _ => None,
    }
}

fn sample_circuit(row_id: &str, fixture: &str) -> Result<Circuit, BenchError> {
    Circuit::from_stim_str(fixture).map_err(|error| stab_runner_error(row_id, error))
}

fn compile_plan(row_id: &str, circuit: &Circuit) -> Result<SamplingPlan, BenchError> {
    SamplingCompiler::new()
        .compile(circuit)
        .map_err(|error| stab_runner_error(row_id, error))
}

fn sampling_session(
    row_id: &str,
    plan: &SamplingPlan,
    seed: u64,
) -> Result<SamplingSession, BenchError> {
    plan.session(RandomPolicy::Seeded(Seed::new(seed)))
        .map_err(|error| stab_runner_error(row_id, error))
}

fn shot_count(row_id: &str, shots: usize) -> Result<ShotCount, BenchError> {
    ShotCount::try_from(shots).map_err(|error| stab_runner_error(row_id, error))
}

fn sample_encoding_batch(
    row_id: &str,
    bits_per_shot: usize,
) -> Result<BitPlane64Batch, BenchError> {
    let mut batch = BitPlane64Batch::zeros(64, bits_per_shot)
        .map_err(|error| stab_runner_error(row_id, error))?;
    for bit_index in 0..bits_per_shot {
        let word = (0..64).fold(0_u64, |word, shot_index| {
            if (shot_index + bit_index * 3).is_multiple_of(5) {
                word | (1_u64 << shot_index)
            } else {
                word
            }
        });
        batch
            .copy_plane_from_word(bit_index, word)
            .map_err(|error| stab_runner_error(row_id, error))?;
    }
    Ok(batch)
}

fn sample_plan_b8(
    row_id: &str,
    plan: &SamplingPlan,
    shots: usize,
    seed: u64,
) -> Result<usize, BenchError> {
    let mut session = sampling_session(row_id, plan, seed)?;
    let mut sink = MeasurementCodecSink::try_new(RecordFormat::B8, plan.measurement_width())
        .map_err(|error| stab_runner_error(row_id, error))?;
    session
        .run(shot_count(row_id, shots)?, &mut sink)
        .map_err(|error| stab_runner_error(row_id, error))?;
    sink.into_bytes()
        .map(|bytes| bytes.len())
        .map_err(|error| stab_runner_error(row_id, error))
}

#[cfg(test)]
fn run_sample_cli(
    row: &BenchmarkRow,
    fixture: &str,
    shots: usize,
    output_format: &str,
    expected: OutputWitness,
) -> Result<(), BenchError> {
    let shots = shots.to_string();
    let args = [
        "stab",
        "sample",
        "--shots",
        shots.as_str(),
        "--out_format",
        output_format,
        "--seed",
        "5",
    ];
    let mut output = WitnessWriter::default();
    let mut stderr = Vec::new();
    let status = stab_cli::run_from(args, fixture.as_bytes(), &mut output, &mut stderr);
    if status != 0 {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "stab-cli sample failed with status {status}: {}",
                String::from_utf8_lossy(&stderr)
            ),
        });
    }
    let actual = output.witness();
    if actual != expected {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "stab-cli sample output witness changed: expected {expected:?}, got {actual:?}"
            ),
        });
    }
    black_box(actual);
    Ok(())
}

#[cfg(test)]
fn sample_cli_witness(
    row: &BenchmarkRow,
    fixture: &str,
    shots: usize,
    output_format: &str,
) -> Result<OutputWitness, BenchError> {
    let shots = shots.to_string();
    let args = [
        "stab",
        "sample",
        "--shots",
        shots.as_str(),
        "--out_format",
        output_format,
        "--seed",
        "5",
    ];
    let mut output = WitnessWriter::default();
    let mut stderr = Vec::new();
    let status = stab_cli::run_from(args, fixture.as_bytes(), &mut output, &mut stderr);
    if status != 0 {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "stab-cli sample witness failed with status {status}: {}",
                String::from_utf8_lossy(&stderr)
            ),
        });
    }
    Ok(output.witness())
}

#[derive(Default)]
struct BoundaryWitnessSink {
    digest: u64,
}

impl MeasurementSink for BoundaryWitnessSink {
    type Error = &'static str;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        self.digest = self
            .digest
            .rotate_left(7)
            .wrapping_add(batch.shot_count() as u64)
            .wrapping_add((batch.width().get() as u64).rotate_left(17));
        if batch.shot_count() != 0 && batch.width().get() != 0 {
            let first = batch
                .get(0, 0)
                .ok_or("sampling benchmark could not read its first witness bit")?;
            let last = batch
                .get(batch.shot_count() - 1, batch.width().get() - 1)
                .ok_or("sampling benchmark could not read its last witness bit")?;
            self.digest ^= u64::from(first) | (u64::from(last) << 1);
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Default)]
struct DigestMeasurementSink {
    digest: u64,
}

impl MeasurementSink for DigestMeasurementSink {
    type Error = &'static str;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        for shot in 0..batch.shot_count() {
            for bit in 0..batch.width().get() {
                let value = batch
                    .get(shot, bit)
                    .ok_or("sampling benchmark received an invalid typed batch view")?;
                self.digest = self.digest.rotate_left(1) ^ u64::from(value);
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OutputWitness {
    bytes: usize,
    digest: u64,
}

impl OutputWitness {
    #[cfg(not(test))]
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut digest = 0xcbf2_9ce4_8422_2325_u64;
        for byte in bytes {
            digest ^= u64::from(*byte);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self {
            bytes: bytes.len(),
            digest,
        }
    }
}

#[cfg(test)]
#[derive(Debug)]
struct WitnessWriter {
    bytes: usize,
    digest: u64,
}

#[cfg(test)]
impl Default for WitnessWriter {
    fn default() -> Self {
        Self {
            bytes: 0,
            digest: 0xcbf2_9ce4_8422_2325,
        }
    }
}

#[cfg(test)]
impl WitnessWriter {
    const fn witness(&self) -> OutputWitness {
        OutputWitness {
            bytes: self.bytes,
            digest: self.digest,
        }
    }
}

#[cfg(test)]
impl Write for WitnessWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("sample benchmark output byte count overflowed"))?;
        for byte in bytes {
            self.digest ^= u64::from(*byte);
            self.digest = self.digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn measure_reader_record(denominator: usize) -> Vec<bool> {
    (0..MEASURE_READER_BITS)
        .map(|index| (index * 17 + 3) % denominator == 0)
        .collect()
}

fn measure_reader_denominator_from_name(name: &str) -> Option<usize> {
    name.rsplit_once("_per")
        .and_then(|(_, denominator)| denominator.parse::<usize>().ok())
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
    text.push_str("SWAP 0 1\n");
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
mod tests;
