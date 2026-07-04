use std::ffi::OsString;
use std::hint::black_box;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use stab_core::{
    Circuit, CircuitError, CodeDistance, CompiledSampler, DemDetectorId, DetectingRegionOptions,
    DetectionConversionOptions, DetectionObservableOutputMode, MissingDetectorOptions, Probability,
    RepetitionCodeParams, RepetitionCodeTask, RoundCount, SampleFormat, circuit_detecting_regions,
    circuit_with_inlined_feedback, convert_measurements_to_detection_events,
    generate_repetition_code_circuit, measurement_record_count, missing_detectors,
    result_formats::{read_records, write_records},
    sample_detection_events, try_for_each_sampled_detection_event, write_detection_records,
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
const M2D_SWEEP_MEASUREMENTS: &[u8] =
    include_bytes!("../../../../oracle/fixtures/inputs/m2d_sweep_measurements.01");
const M2D_SWEEP_B8_MEASUREMENTS: &[u8] =
    include_bytes!("../../../../benchmarks/fixtures/m9_m2d_sweep_b8_measurements.b8");
const M2D_RAN_WITHOUT_FEEDBACK_MEASUREMENTS: &[u8] =
    include_bytes!("../../../../oracle/fixtures/inputs/m2d_ran_without_feedback_measurements.01");
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
#[cfg(not(test))]
const UTILITY_BATCH: usize = 4096;
#[cfg(test)]
const UTILITY_BATCH: usize = 2;
const MISSING_DETECTOR_BASIC_CASES: usize = 10;
const MISSING_DETECTOR_BASIC_SUGGESTIONS: usize = 4;
const DETECTING_REGIONS_PER_CASE: usize = 2;
const DETECTING_REGIONS_SIMPLE: &str = "H 0\n\
                                        TICK\n\
                                        CX 0 1\n\
                                        TICK\n\
                                        MXX 0 1\n\
                                        DETECTOR rec[-1]\n";
const FEEDBACK_INLINE_MPP: &str = "RX 0\n\
                                  RY 1\n\
                                  RZ 2\n\
                                  MPP X0*Y1*Z2 Z5\n\
                                  CX rec[-2] 3\n\
                                  M 3\n\
                                  DETECTOR rec[-1]\n";
const DETECT_SWEEP_DEFAULT_FALSE: &str = "H 0\n\
                                         CX sweep[0] 0\n\
                                         M 0\n\
                                         DETECTOR rec[-1]\n";

pub(super) fn run_detection_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
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
        "m9-m2d-sweep-01-cli" => run_m2d_cli_row(
            row,
            "stab_m2d_sweep_01_dets",
            m2d_sweep_args(false),
            M2D_SWEEP_MEASUREMENTS,
            None,
        )
        .map(Some),
        "m9-m2d-sweep-b8-cli" => run_m2d_cli_row(
            row,
            "stab_m2d_sweep_b8",
            m2d_sweep_b8_args(),
            M2D_SWEEP_B8_MEASUREMENTS,
            None,
        )
        .map(Some),
        "m9-m2d-sweep-obs-out-cli" => run_m2d_cli_row(
            row,
            "stab_m2d_sweep_obs_out",
            m2d_sweep_args(true),
            M2D_SWEEP_MEASUREMENTS,
            Some(obs_out_path()),
        )
        .map(Some),
        "m9-m2d-ran-without-feedback-cli" => run_m2d_cli_row(
            row,
            "stab_m2d_ran_without_feedback",
            m2d_ran_without_feedback_args(),
            M2D_RAN_WITHOUT_FEEDBACK_MEASUREMENTS,
            None,
        )
        .map(Some),
        "m9-detecting-regions-basic-batch" => run_detecting_regions_basic_batch(row).map(Some),
        "m9-missing-detectors-basic-batch" => run_missing_detectors_basic_batch(row).map(Some),
        "m9-feedback-inline-mpp-batch" => run_feedback_inline_mpp_batch(row).map(Some),
        "m9-detect-primary-matrix-contract" => run_primary_detect_row(row).map(Some),
        "m9-m2d-primary-matrix-contract" => run_primary_m2d_row(row).map(Some),
        "pf3-m2d-sweep-b8" => run_m2d_cli_row(
            row,
            "stab_pf3_m2d_sweep_b8",
            m2d_sweep_b8_args(),
            M2D_SWEEP_B8_MEASUREMENTS,
            None,
        )
        .map(Some),
        "pf3-detect-sweep-sampling" => run_detect_sweep_sampling_row(row).map(Some),
        _ => Ok(None),
    }
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m9-m2d-text-cli", "stab_m2d_dets") | ("m9-m2d-bitpacked-contract", "stab_m2d_b8") => {
            Some((2.0, "shots/s"))
        }
        ("m9-m2d-sweep-01-cli", "stab_m2d_sweep_01_dets")
        | ("m9-m2d-sweep-obs-out-cli", "stab_m2d_sweep_obs_out") => Some((4.0, "shots/s")),
        ("m9-m2d-sweep-b8-cli", "stab_m2d_sweep_b8") => Some((5.0, "shots/s")),
        ("pf3-m2d-sweep-b8", "stab_pf3_m2d_sweep_b8") => Some((5.0, "shots/s")),
        ("m9-m2d-ran-without-feedback-cli", "stab_m2d_ran_without_feedback") => {
            Some((6.0, "shots/s"))
        }
        ("m9-detecting-regions-basic-batch", "stab_detecting_regions_basic_cases") => {
            Some((UTILITY_BATCH as f64, "cases/s"))
        }
        ("m9-detecting-regions-basic-batch", "stab_detecting_regions_basic_regions") => Some((
            (UTILITY_BATCH * DETECTING_REGIONS_PER_CASE) as f64,
            "regions/s",
        )),
        ("m9-missing-detectors-basic-batch", "stab_missing_detectors_basic_cases") => Some((
            (UTILITY_BATCH * MISSING_DETECTOR_BASIC_CASES) as f64,
            "cases/s",
        )),
        ("m9-missing-detectors-basic-batch", "stab_missing_detectors_basic_suggestions") => Some((
            (UTILITY_BATCH * MISSING_DETECTOR_BASIC_SUGGESTIONS) as f64,
            "suggestions/s",
        )),
        ("m9-feedback-inline-mpp-batch", "stab_feedback_inline_mpp_transforms") => {
            Some((UTILITY_BATCH as f64, "transforms/s"))
        }
        ("m9-detect-text-cli", "stab_detect_1024_dets")
        | ("m9-detect-bitpacked-cli", "stab_detect_1024_b8") => {
            Some((DETECT_SHOTS as f64, "shots/s"))
        }
        ("pf3-detect-sweep-sampling", "stab_detect_sweep_default_false") => {
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
        "m9-detect-text-cli" | "m9-detect-bitpacked-cli" => Some(
            "report-only: Stab measures in-process detector sampling plus result writing for the public detect contract",
        ),
        "m9-m2d-text-cli" => Some(
            "report-only: Stab measures in-process measurement-to-detection conversion plus result writing",
        ),
        "m9-m2d-bitpacked-contract" => Some(
            "cli-baseline: Stab measures in-process measurement-to-detection conversion with b8 output against pinned Stim m2d on the same fixture",
        ),
        "m9-m2d-sweep-01-cli" => Some(
            "report-only: Stab measures in-process public m2d --sweep text conversion against a pinned-Stim-compatible command shape",
        ),
        "m9-m2d-sweep-b8-cli" => Some(
            "report-only: Stab measures in-process public m2d --sweep packed b8 conversion; threshold ownership awaits repeated probe evidence",
        ),
        "m9-m2d-sweep-obs-out-cli" => Some(
            "report-only: Stab measures in-process public m2d --sweep observable side-output routing; threshold ownership awaits repeated probe evidence",
        ),
        "m9-m2d-ran-without-feedback-cli" => Some(
            "report-only: Stab measures in-process public m2d --ran_without_feedback conversion; threshold ownership awaits repeated probe evidence",
        ),
        "m9-detecting-regions-basic-batch" => Some(
            "report-only: Stab measures the Rust detecting-regions utility subset without a faithful pinned Stim CLI timing ratio",
        ),
        "m9-missing-detectors-basic-batch" => Some(
            "report-only: Stab measures the Rust basic missing-detectors utility subset without a faithful pinned Stim CLI timing ratio",
        ),
        "m9-feedback-inline-mpp-batch" => Some(
            "report-only: Stab measures the Rust MPP feedback-inlining utility subset without a faithful pinned Stim CLI timing ratio",
        ),
        "m9-detect-primary-matrix-contract" => Some(
            "cli-baseline: Stab detects the source-owned generated repetition-code d3/r3 fixture with b8 output against pinned Stim detect on the same fixture",
        ),
        "m9-m2d-primary-matrix-contract" => Some(
            "cli-baseline: Stab converts source-owned generated repetition-code d3/r3 measurement records to b8 detection events against pinned Stim m2d on the same fixture",
        ),
        "pf3-m2d-sweep-b8" => Some(
            "report-only: Stab measures the public m2d --sweep packed b8 path using the source-owned M9 sweep fixture; threshold ownership awaits repeated probe evidence",
        ),
        "pf3-detect-sweep-sampling" => Some(
            "report-only: Stab measures the Rust sweep-conditioned detection sampler using omitted all-false sweep bits; no faithful pinned Stim CLI ratio is claimed for this partial PF3 surface",
        ),
        _ => None,
    }
}

fn run_detecting_regions_basic_batch(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![
        measure_detecting_regions_basic(row, "stab_detecting_regions_basic_cases")?,
        measure_detecting_regions_basic(row, "stab_detecting_regions_basic_regions")?,
    ])
}

fn measure_detecting_regions_basic(
    row: &BenchmarkRow,
    measurement_name: &'static str,
) -> Result<Measurement, BenchError> {
    let circuit = parse_circuit(&row.id, DETECTING_REGIONS_SIMPLE)?;
    let detector = DemDetectorId::try_new(0).map_err(|error| stab_runner_error(&row.id, error))?;
    measure_stab_iterations(measurement_name, super::STAB_COMPARE_ITERATIONS, || {
        let mut regions = 0usize;
        for _ in 0..UTILITY_BATCH {
            let output = circuit_detecting_regions(
                &circuit,
                DetectingRegionOptions {
                    detectors: vec![detector],
                    ticks: vec![0, 1],
                    ignore_anticommutation_errors: false,
                },
            )
            .map_err(|error| stab_runner_error(&row.id, error))?;
            let detector_regions = output
                .get(&detector)
                .ok_or_else(|| BenchError::StabRunner {
                    row_id: row.id.clone(),
                    message: "detecting-regions benchmark output omitted detector D0".to_string(),
                })?;
            regions = regions.checked_add(detector_regions.len()).ok_or_else(|| {
                BenchError::StabRunner {
                    row_id: row.id.clone(),
                    message: "detecting-regions benchmark region count overflowed".to_string(),
                }
            })?;
        }
        black_box(regions);
        Ok(())
    })
}

fn run_missing_detectors_basic_batch(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![
        measure_missing_detectors_basic(row, "stab_missing_detectors_basic_cases")?,
        measure_missing_detectors_basic(row, "stab_missing_detectors_basic_suggestions")?,
    ])
}

fn measure_missing_detectors_basic(
    row: &BenchmarkRow,
    measurement_name: &'static str,
) -> Result<Measurement, BenchError> {
    let cases = missing_detector_basic_corpus(&row.id)?;
    measure_stab_iterations(measurement_name, super::STAB_COMPARE_ITERATIONS, || {
        let mut suggestions = 0usize;
        for _ in 0..UTILITY_BATCH {
            for (circuit, ignore_non_deterministic_measurements) in &cases {
                let output = missing_detectors(
                    circuit,
                    MissingDetectorOptions {
                        ignore_non_deterministic_measurements:
                            *ignore_non_deterministic_measurements,
                    },
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                suggestions = suggestions
                    .checked_add(output.items().len())
                    .ok_or_else(|| BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: "missing-detectors benchmark suggestion count overflowed"
                            .to_string(),
                    })?;
            }
        }
        black_box(suggestions);
        Ok(())
    })
}

fn missing_detector_basic_corpus(row_id: &str) -> Result<Vec<(Circuit, bool)>, BenchError> {
    [
        ("", false),
        ("R 0\nM 0\nDETECTOR rec[-1]\n", false),
        ("R 0\nM 0\nDETECTOR rec[-1]\nDETECTOR rec[-1]\n", false),
        ("R 0\nM 0\n", false),
        ("M 0\n", false),
        ("M 0\n", true),
        ("R 0 1\nM 0 1\nDETECTOR rec[-1]\n", false),
        ("M 0\nDETECTOR rec[-1] rec[-1]\n", false),
        ("MX 0\n", false),
        ("RX 0\nMY 0\n", false),
    ]
    .into_iter()
    .map(|(text, ignore)| parse_circuit(row_id, text).map(|circuit| (circuit, ignore)))
    .collect()
}

fn run_feedback_inline_mpp_batch(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, FEEDBACK_INLINE_MPP)?;
    Ok(vec![measure_stab_iterations(
        "stab_feedback_inline_mpp_transforms",
        super::STAB_COMPARE_ITERATIONS,
        || {
            let mut instructions = 0usize;
            for _ in 0..UTILITY_BATCH {
                let output = circuit_with_inlined_feedback(&circuit)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                instructions = instructions
                    .checked_add(output.items().len())
                    .ok_or_else(|| BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: "feedback-inlining benchmark instruction count overflowed"
                            .to_string(),
                    })?;
            }
            black_box(instructions);
            Ok(())
        },
    )?])
}

fn run_detect_sweep_sampling_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, DETECT_SWEEP_DEFAULT_FALSE)?;
    Ok(vec![measure_stab_iterations(
        "stab_detect_sweep_default_false",
        super::STAB_COMPARE_ITERATIONS,
        || {
            let mut bits = 0usize;
            try_for_each_sampled_detection_event::<CircuitError, _>(
                &circuit,
                DETECT_SHOTS,
                Some(17),
                |record| {
                    bits += record.detectors.len() + record.observables.len();
                    Ok(())
                },
            )
            .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(bits);
            Ok(())
        },
    )?])
}

fn run_m2d_cli_row(
    row: &BenchmarkRow,
    measurement_name: &'static str,
    args: Vec<OsString>,
    input: &'static [u8],
    side_output: Option<PathBuf>,
) -> Result<Vec<Measurement>, BenchError> {
    if let Some(path) = side_output.as_ref() {
        create_parent_dir(row, path)?;
    }
    Ok(vec![measure_stab_iterations(
        measurement_name,
        super::STAB_COMPARE_ITERATIONS,
        || {
            let mut stdout = CountingWriter::default();
            let mut stderr = Vec::new();
            let status = stab_cli::run_from(args.clone(), input, &mut stdout, &mut stderr);
            if status != 0 {
                return Err(BenchError::StabRunner {
                    row_id: row.id.clone(),
                    message: format!(
                        "stab-cli m2d failed with status {status}: {}",
                        String::from_utf8_lossy(&stderr)
                    ),
                });
            }
            if let Some(path) = side_output.as_ref() {
                let side_bytes = std::fs::read(path).map_err(|source| BenchError::StabRunner {
                    row_id: row.id.clone(),
                    message: format!(
                        "failed to read m2d side output {}: {source}",
                        path.display()
                    ),
                })?;
                black_box((stdout.len(), side_bytes.len()));
            } else {
                black_box(stdout.len());
            }
            Ok(())
        },
    )?])
}

#[derive(Default)]
struct CountingWriter {
    bytes: usize,
}

impl CountingWriter {
    fn len(&self) -> usize {
        self.bytes
    }
}

impl Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("m2d benchmark output byte count overflowed"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn m2d_sweep_args(obs_out: bool) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("stab"),
        OsString::from("m2d"),
        OsString::from("--in_format=01"),
        OsString::from(if obs_out {
            "--out_format=01"
        } else {
            "--out_format=dets"
        }),
        OsString::from("--sweep"),
        repo_path("oracle/fixtures/inputs/m2d_sweep_bits.01").into_os_string(),
        OsString::from("--sweep_format=01"),
        OsString::from("--circuit"),
        repo_path("oracle/fixtures/inputs/m2d_sweep.stim").into_os_string(),
    ];
    if obs_out {
        args.extend([
            OsString::from("--obs_out"),
            obs_out_path().into_os_string(),
            OsString::from("--obs_out_format=b8"),
        ]);
    }
    args
}

fn m2d_sweep_b8_args() -> Vec<OsString> {
    vec![
        OsString::from("stab"),
        OsString::from("m2d"),
        OsString::from("--in_format=b8"),
        OsString::from("--out_format=b8"),
        OsString::from("--sweep"),
        repo_path("benchmarks/fixtures/m9_m2d_sweep_b8_sweep.b8").into_os_string(),
        OsString::from("--sweep_format=b8"),
        OsString::from("--circuit"),
        repo_path("benchmarks/fixtures/m9_m2d_sweep_b8.stim").into_os_string(),
    ]
}

fn m2d_ran_without_feedback_args() -> Vec<OsString> {
    vec![
        OsString::from("stab"),
        OsString::from("m2d"),
        OsString::from("--in_format=01"),
        OsString::from("--append_observables"),
        OsString::from("--out_format=dets"),
        OsString::from("--ran_without_feedback"),
        OsString::from("--circuit"),
        repo_path("oracle/fixtures/inputs/m2d_ran_without_feedback.stim").into_os_string(),
    ]
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
    let measurements = m2d_measurements(&row.id, &circuit, SampleFormat::ZeroOne)?;
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

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn obs_out_path() -> PathBuf {
    repo_path("target/benchmarks/cli-scratch/m9-m2d-sweep-obs-out.b8")
}

fn create_parent_dir(row: &BenchmarkRow, path: &Path) -> Result<(), BenchError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent).map_err(|source| BenchError::StabRunner {
        row_id: row.id.clone(),
        message: format!(
            "failed to create m2d side-output directory {}: {source}",
            parent.display()
        ),
    })
}
