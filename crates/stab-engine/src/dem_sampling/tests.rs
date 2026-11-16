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
            retained_bytes: 0,
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
            retained_bytes: 0,
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

/// Random records of exactly `width` bits at `density`, as the reference walk reads them.
fn random_records(rng: &mut SmallRng, shots: usize, width: usize, density: f64) -> Vec<Vec<bool>> {
    (0..shots)
        .map(|_| (0..width).map(|_| rng.random_bool(density)).collect())
        .collect()
}

/// The reference walk's detection record for every record, or its first failure.
fn reference_records(plan: &DemSamplingPlan, records: &[Vec<bool>]) -> Vec<DetectionRecordBuffer> {
    records
        .iter()
        .map(|record| {
            replay_with(plan, record.as_slice(), plan.detector_count(), false)
                .expect("reference walk replays a full-width record")
                .1
        })
        .collect()
}

#[test]
fn error_major_replay_matches_the_sequential_walk() {
    let mut rng = SmallRng::seed_from_u64(0xE77_0A10);
    let mut replayed_errors = 0_usize;
    for _ in 0..300 {
        let mut text = String::new();
        write_random_dem_block(&mut rng, 0, &mut text);
        let model = DetectorErrorModel::from_dem_str(&text).expect("parse random DEM");
        let plan = DemSamplingCompiler::new()
            .compile(&model)
            .expect("compile random DEM");
        let width = plan.error_count();
        for shots in [1, 37, 64] {
            for density in [0.0, 0.02, 0.4, 1.0] {
                let records = random_records(&mut rng, shots, width, density);
                let expected = reference_records(&plan, &records);
                let packed =
                    stab_records::PackedShotBatch::from_records(&records, width).expect("pack");
                let mut error_words = vec![u64::MAX; width];
                packed
                    .view()
                    .copy_into_bit_plane_words(&mut error_words)
                    .expect("transpose records");
                // Reused target words hold garbage, which the replay must overwrite.
                let mut detector_words = vec![u64::MAX; plan.detector_count()];
                let mut observable_words = vec![u64::MAX; plan.observable_count()];
                let cancellation = stab_model::Cancellation::new();
                replay_planes::replay_error_words(
                    &plan.inner.operations,
                    &error_words,
                    &mut detector_words,
                    &mut observable_words,
                    &mut program::DemPoll::new(&cancellation),
                )
                .expect("error-major replay");
                for (shot, expected) in expected.iter().enumerate() {
                    let lane = |words: &[u64], index: usize| words[index] >> shot & 1 == 1;
                    let detectors: Vec<bool> = (0..plan.detector_count())
                        .map(|index| lane(&detector_words, index))
                        .collect();
                    let observables: Vec<bool> = (0..plan.observable_count())
                        .map(|index| lane(&observable_words, index))
                        .collect();
                    assert_eq!(
                        (&detectors, &observables),
                        (&expected.detectors, &expected.observables),
                        "shot {shot} of {shots} at density {density} for\n{text}"
                    );
                }
                replayed_errors += records.iter().flatten().filter(|bit| **bit).count();
            }
        }
    }
    assert!(replayed_errors > 10_000, "fixture must replay many errors");
}

/// Replays `records` through packed deliveries, or through a record store when `retain` is set
/// and the store keeps them, returning the sink and whether the store kept them.
fn replay_packed(
    plan: &DemSamplingPlan,
    records: &[Vec<bool>],
    limits: &DemSamplerLimits,
    retain: bool,
) -> (CollectingSink, bool) {
    let width = plan.error_count();
    let shots = ShotCount::new(records.len() as u64);
    let batches: Vec<_> = records
        .chunks(64)
        .map(|chunk| stab_records::PackedShotBatch::from_records(chunk, width).expect("pack"))
        .collect();
    let mut store = plan.replay_record_store(limits, shots);
    if retain {
        for batch in &batches {
            store.try_retain(batch.view()).expect("retain batch");
        }
    }
    let mut session = plan
        .replay_session_with_limits(shots, limits.clone())
        .expect("replay session");
    let mut sink = CollectingSink::default();
    let mut transaction = session.start_transaction(&mut sink).expect("transaction");
    let retained = retain && store.is_retaining();
    if retained {
        transaction.write_retained(&store).expect("replay retained");
    } else {
        for batch in &batches {
            transaction
                .write_packed_batch(batch.view())
                .expect("replay packed");
        }
    }
    transaction.finish().expect("finish replay");
    (sink, retained)
}

#[test]
fn retained_and_bit_plane_replay_records_replay_like_packed_deliveries() {
    let mut rng = SmallRng::seed_from_u64(0x0057_04E5);
    let mut retained_runs = 0_usize;
    for _ in 0..60 {
        let mut text = String::new();
        write_random_dem_block(&mut rng, 0, &mut text);
        let model = DetectorErrorModel::from_dem_str(&text).expect("parse random DEM");
        let plan = DemSamplingCompiler::new()
            .compile(&model)
            .expect("compile random DEM");
        for (shots, density) in [(0, 0.5), (1, 1.0), (64, 0.4), (130, 0.02), (130, 0.0)] {
            let records = random_records(&mut rng, shots, plan.error_count(), density);
            let expected = reference_records(&plan, &records);
            let limits = DemSamplerLimits::default();
            let (packed, _) = replay_packed(&plan, &records, &limits, false);
            let (stored, retained) = replay_packed(&plan, &records, &limits, true);
            retained_runs += usize::from(retained);

            let width = plan.error_count();
            let shot_count = ShotCount::new(records.len() as u64);
            let mut plane_store = plan.replay_record_store(&limits, shot_count);
            let mut plane_session = plan
                .replay_session_with_limits(shot_count, limits.clone())
                .expect("plane replay session");
            let mut plane_direct = CollectingSink::default();
            let mut tx = plane_session
                .start_transaction(&mut plane_direct)
                .expect("plane tx");
            for chunk in records.chunks(64) {
                let packed_chunk =
                    stab_records::PackedShotBatch::from_records(chunk, width).expect("pack");
                let mut words = vec![u64::MAX; width];
                packed_chunk
                    .view()
                    .copy_rows_into_bit_plane_words(0, chunk.len(), &mut words)
                    .expect("transpose chunk");
                // Set inactive lanes in raw storage to verify bit-plane replay masks them out.
                if chunk.len() < 64 {
                    let inactive_mask = !((1_u64 << chunk.len()) - 1);
                    for word in &mut words {
                        *word |= inactive_mask;
                    }
                }
                let planes =
                    stab_records::BitPlaneBatchView::try_from_words(&words, 1, chunk.len(), width)
                        .expect("planes");
                assert_eq!(
                    plane_store
                        .try_retain_bit_planes(planes)
                        .expect("retain planes"),
                    DemReplayRetention::Retained
                );
                tx.write_bit_plane_batch(planes).expect("write planes");
            }
            tx.finish().expect("finish plane tx");

            plane_session.reset().expect("reset plane session");
            let mut plane_stored = CollectingSink::default();
            let mut tx2 = plane_session
                .start_transaction(&mut plane_stored)
                .expect("plane stored tx");
            tx2.write_retained(&plane_store)
                .expect("write retained planes");
            tx2.finish().expect("finish retained plane tx");

            for sink in [&packed, &stored, &plane_direct, &plane_stored] {
                assert_eq!(sink.records, expected, "{shots} records for\n{text}");
                assert_eq!(sink.error_records, records, "{shots} records for\n{text}");
                assert_eq!(sink.finishes, packed.finishes);
            }
        }
    }
    assert!(
        retained_runs >= 300,
        "every dense and sparse fixture within the default budget must stay retained, got {retained_runs}"
    );
}

#[test]
fn replay_record_store_keeps_records_only_within_the_budget_a_session_leaves() {
    let model = DetectorErrorModel::from_dem_str("repeat 100 {\n    error(0.5) D0 D1\n}\n")
        .expect("parse DEM");
    let plan = DemSamplingCompiler::new()
        .compile(&model)
        .expect("compile DEM");
    let shots = ShotCount::new(200);
    // Find the smallest budget whose store admits at least empty sparse records: below it, a
    // store is released from the start and never holds records the session's budget cannot afford.
    let fits = |bytes: u64| {
        plan.replay_record_store(
            &DemSamplerLimits::new(stab_model::MemoryBudget::from_bytes(bytes)),
            shots,
        )
        .is_retaining()
    };
    let (mut low, mut sparse_min_budget) = (0_u64, stab_model::MemoryBudget::DEFAULT.bytes());
    while low + 1 < sparse_min_budget {
        let middle = low + (sparse_min_budget - low) / 2;
        if fits(middle) {
            sparse_min_budget = middle;
        } else {
            low = middle;
        }
    }
    assert!(fits(sparse_min_budget) && !fits(sparse_min_budget - 1));
    let session_only =
        DemSamplerLimits::new(stab_model::MemoryBudget::from_bytes(sparse_min_budget - 1));
    plan.replay_session_with_limits(shots, session_only)
        .expect("a budget one byte short of the store still admits the session");

    let empty_records = vec![vec![false; plan.error_count()]; 200];
    let empty_batch =
        stab_records::PackedShotBatch::from_records(&empty_records, plan.error_count())
            .expect("pack empty");
    let mut sparse_store = plan.replay_record_store(
        &DemSamplerLimits::new(stab_model::MemoryBudget::from_bytes(sparse_min_budget)),
        shots,
    );
    assert_eq!(
        sparse_store
            .try_retain(empty_batch.view())
            .expect("retain empty sparse"),
        DemReplayRetention::Retained
    );
    assert_eq!(sparse_store.shot_count(), 200);

    let dense_records = random_records(
        &mut SmallRng::seed_from_u64(9),
        200,
        plan.error_count(),
        0.5,
    );
    let dense_batch =
        stab_records::PackedShotBatch::from_records(&dense_records, plan.error_count())
            .expect("pack dense");
    // At sparse_min_budget (201 * 8 = 1608 free bytes), dense error-major retention (4 * 808 =
    // 3232 bytes) does not fit and releases on the first batch.
    let mut dense_too_tight = plan.replay_record_store(
        &DemSamplerLimits::new(stab_model::MemoryBudget::from_bytes(sparse_min_budget)),
        shots,
    );
    assert_eq!(
        dense_too_tight
            .try_retain(dense_batch.view())
            .expect("offer dense to sparse-only budget"),
        DemReplayRetention::Released
    );

    let fits_dense = |bytes: u64| {
        let mut store = plan.replay_record_store(
            &DemSamplerLimits::new(stab_model::MemoryBudget::from_bytes(bytes)),
            shots,
        );
        store.try_retain(dense_batch.view()).expect("try dense") == DemReplayRetention::Retained
    };
    let (mut dense_low, mut dense_budget) =
        (sparse_min_budget, stab_model::MemoryBudget::DEFAULT.bytes());
    while dense_low + 1 < dense_budget {
        let middle = dense_low + (dense_budget - dense_low) / 2;
        if fits_dense(middle) {
            dense_budget = middle;
        } else {
            dense_low = middle;
        }
    }
    assert!(fits_dense(dense_budget) && !fits_dense(dense_budget - 1));
    let mut dense_store = plan.replay_record_store(
        &DemSamplerLimits::new(stab_model::MemoryBudget::from_bytes(dense_budget)),
        shots,
    );
    assert_eq!(
        dense_store
            .try_retain(dense_batch.view())
            .expect("retain dense at exact error-major budget"),
        DemReplayRetention::Retained
    );
    assert_eq!(dense_store.shot_count(), 200);

    let mut released = plan.replay_record_store(
        &DemSamplerLimits::new(stab_model::MemoryBudget::from_bytes(sparse_min_budget - 1)),
        shots,
    );
    assert_eq!(
        released
            .try_retain(dense_batch.view())
            .expect("offer to released store"),
        DemReplayRetention::Released
    );
    let mut session = plan.replay_session(shots).expect("replay session");
    let mut sink = CollectingSink::default();
    let mut transaction = session.start_transaction(&mut sink).expect("transaction");
    assert!(
        transaction.write_retained(&released).is_err(),
        "a released store must not replay as if it were empty"
    );
}

#[test]
fn compiler_admits_retained_bytes_and_honors_cancellation() {
    let model = DetectorErrorModel::from_dem_str(
        "error(0.2) D0 D1 L0\nrepeat 6 {\n    error(0.1) D2\n}\nrepeat 3 {\n    error(0.05) D0\n    shift_detectors 1\n}\n",
    )
    .expect("parse DEM");
    let plan = DemSamplingCompiler::new().compile(&model).expect("compile");
    let exact = plan.retained_bytes();
    assert!(exact > 0);

    let admitted = DemSamplingCompiler::new()
        .memory_budget(stab_model::MemoryBudget::from_bytes(exact))
        .compile(&model)
        .expect("exact retained-bytes budget admits compilation");
    assert_eq!(admitted, plan);

    let refused = DemSamplingCompiler::new()
        .memory_budget(stab_model::MemoryBudget::from_bytes(exact - 1))
        .compile(&model)
        .expect_err("one byte below retained_bytes is refused");
    assert!(
        matches!(
            &refused,
            super::DemError::ResourceLimit(limit)
                if limit.kind() == crate::DemResourceKind::CompiledBytes
                    && limit.limit() == exact - 1
                    && limit.actual() > limit.limit()
                    && limit.actual() <= exact
        ),
        "unexpected {refused}"
    );

    let cancelled = stab_model::Cancellation::new();
    cancelled.cancel();
    let err = DemSamplingCompiler::new()
        .cancellation(cancelled)
        .compile(&model)
        .expect_err("cancelled compile");
    assert!(matches!(err, super::DemError::Cancelled(_)));
}
