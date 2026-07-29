use std::ffi::OsString;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use stab_core::{
    Circuit, CircuitError, Flow, MeasurementBatchView, PackedShotBatch, RandomPolicy, SampleFormat,
    Seed, ShotCount, check_if_circuit_has_unsigned_stabilizer_flows,
    circuit_has_all_unsigned_stabilizer_flows, circuit_with_inlined_feedback,
    measurement_record_count, result_formats::read_records,
    result_formats::write_ptb64_records_checked, try_for_each_sampled_detection_event,
};
use stab_engine::{DetectionSamplingCompiler, MeasurementToDetectionCompiler};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;
use crate::root::RepoRoot;

use super::{
    batch_sinks::{ByteDigestWriter, DetectionDigestSink, OutputWitness, u64_sequence_digest},
    cli_process::run_stab_cli_process_row,
    measure_stab_iterations, measure_stab_iterations_with_postprocess_and_memory_operation,
    measure_stab_preflighted_compile_and_release, stab_runner_error,
};

mod detecting_region_rows;
mod gate_semantic;
mod missing_detector_rows;

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
#[cfg(not(test))]
const DETECT_SHOTS: usize = 1024;
#[cfg(test)]
const DETECT_SHOTS: usize = 4;
const DETECT_CLI_SHOTS: usize = 1024;
const PRIMARY_CLI_SHOTS: usize = 64;
#[cfg(not(test))]
const UTILITY_BATCH: usize = 4096;
#[cfg(test)]
const UTILITY_BATCH: usize = 2;
const FLOW_CHECK_CASES: usize = 4;
const FLOW_CHECK_FLOWS: usize = 27;
const M2D_PHASE_BATCH_SHOTS: usize = 64;
#[cfg(not(test))]
const DETECTION_PHASE_FIRST_WITNESS: (u64, u64) = (1_703_389_565_409_843_255, DETECT_SHOTS as u64);
#[cfg(test)]
const DETECTION_PHASE_FIRST_WITNESS: (u64, u64) = (3_477_855_591_213_421_345, DETECT_SHOTS as u64);
const DETECTION_PHASE_SEQUENCE_DOMAIN: &[u8] = b"a5-m9-detection-session-v1";
#[cfg(not(test))]
const DETECTION_PHASE_SEQUENCE_DIGEST: &str =
    "103868334694bd8325b9c75f76f4564fffc78b953d3fda650a199cba33dfa7cd";
#[cfg(test)]
const DETECTION_PHASE_SEQUENCE_DIGEST: &str =
    "14c6b3e2b6c4dc3f13a5ef867b6e7efcab706dfbbde9771e9760bfc1cf97d3a5";
#[cfg(not(test))]
const M2D_PHASE_WITNESS: (u64, u64) = (13_532_626_590_392_138_993, M2D_PHASE_BATCH_SHOTS as u64);
#[cfg(test)]
const M2D_PHASE_WITNESS: (u64, u64) = (13_532_626_590_392_138_993, M2D_PHASE_BATCH_SHOTS as u64);
#[cfg(not(test))]
const DETECT_PTB64_SHOTS: usize = 1024;
#[cfg(test)]
const DETECT_PTB64_SHOTS: usize = 64;
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
const DETECT_FRAME_SWEEP_DEFAULT_FALSE: &str = "RX 0\n\
                                               CX sweep[0] 0\n\
                                               CY sweep[1] 0\n\
                                               CZ 0 sweep[2]\n\
                                               OBSERVABLE_INCLUDE(0) X0\n";
const SWEEP_PTB64_SHOTS: usize = 64;
const SWEEP_PTB64_WIDTH: usize = 8;

type FlowCheckCase = (Circuit, Vec<Flow>, Vec<bool>);

pub(super) fn run_detection_compare_row(
    root: &RepoRoot,
    profile: &str,
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    if let Some(measurement_name) = process_cli_measurement_name(&row.id) {
        return run_process_cli_row(root, profile, row, measurement_name).map(Some);
    }
    match row.id.as_str() {
        "m9-m2d-sweep-01-cli" => run_m2d_cli_row(
            row,
            "stab_m2d_sweep_01_dets",
            m2d_sweep_args(root, false),
            M2D_SWEEP_MEASUREMENTS,
            None,
        )
        .map(Some),
        "m9-m2d-sweep-b8-cli" => run_m2d_cli_row(
            row,
            "stab_m2d_sweep_b8",
            m2d_sweep_b8_args(root),
            M2D_SWEEP_B8_MEASUREMENTS,
            None,
        )
        .map(Some),
        "m9-m2d-sweep-obs-out-cli" => run_m2d_cli_row(
            row,
            "stab_m2d_sweep_obs_out",
            m2d_sweep_args(root, true),
            M2D_SWEEP_MEASUREMENTS,
            Some(obs_out_path(root)),
        )
        .map(Some),
        "m9-m2d-ran-without-feedback-cli" => run_m2d_cli_row(
            row,
            "stab_m2d_ran_without_feedback",
            m2d_ran_without_feedback_args(root),
            M2D_RAN_WITHOUT_FEEDBACK_MEASUREMENTS,
            None,
        )
        .map(Some),
        "m9-detecting-regions-basic-batch" => detecting_region_rows::run_basic_batch(row).map(Some),
        "m9-missing-detectors-basic-batch" => missing_detector_rows::run_basic_batch(row).map(Some),
        "m9-feedback-inline-mpp-batch" => run_feedback_inline_mpp_batch(row).map(Some),
        "m9-detection-batch-phases" => run_detection_phase_row(row).map(Some),
        "m9-m2d-batch-phases" => run_m2d_phase_row(row).map(Some),
        "pf5-detecting-regions-repeat" => detecting_region_rows::run_repeat_row(row).map(Some),
        "pf5-detecting-regions-targets" => detecting_region_rows::run_targets_row(row).map(Some),
        "pf5-detecting-regions-clifford" => detecting_region_rows::run_clifford_row(row).map(Some),
        "pf5-detecting-regions-generated-repetition" => {
            detecting_region_rows::run_generated_repetition_row(row).map(Some)
        }
        "pf5-detecting-regions-generated-surface" => {
            detecting_region_rows::run_generated_surface_row(row).map(Some)
        }
        "pf5-missing-detectors-mpp" => missing_detector_rows::run_mpp_batch(row).map(Some),
        "pf5-missing-detectors-mpad" => missing_detector_rows::run_mpad_batch(row).map(Some),
        "pf5-missing-detectors-generated-code" => {
            missing_detector_rows::run_generated_code_batch(row).map(Some)
        }
        "pf5-has-all-flows-batch" => run_has_all_flows_batch(row).map(Some),
        "pf3-m2d-sweep-b8" => run_m2d_cli_row(
            row,
            "stab_pf3_m2d_sweep_b8",
            m2d_sweep_b8_args(root),
            M2D_SWEEP_B8_MEASUREMENTS,
            None,
        )
        .map(Some),
        "pf3-m2d-sweep-ptb64-input" => run_m2d_sweep_ptb64_cli_row(root, row).map(Some),
        "pf3-detect-sweep-sampling" => run_detect_sweep_sampling_row(row).map(Some),
        "pf3-gate-semantic-wide" => gate_semantic::run(row).map(Some),
        "pf7-cli-m2d-sweep-b8" => run_m2d_cli_row(
            row,
            "stab_pf7_cli_m2d_sweep_b8",
            m2d_sweep_b8_args(root),
            M2D_SWEEP_B8_MEASUREMENTS,
            None,
        )
        .map(Some),
        "pf7-cli-m2d-feedback-inline" => run_m2d_cli_row(
            row,
            "stab_pf7_cli_m2d_feedback_inline",
            m2d_ran_without_feedback_args(root),
            M2D_RAN_WITHOUT_FEEDBACK_MEASUREMENTS,
            None,
        )
        .map(Some),
        _ => Ok(None),
    }
}

pub(super) fn process_cli_measurement_name(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m9-detect-text-cli" => Some("stab_detect_1024_dets"),
        "m9-detect-bitpacked-cli" => Some("stab_detect_1024_b8"),
        "m9-detect-primary-matrix-contract" => Some("stab_detect_primary_repetition_d3_r3_b8"),
        "m9-m2d-text-cli" => Some("stab_m2d_dets"),
        "m9-m2d-bitpacked-contract" => Some("stab_m2d_b8"),
        "m9-m2d-primary-matrix-contract" => Some("stab_m2d_primary_repetition_d3_r3_b8"),
        _ => None,
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
        ("pf5-has-all-flows-batch", "stab_pf5_has_flows_batch_cases") => {
            Some(((UTILITY_BATCH * FLOW_CHECK_CASES) as f64, "cases/s"))
        }
        ("pf5-has-all-flows-batch", "stab_pf5_has_flows_batch_flows") => {
            Some(((UTILITY_BATCH * FLOW_CHECK_FLOWS) as f64, "flows/s"))
        }
        ("m9-feedback-inline-mpp-batch", "stab_feedback_inline_mpp_transforms") => {
            Some((UTILITY_BATCH as f64, "transforms/s"))
        }
        ("m9-detect-text-cli", "stab_detect_1024_dets")
        | ("m9-detect-bitpacked-cli", "stab_detect_1024_b8") => {
            Some((DETECT_CLI_SHOTS as f64, "shots/s"))
        }
        ("m9-detection-batch-phases", "stab_detection_plan_compile_and_release_basic") => {
            Some((1.0, "plans/s"))
        }
        ("m9-detection-batch-phases", "stab_detection_session_sample_to_detection") => {
            Some((DETECT_SHOTS as f64, "shots/s"))
        }
        ("m9-detection-batch-phases", "stab_detect_ptb64_routing") => {
            Some((DETECT_PTB64_SHOTS as f64, "shots/s"))
        }
        ("m9-m2d-batch-phases", "stab_m2d_plan_compile_and_release_basic") => {
            Some((1.0, "plans/s"))
        }
        ("m9-m2d-batch-phases", "stab_m2d_session_convert_batch") => {
            Some((M2D_PHASE_BATCH_SHOTS as f64, "shots/s"))
        }
        ("pf3-detect-sweep-sampling", "stab_detect_sweep_default_false") => {
            Some((DETECT_SHOTS as f64, "shots/s"))
        }
        ("pf3-detect-sweep-sampling", "stab_detect_frame_sweep_default_false") => {
            Some((DETECT_SHOTS as f64, "shots/s"))
        }
        ("m9-detect-primary-matrix-contract", "stab_detect_primary_repetition_d3_r3_b8")
        | ("m9-m2d-primary-matrix-contract", "stab_m2d_primary_repetition_d3_r3_b8") => {
            Some((PRIMARY_CLI_SHOTS as f64, "shots/s"))
        }
        _ => gate_semantic::measurement_work(name)
            .or_else(|| detecting_region_rows::measurement_work(row_id, name))
            .or_else(|| missing_detector_rows::measurement_work(row_id, name)),
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m9-detect-text-cli" | "m9-detect-bitpacked-cli" => Some(
            "report-only: Stab and pinned Stim execute the same seeded detect command as bounded subprocesses with identical input, launch count, discarded timed stdout, and independent untimed frozen output witnesses",
        ),
        "m9-m2d-text-cli" => Some(
            "report-only: Stab and pinned Stim execute the same m2d command as bounded subprocesses with identical input, launch count, discarded timed stdout, and independent untimed frozen output witnesses",
        ),
        "m9-m2d-bitpacked-contract" => Some(
            "cli-baseline: Stab and pinned Stim execute the same b8 m2d command as bounded subprocesses on the same fixture with independent untimed frozen output witnesses",
        ),
        "m9-detection-batch-phases" => Some(
            "report-only: source-owned Stab diagnostics separately measure detection plan compile-and-release with exact plan-dimension witnesses, bounded session execution with a frozen ordered SHA-256 sequence of 64-bit output witnesses, and PTB64 CLI routing with a frozen output witness, without claiming a Stim ratio",
        ),
        "m9-m2d-batch-phases" => Some(
            "report-only: source-owned Stab diagnostics separately measure measurement-to-detection plan compile-and-release with exact plan-dimension witnesses and bounded session conversion with a frozen shot-count and output witness, without claiming a Stim ratio",
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
        "pf5-has-all-flows-batch" => Some(
            "report-only: Stab measures the Rust unsigned has_all_flow helper over measurement-record observable-dependency and false-flow batches without a faithful pinned Stim CLI timing ratio",
        ),
        "m9-feedback-inline-mpp-batch" => Some(
            "report-only: Stab measures the Rust MPP feedback-inlining utility subset without a faithful pinned Stim CLI timing ratio",
        ),
        "m9-detect-primary-matrix-contract" => Some(
            "cli-baseline: Stab and pinned Stim execute the same seeded b8 detect command as bounded subprocesses on the source-owned generated repetition-code d3/r3 fixture with independent untimed frozen output witnesses",
        ),
        "m9-m2d-primary-matrix-contract" => Some(
            "cli-baseline: Stab and pinned Stim execute the same b8 m2d command as bounded subprocesses on source-owned generated repetition-code d3/r3 measurement records with independent untimed frozen output witnesses",
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
            "report-only: Stab measures the Rust sweep-conditioned detection sampler using omitted all-false sweep bits for non-frame and frame-path workloads; no faithful pinned Stim CLI ratio is claimed for this partial PF3 surface",
        ),
        "pf3-gate-semantic-wide" => Some(
            "report-only: separate source-owned sampler, reference, conversion, detection, analyzer, and flow submeasurements cover representative fixed-tableau, measurement, Pauli-product, stochastic-noise, annotation, classical-control, and repeat circuits; no aggregate ratio or faithful pinned Stim CLI timing ratio is claimed",
        ),
        _ => detecting_region_rows::compare_note(row_id)
            .or_else(|| missing_detector_rows::compare_note(row_id)),
    }
}

fn run_has_all_flows_batch(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![
        measure_has_all_flows(row, "stab_pf5_has_flows_batch_cases")?,
        measure_has_all_flows(row, "stab_pf5_has_flows_batch_flows")?,
    ])
}

fn measure_has_all_flows(
    row: &BenchmarkRow,
    measurement_name: &'static str,
) -> Result<Measurement, BenchError> {
    let cases = flow_check_corpus(&row.id)?;
    let mut expected_all_by_case = Vec::with_capacity(cases.len());
    for (circuit, flows, expected) in &cases {
        let actual = check_if_circuit_has_unsigned_stabilizer_flows(circuit, flows);
        if actual != *expected {
            return Err(BenchError::StabRunner {
                row_id: row.id.clone(),
                message: format!("has-flow benchmark expected {expected:?} but got {actual:?}"),
            });
        }
        expected_all_by_case.push(expected.iter().all(|value| *value));
    }

    measure_stab_iterations(measurement_name, super::STAB_COMPARE_ITERATIONS, || {
        let mut true_count = 0usize;
        for _ in 0..UTILITY_BATCH {
            for ((circuit, flows, _expected), expected_all) in
                cases.iter().zip(expected_all_by_case.iter().copied())
            {
                let actual_all = circuit_has_all_unsigned_stabilizer_flows(circuit, flows);
                if actual_all != expected_all {
                    return Err(BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: format!(
                            "has-all-flow benchmark expected {expected_all} but got {actual_all}"
                        ),
                    });
                }
                true_count = true_count
                    .checked_add(usize::from(actual_all))
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
    let non_frame = parse_circuit(&row.id, DETECT_SWEEP_DEFAULT_FALSE)?;
    let frame = parse_circuit(&row.id, DETECT_FRAME_SWEEP_DEFAULT_FALSE)?;
    Ok(vec![
        measure_detect_sweep_sampling(row, &non_frame, "stab_detect_sweep_default_false")?,
        measure_detect_sweep_sampling(row, &frame, "stab_detect_frame_sweep_default_false")?,
    ])
}

fn measure_detect_sweep_sampling(
    row: &BenchmarkRow,
    circuit: &Circuit,
    name: &str,
) -> Result<Measurement, BenchError> {
    measure_stab_iterations(name, super::STAB_COMPARE_ITERATIONS, || {
        let mut bits = 0usize;
        try_for_each_sampled_detection_event::<CircuitError, _>(
            circuit,
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
    })
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
    let expected = run_m2d_cli_once(row, &args, input, side_output.as_deref())?;
    Ok(vec![measure_stab_iterations(
        measurement_name,
        super::STAB_COMPARE_ITERATIONS,
        || {
            let actual = run_m2d_cli_once(row, &args, input, side_output.as_deref())?;
            if actual != expected {
                return Err(stab_runner_error(
                    &row.id,
                    format!("m2d diagnostic output changed: expected {expected:?}, got {actual:?}"),
                ));
            }
            black_box(actual);
            Ok(())
        },
    )?])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct M2dCliWitness {
    stdout: OutputWitness,
    side_output: Option<OutputWitness>,
}

fn run_m2d_cli_once(
    row: &BenchmarkRow,
    args: &[OsString],
    input: &[u8],
    side_output: Option<&Path>,
) -> Result<M2dCliWitness, BenchError> {
    let mut stdout = ByteDigestWriter::default();
    let mut stderr = Vec::new();
    let status = stab_cli::run_from(args.iter().cloned(), input, &mut stdout, &mut stderr);
    if status != 0 {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "stab-cli m2d failed with status {status}: {}",
                String::from_utf8_lossy(&stderr)
            ),
        });
    }
    let side_output = side_output
        .map(|path| {
            std::fs::read(path)
                .map(|bytes| OutputWitness::from_bytes(&bytes))
                .map_err(|source| BenchError::StabRunner {
                    row_id: row.id.clone(),
                    message: format!(
                        "failed to read m2d side output {}: {source}",
                        path.display()
                    ),
                })
        })
        .transpose()?;
    Ok(M2dCliWitness {
        stdout: stdout.witness(),
        side_output,
    })
}

fn run_m2d_sweep_ptb64_cli_row(
    root: &RepoRoot,
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let measurement_input = sweep_ptb64_records(row, false)?;
    let sweep_input = sweep_ptb64_records(row, true)?;
    let sweep_path = sweep_ptb64_path(root);
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
        m2d_sweep_ptb64_args(root, &sweep_path),
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

fn m2d_sweep_args(root: &RepoRoot, obs_out: bool) -> Vec<OsString> {
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
        repo_path(root, "oracle/fixtures/inputs/m2d_sweep_bits.01").into_os_string(),
        OsString::from("--sweep_format=01"),
        OsString::from("--circuit"),
        repo_path(root, "oracle/fixtures/inputs/m2d_sweep.stim").into_os_string(),
    ];
    if obs_out {
        args.extend([
            OsString::from("--obs_out"),
            obs_out_path(root).into_os_string(),
            OsString::from("--obs_out_format=b8"),
        ]);
    }
    args
}

fn m2d_sweep_b8_args(root: &RepoRoot) -> Vec<OsString> {
    vec![
        OsString::from("stab"),
        OsString::from("m2d"),
        OsString::from("--in_format=b8"),
        OsString::from("--out_format=b8"),
        OsString::from("--sweep"),
        repo_path(root, "benchmarks/fixtures/m9_m2d_sweep_b8_sweep.b8").into_os_string(),
        OsString::from("--sweep_format=b8"),
        OsString::from("--circuit"),
        repo_path(root, "benchmarks/fixtures/m9_m2d_sweep_b8.stim").into_os_string(),
    ]
}

fn m2d_sweep_ptb64_args(root: &RepoRoot, sweep_path: &Path) -> Vec<OsString> {
    vec![
        OsString::from("stab"),
        OsString::from("m2d"),
        OsString::from("--in_format=ptb64"),
        OsString::from("--out_format=b8"),
        OsString::from("--sweep"),
        sweep_path.as_os_str().to_os_string(),
        OsString::from("--sweep_format=ptb64"),
        OsString::from("--circuit"),
        repo_path(root, "benchmarks/fixtures/m9_m2d_sweep_b8.stim").into_os_string(),
    ]
}

fn m2d_ran_without_feedback_args(root: &RepoRoot) -> Vec<OsString> {
    vec![
        OsString::from("stab"),
        OsString::from("m2d"),
        OsString::from("--in_format=01"),
        OsString::from("--append_observables"),
        OsString::from("--out_format=dets"),
        OsString::from("--ran_without_feedback"),
        OsString::from("--circuit"),
        repo_path(root, "oracle/fixtures/inputs/m2d_ran_without_feedback.stim").into_os_string(),
    ]
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
            "process-equivalent M9 CLI row has no frozen output witness",
        )
    })?;
    run_stab_cli_process_row(root, profile, row, measurement_name, expected)
}

fn run_detection_phase_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, DETECT_BASIC_FIXTURE)?;
    Ok(vec![
        measure_detection_plan_compile(row, &circuit)?,
        measure_detection_session(row, &circuit)?,
        measure_detect_cli(
            row,
            "stab_detect_ptb64_routing",
            "ptb64",
            DETECT_PTB64_SHOTS,
            detect_ptb64_witness(),
        )?,
    ])
}

#[cfg(not(test))]
const fn detect_ptb64_witness() -> OutputWitness {
    OutputWitness::new(128, 0x8421_ae12_6c7c_ed25)
}

#[cfg(test)]
const fn detect_ptb64_witness() -> OutputWitness {
    OutputWitness::new(8, 0xa8c7_f832_281a_39c5)
}

fn run_m2d_phase_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, M2D_BASIC_CIRCUIT)?;
    Ok(vec![
        measure_m2d_plan_compile(row, &circuit)?,
        measure_m2d_session_batch(row, &circuit)?,
    ])
}

fn frozen_process_cli_witness(row_id: &str) -> Option<OutputWitness> {
    match row_id {
        "m9-detect-text-cli" => Some(OutputWitness::new(5_120, 0xfaf5_740c_b1b3_c325)),
        "m9-detect-bitpacked-cli" => Some(OutputWitness::new(1_024, 0x51d8_8627_df28_7325)),
        "m9-detect-primary-matrix-contract" => Some(OutputWitness::new(64, 0x266f_92fc_8d95_4165)),
        "m9-m2d-text-cli" => Some(OutputWitness::new(13, 0x82da_2292_951b_1973)),
        "m9-m2d-bitpacked-contract" => Some(OutputWitness::new(2, 0x0832_8707_b4eb_6e3a)),
        "m9-m2d-primary-matrix-contract" => Some(OutputWitness::new(64, 0xb9b2_3f3a_46fd_0825)),
        _ => None,
    }
}

fn measure_detect_cli(
    row: &BenchmarkRow,
    measurement_name: &'static str,
    output_format: &'static str,
    shots: usize,
    expected: OutputWitness,
) -> Result<Measurement, BenchError> {
    let args = [
        OsString::from("stab"),
        OsString::from("detect"),
        OsString::from("--shots"),
        OsString::from(shots.to_string()),
        OsString::from("--seed=5"),
        OsString::from("--out_format"),
        OsString::from(output_format),
    ];
    let preflight = run_detect_cli(args.clone());
    ensure_detect_cli_output(row, expected, &preflight)?;
    black_box(preflight.2.witness());
    let mut timing_state = ();
    measure_stab_iterations_with_postprocess_and_memory_operation(
        measurement_name,
        super::STAB_COMPARE_ITERATIONS,
        &mut timing_state,
        |_| Ok(run_detect_cli(args.clone())),
        |_, actual| {
            ensure_detect_cli_output(row, expected, &actual)?;
            black_box(actual.2.witness());
            Ok(())
        },
        || {
            let actual = run_detect_cli(args.clone());
            ensure_detect_cli_output(row, expected, &actual)?;
            black_box(actual.2.witness());
            Ok(())
        },
    )
}

type DetectCliOutput = (i32, Vec<u8>, ByteDigestWriter);

fn run_detect_cli(args: [OsString; 7]) -> DetectCliOutput {
    let mut stdout = ByteDigestWriter::default();
    let mut stderr = Vec::new();
    let status = stab_cli::run_from(
        args,
        DETECT_BASIC_FIXTURE.as_bytes(),
        &mut stdout,
        &mut stderr,
    );
    (status, stderr, stdout)
}

fn ensure_detect_cli_output(
    row: &BenchmarkRow,
    expected: OutputWitness,
    actual: &DetectCliOutput,
) -> Result<(), BenchError> {
    if actual.0 != 0 {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "stab-cli detect failed with status {}: {}",
                actual.0,
                String::from_utf8_lossy(&actual.1)
            ),
        });
    }
    let actual_witness = actual.2.witness();
    if actual_witness != expected {
        return Err(stab_runner_error(
            &row.id,
            format!("detect PTB64 output changed: expected {expected:?}, got {actual_witness:?}"),
        ));
    }
    Ok(())
}

fn measure_detection_plan_compile(
    row: &BenchmarkRow,
    circuit: &Circuit,
) -> Result<Measurement, BenchError> {
    measure_stab_preflighted_compile_and_release(
        "stab_detection_plan_compile_and_release_basic",
        super::STAB_COMPARE_ITERATIONS,
        || {
            DetectionSamplingCompiler::new()
                .compile(black_box(circuit))
                .map_err(|error| stab_runner_error(&row.id, error))
        },
        |plan| {
            ensure_detection_plan_witness(
                row,
                (
                    plan.measurement_width().get(),
                    plan.detector_width().get(),
                    plan.observable_width().get(),
                ),
            )
        },
        || {
            DetectionSamplingCompiler::new()
                .compile(black_box(circuit))
                .map_err(|error| stab_runner_error(&row.id, error))
        },
        || {
            DetectionSamplingCompiler::new()
                .compile(black_box(circuit))
                .map_err(|error| stab_runner_error(&row.id, error))
        },
    )
}

fn measure_detection_session(
    row: &BenchmarkRow,
    circuit: &Circuit,
) -> Result<Measurement, BenchError> {
    let plan = DetectionSamplingCompiler::new()
        .compile(circuit)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut preflight_session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut preflight_sink = DetectionDigestSink::default();
    let preflight_summary = preflight_session
        .run(ShotCount::new(DETECT_SHOTS as u64), &mut preflight_sink)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    ensure_detection_phase_witness(
        row,
        "detection session preflight",
        DETECTION_PHASE_FIRST_WITNESS,
        preflight_summary.committed_shots().get(),
        preflight_sink.witness(),
    )?;
    let session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let sink = DetectionDigestSink::default();
    let mut memory_session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut memory_sink = DetectionDigestSink::default();
    let mut timing_state = (
        session,
        sink,
        Vec::with_capacity(super::STAB_COMPARE_ITERATIONS),
    );
    let measurement = measure_stab_iterations_with_postprocess_and_memory_operation(
        "stab_detection_session_sample_to_detection",
        super::STAB_COMPARE_ITERATIONS,
        &mut timing_state,
        |state| {
            state
                .0
                .run(ShotCount::new(DETECT_SHOTS as u64), &mut state.1)
                .map_err(|error| stab_runner_error(&row.id, error))
        },
        |state, summary| {
            let actual = state.1.witness();
            if summary.committed_shots().get() != DETECT_SHOTS as u64
                || actual.1 != DETECT_SHOTS as u64
            {
                return Err(stab_runner_error(
                    &row.id,
                    format!(
                        "detection session committed {} shots and witnessed {} instead of {DETECT_SHOTS}",
                        summary.committed_shots().get(),
                        actual.1
                    ),
                ));
            }
            state.2.push([actual.0, actual.1]);
            state.1.reset();
            black_box(actual);
            Ok(())
        },
        || {
            let summary = memory_session
                .run(ShotCount::new(DETECT_SHOTS as u64), &mut memory_sink)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let actual = memory_sink.witness();
            ensure_detection_phase_witness(
                row,
                "detection session memory operation",
                DETECTION_PHASE_FIRST_WITNESS,
                summary.committed_shots().get(),
                actual,
            )?;
            black_box(actual);
            Ok(())
        },
    )?;
    ensure_detection_sequence_witness(row, &timing_state.2)?;
    Ok(measurement)
}

fn measure_m2d_plan_compile(
    row: &BenchmarkRow,
    circuit: &Circuit,
) -> Result<Measurement, BenchError> {
    measure_stab_preflighted_compile_and_release(
        "stab_m2d_plan_compile_and_release_basic",
        super::STAB_COMPARE_ITERATIONS,
        || {
            MeasurementToDetectionCompiler::new()
                .compile(black_box(circuit))
                .map_err(|error| stab_runner_error(&row.id, error))
        },
        |plan| {
            ensure_m2d_plan_witness(
                row,
                (
                    plan.measurement_width().get(),
                    plan.sweep_width().get(),
                    plan.detector_width().get(),
                    plan.observable_width().get(),
                ),
            )
        },
        || {
            MeasurementToDetectionCompiler::new()
                .compile(black_box(circuit))
                .map_err(|error| stab_runner_error(&row.id, error))
        },
        || {
            MeasurementToDetectionCompiler::new()
                .compile(black_box(circuit))
                .map_err(|error| stab_runner_error(&row.id, error))
        },
    )
}

fn ensure_detection_plan_witness(
    row: &BenchmarkRow,
    actual: (usize, usize, usize),
) -> Result<(), BenchError> {
    if actual == (1, 1, 0) {
        return Ok(());
    }
    Err(stab_runner_error(
        &row.id,
        format!("detection compile plan dimensions changed: expected (1, 1, 0), got {actual:?}"),
    ))
}

fn ensure_m2d_plan_witness(
    row: &BenchmarkRow,
    actual: (usize, usize, usize, usize),
) -> Result<(), BenchError> {
    if actual == (1, 0, 1, 0) {
        return Ok(());
    }
    Err(stab_runner_error(
        &row.id,
        format!("m2d compile plan dimensions changed: expected (1, 0, 1, 0), got {actual:?}"),
    ))
}

fn ensure_detection_sequence_witness(
    row: &BenchmarkRow,
    witnesses: &[[u64; 2]],
) -> Result<(), BenchError> {
    let actual = u64_sequence_digest(DETECTION_PHASE_SEQUENCE_DOMAIN, witnesses);
    if actual == DETECTION_PHASE_SEQUENCE_DIGEST {
        return Ok(());
    }
    Err(stab_runner_error(
        &row.id,
        format!(
            "detection session ordered witness digest changed: expected {DETECTION_PHASE_SEQUENCE_DIGEST}, got {actual}"
        ),
    ))
}

fn measure_m2d_session_batch(
    row: &BenchmarkRow,
    circuit: &Circuit,
) -> Result<Measurement, BenchError> {
    let source_records = m2d_measurements(&row.id, circuit)?;
    if source_records.is_empty() {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: "m2d phase benchmark fixture contains no records".to_owned(),
        });
    }
    let records = source_records
        .iter()
        .cycle()
        .take(M2D_PHASE_BATCH_SHOTS)
        .cloned()
        .collect::<Vec<_>>();
    let width =
        measurement_record_count(circuit).map_err(|error| stab_runner_error(&row.id, error))?;
    let batch = PackedShotBatch::from_records(&records, width)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let plan = MeasurementToDetectionCompiler::new()
        .compile(circuit)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut preflight_session = plan
        .session()
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut preflight_sink = DetectionDigestSink::default();
    let preflight_summary = preflight_session
        .run(
            MeasurementBatchView::new(batch.view()),
            None,
            &mut preflight_sink,
        )
        .map_err(|error| stab_runner_error(&row.id, error))?;
    ensure_detection_phase_witness(
        row,
        "m2d session preflight",
        M2D_PHASE_WITNESS,
        preflight_summary.committed_shots().get(),
        preflight_sink.witness(),
    )?;
    let session = plan
        .session()
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let sink = DetectionDigestSink::default();
    let mut memory_session = plan
        .session()
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut memory_sink = DetectionDigestSink::default();
    let mut timing_state = (session, sink);
    measure_stab_iterations_with_postprocess_and_memory_operation(
        "stab_m2d_session_convert_batch",
        super::STAB_COMPARE_ITERATIONS,
        &mut timing_state,
        |state| {
            state
                .0
                .run(MeasurementBatchView::new(batch.view()), None, &mut state.1)
                .map_err(|error| stab_runner_error(&row.id, error))
        },
        |state, summary| {
            let actual = state.1.witness();
            ensure_detection_phase_witness(
                row,
                "m2d session",
                M2D_PHASE_WITNESS,
                summary.committed_shots().get(),
                actual,
            )?;
            state.1.reset();
            black_box(actual);
            Ok(())
        },
        || {
            let summary = memory_session
                .run(
                    MeasurementBatchView::new(batch.view()),
                    None,
                    &mut memory_sink,
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let actual = memory_sink.witness();
            ensure_detection_phase_witness(
                row,
                "m2d session memory operation",
                M2D_PHASE_WITNESS,
                summary.committed_shots().get(),
                actual,
            )?;
            black_box(actual);
            Ok(())
        },
    )
}

fn ensure_detection_phase_witness(
    row: &BenchmarkRow,
    phase: &str,
    expected: (u64, u64),
    committed_shots: u64,
    actual: (u64, u64),
) -> Result<(), BenchError> {
    if committed_shots == expected.1 && actual == expected {
        return Ok(());
    }
    Err(stab_runner_error(
        &row.id,
        format!(
            "{phase} witness changed: expected digest/shots {expected:?}, got {actual:?} with {committed_shots} committed shots"
        ),
    ))
}

fn m2d_measurements(row_id: &str, circuit: &Circuit) -> Result<Vec<Vec<bool>>, BenchError> {
    let width =
        measurement_record_count(circuit).map_err(|error| stab_runner_error(row_id, error))?;
    read_records(M2D_BASIC_MEASUREMENTS, SampleFormat::ZeroOne, width)
        .map_err(|error| stab_runner_error(row_id, error))
}

fn parse_circuit(row_id: &str, text: &str) -> Result<Circuit, BenchError> {
    Circuit::from_stim_str(text).map_err(|error| stab_runner_error(row_id, error))
}

fn repo_path(root: &RepoRoot, relative: &str) -> PathBuf {
    root.path.join(relative)
}

fn obs_out_path(root: &RepoRoot) -> PathBuf {
    repo_path(
        root,
        "target/benchmarks/cli-scratch/m9-m2d-sweep-obs-out.b8",
    )
}

fn sweep_ptb64_path(root: &RepoRoot) -> PathBuf {
    repo_path(
        root,
        "target/benchmarks/cli-scratch/pf3-m2d-sweep-ptb64.sweep",
    )
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
