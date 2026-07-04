use std::ffi::OsString;
use std::hint::black_box;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use stab_core::{
    Circuit, CircuitError, CodeDistance, CompiledDetectionConverter, CompiledSampler,
    DemDetectorId, DetectingRegionOptions, DetectionConversionOptions,
    DetectionObservableOutputMode, ErrorAnalyzerOptions, Flow, Gate, MissingDetectorOptions,
    Probability, RepetitionCodeParams, RepetitionCodeTask, RoundCount, SampleFormat,
    check_if_circuit_has_unsigned_stabilizer_flows, circuit_detecting_regions,
    circuit_to_detector_error_model, circuit_with_inlined_feedback,
    convert_measurements_to_detection_events, generate_repetition_code_circuit,
    measurement_record_count, missing_detectors,
    result_formats::write_ptb64_records_checked,
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
const MISSING_DETECTOR_MPP_CASES: usize = 4;
const MISSING_DETECTOR_MPP_SUGGESTIONS: usize = 3;
const FLOW_CHECK_CASES: usize = 4;
const FLOW_CHECK_FLOWS: usize = 27;
const GATE_SEMANTIC_FIXED_TABLEAU_GATES: usize = 46;
const GATE_SEMANTIC_SURFACES_PER_GATE: usize = 3;
const DETECTING_REGIONS_PER_CASE: usize = 2;
const DETECTING_REGIONS_SIMPLE: &str = "H 0\n\
                                        TICK\n\
                                        CX 0 1\n\
                                        TICK\n\
                                        MXX 0 1\n\
                                        DETECTOR rec[-1]\n";
const DETECTING_REGIONS_REPEAT: &str = "H 0\n\
                                        REPEAT 2 {\n\
                                            TICK\n\
                                        }\n\
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
const SWEEP_PTB64_SHOTS: usize = 64;
const SWEEP_PTB64_WIDTH: usize = 8;

type FlowCheckCase = (Circuit, Vec<Flow>, Vec<bool>);

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
        "pf5-detecting-regions-repeat" => run_detecting_regions_repeat_row(row).map(Some),
        "pf5-missing-detectors-mpp" => run_missing_detectors_mpp_batch(row).map(Some),
        "pf5-has-all-flows-batch" => run_has_flows_batch(row).map(Some),
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
        "pf3-m2d-sweep-ptb64-input" => run_m2d_sweep_ptb64_cli_row(row).map(Some),
        "pf3-detect-sweep-sampling" => run_detect_sweep_sampling_row(row).map(Some),
        "pf3-gate-semantic-wide" => run_gate_semantic_wide_row(row).map(Some),
        "pf7-cli-m2d-sweep-b8" => run_m2d_cli_row(
            row,
            "stab_pf7_cli_m2d_sweep_b8",
            m2d_sweep_b8_args(),
            M2D_SWEEP_B8_MEASUREMENTS,
            None,
        )
        .map(Some),
        "pf7-cli-m2d-feedback-inline" => run_m2d_cli_row(
            row,
            "stab_pf7_cli_m2d_feedback_inline",
            m2d_ran_without_feedback_args(),
            M2D_RAN_WITHOUT_FEEDBACK_MEASUREMENTS,
            None,
        )
        .map(Some),
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
        ("pf3-m2d-sweep-ptb64-input", "stab_pf3_m2d_sweep_ptb64") => {
            Some((SWEEP_PTB64_SHOTS as f64, "shots/s"))
        }
        ("pf7-cli-m2d-sweep-b8", "stab_pf7_cli_m2d_sweep_b8") => Some((5.0, "shots/s")),
        ("m9-m2d-ran-without-feedback-cli", "stab_m2d_ran_without_feedback") => {
            Some((6.0, "shots/s"))
        }
        ("pf7-cli-m2d-feedback-inline", "stab_pf7_cli_m2d_feedback_inline") => {
            Some((6.0, "shots/s"))
        }
        ("m9-detecting-regions-basic-batch", "stab_detecting_regions_basic_cases") => {
            Some((UTILITY_BATCH as f64, "cases/s"))
        }
        ("m9-detecting-regions-basic-batch", "stab_detecting_regions_basic_regions") => Some((
            (UTILITY_BATCH * DETECTING_REGIONS_PER_CASE) as f64,
            "regions/s",
        )),
        ("pf5-detecting-regions-repeat", "stab_pf5_detecting_regions_repeat_ticks") => {
            Some((UTILITY_BATCH as f64, "cases/s"))
        }
        ("m9-missing-detectors-basic-batch", "stab_missing_detectors_basic_cases") => Some((
            (UTILITY_BATCH * MISSING_DETECTOR_BASIC_CASES) as f64,
            "cases/s",
        )),
        ("m9-missing-detectors-basic-batch", "stab_missing_detectors_basic_suggestions") => Some((
            (UTILITY_BATCH * MISSING_DETECTOR_BASIC_SUGGESTIONS) as f64,
            "suggestions/s",
        )),
        ("pf5-missing-detectors-mpp", "stab_pf5_missing_detectors_mpp_cases") => Some((
            (UTILITY_BATCH * MISSING_DETECTOR_MPP_CASES) as f64,
            "cases/s",
        )),
        ("pf5-missing-detectors-mpp", "stab_pf5_missing_detectors_mpp_suggestions") => Some((
            (UTILITY_BATCH * MISSING_DETECTOR_MPP_SUGGESTIONS) as f64,
            "suggestions/s",
        )),
        ("pf5-has-all-flows-batch", "stab_pf5_has_flows_batch_cases") => {
            Some(((UTILITY_BATCH * FLOW_CHECK_CASES) as f64, "cases/s"))
        }
        ("pf5-has-all-flows-batch", "stab_pf5_has_flows_batch_flows") => {
            Some(((UTILITY_BATCH * FLOW_CHECK_FLOWS) as f64, "flows/s"))
        }
        ("pf3-gate-semantic-wide", "stab_pf3_gate_semantic_tableau_contract") => Some((
            (GATE_SEMANTIC_FIXED_TABLEAU_GATES * GATE_SEMANTIC_SURFACES_PER_GATE) as f64,
            "surface-checks/s",
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
        "pf7-cli-m2d-feedback-inline" => Some(
            "report-only: Stab measures the public CLI m2d --ran_without_feedback path for PF7 visible CLI parity using the source-owned M9 feedback fixture",
        ),
        "m9-detecting-regions-basic-batch" => Some(
            "report-only: Stab measures the Rust detecting-regions utility subset without a faithful pinned Stim CLI timing ratio",
        ),
        "pf5-detecting-regions-repeat" => Some(
            "report-only: Stab measures bounded repeat traversal in the Rust detecting-regions utility without a faithful pinned Stim CLI timing ratio",
        ),
        "m9-missing-detectors-basic-batch" => Some(
            "report-only: Stab measures the Rust basic missing-detectors utility subset without a faithful pinned Stim CLI timing ratio",
        ),
        "pf5-missing-detectors-mpp" => Some(
            "report-only: Stab measures the Rust missing-detectors MPP and observable row-reduction subset without a faithful pinned Stim CLI timing ratio",
        ),
        "pf5-has-all-flows-batch" => Some(
            "report-only: Stab measures the Rust unsigned has_flow measurement-record and observable-dependency subset without a faithful pinned Stim CLI timing ratio",
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
        "pf3-m2d-sweep-ptb64-input" => Some(
            "report-only: Stab measures public m2d --sweep with ptb64 measurement and sweep inputs generated from source-owned deterministic records",
        ),
        "pf7-cli-m2d-sweep-b8" => Some(
            "report-only: Stab measures the public CLI m2d --sweep packed b8 path for PF7 visible CLI parity using the source-owned M9 sweep fixture",
        ),
        "pf3-detect-sweep-sampling" => Some(
            "report-only: Stab measures the Rust sweep-conditioned detection sampler using omitted all-false sweep bits; no faithful pinned Stim CLI ratio is claimed for this partial PF3 surface",
        ),
        "pf3-gate-semantic-wide" => Some(
            "report-only: Stab measures fixed-tableau gate execution contract coverage across sampler compilation, detection-conversion compilation, and analyzer propagation without a faithful pinned Stim CLI timing ratio",
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

fn run_detecting_regions_repeat_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, DETECTING_REGIONS_REPEAT)?;
    let detector = DemDetectorId::try_new(0).map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![measure_stab_iterations(
        "stab_pf5_detecting_regions_repeat_ticks",
        super::STAB_COMPARE_ITERATIONS,
        || {
            let mut regions = 0usize;
            for _ in 0..UTILITY_BATCH {
                let output = circuit_detecting_regions(
                    &circuit,
                    DetectingRegionOptions {
                        detectors: vec![detector],
                        ticks: vec![0, 1, 2],
                        ignore_anticommutation_errors: false,
                    },
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                regions = regions
                    .checked_add(output.get(&detector).map_or(0, |regions| regions.len()))
                    .ok_or_else(|| BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: "detecting-regions repeat benchmark region count overflowed"
                            .to_string(),
                    })?;
            }
            black_box(regions);
            Ok(())
        },
    )?])
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

fn run_missing_detectors_mpp_batch(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![
        measure_missing_detectors_mpp(row, "stab_pf5_missing_detectors_mpp_cases")?,
        measure_missing_detectors_mpp(row, "stab_pf5_missing_detectors_mpp_suggestions")?,
    ])
}

fn measure_missing_detectors_mpp(
    row: &BenchmarkRow,
    measurement_name: &'static str,
) -> Result<Measurement, BenchError> {
    let cases = missing_detector_mpp_corpus(&row.id)?;
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
                        message: "missing-detectors MPP benchmark suggestion count overflowed"
                            .to_string(),
                    })?;
            }
        }
        black_box(suggestions);
        Ok(())
    })
}

fn missing_detector_mpp_corpus(row_id: &str) -> Result<Vec<(Circuit, bool)>, BenchError> {
    [
        (
            "MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             DETECTOR rec[-1] rec[-3]\n\
             DETECTOR rec[-2] rec[-4]\n",
            false,
        ),
        (
            "MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             DETECTOR rec[-1] rec[-3]\n\
             DETECTOR rec[-2] rec[-4]\n\
             DETECTOR rec[-1] rec[-3] rec[-2] rec[-4]\n",
            false,
        ),
        (
            "MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\n\
             DETECTOR rec[-2] rec[-4]\n\
             OBSERVABLE_INCLUDE(0) rec[-3]\n",
            true,
        ),
        (
            "OBSERVABLE_INCLUDE(0) Z0 Z1\n\
             MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             OBSERVABLE_INCLUDE(0) Z0 Z1\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\n\
             DETECTOR rec[-2] rec[-4]\n\
             OBSERVABLE_INCLUDE(0) rec[-3]\n",
            true,
        ),
    ]
    .into_iter()
    .map(|(text, ignore)| parse_circuit(row_id, text).map(|circuit| (circuit, ignore)))
    .collect()
}

fn run_has_flows_batch(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![
        measure_has_flows(row, "stab_pf5_has_flows_batch_cases")?,
        measure_has_flows(row, "stab_pf5_has_flows_batch_flows")?,
    ])
}

fn measure_has_flows(
    row: &BenchmarkRow,
    measurement_name: &'static str,
) -> Result<Measurement, BenchError> {
    let cases = flow_check_corpus(&row.id)?;
    measure_stab_iterations(measurement_name, super::STAB_COMPARE_ITERATIONS, || {
        let mut true_count = 0usize;
        for _ in 0..UTILITY_BATCH {
            for (circuit, flows, expected) in &cases {
                let actual = check_if_circuit_has_unsigned_stabilizer_flows(circuit, flows);
                if actual != *expected {
                    return Err(BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: format!(
                            "has-flow benchmark expected {expected:?} but got {actual:?}"
                        ),
                    });
                }
                true_count = true_count
                    .checked_add(actual.iter().filter(|value| **value).count())
                    .ok_or_else(|| BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: "has-flow benchmark true count overflowed".to_string(),
                    })?;
            }
        }
        black_box(true_count);
        Ok(())
    })
}

fn flow_check_corpus(row_id: &str) -> Result<Vec<FlowCheckCase>, BenchError> {
    Ok(vec![
        (
            parse_circuit(
                row_id,
                "R 4\n\
                 CX 0 4 1 4 2 4 3 4\n\
                 M 4\n",
            )?,
            parse_flows(
                row_id,
                &[
                    "Z___ -> Z____",
                    "_Z__ -> _Z__",
                    "__Z_ -> __Z_",
                    "___Z -> ___Z",
                    "XX__ -> XX__",
                    "XXXX -> XXXX",
                    "XYZ_ -> XYZ_",
                    "XXX_ -> XXX_",
                    "ZZZZ -> ____ xor rec[-1]",
                    "+___Z -> -___Z",
                    "-___Z -> -___Z",
                    "-___Z -> +___Z",
                ],
            )?,
            vec![
                true, true, true, true, true, true, true, false, true, true, true, true,
            ],
        ),
        (
            parse_circuit(row_id, "MZZ 0 1\n")?,
            parse_flows(
                row_id,
                &[
                    "X0*X1 -> Y0*Y1 xor rec[-1]",
                    "X0*X1 -> Z0*Z1 xor rec[-1]",
                    "X0*X1 -> X0*X1",
                    "Z0 -> Z1 xor rec[-1]",
                    "Z0 -> Z0",
                ],
            )?,
            vec![true, false, true, true, true],
        ),
        (
            parse_circuit(row_id, "MZZ 0 1\nOBSERVABLE_INCLUDE(2) rec[-1]\n")?,
            parse_flows(
                row_id,
                &[
                    "Z0*Z1 -> obs[2]",
                    "1 -> Z0*Z1 xor obs[2]",
                    "X0*X1 -> X0*X1 xor obs[0]",
                    "X0*X1 -> Y0*Y1 xor obs[2]",
                    "X0*X1 -> Y0*Y1 xor obs[1]",
                    "X0*X1 -> Y0*Y1 xor rec[-1]",
                ],
            )?,
            vec![true, true, true, true, false, true],
        ),
        (
            parse_circuit(
                row_id,
                "OBSERVABLE_INCLUDE(3) X0 Y1 Z2\n\
                 OBSERVABLE_INCLUDE(2) Y0\n",
            )?,
            parse_flows(
                row_id,
                &[
                    "X0*Y1*Z2 -> obs[3]",
                    "-Y0 -> obs[2]",
                    "Y0 -> obs[3]",
                    "1 -> X0*Y1*Z2 xor obs[3]",
                ],
            )?,
            vec![true, true, false, true],
        ),
    ])
}

fn parse_flows(row_id: &str, flows: &[&str]) -> Result<Vec<Flow>, BenchError> {
    flows
        .iter()
        .map(|flow| Flow::from_str(flow).map_err(|error| stab_runner_error(row_id, error)))
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

fn run_gate_semantic_wide_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuits = fixed_tableau_gate_execution_circuits(&row.id)?;
    Ok(vec![measure_stab_iterations(
        "stab_pf3_gate_semantic_tableau_contract",
        super::STAB_COMPARE_ITERATIONS,
        || {
            let mut checked = 0usize;
            for circuit in &circuits {
                let sampler = CompiledSampler::compile(circuit)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(sampler.sample_zero_one(1));
                checked = checked
                    .checked_add(1)
                    .ok_or_else(|| gate_semantic_count_overflow_error(row, "sampler checks"))?;

                let converter = CompiledDetectionConverter::compile(
                    circuit,
                    DetectionConversionOptions {
                        skip_reference_sample: false,
                    },
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(converter.detector_count());
                checked = checked.checked_add(1).ok_or_else(|| {
                    gate_semantic_count_overflow_error(row, "detection-conversion checks")
                })?;

                let dem = circuit_to_detector_error_model(circuit, ErrorAnalyzerOptions::default())
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(dem.items().len());
                checked = checked
                    .checked_add(1)
                    .ok_or_else(|| gate_semantic_count_overflow_error(row, "analyzer checks"))?;
            }
            black_box(checked);
            Ok(())
        },
    )?])
}

fn run_m2d_cli_row(
    row: &BenchmarkRow,
    measurement_name: &'static str,
    args: Vec<OsString>,
    input: &[u8],
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

fn run_m2d_sweep_ptb64_cli_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let measurement_input = sweep_ptb64_records(row, false)?;
    let sweep_input = sweep_ptb64_records(row, true)?;
    let sweep_path = sweep_ptb64_path();
    create_parent_dir(row, &sweep_path)?;
    std::fs::write(&sweep_path, &sweep_input).map_err(|source| BenchError::StabRunner {
        row_id: row.id.clone(),
        message: format!(
            "failed to write ptb64 sweep input {}: {source}",
            sweep_path.display()
        ),
    })?;
    run_m2d_cli_row(
        row,
        "stab_pf3_m2d_sweep_ptb64",
        m2d_sweep_ptb64_args(&sweep_path),
        &measurement_input,
        None,
    )
}

fn sweep_ptb64_records(row: &BenchmarkRow, sweep: bool) -> Result<Vec<u8>, BenchError> {
    let records = (0..SWEEP_PTB64_SHOTS)
        .map(|shot| {
            (0..SWEEP_PTB64_WIDTH)
                .map(|bit| {
                    if sweep {
                        (shot * 3 + bit) % 5 == 0
                    } else {
                        (shot + bit * 2) % 3 == 0
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    write_ptb64_records_checked(&records).map_err(|error| stab_runner_error(&row.id, error))
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

fn m2d_sweep_ptb64_args(sweep_path: &Path) -> Vec<OsString> {
    vec![
        OsString::from("stab"),
        OsString::from("m2d"),
        OsString::from("--in_format=ptb64"),
        OsString::from("--out_format=b8"),
        OsString::from("--sweep"),
        sweep_path.as_os_str().to_os_string(),
        OsString::from("--sweep_format=ptb64"),
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

fn fixed_tableau_gate_execution_circuits(row_id: &str) -> Result<Vec<Circuit>, BenchError> {
    Gate::all()
        .filter(|gate| gate.has_tableau())
        .map(|gate| fixed_tableau_gate_execution_circuit(row_id, gate))
        .collect()
}

fn fixed_tableau_gate_execution_circuit(row_id: &str, gate: Gate) -> Result<Circuit, BenchError> {
    let gate_name = gate.canonical_name();
    let inverse_name = gate
        .inverse()
        .ok_or_else(|| BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!("{gate_name} has tableau metadata but no inverse"),
        })?
        .canonical_name();
    let arity = gate
        .tableau()
        .map_err(|error| stab_runner_error(row_id, error))?
        .len();
    let targets = match arity {
        1 => "0",
        2 => "0 1",
        _ => {
            return Err(BenchError::StabRunner {
                row_id: row_id.to_string(),
                message: format!("{gate_name} has unsupported benchmark arity {arity}"),
            });
        }
    };
    parse_circuit(
        row_id,
        &format!("{gate_name} {targets}\n{inverse_name} {targets}\nM 0\nDETECTOR rec[-1]\n"),
    )
}

fn gate_semantic_count_overflow_error(row: &BenchmarkRow, context: &str) -> BenchError {
    BenchError::StabRunner {
        row_id: row.id.clone(),
        message: format!("PF3 gate semantic benchmark overflowed while counting {context}"),
    }
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn obs_out_path() -> PathBuf {
    repo_path("target/benchmarks/cli-scratch/m9-m2d-sweep-obs-out.b8")
}

fn sweep_ptb64_path() -> PathBuf {
    repo_path("target/benchmarks/cli-scratch/pf3-m2d-sweep-ptb64.sweep")
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
