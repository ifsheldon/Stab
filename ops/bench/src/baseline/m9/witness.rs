use std::ffi::OsString;
use std::path::Path;

use stab_core::advanced::compat::try_for_each_sampled_detection_event;
use stab_core::{Circuit, CircuitError};

use crate::error::BenchError;

use super::{
    DETECT_SHOTS, M2D_SWEEP_B8_MEASUREMENTS, M2D_SWEEP_B8_SWEEP, SWEEP_PTB64_SHOTS,
    SWEEP_PTB64_WIDTH,
};
use crate::baseline::{
    batch_sinks::{ByteDigestWriter, OutputWitness},
    stab_runner_error,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DetectSweepExpectation {
    FairDetector,
    DeterministicFalseObservable,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct DetectSweepWitness {
    pub(super) shots: usize,
    pub(super) detector_bits: usize,
    pub(super) detector_ones: usize,
    pub(super) observable_bits: usize,
    pub(super) observable_ones: usize,
}

pub(super) fn sample_detect_sweep_witness(
    row_id: &str,
    circuit: &Circuit,
) -> Result<DetectSweepWitness, BenchError> {
    let mut witness = DetectSweepWitness::default();
    try_for_each_sampled_detection_event::<CircuitError, _>(
        circuit,
        DETECT_SHOTS,
        Some(17),
        |record| {
            witness.shots += 1;
            witness.detector_bits += record.detectors.len();
            witness.detector_ones += record.detectors.iter().filter(|bit| **bit).count();
            witness.observable_bits += record.observables.len();
            witness.observable_ones += record.observables.iter().filter(|bit| **bit).count();
            Ok(())
        },
    )
    .map_err(|error| stab_runner_error(row_id, error))?;
    Ok(witness)
}

pub(super) fn ensure_detect_sweep_witness(
    row_id: &str,
    expectation: DetectSweepExpectation,
    actual: DetectSweepWitness,
) -> Result<(), BenchError> {
    let shape_matches = match expectation {
        DetectSweepExpectation::FairDetector => {
            actual.shots == DETECT_SHOTS
                && actual.detector_bits == DETECT_SHOTS
                && actual.observable_bits == 0
                && actual.observable_ones == 0
        }
        DetectSweepExpectation::DeterministicFalseObservable => {
            actual.shots == DETECT_SHOTS
                && actual.detector_bits == 0
                && actual.detector_ones == 0
                && actual.observable_bits == DETECT_SHOTS
                && actual.observable_ones == 0
        }
    };
    let distribution_matches = expectation != DetectSweepExpectation::FairDetector
        || if DETECT_SHOTS < 64 {
            actual.detector_ones > 0 && actual.detector_ones < DETECT_SHOTS
        } else {
            actual.detector_ones >= DETECT_SHOTS / 4 && actual.detector_ones <= DETECT_SHOTS * 3 / 4
        };
    if shape_matches && distribution_matches {
        return Ok(());
    }
    Err(stab_runner_error(
        row_id,
        format!(
            "sweep-conditioned detection semantic witness changed for {expectation:?}: got {actual:?}"
        ),
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct M2dCliWitness {
    pub(super) stdout: OutputWitness,
    pub(super) side_output: Option<OutputWitness>,
}

pub(super) fn m2d_sweep_01_expected() -> M2dCliWitness {
    m2d_cli_witness(b"shot\nshot D0\nshot D0\nshot\n", None)
}

pub(super) fn m2d_sweep_b8_expected() -> M2dCliWitness {
    let expected = M2D_SWEEP_B8_MEASUREMENTS
        .iter()
        .zip(M2D_SWEEP_B8_SWEEP)
        .map(|(measurement, sweep)| measurement ^ sweep)
        .collect::<Vec<_>>();
    m2d_cli_witness(&expected, None)
}

pub(super) fn m2d_sweep_obs_out_expected() -> M2dCliWitness {
    m2d_cli_witness(b"0\n1\n1\n0\n", Some(b""))
}

pub(super) fn m2d_feedback_inline_expected() -> M2dCliWitness {
    m2d_cli_witness(
        b"shot\nshot D0 D1\nshot D1 D2\nshot D2 D3\nshot D3\nshot D3 L0\n",
        None,
    )
}

pub(super) fn m2d_sweep_ptb64_expected() -> M2dCliWitness {
    let mut bytes = [0_u8; SWEEP_PTB64_SHOTS];
    for (shot, output) in bytes.iter_mut().enumerate() {
        for bit in 0..SWEEP_PTB64_WIDTH {
            let measurement = (shot + bit * 2) % 3 == 0;
            let sweep = (shot * 3 + bit) % 5 == 0;
            *output |= u8::from(measurement ^ sweep) << bit;
        }
    }
    m2d_cli_witness(&bytes, None)
}

fn m2d_cli_witness(stdout: &[u8], side_output: Option<&[u8]>) -> M2dCliWitness {
    M2dCliWitness {
        stdout: OutputWitness::from_bytes(stdout),
        side_output: side_output.map(OutputWitness::from_bytes),
    }
}

pub(super) fn ensure_m2d_cli_witness(
    row_id: &str,
    expected: M2dCliWitness,
    actual: M2dCliWitness,
) -> Result<(), BenchError> {
    if actual == expected {
        return Ok(());
    }
    Err(stab_runner_error(
        row_id,
        format!("m2d semantic output changed: expected {expected:?}, got {actual:?}"),
    ))
}

pub(super) fn run_m2d_cli_once(
    row_id: &str,
    args: &[OsString],
    input: &[u8],
    side_output: Option<&Path>,
) -> Result<M2dCliWitness, BenchError> {
    let mut stdout = ByteDigestWriter::default();
    let mut stderr = Vec::new();
    let status = stab_cli::run_from(args.iter().cloned(), input, &mut stdout, &mut stderr);
    if status != 0 {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
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
                    row_id: row_id.to_string(),
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
