#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test-only circuits and collected records have dimensions fixed by construction"
)]

use std::convert::Infallible;
use std::fmt::Write as _;

use rand::rngs::SmallRng;
use rand::{RngExt as _, SeedableRng as _};
use stab_model::{Circuit, MemoryBudget};
use stab_records::{
    BitIndex, DetectionBatchView, DetectionSink, MeasurementBatchView, MeasurementSink, ShotIndex,
};

use crate::detection::frame::with_window_planes;
use crate::{
    DetectionConversionLimits, DetectionSamplingCompiler, LaneWidth, RandomPolicy,
    ReferenceSampleMode, SamplingCompiler, Seed, ShotCount,
};

#[derive(Default)]
struct Records(Vec<Vec<bool>>);

impl MeasurementSink for Records {
    type Error = Infallible;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        for shot in 0..batch.shot_count() {
            self.0.push(
                (0..batch.width().get())
                    .map(|bit| {
                        batch
                            .get(ShotIndex(shot), BitIndex(bit))
                            .expect("bit inside the batch")
                    })
                    .collect(),
            );
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl DetectionSink for Records {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        let detectors = batch.detectors();
        let observables = batch.observables();
        for shot in 0..batch.shot_count() {
            let detector_bits = (0..detectors.bits_per_shot()).map(|bit| {
                detectors
                    .get(ShotIndex(shot), BitIndex(bit))
                    .expect("detector inside the batch")
            });
            let observable_bits = (0..observables.bits_per_shot()).map(|bit| {
                observables
                    .get(ShotIndex(shot), BitIndex(bit))
                    .expect("observable inside the batch")
            });
            self.0.push(detector_bits.chain(observable_bits).collect());
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// A random circuit with feedback, record lookbacks across many windows, products, heralds,
/// pads, Pauli observables, and nested repeats; every lookback is valid in the first iteration.
fn random_circuit(rng: &mut SmallRng) -> String {
    let qubits = 5;
    let mut text = String::from("R 0 1 2 3 4\nOBSERVABLE_INCLUDE(1) Z0\n");
    // Measurements recorded before the current point of the first iteration, which bounds every
    // lookback, and in the whole expanded circuit.
    let mut measured = 0_usize;
    let mut expanded = 0_usize;
    while expanded < 1500 {
        let inner = rng.random_range(1..4_usize);
        let outer = rng.random_range(20..60_usize);
        let _ = writeln!(text, "REPEAT {outer} {{");
        text.push_str("H 0\nCX 0 1 2 3\nDEPOLARIZE1(0.1) 0 1 2 3 4\n");
        let _ = writeln!(text, "REPEAT {inner} {{");
        let mut body = 0;
        for _ in 0..2 {
            let (instruction, count) = match rng.random_range(0..6) {
                0 => ("MPP X0*Z1 Y2*Y3\n", 2),
                1 => ("HERALDED_ERASE(0.2) 1 3\n", 2),
                2 => ("MPAD 1 0 1\n", 3),
                3 => ("MX(0.1) 2\n", 1),
                _ => ("M(0.05) 0 1 2 3 4\nMR 4\n", 6),
            };
            text.push_str(instruction);
            body += count;
        }
        measured += body;
        expanded += body * inner * outer;
        let near = rng.random_range(1..=measured.min(5));
        let far = rng.random_range(1..=measured);
        let qubit = rng.random_range(0..qubits);
        let _ = writeln!(text, "CX rec[-{near}] {qubit}\nCZ rec[-{far}] {qubit}");
        let _ = writeln!(text, "DETECTOR rec[-1] rec[-{far}]\nDETECTOR rec[-{near}]");
        text.push_str("}\n}\n");
        let far = rng.random_range(1..=measured);
        let _ = writeln!(text, "OBSERVABLE_INCLUDE(0) rec[-{far}]");
    }
    text.push_str("M 0 1 2 3 4\nDETECTOR rec[-1] rec[-5]\n");
    text
}

fn shot_counts(lane_width: LaneWidth) -> [u64; 5] {
    let lanes = lane_width.shots_per_batch() as u64;
    [1, 3, lanes - 1, lanes + 1, 2 * lanes + 5]
}

/// A streaming run hands its planes over in windows; with 64-plane windows every circuit
/// crosses many window boundaries, in the middle of repeat bodies and of record lookbacks, and
/// must still deliver exactly the records of the materializing session's first run.
#[test]
fn streaming_runs_match_materializing_sessions_across_window_boundaries() {
    let mut rng = SmallRng::seed_from_u64(0x5eed);
    with_window_planes(64, || {
        for case in 0..6_u64 {
            let text = random_circuit(&mut rng);
            let circuit = Circuit::from_stim_str(&text).expect("generated circuit parses");
            let sampling = SamplingCompiler::new()
                .compile(&circuit)
                .expect("compile sampling plan");
            let detection = DetectionSamplingCompiler::new()
                .compile(&circuit)
                .expect("compile detection plan");
            assert!(
                sampling.measurement_width().get() > 4 * 64,
                "case {case} must span several windows"
            );
            for lane_width in LaneWidth::ALL.iter().copied() {
                for shots in shot_counts(lane_width) {
                    let policy = RandomPolicy::Seeded(Seed::new(case * 1000 + shots));
                    let shots = ShotCount::new(shots);
                    for mode in [
                        ReferenceSampleMode::UseReferenceSample,
                        ReferenceSampleMode::SkipReferenceSample,
                    ] {
                        let mut expected = Records::default();
                        sampling
                            .session_with_lane_width(policy, mode, lane_width)
                            .expect("materializing session")
                            .run(shots, &mut expected)
                            .expect("materializing run");
                        let mut streamed = Records::default();
                        sampling
                            .streaming_run(policy, mode, lane_width, shots)
                            .expect("streaming run")
                            .run(&mut streamed)
                            .expect("streaming run delivers");
                        assert_eq!(
                            streamed.0, expected.0,
                            "sampling case {case} at {lane_width:?}, {shots:?}, {mode:?}\n{text}"
                        );
                    }
                    let mut expected = Records::default();
                    detection
                        .session_with_lane_width(policy, lane_width)
                        .expect("materializing detection session")
                        .run(shots, &mut expected)
                        .expect("materializing detection run");
                    let mut streamed = Records::default();
                    detection
                        .streaming_run(policy, lane_width, shots)
                        .expect("streaming detection run")
                        .run(&mut streamed)
                        .expect("streaming detection run delivers");
                    assert_eq!(
                        streamed.0, expected.0,
                        "detection case {case} at {lane_width:?}, {shots:?}\n{text}"
                    );
                }
            }
        }
    });
}

/// The transposer works in groups of 64 tiles of 64 planes; with the default window, one
/// hand-over of a record of several thousand planes spans several groups and ends in a partial
/// tile, and the reference flips of every group must land on the right words of every lane.
#[test]
fn streaming_transposes_whole_groups_and_partial_tiles_like_materializing_sessions() {
    let circuit = Circuit::from_stim_str(
        "R 0 1 2\nX 1\nREPEAT 3001 {\n    X_ERROR(0.2) 0 2\n    CX 0 1 2 1\n    M 0 1 2\n    \
         DETECTOR rec[-1] rec[-2]\n    DETECTOR rec[-3]\n}\nOBSERVABLE_INCLUDE(0) rec[-2]\n",
    )
    .expect("parse wide record circuit");
    let sampling = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile sampling plan");
    let detection = DetectionSamplingCompiler::new()
        .compile(&circuit)
        .expect("compile detection plan");
    for lane_width in LaneWidth::ALL.iter().copied() {
        let lanes = lane_width.shots_per_batch() as u64;
        for shots in [lanes, lanes + 7] {
            let policy = RandomPolicy::Seeded(Seed::new(shots));
            let shots = ShotCount::new(shots);
            let mut expected = Records::default();
            sampling
                .session_with_lane_width(
                    policy,
                    ReferenceSampleMode::UseReferenceSample,
                    lane_width,
                )
                .expect("materializing session")
                .run(shots, &mut expected)
                .expect("materializing run");
            let mut streamed = Records::default();
            sampling
                .streaming_run(
                    policy,
                    ReferenceSampleMode::UseReferenceSample,
                    lane_width,
                    shots,
                )
                .expect("streaming run")
                .run(&mut streamed)
                .expect("streaming run delivers");
            assert_eq!(
                streamed.0, expected.0,
                "sampling at {lane_width:?}, {shots:?}"
            );
            let mut expected = Records::default();
            detection
                .session_with_lane_width(policy, lane_width)
                .expect("materializing detection session")
                .run(shots, &mut expected)
                .expect("materializing detection run");
            let mut streamed = Records::default();
            detection
                .streaming_run(policy, lane_width, shots)
                .expect("streaming detection run")
                .run(&mut streamed)
                .expect("streaming detection run delivers");
            assert_eq!(
                streamed.0, expected.0,
                "detection at {lane_width:?}, {shots:?}"
            );
        }
    }
}

/// A streaming run of one shot retains a bounded window and one packed record instead of one
/// plane per measurement for every lane, so a budget far below either materializing session
/// admits it at both lane widths.
#[test]
fn single_shot_streaming_admission_does_not_scale_with_lane_width() {
    let circuit = Circuit::from_stim_str(
        "R 0 1 2\nREPEAT 1000000 {\n    X_ERROR(0.1) 0 2\n    CX 0 1\n    M 0 1\n    DETECTOR rec[-1] rec[-2]\n}\n",
    )
    .expect("parse wide record circuit");
    // Two million measurement planes cost 16 MB at 64 lanes and 64 MB at 256 lanes.
    let budget = MemoryBudget::from_bytes(8 << 20);
    let sampling = SamplingCompiler::new()
        .memory_budget(budget)
        .compile(&circuit)
        .expect("compile sampling plan");
    let detection = DetectionSamplingCompiler::new()
        .limits(DetectionConversionLimits::new(budget))
        .compile(&circuit)
        .expect("compile detection plan");
    let policy = RandomPolicy::Seeded(Seed::new(3));
    for lane_width in LaneWidth::ALL.iter().copied() {
        for mode in [
            ReferenceSampleMode::UseReferenceSample,
            ReferenceSampleMode::SkipReferenceSample,
        ] {
            assert!(
                sampling
                    .session_with_lane_width(policy, mode, lane_width)
                    .is_err(),
                "a materializing session stores every plane at {lane_width:?}"
            );
            let mut streamed = Records::default();
            sampling
                .streaming_run(policy, mode, lane_width, ShotCount::new(1))
                .expect("one streamed shot fits the budget")
                .run(&mut streamed)
                .expect("streaming run delivers");
            assert_eq!(streamed.0.len(), 1);
            assert_eq!(streamed.0[0].len(), 2_000_000);
        }
        assert!(
            detection
                .session_with_lane_width(policy, lane_width)
                .is_err(),
            "a materializing detection session stores every plane at {lane_width:?}"
        );
        let mut streamed = Records::default();
        detection
            .streaming_run(policy, lane_width, ShotCount::new(1))
            .expect("one streamed detection shot fits the budget")
            .run(&mut streamed)
            .expect("streaming detection run delivers");
        assert_eq!(streamed.0.len(), 1);
        assert_eq!(streamed.0[0].len(), 1_000_000);
    }
}

/// 4096 noisy measurements, 2048 detectors, and one observable: 256 lanes of these records hold
/// 16384 measurement words and 8192 detector words, so zeroing them polls many times at
/// [`CREATION_UNITS_PER_POLL`].
const WIDE_RECORDS: &str = "\
X_ERROR(0.3) 0 1 2 3
REPEAT 1024 {
    M(0.1) 0 1 2 3
    DETECTOR rec[-1] rec[-2]
    DETECTOR rec[-3]
}
OBSERVABLE_INCLUDE(0) rec[-1]
";

const CREATION_UNITS_PER_POLL: u32 = 512;

/// Creating a streaming sampling or detection run zeroes its shot-major records in polled
/// chunks: a cancellation landing among those polls fails creation with the typed `Cancelled`
/// error, and a run created afterwards delivers exactly the uninterrupted seeded records.
#[test]
fn streaming_run_creation_polls_while_zeroing_its_records() {
    use crate::work_poll::test_hook;
    use crate::{DetectionExecutionError, SamplingExecutionError};

    let circuit = Circuit::from_stim_str(WIDE_RECORDS).expect("fixture parses");
    let sampling = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile sampling plan");
    let detection = DetectionSamplingCompiler::new()
        .compile(&circuit)
        .expect("compile detection plan");
    let policy = RandomPolicy::Seeded(Seed::new(11));
    let mode = ReferenceSampleMode::SkipReferenceSample;
    let lane_width = LaneWidth::Lanes256;
    let shots = ShotCount::new(300);
    let cancelled = stab_model::Cancelled;

    let mut expected = Records::default();
    sampling
        .streaming_run(policy, mode, lane_width, shots)
        .expect("streaming run")
        .run(&mut expected)
        .expect("streaming run delivers");
    let mut expected_detection = Records::default();
    detection
        .streaming_run(policy, lane_width, shots)
        .expect("streaming detection run")
        .run(&mut expected_detection)
        .expect("streaming detection run delivers");

    test_hook::with_units_per_poll(CREATION_UNITS_PER_POLL, || {
        let (created, polls) =
            test_hook::count_polls(|| sampling.streaming_run(policy, mode, lane_width, shots));
        drop(created.expect("counted streaming run"));
        // 16384 words at 4 units each, one poll per 512 units.
        assert!(polls >= 100, "zeroing the records polled {polls} times");
        let (interrupted, fired) = test_hook::cancel_at_poll(polls / 2, || {
            sampling
                .streaming_run(policy, mode, lane_width, shots)
                .map(drop)
        });
        assert!(fired);
        assert_eq!(
            interrupted,
            Err(SamplingExecutionError::Cancelled(cancelled))
        );
        let mut retried = Records::default();
        sampling
            .streaming_run(policy, mode, lane_width, shots)
            .expect("streaming run after cancellation")
            .run(&mut retried)
            .expect("retried run delivers");
        assert!(retried.0 == expected.0, "retried streaming run diverged");

        let (created, polls) =
            test_hook::count_polls(|| detection.streaming_run(policy, lane_width, shots));
        drop(created.expect("counted streaming detection run"));
        assert!(
            polls >= 50,
            "zeroing the detection records polled {polls} times"
        );
        let (interrupted, fired) = test_hook::cancel_at_poll(polls / 2, || {
            detection.streaming_run(policy, lane_width, shots).map(drop)
        });
        assert!(fired);
        assert!(
            matches!(interrupted, Err(DetectionExecutionError::Cancelled(_))),
            "{interrupted:?}"
        );
        let mut retried = Records::default();
        detection
            .streaming_run(policy, lane_width, shots)
            .expect("streaming detection run after cancellation")
            .run(&mut retried)
            .expect("retried detection run delivers");
        assert!(
            retried.0 == expected_detection.0,
            "retried streaming detection run diverged"
        );
    });
}

/// A windowed measurement-to-detection session zeroes its records when its first full batch
/// arrives, in polled chunks before any other work of that batch: cancellation there reports a
/// cancelled batch without poisoning the session, and converting the batch again delivers what
/// an uninterrupted session does.
#[test]
fn windowed_conversion_polls_while_zeroing_its_records() {
    use stab_records::PackedShotBatch;

    use crate::work_poll::test_hook;
    use crate::{DetectionRunStatus, MAX_BATCH_SHOTS, MeasurementToDetectionCompiler};

    let circuit = Circuit::from_stim_str(WIDE_RECORDS).expect("fixture parses");
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile conversion plan");
    let mut rng = SmallRng::seed_from_u64(7);
    let measurements = (0..MAX_BATCH_SHOTS)
        .map(|_| (0..4096).map(|_| rng.random_bool(0.5)).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let measurements = PackedShotBatch::from_records(&measurements, 4096).expect("pack input");
    let input = || stab_records::MeasurementBatchView::new(measurements.view());

    let mut expected = Records::default();
    let mut uninterrupted = plan.session().expect("conversion session");
    uninterrupted
        .run(input(), None, &mut expected)
        .expect("convert a batch");

    test_hook::with_units_per_poll(CREATION_UNITS_PER_POLL, || {
        let mut counted = plan.session().expect("conversion session");
        let (first, first_polls) =
            test_hook::count_polls(|| counted.run(input(), None, &mut Records::default()));
        first.expect("first batch");
        let (later, later_polls) =
            test_hook::count_polls(|| counted.run(input(), None, &mut Records::default()));
        later.expect("later batch");
        // 8192 detector words at 4 units each, one poll per 512 units.
        assert!(
            first_polls >= later_polls + 50,
            "zeroing polled {first_polls} - {later_polls} times"
        );

        let mut session = plan.session().expect("conversion session");
        let mut delivered = Records::default();
        let (interrupted, fired) =
            test_hook::cancel_at_poll(1, || session.run(input(), None, &mut delivered));
        assert!(fired);
        let summary = interrupted.expect("cancellation is a status, not an error");
        assert_eq!(summary.status(), DetectionRunStatus::Cancelled);
        assert!(delivered.0.is_empty());
        assert!(!session.is_poisoned());
        session
            .run(input(), None, &mut delivered)
            .expect("convert the batch again");
        assert!(delivered.0 == expected.0, "retried conversion diverged");
    });
}

/// A measurement-to-detection session with sweep correction keeps full-width planes: creating
/// it zeroes them in polled chunks, so cancellation there fails creation with the typed error,
/// and a batch cancelled inside its plane passes reports a cancelled batch, leaves the session
/// usable, and converts again to what an uninterrupted session delivers.
#[test]
fn sweep_corrected_conversion_polls_its_full_width_passes() {
    use stab_records::PackedShotBatch;

    use crate::work_poll::test_hook;
    use crate::{
        DetectionExecutionError, DetectionRunStatus, MAX_BATCH_SHOTS,
        MeasurementToDetectionCompiler,
    };

    let circuit =
        Circuit::from_stim_str(&format!("CX sweep[0] 0\n{WIDE_RECORDS}")).expect("fixture parses");
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile conversion plan");
    let mut rng = SmallRng::seed_from_u64(9);
    let measurements = (0..MAX_BATCH_SHOTS)
        .map(|_| (0..4096).map(|_| rng.random_bool(0.5)).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let measurements = PackedShotBatch::from_records(&measurements, 4096).expect("pack input");
    let input = || stab_records::MeasurementBatchView::new(measurements.view());

    let mut expected = Records::default();
    plan.session()
        .expect("conversion session")
        .run(input(), None, &mut expected)
        .expect("convert a batch");

    test_hook::with_units_per_poll(CREATION_UNITS_PER_POLL, || {
        let (created, polls) = test_hook::count_polls(|| plan.session());
        drop(created.expect("counted conversion session"));
        // 6144 frame planes at 16 units and 4096 column words at 4 units, one poll per 512.
        assert!(polls >= 200, "zeroing the planes polled {polls} times");
        let (interrupted, fired) =
            test_hook::cancel_at_poll(polls / 2, || plan.session().map(drop));
        assert!(fired);
        assert!(
            matches!(interrupted, Err(DetectionExecutionError::Cancelled(_))),
            "{interrupted:?}"
        );

        let mut counted = plan.session().expect("conversion session");
        let (first, batch_polls) =
            test_hook::count_polls(|| counted.run(input(), None, &mut Records::default()));
        first.expect("counted batch");
        for fire_at in [1, batch_polls / 2] {
            let mut session = plan.session().expect("conversion session");
            let mut delivered = Records::default();
            let (interrupted, fired) =
                test_hook::cancel_at_poll(fire_at, || session.run(input(), None, &mut delivered));
            assert!(fired);
            let summary = interrupted.expect("cancellation is a status, not an error");
            assert_eq!(summary.status(), DetectionRunStatus::Cancelled);
            assert!(delivered.0.is_empty());
            assert!(!session.is_poisoned());
            session
                .run(input(), None, &mut delivered)
                .expect("convert the batch again");
            assert!(delivered.0 == expected.0, "retried conversion diverged");
        }
    });
}
