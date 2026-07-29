use std::ffi::OsString;
use std::hint::black_box;

use stab_core::CompiledDemSampler;
use stab_engine::{DemSamplingCompiler, RandomPolicy, Seed, ShotCount};
use stab_model::DetectorErrorModel;

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;
use crate::root::RepoRoot;

use super::{
    batch_sinks::{ByteDigestWriter, DemDigestSink, OutputWitness, u64_sequence_digest},
    cli_process::run_stab_cli_process_row,
    measure_stab_iterations, measure_stab_iterations_with_postprocess_and_memory_operation,
    measure_stab_preflighted_compile_and_release, stab_runner_error,
};

const SAMPLE_DEM_NOISY_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/sample_dem_noisy.dem");
#[cfg(not(test))]
const M11_SAMPLE_DEM_SHOTS: usize = 1024;
#[cfg(test)]
const M11_SAMPLE_DEM_SHOTS: usize = 4;
#[cfg(not(test))]
const M11_PTB64_SHOTS: usize = 1024;
#[cfg(test)]
const M11_PTB64_SHOTS: usize = 64;
#[cfg(not(test))]
const M11_CONTRACT_SHOTS: usize = 64;
#[cfg(test)]
const M11_CONTRACT_SHOTS: usize = 2;
#[cfg(not(test))]
const M11_CONTRACT_ITERATIONS: usize = 8;
#[cfg(test)]
const M11_CONTRACT_ITERATIONS: usize = 1;
#[cfg(not(test))]
const DEM_DETECTOR_FIRST_WITNESS: (u64, u64, u64) =
    (10_655_702_427_225_054_044, 0, M11_SAMPLE_DEM_SHOTS as u64);
#[cfg(test)]
const DEM_DETECTOR_FIRST_WITNESS: (u64, u64, u64) =
    (6_865_066_094_078_859_835, 0, M11_SAMPLE_DEM_SHOTS as u64);
#[cfg(not(test))]
const DEM_SAMPLED_ERROR_FIRST_WITNESS: (u64, u64, u64) = (
    10_655_702_427_225_054_044,
    19_048_448_694_863_617,
    M11_SAMPLE_DEM_SHOTS as u64,
);
#[cfg(test)]
const DEM_SAMPLED_ERROR_FIRST_WITNESS: (u64, u64, u64) = (
    6_865_066_094_078_859_835,
    15_198_978_754_495_808_650,
    M11_SAMPLE_DEM_SHOTS as u64,
);
const DEM_DETECTOR_SEQUENCE_DOMAIN: &[u8] = b"a5-m11-dem-detector-session-v1";
const DEM_SAMPLED_ERROR_SEQUENCE_DOMAIN: &[u8] = b"a5-m11-dem-sampled-error-session-v1";
#[cfg(not(test))]
const DEM_DETECTOR_SEQUENCE_DIGEST: &str =
    "95f94705af778f2db4e783c6506929acf9a641215e0cd97655c806ac3bed1578";
#[cfg(test)]
const DEM_DETECTOR_SEQUENCE_DIGEST: &str =
    "e999124aa02407f776aa3555c3524b4e8ae55928f10e0a8508a7ce2b5592fe19";
#[cfg(not(test))]
const DEM_SAMPLED_ERROR_SEQUENCE_DIGEST: &str =
    "39feb406cd1556b729f975362b4f9fa29ded61e87ee586fad6a4dab41e7eb87c";
#[cfg(test)]
const DEM_SAMPLED_ERROR_SEQUENCE_DIGEST: &str =
    "296dcf975c6201d588d919028f536d4fe0a331ac167894e2938dcf90e7885e6d";
#[cfg(not(test))]
const DEM_REPLAY_WITNESS: (u64, u64, u64) = (
    6_915_694_862_520_406_541,
    13_241_288_651_169_304_623,
    M11_SAMPLE_DEM_SHOTS as u64,
);
#[cfg(test)]
const DEM_REPLAY_WITNESS: (u64, u64, u64) = (
    13_834_261_287_016_441_736,
    7_322_621_584_440_746_172,
    M11_SAMPLE_DEM_SHOTS as u64,
);
#[cfg(not(test))]
const DENSE_DETECTOR_COUNT: usize = 128;
#[cfg(test)]
const DENSE_DETECTOR_COUNT: usize = 16;
const HIGH_DETECTOR_COUNT: usize = 4096;

pub(super) fn run_dem_sampling_compare_row(
    root: &RepoRoot,
    profile: &str,
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    if let Some(measurement_name) = process_cli_measurement_name(&row.id) {
        return run_process_cli_row(root, profile, row, measurement_name).map(Some);
    }
    match row.id.as_str() {
        "m11-dem-sampler" => run_compiled_dem_sampler_row(row).map(Some),
        "m11-dem-batch-phases" => run_dem_phase_row(row).map(Some),
        _ => Ok(None),
    }
}

pub(super) fn process_cli_measurement_name(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m11-sample-dem-cli" => Some("stab_sample_dem_cli_1024_zero_one"),
        "m11-sample-dem-sparse-contract" => Some("stab_sample_dem_sparse_b8"),
        "m11-sample-dem-dense-contract" => Some("stab_sample_dem_dense_b8"),
        "m11-sample-dem-repeated-contract" => Some("stab_sample_dem_repeated_b8"),
        "m11-sample-dem-high-detector-contract" => Some("stab_sample_dem_high_detector_b8"),
        _ => None,
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
        ("m11-dem-batch-phases", "stab_dem_plan_compile_and_release_surface_like") => {
            Some((1.0, "plans/s"))
        }
        ("m11-dem-batch-phases", "stab_dem_session_detector_only") => {
            Some((M11_SAMPLE_DEM_SHOTS as f64, "shots/s"))
        }
        ("m11-dem-batch-phases", "stab_dem_session_with_sampled_errors") => {
            Some((M11_SAMPLE_DEM_SHOTS as f64, "shots/s"))
        }
        ("m11-dem-batch-phases", "stab_dem_session_replay") => {
            Some((M11_SAMPLE_DEM_SHOTS as f64, "shots/s"))
        }
        ("m11-dem-batch-phases", "stab_sample_dem_cli_ptb64_routing") => {
            Some((M11_PTB64_SHOTS as f64, "shots/s"))
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
            "report-only: Stab and pinned Stim execute the same seeded sample_dem command as bounded subprocesses with identical input, launch count, discarded timed stdout, and independent untimed frozen output witnesses",
        ),
        "m11-sample-dem-sparse-contract" => Some(
            "cli-baseline: Stab and pinned Stim execute the same bounded sample_dem subprocess workload for sparse detector-id b8 output on the same fixture with independent untimed frozen output witnesses",
        ),
        "m11-sample-dem-dense-contract" => Some(
            "cli-baseline: Stab and pinned Stim execute the same bounded sample_dem subprocess workload for dense detector-target b8 output on the same fixture with independent untimed frozen output witnesses",
        ),
        "m11-sample-dem-repeated-contract" => Some(
            "cli-baseline: Stab and pinned Stim execute the same bounded sample_dem subprocess workload for repeated detector-shift b8 output on the same fixture with independent untimed frozen output witnesses",
        ),
        "m11-sample-dem-high-detector-contract" => Some(
            "cli-baseline: Stab and pinned Stim execute the same bounded sample_dem subprocess workload for high detector index b8 output on the same fixture with independent untimed frozen output witnesses",
        ),
        "m11-dem-batch-phases" => Some(
            "report-only: source-owned Stab diagnostics separately measure DEM plan compile-and-release with exact plan-dimension witnesses, detector-only and sampled-error execution with frozen ordered SHA-256 sequences of 64-bit output witnesses, replay with a frozen output witness, and PTB64 CLI routing with a frozen output witness, without claiming a Stim ratio",
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

fn run_process_cli_row(
    root: &RepoRoot,
    profile: &str,
    row: &BenchmarkRow,
    measurement_name: &'static str,
) -> Result<Vec<Measurement>, BenchError> {
    let expected = frozen_process_cli_witness(&row.id).ok_or_else(|| {
        stab_runner_error(
            &row.id,
            "process-equivalent M11 CLI row has no frozen output witness",
        )
    })?;
    run_stab_cli_process_row(root, profile, row, measurement_name, expected)
}

fn run_dem_phase_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let surface_like = surface_like_dem_fixture();
    let model = parse_dem(&row.id, &surface_like)?;
    Ok(vec![
        measure_dem_plan_compile(row, &model)?,
        measure_dem_session_detector_only(row, &model)?,
        measure_dem_session_with_sampled_errors(row, &model)?,
        measure_dem_session_replay(row, &model)?,
        measure_sample_dem_ptb64_routing(
            row,
            "stab_sample_dem_cli_ptb64_routing",
            M11_PTB64_SHOTS,
        )?,
    ])
}

fn frozen_process_cli_witness(row_id: &str) -> Option<OutputWitness> {
    match row_id {
        "m11-sample-dem-cli" => Some(OutputWitness::new(2_048, 0x6812_4a44_9ddc_4ddd)),
        "m11-sample-dem-sparse-contract" => Some(OutputWitness::new(16_448, 0xd0a3_49ec_b8be_ae8a)),
        "m11-sample-dem-dense-contract" => Some(OutputWitness::new(1_024, 0x51d8_8627_df28_7325)),
        "m11-sample-dem-repeated-contract" => {
            Some(OutputWitness::new(1_088, 0xa90f_4905_1fc5_9427))
        }
        "m11-sample-dem-high-detector-contract" => {
            Some(OutputWitness::new(32_768, 0x8f69_55bf_94ec_2325))
        }
        _ => None,
    }
}

fn measure_sample_dem_ptb64_routing(
    row: &BenchmarkRow,
    measurement_name: &'static str,
    shots: usize,
) -> Result<Measurement, BenchError> {
    let args = [
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--shots"),
        OsString::from(shots.to_string()),
        OsString::from("--seed=5"),
        OsString::from("--out_format=ptb64"),
    ];
    let expected = sample_dem_ptb64_witness();
    let preflight = run_sample_dem_cli(args.clone());
    ensure_sample_dem_cli_output(row, expected, &preflight)?;
    black_box(preflight.2.witness());
    let mut timing_state = ();
    measure_stab_iterations_with_postprocess_and_memory_operation(
        measurement_name,
        M11_CONTRACT_ITERATIONS,
        &mut timing_state,
        |_| Ok(run_sample_dem_cli(args.clone())),
        |_, actual| {
            ensure_sample_dem_cli_output(row, expected, &actual)?;
            black_box(actual.2.witness());
            Ok(())
        },
        || {
            let actual = run_sample_dem_cli(args.clone());
            ensure_sample_dem_cli_output(row, expected, &actual)?;
            black_box(actual.2.witness());
            Ok(())
        },
    )
}

#[cfg(not(test))]
const fn sample_dem_ptb64_witness() -> OutputWitness {
    OutputWitness::new(128, 0xa1c2_c833_a4ca_07f2)
}

#[cfg(test)]
const fn sample_dem_ptb64_witness() -> OutputWitness {
    OutputWitness::new(8, 0x095a_8dff_6d98_bcea)
}

type SampleDemCliOutput = (i32, Vec<u8>, ByteDigestWriter);

fn run_sample_dem_cli(args: [OsString; 6]) -> SampleDemCliOutput {
    let mut stdout = ByteDigestWriter::default();
    let mut stderr = Vec::new();
    let status = stab_cli::run_from(
        args,
        SAMPLE_DEM_NOISY_FIXTURE.as_bytes(),
        &mut stdout,
        &mut stderr,
    );
    (status, stderr, stdout)
}

fn ensure_sample_dem_cli_output(
    row: &BenchmarkRow,
    expected: OutputWitness,
    actual: &SampleDemCliOutput,
) -> Result<(), BenchError> {
    if actual.0 != 0 {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "stab-cli sample_dem failed with status {}: {}",
                actual.0,
                String::from_utf8_lossy(&actual.1)
            ),
        });
    }
    let actual_witness = actual.2.witness();
    if actual_witness != expected {
        return Err(stab_runner_error(
            &row.id,
            format!(
                "sample_dem PTB64 output changed: expected {expected:?}, got {actual_witness:?}"
            ),
        ));
    }
    Ok(())
}

fn measure_dem_plan_compile(
    row: &BenchmarkRow,
    model: &DetectorErrorModel,
) -> Result<Measurement, BenchError> {
    measure_stab_preflighted_compile_and_release(
        "stab_dem_plan_compile_and_release_surface_like",
        M11_CONTRACT_ITERATIONS,
        || {
            DemSamplingCompiler::new()
                .compile(black_box(model))
                .map_err(|error| stab_runner_error(&row.id, error))
        },
        |plan| {
            ensure_dem_plan_witness(
                row,
                (
                    plan.detector_width().get(),
                    plan.observable_width().get(),
                    plan.sampled_error_width().get(),
                ),
            )
        },
        || {
            DemSamplingCompiler::new()
                .compile(black_box(model))
                .map_err(|error| stab_runner_error(&row.id, error))
        },
        || {
            DemSamplingCompiler::new()
                .compile(black_box(model))
                .map_err(|error| stab_runner_error(&row.id, error))
        },
    )
}

fn measure_dem_session_detector_only(
    row: &BenchmarkRow,
    model: &DetectorErrorModel,
) -> Result<Measurement, BenchError> {
    let plan = DemSamplingCompiler::new()
        .compile(model)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut preflight_session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut preflight_sink = DemDigestSink::default();
    let preflight_summary = preflight_session
        .run(
            ShotCount::new(M11_SAMPLE_DEM_SHOTS as u64),
            &mut preflight_sink,
        )
        .map_err(|error| stab_runner_error(&row.id, error))?;
    ensure_dem_phase_witness(
        row,
        "DEM detector-only preflight",
        DEM_DETECTOR_FIRST_WITNESS,
        preflight_summary.committed_shots().get(),
        preflight_sink.witness(),
    )?;
    let session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let sink = DemDigestSink::default();
    let mut memory_session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut memory_sink = DemDigestSink::default();
    let mut timing_state = (session, sink, Vec::with_capacity(M11_CONTRACT_ITERATIONS));
    let measurement = measure_stab_iterations_with_postprocess_and_memory_operation(
        "stab_dem_session_detector_only",
        M11_CONTRACT_ITERATIONS,
        &mut timing_state,
        |state| {
            state
                .0
                .run(ShotCount::new(M11_SAMPLE_DEM_SHOTS as u64), &mut state.1)
                .map_err(|error| stab_runner_error(&row.id, error))
        },
        |state, summary| {
            let actual = state.1.witness();
            ensure_dem_shot_count(
                row,
                "DEM detector-only session",
                summary.committed_shots().get(),
                actual.2,
            )?;
            state.2.push([actual.0, actual.1, actual.2]);
            state.1.reset();
            black_box(actual);
            Ok(())
        },
        || {
            let summary = memory_session
                .run(
                    ShotCount::new(M11_SAMPLE_DEM_SHOTS as u64),
                    &mut memory_sink,
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let actual = memory_sink.witness();
            ensure_dem_phase_witness(
                row,
                "DEM detector-only memory operation",
                DEM_DETECTOR_FIRST_WITNESS,
                summary.committed_shots().get(),
                actual,
            )?;
            black_box(actual);
            Ok(())
        },
    )?;
    ensure_dem_sequence_witness(
        row,
        "DEM detector-only session",
        DEM_DETECTOR_SEQUENCE_DOMAIN,
        DEM_DETECTOR_SEQUENCE_DIGEST,
        &timing_state.2,
    )?;
    Ok(measurement)
}

fn measure_dem_session_with_sampled_errors(
    row: &BenchmarkRow,
    model: &DetectorErrorModel,
) -> Result<Measurement, BenchError> {
    let plan = DemSamplingCompiler::new()
        .compile(model)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut preflight_session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut preflight_sink = DemDigestSink::default();
    let preflight_summary = preflight_session
        .run_with_sampled_errors(
            ShotCount::new(M11_SAMPLE_DEM_SHOTS as u64),
            &mut preflight_sink,
        )
        .map_err(|error| stab_runner_error(&row.id, error))?;
    ensure_dem_phase_witness(
        row,
        "DEM sampled-error preflight",
        DEM_SAMPLED_ERROR_FIRST_WITNESS,
        preflight_summary.committed_shots().get(),
        preflight_sink.witness(),
    )?;
    let session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let sink = DemDigestSink::default();
    let mut memory_session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut memory_sink = DemDigestSink::default();
    let mut timing_state = (session, sink, Vec::with_capacity(M11_CONTRACT_ITERATIONS));
    let measurement = measure_stab_iterations_with_postprocess_and_memory_operation(
        "stab_dem_session_with_sampled_errors",
        M11_CONTRACT_ITERATIONS,
        &mut timing_state,
        |state| {
            state
                .0
                .run_with_sampled_errors(ShotCount::new(M11_SAMPLE_DEM_SHOTS as u64), &mut state.1)
                .map_err(|error| stab_runner_error(&row.id, error))
        },
        |state, summary| {
            let actual = state.1.witness();
            ensure_dem_shot_count(
                row,
                "DEM sampled-error session",
                summary.committed_shots().get(),
                actual.2,
            )?;
            state.2.push([actual.0, actual.1, actual.2]);
            state.1.reset();
            black_box(actual);
            Ok(())
        },
        || {
            let summary = memory_session
                .run_with_sampled_errors(
                    ShotCount::new(M11_SAMPLE_DEM_SHOTS as u64),
                    &mut memory_sink,
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let actual = memory_sink.witness();
            ensure_dem_phase_witness(
                row,
                "DEM sampled-error memory operation",
                DEM_SAMPLED_ERROR_FIRST_WITNESS,
                summary.committed_shots().get(),
                actual,
            )?;
            black_box(actual);
            Ok(())
        },
    )?;
    ensure_dem_sequence_witness(
        row,
        "DEM sampled-error session",
        DEM_SAMPLED_ERROR_SEQUENCE_DOMAIN,
        DEM_SAMPLED_ERROR_SEQUENCE_DIGEST,
        &timing_state.2,
    )?;
    Ok(measurement)
}

fn measure_dem_session_replay(
    row: &BenchmarkRow,
    model: &DetectorErrorModel,
) -> Result<Measurement, BenchError> {
    let plan = DemSamplingCompiler::new()
        .compile(model)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let replay_records = (0..M11_SAMPLE_DEM_SHOTS)
        .map(|shot| {
            (0..plan.error_count())
                .map(|error| (shot + error * 3) % 17 == 0)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut preflight_session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut preflight_sink = DemDigestSink::default();
    let preflight_summary = preflight_session
        .replay(&replay_records, &mut preflight_sink)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    ensure_dem_phase_witness(
        row,
        "DEM replay preflight",
        DEM_REPLAY_WITNESS,
        preflight_summary.committed_shots().get(),
        preflight_sink.witness(),
    )?;
    let session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let sink = DemDigestSink::default();
    let mut memory_session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut memory_sink = DemDigestSink::default();
    let mut timing_state = (session, sink);
    measure_stab_iterations_with_postprocess_and_memory_operation(
        "stab_dem_session_replay",
        M11_CONTRACT_ITERATIONS,
        &mut timing_state,
        |state| {
            state
                .0
                .replay(&replay_records, &mut state.1)
                .map_err(|error| stab_runner_error(&row.id, error))
        },
        |state, summary| {
            let actual = state.1.witness();
            ensure_dem_phase_witness(
                row,
                "DEM replay session",
                DEM_REPLAY_WITNESS,
                summary.committed_shots().get(),
                actual,
            )?;
            state.1.reset();
            black_box(actual);
            Ok(())
        },
        || {
            let summary = memory_session
                .replay(&replay_records, &mut memory_sink)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let actual = memory_sink.witness();
            ensure_dem_phase_witness(
                row,
                "DEM replay memory operation",
                DEM_REPLAY_WITNESS,
                summary.committed_shots().get(),
                actual,
            )?;
            black_box(actual);
            Ok(())
        },
    )
}

fn ensure_dem_phase_witness(
    row: &BenchmarkRow,
    phase: &str,
    expected: (u64, u64, u64),
    committed_shots: u64,
    actual: (u64, u64, u64),
) -> Result<(), BenchError> {
    if committed_shots == expected.2 && actual == expected {
        return Ok(());
    }
    Err(stab_runner_error(
        &row.id,
        format!(
            "{phase} witness changed: expected detection/error/shots {expected:?}, got {actual:?} with {committed_shots} committed shots"
        ),
    ))
}

fn ensure_dem_plan_witness(
    row: &BenchmarkRow,
    actual: (usize, usize, usize),
) -> Result<(), BenchError> {
    let expected = (DENSE_DETECTOR_COUNT, 1, DENSE_DETECTOR_COUNT);
    if actual == expected {
        return Ok(());
    }
    Err(stab_runner_error(
        &row.id,
        format!("DEM compile plan dimensions changed: expected {expected:?}, got {actual:?}"),
    ))
}

fn ensure_dem_shot_count(
    row: &BenchmarkRow,
    phase: &str,
    committed_shots: u64,
    witnessed_shots: u64,
) -> Result<(), BenchError> {
    if committed_shots == M11_SAMPLE_DEM_SHOTS as u64
        && witnessed_shots == M11_SAMPLE_DEM_SHOTS as u64
    {
        return Ok(());
    }
    Err(stab_runner_error(
        &row.id,
        format!(
            "{phase} committed {committed_shots} shots and witnessed {witnessed_shots} instead of {M11_SAMPLE_DEM_SHOTS}"
        ),
    ))
}

fn ensure_dem_sequence_witness(
    row: &BenchmarkRow,
    phase: &str,
    domain: &[u8],
    expected: &str,
    witnesses: &[[u64; 3]],
) -> Result<(), BenchError> {
    let actual = u64_sequence_digest(domain, witnesses);
    if actual == expected {
        return Ok(());
    }
    Err(stab_runner_error(
        &row.id,
        format!("{phase} ordered witness digest changed: expected {expected}, got {actual}"),
    ))
}

fn parse_dem(row_id: &str, fixture: &str) -> Result<DetectorErrorModel, BenchError> {
    DetectorErrorModel::from_dem_str(fixture).map_err(|error| stab_runner_error(row_id, error))
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

    use std::path::Path;

    use super::{compare_note, measurement_work, run_dem_sampling_compare_row};
    use crate::{manifest::BenchmarkManifest, root::RepoRoot};

    #[test]
    fn m11_benchmark_rows_have_stab_compare_runners() {
        let root = RepoRoot::resolve(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("repository root"),
        )
        .expect("resolve repository root");
        let manifest = BenchmarkManifest::read(&root).expect("read benchmark manifest");
        for (id, expected_measurements) in [
            (
                "m11-dem-sampler",
                &["stab_dem_sampler_sample_surface_like_1024"][..],
            ),
            (
                "m11-sample-dem-cli",
                &["stab_sample_dem_cli_1024_zero_one"][..],
            ),
            (
                "m11-sample-dem-sparse-contract",
                &["stab_sample_dem_sparse_b8"][..],
            ),
            (
                "m11-sample-dem-dense-contract",
                &["stab_sample_dem_dense_b8"][..],
            ),
            (
                "m11-sample-dem-repeated-contract",
                &["stab_sample_dem_repeated_b8"][..],
            ),
            (
                "m11-sample-dem-high-detector-contract",
                &["stab_sample_dem_high_detector_b8"][..],
            ),
            (
                "m11-dem-batch-phases",
                &[
                    "stab_dem_plan_compile_and_release_surface_like",
                    "stab_dem_session_detector_only",
                    "stab_dem_session_with_sampled_errors",
                    "stab_dem_session_replay",
                    "stab_sample_dem_cli_ptb64_routing",
                ][..],
            ),
        ] {
            let row = manifest
                .rows
                .iter()
                .find(|row| row.id == id)
                .expect("manifest row");
            let measurements = run_dem_sampling_compare_row(&root, "release", row)
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
