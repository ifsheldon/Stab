//! First-principles oracle for measurement-to-detection conversion.
//!
//! The expected detector and observable bits are derived in this file from the circuit's
//! semantics: each output is the parity of its measured bits, XOR the parity of the flips that
//! the shot's sweep bits apply to those measurements, XOR (only with a reference sample) the
//! parity of the noiseless reference measurements. Nothing here calls the conversion engine to
//! produce expectations, so the engine is compared against an independent model.

use rand::RngExt as _;
use stab_records::BitPlaneBatch;

use super::*;

/// Measurements: m0=q0, m1=q1, then m2=q0, m3=q1, m4=q2 after sweep-controlled X flips, then
/// m5=q3 after a sweep-controlled Z sandwiched in Hadamards. `sweep[3]` targets an unmeasured
/// qubit and must not leak into any output.
const ORACLE_CIRCUIT: &str = "X 1
M 0 1
CX sweep[0] 0
CX sweep[1] 1 sweep[1] 2
CX sweep[3] 4
M 0 1 2
H 3
CZ sweep[2] 3
H 3
M 3
DETECTOR rec[-6] rec[-4]
DETECTOR rec[-5] rec[-3]
DETECTOR rec[-2]
DETECTOR rec[-1]
DETECTOR rec[-5]
OBSERVABLE_INCLUDE(0) rec[-4] rec[-1]
OBSERVABLE_INCLUDE(1) rec[-3]
OBSERVABLE_INCLUDE(2) Z0
";
const MEASUREMENTS: usize = 6;
const SWEEPS: usize = 4;
/// Noiseless measurement results with every sweep bit false.
const REFERENCE: [bool; MEASUREMENTS] = [false, true, false, true, false, false];
const DETECTORS: [&[usize]; 5] = [&[0, 2], &[1, 3], &[4], &[5], &[1]];
const MEASUREMENT_OBSERVABLES: [&[usize]; 2] = [&[2, 5], &[3]];

/// Which measurements each sweep bit flips (its controlled Pauli anticommutes with them).
fn sweep_flips(sweep: &[bool]) -> [bool; MEASUREMENTS] {
    [false, false, sweep[0], sweep[1], sweep[1], sweep[2]]
}

fn expected_record(
    measurements: &[bool],
    sweep: &[bool],
    mode: ReferenceSampleMode,
) -> DetectionRecordBuffer {
    let flips = sweep_flips(sweep);
    let expected_measurement = |index: usize| {
        flips[index] ^ (mode == ReferenceSampleMode::UseReferenceSample && REFERENCE[index])
    };
    let parity = |indices: &[usize]| {
        indices.iter().fold(false, |parity, &index| {
            parity ^ measurements[index] ^ expected_measurement(index)
        })
    };
    let mut observables = MEASUREMENT_OBSERVABLES.map(parity).to_vec();
    // `Z0` reads the frame: only the `sweep[0]` X flip on qubit 0 anticommutes with it, and the
    // noiseless state is a +1 eigenstate in both reference modes.
    observables.push(sweep[0]);
    DetectionRecordBuffer {
        detectors: DETECTORS.map(parity).to_vec(),
        observables,
    }
}

fn random_records(rng: &mut SmallRng, shots: usize, width: usize) -> Vec<Vec<bool>> {
    (0..shots)
        .map(|_| (0..width).map(|_| rng.random::<bool>()).collect())
        .collect()
}

#[test]
fn conversion_matches_first_principles_oracle_for_packed_and_bit_plane_input() {
    assert_send_sync::<MeasurementToDetectionPlan>();
    assert_send_sync::<DetectionSamplingPlan>();

    let circuit = circuit(ORACLE_CIRCUIT);
    let mut rng = SmallRng::seed_from_u64(0x0A11_0C1E);
    for shots in [1, 63, 64, 65, 129, 1000] {
        let measurements = random_records(&mut rng, shots, MEASUREMENTS);
        let sweeps = random_records(&mut rng, shots, SWEEPS);
        for mode in [
            ReferenceSampleMode::UseReferenceSample,
            ReferenceSampleMode::SkipReferenceSample,
        ] {
            let expected = measurements
                .iter()
                .zip(&sweeps)
                .map(|(measurement, sweep)| expected_record(measurement, sweep, mode))
                .collect::<Vec<_>>();
            let plan = MeasurementToDetectionCompiler {
                reference_sample_mode: mode,
                ..MeasurementToDetectionCompiler::default()
            }
            .compile(&circuit)
            .expect("compile oracle conversion");
            let shared = plan.clone();
            assert_eq!(shared.measurement_width().get(), MEASUREMENTS);
            assert_eq!(shared.sweep_width().get(), SWEEPS);
            assert_eq!(shared.detector_width().get(), DETECTORS.len());
            assert_eq!(
                shared.observable_width().get(),
                MEASUREMENT_OBSERVABLES.len() + 1
            );
            let label = format!("{shots} shots, {mode:?}");

            // One whole packed request; a request carries at most one 64-shot batch.
            let mut session = shared
                .session(&Default::default())
                .expect("create conversion session");
            if shots <= 64 {
                let packed_measurements = packed(&measurements, MEASUREMENTS);
                let packed_sweeps = packed(&sweeps, SWEEPS);
                let mut sink = CollectSink::default();
                let summary = session
                    .run(
                        MeasurementBatchView::new(packed_measurements.view()),
                        Some(MeasurementBatchView::new(packed_sweeps.view())),
                        &mut sink,
                    )
                    .expect("convert packed request");
                assert_eq!(summary.committed_shots(), ShotCount::new(shots as u64));
                assert_eq!(sink.records, expected, "packed: {label}");
                assert_eq!(sink.batch_sizes, vec![shots], "{label}");
                assert_eq!(sink.finish_count, 1, "{label}");
            }

            // Irregular packed chunks through one transaction, then irregular multi-word
            // bit-plane chunks through another on the same reused session.
            for bit_planes in [false, true] {
                let chunk_sizes = if bit_planes {
                    [256, 65, 130]
                } else {
                    [17, 1, 256]
                };
                let mut sink = CollectSink::default();
                let mut transaction = session
                    .start_transaction(&mut sink)
                    .expect("start chunked conversion");
                let mut start = 0;
                let mut expected_batches = Vec::new();
                for chunk in chunk_sizes.iter().cycle() {
                    if start == shots {
                        break;
                    }
                    let end = (start + chunk).min(shots);
                    let chunk_measurements = packed(&measurements[start..end], MEASUREMENTS);
                    let chunk_sweeps = packed(&sweeps[start..end], SWEEPS);
                    if bit_planes {
                        let measurement_planes =
                            BitPlaneBatch::from_shot_major(chunk_measurements.view())
                                .expect("transpose measurements");
                        let sweep_planes = BitPlaneBatch::from_shot_major(chunk_sweeps.view())
                            .expect("transpose sweeps");
                        transaction
                            .write_batch_with_sweep(
                                MeasurementBatchView::from_bit_planes(measurement_planes.view()),
                                Some(MeasurementBatchView::from_bit_planes(sweep_planes.view())),
                            )
                            .expect("convert bit-plane chunk");
                    } else {
                        transaction
                            .write_batch_with_sweep(
                                MeasurementBatchView::new(chunk_measurements.view()),
                                Some(MeasurementBatchView::new(chunk_sweeps.view())),
                            )
                            .expect("convert packed chunk");
                    }
                    expected_batches.push(end - start);
                    start = end;
                }
                transaction.finish().expect("finish chunked conversion");
                assert_eq!(
                    sink.records, expected,
                    "chunked (bit planes: {bit_planes}): {label}"
                );
                assert_eq!(sink.batch_sizes, expected_batches, "{label}");
                assert_eq!(sink.finish_count, 1, "{label}");
            }
        }
    }
}

/// Rounds of the windowed oracle circuit; three measurements per round.
const WINDOWED_ROUNDS: usize = 300;

/// Without sweep correction a batch converts through windows of planes. This circuit's
/// detectors and observables read measurements up to most of the record back, so with 64- and
/// 192-plane windows they straddle many hand-overs; `X 1` makes every second-qubit reference
/// bit one. Expectations are parities of input bits, XOR reference bits when used.
const WINDOWED_ORACLE_CIRCUIT: &str = "R 0 1 2
X 1
REPEAT 300 {
    M 0 1 2
    DETECTOR rec[-1] rec[-2]
}
DETECTOR rec[-900] rec[-1]
DETECTOR rec[-451] rec[-2] rec[-899]
OBSERVABLE_INCLUDE(0) rec[-450] rec[-3]
OBSERVABLE_INCLUDE(1) rec[-898]
";

fn windowed_expected(measurements: &[bool], mode: ReferenceSampleMode) -> DetectionRecordBuffer {
    let count = 3 * WINDOWED_ROUNDS;
    let bit = |index: usize| {
        measurements[index] ^ (mode == ReferenceSampleMode::UseReferenceSample && index % 3 == 1)
    };
    let parity = |indices: &[usize]| indices.iter().fold(false, |parity, &i| parity ^ bit(i));
    let mut detectors = (0..WINDOWED_ROUNDS)
        .map(|round| parity(&[3 * round + 2, 3 * round + 1]))
        .collect::<Vec<_>>();
    detectors.push(parity(&[0, count - 1]));
    detectors.push(parity(&[count - 451, count - 2, 1]));
    DetectionRecordBuffer {
        detectors,
        observables: vec![parity(&[count - 450, count - 3]), parity(&[2])],
    }
}

#[test]
fn windowed_conversion_matches_first_principles_across_window_boundaries() {
    let circuit = circuit(WINDOWED_ORACLE_CIRCUIT);
    let width = 3 * WINDOWED_ROUNDS;
    let mut rng = SmallRng::seed_from_u64(0x0A11_0C1F);
    let measurements = random_records(&mut rng, 600, width);
    for window in [64, 192] {
        crate::detection::frame::with_window_planes(window, || {
            for mode in [
                ReferenceSampleMode::UseReferenceSample,
                ReferenceSampleMode::SkipReferenceSample,
            ] {
                let plan = MeasurementToDetectionCompiler {
                    reference_sample_mode: mode,
                    ..MeasurementToDetectionCompiler::default()
                }
                .compile(&circuit)
                .expect("compile windowed oracle conversion");
                let mut session = plan
                    .session(&Default::default())
                    .expect("create conversion session");
                for bit_planes in [false, true] {
                    let mut sink = CollectSink::default();
                    let mut transaction = session
                        .start_transaction(&mut sink)
                        .expect("start conversion");
                    // Batch sizes grow, as the CLI's do, and end in a partial 64-shot block.
                    let mut start = 0;
                    for chunk in [1, 63, 64, 256, 216] {
                        let end = start + chunk;
                        let records = packed(&measurements[start..end], width);
                        if bit_planes {
                            let planes = BitPlaneBatch::from_shot_major(records.view())
                                .expect("transpose measurements");
                            transaction
                                .write_batch_with_sweep(
                                    MeasurementBatchView::from_bit_planes(planes.view()),
                                    None,
                                )
                                .expect("convert bit-plane chunk");
                        } else {
                            transaction
                                .write_batch_with_sweep(
                                    MeasurementBatchView::new(records.view()),
                                    None,
                                )
                                .expect("convert packed chunk");
                        }
                        start = end;
                    }
                    transaction.finish().expect("finish conversion");
                    let expected = measurements
                        .iter()
                        .map(|record| windowed_expected(record, mode))
                        .collect::<Vec<_>>();
                    assert!(
                        sink.records == expected,
                        "{window}-plane windows, {mode:?}, bit planes {bit_planes}"
                    );
                }
            }
        });
    }
}
