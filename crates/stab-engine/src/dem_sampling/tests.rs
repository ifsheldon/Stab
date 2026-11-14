#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "DEM sampling tests use compact fixture assertions"
)]

use std::convert::Infallible;
use std::sync::Arc;

use rand::rngs::SmallRng;
use rand::{RngExt as _, SeedableRng as _};
use stab_bits::BitSlice;

use stab_model::DetectorErrorModel;
use stab_records::{DemSampleBatchView, DemSampleSink};

use super::plan::DemSamplingPlanInner;
use super::program::{
    self, DemSampleBlock, DemSampleError, DemSampleOperation, odd_parity_probability,
};
use super::*;
use crate::{DetectionRecordBuffer, RandomPolicy, Seed, ShotCount};

#[derive(Default)]
struct CollectingSink {
    records: Vec<DetectionRecordBuffer>,
    error_records: Vec<Vec<bool>>,
    writes: usize,
    finishes: usize,
}

impl DemSampleSink for CollectingSink {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        self.writes += 1;
        let detection = batch.detection();
        for shot_index in 0..detection.shot_count() {
            let detectors = (0..detection.detector_width().get())
                .map(|bit_index| {
                    detection
                        .detectors()
                        .get(shot_index, bit_index)
                        .expect("detector bit")
                })
                .collect();
            let observables = (0..detection.observable_width().get())
                .map(|bit_index| {
                    detection
                        .observables()
                        .get(shot_index, bit_index)
                        .expect("observable bit")
                })
                .collect();
            self.records.push(DetectionRecordBuffer {
                detectors,
                observables,
            });
            if let Some(sampled_errors) = batch.sampled_errors() {
                self.error_records.push(
                    (0..sampled_errors.bits_per_shot())
                        .map(|bit_index| {
                            sampled_errors
                                .get(shot_index, bit_index)
                                .expect("sampled-error bit")
                        })
                        .collect(),
                );
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finishes += 1;
        Ok(())
    }
}

#[test]
fn odd_parity_probability_matches_repeated_independent_error_parity() {
    assert_eq!(odd_parity_probability(0.0, 1_000_000), 0.0);
    assert_eq!(odd_parity_probability(1.0, 4), 0.0);
    assert_eq!(odd_parity_probability(1.0, 5), 1.0);
    assert!((odd_parity_probability(0.25, 2) - 0.375).abs() < 1e-12);
    assert!((odd_parity_probability(0.5, 64_000_001) - 0.5).abs() < 1e-12);

    let tiny_probability = odd_parity_probability(1e-18, 1_000_000_000_000_000_000);
    let expected_tiny_probability = -0.5 * (-2.0_f64).exp_m1();
    assert!((tiny_probability - expected_tiny_probability).abs() < 1e-12);

    let near_one = 1.0 - 1e-12;
    let near_one_probability = odd_parity_probability(near_one, 1_000_000_000_001);
    let expected_near_one_probability =
        1.0 - odd_parity_probability(1.0 - near_one, 1_000_000_000_001);
    assert!((near_one_probability - expected_near_one_probability).abs() < 1e-12);
}

#[test]
fn replay_admission_does_not_cap_the_shot_count() {
    let plan = DemSamplingPlan {
        inner: Arc::new(DemSamplingPlanInner {
            detector_count: 1,
            observable_count: 1,
            operations: DemSampleBlock {
                error_count: 1,
                ..DemSampleBlock::default()
            },
        }),
    };
    plan.validate_replay(ShotCount::new(u64::MAX))
        .expect("replay work is not capped, so any representable shot count is admitted");
}

#[test]
fn seeded_partitioning_and_replay_preserve_exact_dem_streams() {
    for dem_text in [
        "error(1) D0\n",
        "error(0.25) D0\n",
        "error(0.25) L2\n",
        "error(0.25) D0 D2\nerror(0.25) D2 D3\n",
        "error(0.25) D0\nshift_detectors 1\nrepeat 2 {\n    error(0.25) D0\n    shift_detectors 1\n}\nerror(0) D0\n",
    ] {
        let model = DetectorErrorModel::from_dem_str(dem_text).expect("parse DEM");
        let plan = DemSamplingCompiler::new()
            .compile(&model)
            .expect("compile DEM sampler");

        let mut whole = plan
            .session(RandomPolicy::Seeded(Seed::new(7)))
            .expect("whole session");
        let mut whole_sink = CollectingSink::default();
        whole
            .run_with_sampled_errors(ShotCount::new(65), &mut whole_sink)
            .expect("whole run");

        let mut partitioned = plan
            .session(RandomPolicy::Seeded(Seed::new(7)))
            .expect("partitioned session");
        let mut partitioned_sink = CollectingSink::default();
        partitioned
            .run_with_sampled_errors(ShotCount::new(1), &mut partitioned_sink)
            .expect("first partition");
        partitioned
            .run_with_sampled_errors(ShotCount::new(64), &mut partitioned_sink)
            .expect("second partition");

        assert_eq!(partitioned_sink.records, whole_sink.records);
        assert_eq!(partitioned_sink.error_records, whole_sink.error_records);

        let mut replay = plan
            .replay_session(ShotCount::new(65))
            .expect("replay session");
        let mut replay_sink = CollectingSink::default();
        replay
            .run(&whole_sink.error_records, &mut replay_sink)
            .expect("replay sampled errors");
        assert_eq!(replay_sink.records, whole_sink.records);
        assert_eq!(replay_sink.error_records, whole_sink.error_records);
    }
}

#[test]
fn execution_failure_preserves_first_error_progress_and_poisons_session() {
    let plan = DemSamplingPlan {
        inner: Arc::new(DemSamplingPlanInner {
            detector_count: 1,
            observable_count: 0,
            operations: DemSampleBlock {
                operations: vec![DemSampleOperation::Error(DemSampleError::new(
                    1.0,
                    vec![1],
                    Vec::new(),
                ))],
                error_count: 1,
                direct_sample_effect_count: 1,
                direct_sample_work_count: 1,
                ..DemSampleBlock::default()
            },
        }),
    };
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(3)))
        .expect("construct malformed-program test session");
    let mut sink = CollectingSink::default();
    let error = session
        .run(ShotCount::new(1), &mut sink)
        .expect_err("out-of-range lowered detector must fail execution");
    assert!(
        error
            .to_string()
            .contains("detector index 1 is out of range"),
        "{error}"
    );
    assert_eq!(error.progress().committed_shots().get(), 0);
    assert_eq!(error.progress().attempted_batch_shots().get(), 1);
    assert_eq!(sink.writes, 0);
    assert_eq!(sink.finishes, 0);
    assert!(session.is_poisoned());
}

/// Writes a random DEM block with nested repeats, zero repeat counts, error-free bodies, and
/// detector shifts.
fn write_random_dem_block(rng: &mut SmallRng, depth: usize, text: &mut String) {
    use std::fmt::Write as _;

    for _ in 0..rng.random_range(0..5_u32) {
        match rng.random_range(0..10_u32) {
            0..=4 => {
                text.push_str("error(0.125)");
                for _ in 0..rng.random_range(1..4_u32) {
                    if rng.random_bool(0.2) {
                        write!(text, " L{}", rng.random_range(0..2_u32)).expect("write");
                    } else {
                        write!(text, " D{}", rng.random_range(0..6_u32)).expect("write");
                    }
                }
                text.push('\n');
            }
            5 | 6 => {
                writeln!(text, "shift_detectors {}", rng.random_range(0..4_u32)).expect("write");
            }
            _ if depth < 3 => {
                let repeat_count = [0, 1, 2, 3, 7][rng.random_range(0..5_usize)];
                writeln!(text, "repeat {repeat_count} {{").expect("write");
                write_random_dem_block(rng, depth + 1, text);
                text.push_str("}\n");
            }
            _ => text.push_str("detector D0\n"),
        }
    }
}

type ReplayOutcome = Result<(usize, DetectionRecordBuffer), String>;

fn replay_with<B: replay::ErrorRecordBits + ?Sized>(
    plan: &DemSamplingPlan,
    bits: &B,
    detector_width: usize,
    use_locator: bool,
) -> ReplayOutcome {
    let cancellation = stab_model::Cancellation::new();
    let mut poll = program::DemPoll::new(&cancellation);
    let mut record = DetectionRecordBuffer {
        detectors: Vec::new(),
        observables: Vec::new(),
    };
    replay::reset_detection_record(&mut record, detector_width, plan.observable_count());
    let block = &plan.inner.operations;
    let consumed = if use_locator {
        replay::apply_error_record(block, bits, &mut record, &mut poll)
    } else {
        let mut cursor = 0;
        replay::apply_error_record_block(block, 0, bits, &mut cursor, &mut record, &mut poll)
            .map(|()| cursor)
    };
    consumed
        .map(|consumed| (consumed, record))
        .map_err(|error| error.to_string())
}

#[test]
fn error_locator_replay_matches_the_sequential_walk() {
    let mut rng = SmallRng::seed_from_u64(0x5EED_D3A1);
    let mut replayed_errors = 0_usize;
    let mut failures = 0_usize;
    for _ in 0..300 {
        let mut text = String::new();
        write_random_dem_block(&mut rng, 0, &mut text);
        let model = DetectorErrorModel::from_dem_str(&text).expect("parse random DEM");
        let plan = DemSamplingCompiler::new()
            .compile(&model)
            .expect("compile random DEM");
        let error_count = plan.error_count();
        let lengths = [
            Some(error_count),
            error_count.checked_sub(1),
            Some(error_count + 1),
            Some(error_count + 70),
        ];
        for length in lengths.into_iter().flatten() {
            for density in [0.0, 0.02, 0.4, 1.0] {
                let bools: Vec<bool> = (0..length).map(|_| rng.random_bool(density)).collect();
                // Packed storage carries garbage beyond the logical length, which must not replay.
                let mut words = vec![0_u64; length.div_ceil(64)];
                for (index, bit) in bools.iter().enumerate() {
                    words[index / 64] |= u64::from(*bit) << (index % 64);
                }
                if length % 64 != 0
                    && let Some(last) = words.last_mut()
                {
                    *last |= u64::MAX << (length % 64);
                }
                let packed = BitSlice::new(&words, length).expect("packed record");
                // A narrow detector buffer makes target range failures comparable too.
                for detector_width in [plan.detector_count(), plan.detector_count() / 2] {
                    let expected = replay_with(&plan, bools.as_slice(), detector_width, false);
                    let dense = replay_with(&plan, bools.as_slice(), detector_width, true);
                    let packed = replay_with(&plan, &packed, detector_width, true);
                    assert_eq!(dense, expected, "dense record of {length} bits for\n{text}");
                    assert_eq!(
                        packed, expected,
                        "packed record of {length} bits for\n{text}"
                    );
                    match expected {
                        Ok(_) => replayed_errors += bools.iter().filter(|bit| **bit).count(),
                        Err(_) => failures += 1,
                    }
                }
            }
        }
    }
    assert!(replayed_errors > 10_000, "fixture must replay many errors");
    assert!(failures > 100, "fixture must compare failure diagnostics");
}
