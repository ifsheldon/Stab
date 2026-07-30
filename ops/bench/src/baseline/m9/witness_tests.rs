#![allow(
    clippy::expect_used,
    reason = "focused benchmark witness tests use direct assertions"
)]

use super::DETECT_SHOTS;
use super::witness::{
    DetectSweepExpectation, DetectSweepWitness, M2dCliWitness, ensure_detect_sweep_witness,
    ensure_m2d_cli_witness, m2d_sweep_b8_expected, m2d_sweep_ptb64_expected,
};
use crate::baseline::batch_sinks::OutputWitness;

#[test]
fn m2d_rejects_same_width_wrong_content() {
    let expected = m2d_sweep_b8_expected();
    let actual = M2dCliWitness {
        stdout: OutputWitness::new(expected.stdout.bytes, expected.stdout.digest ^ 1),
        side_output: expected.side_output,
    };

    ensure_m2d_cli_witness("pf3-m2d-sweep-b8", expected, actual)
        .expect_err("same-width m2d output with changed content must be rejected");
}

#[test]
fn ptb64_expectation_is_fixed_by_the_measurement_xor_sweep_equation() {
    assert_eq!(
        m2d_sweep_ptb64_expected().stdout,
        OutputWitness::new(64, 0x432e_52c3_e6ee_3f4a)
    );
}

#[test]
fn random_detection_witness_rejects_degenerate_same_width_output() {
    let all_false = DetectSweepWitness {
        shots: DETECT_SHOTS,
        detector_bits: DETECT_SHOTS,
        detector_ones: 0,
        observable_bits: 0,
        observable_ones: 0,
    };

    ensure_detect_sweep_witness(
        "pf3-detect-sweep-sampling",
        DetectSweepExpectation::FairDetector,
        all_false,
    )
    .expect_err("same-width all-false output is not a fair detector sample");
}

#[test]
fn deterministic_frame_witness_rejects_a_wrong_observable_bit() {
    let wrong = DetectSweepWitness {
        shots: DETECT_SHOTS,
        detector_bits: 0,
        detector_ones: 0,
        observable_bits: DETECT_SHOTS,
        observable_ones: 1,
    };

    ensure_detect_sweep_witness(
        "pf3-detect-sweep-sampling",
        DetectSweepExpectation::DeterministicFalseObservable,
        wrong,
    )
    .expect_err("same-width frame output with a true observable must be rejected");
}
