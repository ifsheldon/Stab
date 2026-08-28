#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "DEM session tests use exact fixture failures for compact diagnostics"
)]

use std::fmt;

use stab_engine::{
    DemError, DemResourceKind, DemSamplerLimits, DemSamplingCompiler, DemSamplingExecutionError,
    DemSamplingPlan, DemSamplingRunError, RandomPolicy, Seed, ShotCount,
};
use stab_model::DetectorErrorModel;
use stab_records::{DemSampleBatchView, DemSampleSink, PackedShotBatchView};

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestDetectionRecord {
    detectors: Vec<bool>,
    observables: Vec<bool>,
}

#[derive(Clone)]
struct TestSampler {
    plan: DemSamplingPlan,
}

#[derive(Debug)]
struct TestDetectionOutput {
    records: Vec<TestDetectionRecord>,
}

fn compile_dem(text: &str) -> TestSampler {
    let model = DetectorErrorModel::from_dem_str(text).expect("parse DEM fixture");
    let plan = DemSamplingCompiler::new()
        .compile(&model)
        .expect("compile DEM sampler");
    TestSampler { plan }
}

impl TestSampler {
    fn plan(&self) -> DemSamplingPlan {
        self.plan.clone()
    }

    fn session(
        &self,
        random_policy: RandomPolicy,
    ) -> Result<stab_engine::DemSamplingSession, DemSamplingExecutionError> {
        self.plan.session(random_policy)
    }

    fn session_with_limits(
        &self,
        random_policy: RandomPolicy,
        limits: DemSamplerLimits,
    ) -> Result<stab_engine::DemSamplingSession, DemSamplingExecutionError> {
        self.plan.session_with_limits(random_policy, limits)
    }

    fn collect_samples(
        &self,
        shots: usize,
        seed: Option<u64>,
    ) -> Result<TestDetectionOutput, DemSamplingExecutionError> {
        let mut session = self.session(random_policy(seed))?;
        let mut sink = CollectSink::default();
        session
            .run(shot_count(shots)?, &mut sink)
            .map_err(engine_run_error)?;
        Ok(TestDetectionOutput {
            records: sink
                .detectors
                .into_iter()
                .zip(sink.observables)
                .map(|(detectors, observables)| TestDetectionRecord {
                    detectors,
                    observables,
                })
                .collect(),
        })
    }

    fn collect_samples_with_errors(
        &self,
        shots: usize,
        seed: Option<u64>,
    ) -> Result<(TestDetectionOutput, Vec<Vec<bool>>), DemSamplingExecutionError> {
        let mut session = self.session(random_policy(seed))?;
        let mut sink = CollectSink::default();
        session
            .run_with_sampled_errors(shot_count(shots)?, &mut sink)
            .map_err(engine_run_error)?;
        let records = sink
            .detectors
            .into_iter()
            .zip(sink.observables)
            .map(|(detectors, observables)| TestDetectionRecord {
                detectors,
                observables,
            })
            .collect();
        let error_records = sink
            .sampled_errors
            .into_iter()
            .map(|record| record.expect("sampled-error run produces error records"))
            .collect();
        Ok((TestDetectionOutput { records }, error_records))
    }

    fn replay_samples(
        &self,
        error_records: &[Vec<bool>],
    ) -> Result<TestDetectionOutput, DemSamplingExecutionError> {
        let mut session = self.plan.replay_session(shot_count(error_records.len())?)?;
        let mut sink = CollectSink::default();
        session
            .run(error_records, &mut sink)
            .map_err(engine_run_error)?;
        Ok(TestDetectionOutput {
            records: sink
                .detectors
                .into_iter()
                .zip(sink.observables)
                .map(|(detectors, observables)| TestDetectionRecord {
                    detectors,
                    observables,
                })
                .collect(),
        })
    }
}

fn random_policy(seed: Option<u64>) -> RandomPolicy {
    seed.map_or(RandomPolicy::Entropy, |seed| {
        RandomPolicy::Seeded(Seed::new(seed))
    })
}

fn shot_count(shots: usize) -> Result<ShotCount, DemSamplingExecutionError> {
    u64::try_from(shots)
        .map(ShotCount::new)
        .map_err(|_| DemSamplingExecutionError::ShotCounterOverflow)
}

fn engine_run_error(error: DemSamplingRunError<SinkFailure>) -> DemSamplingExecutionError {
    match error {
        DemSamplingRunError::Engine { source, .. } => source,
        DemSamplingRunError::Sink { source, .. } => DemSamplingExecutionError::InternalInvariant {
            message: source.to_string(),
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SinkFailure(&'static str);

impl fmt::Display for SinkFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for SinkFailure {}

#[derive(Default)]
struct CollectSink {
    detectors: Vec<Vec<bool>>,
    observables: Vec<Vec<bool>>,
    sampled_errors: Vec<Option<Vec<bool>>>,
    write_calls: usize,
    finish_calls: usize,
    fail_write_at: Option<usize>,
    fail_finish: bool,
    after_write: Option<Box<dyn FnMut()>>,
}

impl CollectSink {
    fn failing_write(call: usize) -> Self {
        Self {
            fail_write_at: Some(call),
            ..Self::default()
        }
    }

    fn failing_finish() -> Self {
        Self {
            fail_finish: true,
            ..Self::default()
        }
    }

    fn after_write(action: impl FnMut() + 'static) -> Self {
        Self {
            after_write: Some(Box::new(action)),
            ..Self::default()
        }
    }
}

impl DemSampleSink for CollectSink {
    type Error = SinkFailure;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        if self.fail_write_at == Some(self.write_calls) {
            return Err(SinkFailure("write-failure"));
        }
        self.write_calls += 1;
        let detection = batch.detection();
        let sampled_errors = batch.sampled_errors();
        for shot_index in 0..detection.shot_count() {
            self.detectors
                .push(collect_row(detection.detectors(), shot_index)?);
            self.observables
                .push(collect_row(detection.observables(), shot_index)?);
            self.sampled_errors.push(
                sampled_errors
                    .map(|records| collect_row(records, shot_index))
                    .transpose()?,
            );
        }
        if let Some(after_write) = self.after_write.as_mut() {
            after_write();
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finish_calls += 1;
        if self.fail_finish {
            return Err(SinkFailure("finish-failure"));
        }
        Ok(())
    }
}

fn collect_row(
    records: PackedShotBatchView<'_>,
    shot_index: usize,
) -> Result<Vec<bool>, SinkFailure> {
    (0..records.bits_per_shot())
        .map(|bit_index| {
            records
                .get(shot_index, bit_index)
                .ok_or(SinkFailure("batch-bit-out-of-range"))
        })
        .collect()
}

#[derive(Default)]
struct WitnessSink {
    witness: u64,
    write_calls: usize,
    finish_calls: usize,
}

impl DemSampleSink for WitnessSink {
    type Error = SinkFailure;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        self.write_calls += 1;
        let detection = batch.detection();
        self.witness ^= witness(detection.detectors(), detection.shot_count());
        self.witness =
            self.witness.rotate_left(7) ^ witness(detection.observables(), detection.shot_count());
        if let Some(sampled_errors) = batch.sampled_errors() {
            self.witness =
                self.witness.rotate_left(11) ^ witness(sampled_errors, detection.shot_count());
        }
        std::hint::black_box(self.witness);
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finish_calls += 1;
        Ok(())
    }
}

fn witness(records: PackedShotBatchView<'_>, shot_count: usize) -> u64 {
    let first = records.get(0, 0).unwrap_or(false) as u64;
    let last = shot_count
        .checked_sub(1)
        .and_then(|shot| {
            records
                .bits_per_shot()
                .checked_sub(1)
                .and_then(|bit| records.get(shot, bit))
        })
        .unwrap_or(false) as u64;
    first | (last << 1) | ((shot_count as u64) << 2)
}

fn assert_plan_traits<T: Clone + Send + Sync>(_: &T) {}

#[test]
fn public_compiler_exposes_the_engine_plan_contract() {
    let model =
        DetectorErrorModel::from_dem_str("error(0.25) D0 D2 L3\n").expect("parse compiler fixture");
    let plan = DemSamplingCompiler::new()
        .compile(&model)
        .expect("compile public DEM plan");
    assert_plan_traits(&plan);
    assert_eq!(plan.detector_width().get(), 3);
    assert_eq!(plan.observable_width().get(), 4);
    assert_eq!(plan.sampled_error_width().get(), 1);
    assert_eq!(plan.detector_count(), 3);
    assert_eq!(plan.observable_count(), 4);
    assert_eq!(plan.error_count(), 1);
    plan.validate_replay(ShotCount::new(2))
        .expect("admit bounded replay work");

    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(17)))
        .expect("create public DEM session");
    let mut sink = CollectSink::default();
    let summary = session
        .run(ShotCount::new(2), &mut sink)
        .expect("run public DEM session");
    assert!(summary.status().is_completed());
    assert_eq!(summary.requested_shots(), ShotCount::new(2));
    assert_eq!(summary.committed_shots(), ShotCount::new(2));
    assert_eq!(summary.total_committed_shots(), ShotCount::new(2));
}

#[test]
fn streamed_sessions_match_materialized_sampling_across_dem_families() {
    for text in [
        "error(1) D0\n",
        "error(0.25) D0\nerror(0.75) D1 L0\n",
        "repeat 5 {\n  error(0.2) D0 L1\n  shift_detectors 1\n}\n",
        "error(0.4) D0 D2 ^ D1 L0\n",
        "error(0.5) L3\n",
        "repeat 3 {\n  repeat 2 {\n    error(0.125) D0 L0\n  }\n}\n",
    ] {
        let sampler = compile_dem(text);
        let plan = sampler.plan();
        assert_plan_traits(&plan);

        let materialized = sampler
            .collect_samples(65, Some(7))
            .expect("materialized detector-only samples");
        let mut session = sampler
            .session(RandomPolicy::Seeded(Seed::new(7)))
            .expect("detector-only session");
        let mut streamed = CollectSink::default();
        let summary = session
            .run(ShotCount::new(65), &mut streamed)
            .expect("stream detector-only samples");
        assert!(summary.status().is_completed());
        assert_eq!(summary.committed_shots().get(), 65);
        assert_eq!(
            streamed.detectors,
            materialized
                .records
                .iter()
                .map(|record| record.detectors.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            streamed.observables,
            materialized
                .records
                .iter()
                .map(|record| record.observables.clone())
                .collect::<Vec<_>>()
        );
        assert!(streamed.sampled_errors.iter().all(Option::is_none));

        let (materialized, materialized_errors) = sampler
            .collect_samples_with_errors(65, Some(7))
            .expect("materialized samples with errors");
        let mut session = sampler
            .session(RandomPolicy::Seeded(Seed::new(7)))
            .expect("sampled-error session");
        let mut streamed = CollectSink::default();
        session
            .run_with_sampled_errors(ShotCount::new(65), &mut streamed)
            .expect("stream samples with errors");
        assert_eq!(
            streamed.detectors,
            materialized
                .records
                .iter()
                .map(|record| record.detectors.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            streamed.observables,
            materialized
                .records
                .iter()
                .map(|record| record.observables.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            streamed.sampled_errors,
            materialized_errors
                .iter()
                .cloned()
                .map(Some)
                .collect::<Vec<_>>()
        );

        let replayed = sampler
            .replay_samples(&materialized_errors)
            .expect("materialize replay reference");
        let mut replay_session = plan
            .replay_session(ShotCount::new(65))
            .expect("create replay session");
        let mut replay_sink = CollectSink::default();
        replay_session
            .run(&materialized_errors, &mut replay_sink)
            .expect("stream replay records");
        assert_eq!(
            replay_sink.detectors,
            replayed
                .records
                .iter()
                .map(|record| record.detectors.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            replay_sink.observables,
            replayed
                .records
                .iter()
                .map(|record| record.observables.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            replay_sink.sampled_errors,
            materialized_errors
                .iter()
                .cloned()
                .map(Some)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn seeded_sessions_partition_exactly_across_the_indexed_block_boundary() {
    let sampler = compile_dem("repeat 4 {\n  error(0.25) D0 L0\n  shift_detectors 1\n}\n");
    for sampled_errors in [false, true] {
        let mut partitioned = sampler
            .session(RandomPolicy::Seeded(Seed::new(41)))
            .expect("partitioned session");
        let mut first = CollectSink::default();
        let mut second = CollectSink::default();
        if sampled_errors {
            partitioned
                .run_with_sampled_errors(ShotCount::new(17), &mut first)
                .expect("first sampled-error partition");
            partitioned
                .run_with_sampled_errors(ShotCount::new(1_008), &mut second)
                .expect("second sampled-error partition");
        } else {
            partitioned
                .run(ShotCount::new(17), &mut first)
                .expect("first detector-only partition");
            partitioned
                .run(ShotCount::new(1_008), &mut second)
                .expect("second detector-only partition");
        }

        let mut whole = sampler
            .session(RandomPolicy::Seeded(Seed::new(41)))
            .expect("whole session");
        let mut expected = CollectSink::default();
        if sampled_errors {
            whole
                .run_with_sampled_errors(ShotCount::new(1_025), &mut expected)
                .expect("whole sampled-error run");
        } else {
            whole
                .run(ShotCount::new(1_025), &mut expected)
                .expect("whole detector-only run");
        }

        first.detectors.extend(second.detectors);
        first.observables.extend(second.observables);
        first.sampled_errors.extend(second.sampled_errors);
        assert_eq!(first.detectors, expected.detectors);
        assert_eq!(first.observables, expected.observables);
        assert_eq!(first.sampled_errors, expected.sampled_errors);
        assert_eq!(partitioned.total_committed_shots().get(), 1_025);
    }
}

#[test]
fn replay_session_owns_plan_state_and_binds_one_sink_per_transaction() {
    let mut replay = {
        let sampler = compile_dem("error(1) D0 L0\n");
        sampler
            .plan()
            .replay_session(ShotCount::new(2))
            .expect("owned replay session")
    };
    let mut sink = CollectSink::default();
    let mut transaction = replay
        .start_transaction(&mut sink)
        .expect("bind replay sink");
    transaction
        .write_batch(&[vec![true]])
        .expect("deliver first replay record");
    transaction
        .write_batch(&[vec![false]])
        .expect("deliver second replay record");
    let summary = transaction.finish().expect("finish owned replay");

    assert_eq!(summary.committed_shots(), ShotCount::new(2));
    assert_eq!(sink.finish_calls, 1);

    replay.reset().expect("reuse completed replay storage");
    let mut second_sink = CollectSink::default();
    let summary = replay
        .run(&[vec![false], vec![true]], &mut second_sink)
        .expect("run reset replay session");
    assert_eq!(summary.total_committed_shots(), ShotCount::new(4));
    assert_eq!(second_sink.finish_calls, 1);
}

#[test]
fn incremental_replay_matches_complete_replay_across_chunking() {
    let sampler = compile_dem("error(0.25) D0 L0\nerror(0.6) D1\n");
    let (_, error_records) = sampler
        .collect_samples_with_errors(65, Some(13))
        .expect("sample replay records");
    let expected = sampler
        .replay_samples(&error_records)
        .expect("materialized replay");

    let mut replay = sampler
        .plan()
        .replay_session(ShotCount::new(65))
        .expect("incremental replay session");
    let mut replayed = CollectSink::default();
    let mut transaction = replay
        .start_transaction(&mut replayed)
        .expect("bind incremental replay sink");
    assert!(
        transaction
            .write_batch(error_records.get(..1).expect("first replay record"))
            .expect("write first replay batch")
            .is_accepted()
    );
    for records in error_records
        .get(1..)
        .expect("remaining replay records")
        .chunks(11)
    {
        transaction
            .write_batch(records)
            .expect("write incremental replay batch");
    }
    let summary = transaction.finish().expect("finish replay");
    assert_eq!(summary.committed_shots().get(), 65);
    assert_eq!(replayed.finish_calls, 1);
    assert_eq!(
        replayed.detectors,
        expected
            .records
            .iter()
            .map(|record| record.detectors.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        replayed.observables,
        expected
            .records
            .iter()
            .map(|record| record.observables.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        replayed.sampled_errors,
        error_records.iter().cloned().map(Some).collect::<Vec<_>>()
    );
}

#[test]
#[allow(
    clippy::mem_forget,
    reason = "prove forgotten transactions fail closed instead of rebinding a sink"
)]
fn replay_batch_validation_is_retryable_and_abandonment_retains_no_sink() {
    let sampler = compile_dem("error(1) D0\n");
    let valid = vec![vec![true], vec![false]];
    let invalid = vec![Vec::new()];

    let mut replay = sampler
        .plan()
        .replay_session(ShotCount::new(2))
        .expect("retryable replay session");
    let mut sink = CollectSink::default();
    let mut transaction = replay
        .start_transaction(&mut sink)
        .expect("bind retryable replay sink");
    let error = transaction
        .write_batch(&invalid)
        .expect_err("reject invalid replay width");
    assert_eq!(error.progress().committed_shots().get(), 0);
    transaction
        .write_batch(&valid)
        .expect("retry valid replay delivery");
    transaction.finish().expect("finish retried replay");
    assert!(!replay.is_poisoned());
    assert_eq!(sink.write_calls, 1);
    assert_eq!(sink.finish_calls, 1);

    let mut replay = sampler
        .plan()
        .replay_session(ShotCount::new(2))
        .expect("prefix-progress replay session");
    let mut sink = CollectSink::default();
    let mut transaction = replay
        .start_transaction(&mut sink)
        .expect("bind prefix-progress replay sink");
    transaction
        .write_batch(valid.get(..1).expect("first valid replay record"))
        .expect("commit valid replay prefix");
    let error = transaction
        .write_batch(&invalid)
        .expect_err("reject invalid replay width after a valid prefix");
    assert_eq!(error.progress().committed_shots(), ShotCount::new(1));
    assert_eq!(error.progress().attempted_batch_shots(), ShotCount::new(1));
    transaction
        .write_batch(valid.get(1..).expect("second valid replay record"))
        .expect("retry the second replay record");
    transaction.finish().expect("finish prefix-progress replay");
    assert!(!replay.is_poisoned());
    assert_eq!(sink.write_calls, 2);
    assert_eq!(sink.finish_calls, 1);

    let mut replay = sampler
        .plan()
        .replay_session(ShotCount::new(2))
        .expect("abandoned replay session");
    let mut sink = CollectSink::default();
    let mut transaction = replay
        .start_transaction(&mut sink)
        .expect("bind abandoned replay sink");
    transaction
        .write_batch(valid.get(..1).expect("first valid replay record"))
        .expect("commit replay prefix");
    drop(transaction);
    assert!(replay.is_poisoned());
    assert_eq!(sink.write_calls, 1);
    assert_eq!(sink.finish_calls, 0);

    let mut replay = sampler
        .plan()
        .replay_session(ShotCount::new(2))
        .expect("forgotten replay transaction session");
    let mut first_sink = CollectSink::default();
    let mut transaction = replay
        .start_transaction(&mut first_sink)
        .expect("bind forgotten replay transaction");
    transaction
        .write_batch(valid.get(..1).expect("first replay record"))
        .expect("commit forgotten replay prefix");
    std::mem::forget(transaction);
    let mut second_sink = CollectSink::default();
    let error = replay
        .run(&valid, &mut second_sink)
        .expect_err("an active forgotten transaction must block another sink");
    assert_eq!(error.progress().committed_shots(), ShotCount::new(1));
    assert_eq!(second_sink.write_calls, 0);
    assert_eq!(second_sink.finish_calls, 0);
    assert!(matches!(
        replay.reset(),
        Err(DemSamplingExecutionError::ReplayLifecycle { .. })
    ));
}

#[test]
fn cancellation_commits_only_complete_batches_and_session_resumes() {
    let sampler = compile_dem("error(0.5) D0 L0\n");
    let mut session = sampler
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .expect("cancellable session");
    let cancellation = session.cancellation();
    let cancellation_for_sink = cancellation.clone();
    let mut first = CollectSink::after_write(move || {
        cancellation_for_sink.cancel();
    });
    let summary = session
        .run(ShotCount::new(130), &mut first)
        .expect("cancelled run");
    assert!(summary.status().is_cancelled());
    assert_eq!(summary.committed_shots().get(), 64);
    assert_eq!(first.finish_calls, 1);
    assert!(!session.is_poisoned());

    cancellation.reset();
    let mut resumed = CollectSink::default();
    session
        .run(ShotCount::new(66), &mut resumed)
        .expect("resumed run");
    let mut control = sampler
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .expect("control session");
    let mut expected = CollectSink::default();
    control
        .run(ShotCount::new(130), &mut expected)
        .expect("whole control run");
    first.detectors.extend(resumed.detectors);
    first.observables.extend(resumed.observables);
    assert_eq!(first.detectors, expected.detectors);
    assert_eq!(first.observables, expected.observables);

    let mut replay_session = sampler
        .plan()
        .replay_session(ShotCount::new(65))
        .expect("cancellable replay session");
    let replay_cancellation = replay_session.cancellation();
    replay_cancellation.cancel();
    let replay_records = vec![vec![true]; 64];
    let mut replay_sink = CollectSink::default();
    let mut transaction = replay_session
        .start_transaction(&mut replay_sink)
        .expect("bind cancelled replay sink");
    let status = transaction
        .write_batch(&replay_records)
        .expect("observe replay cancellation");
    assert!(status.is_cancelled());
    let replay_summary = transaction.finish().expect("finish cancelled replay");
    assert!(replay_summary.status().is_cancelled());
    assert_eq!(replay_summary.committed_shots(), ShotCount::new(0));
    assert_eq!(replay_sink.write_calls, 0);
    assert_eq!(replay_sink.finish_calls, 1);
    assert!(!replay_session.is_poisoned());

    let mut replay_session = sampler
        .plan()
        .replay_session(ShotCount::new(2))
        .expect("replay session cancelled before finish");
    let replay_cancellation = replay_session.cancellation();
    let cancellation_for_sink = replay_cancellation.clone();
    let mut replay_sink = CollectSink::after_write(move || {
        cancellation_for_sink.cancel();
    });
    let mut transaction = replay_session
        .start_transaction(&mut replay_sink)
        .expect("bind replay sink cancelled after write");
    transaction
        .write_batch(&[vec![true], vec![false]])
        .expect("commit replay batch before cancellation");
    let replay_summary = transaction
        .finish()
        .expect("finish observes cooperative cancellation");
    assert!(replay_summary.status().is_cancelled());
    assert_eq!(replay_summary.committed_shots(), ShotCount::new(2));
    assert_eq!(replay_sink.write_calls, 1);
    assert_eq!(replay_sink.finish_calls, 1);
    assert!(!replay_session.is_poisoned());

    assert!(matches!(
        replay_session.reset(),
        Err(DemSamplingExecutionError::ReplayLifecycle { .. })
    ));
    replay_cancellation.reset();
    replay_session
        .reset()
        .expect("reset replay after resetting cancellation token");
    let mut resumed_replay_sink = CollectSink::default();
    replay_session
        .run(&[vec![false], vec![true]], &mut resumed_replay_sink)
        .expect("run replay after cancellation reset");
    assert_eq!(resumed_replay_sink.finish_calls, 1);
}

#[test]
fn session_batch_capacity_obeys_the_caller_active_byte_budget() {
    let sampler = compile_dem("error(1) D0\n");
    let detector_record_bytes = std::mem::size_of::<TestDetectionRecord>() + 1;
    let detector_plane_bytes = std::mem::size_of::<u64>();
    let active_scratch_bytes = detector_record_bytes + detector_plane_bytes;
    let limits = DemSamplerLimits::default().with_max_active_batch_bytes(active_scratch_bytes + 16);
    let mut session = sampler
        .session_with_limits(RandomPolicy::Seeded(Seed::new(3)), limits)
        .expect("two-shot detector-only batch fits the active byte budget");
    let mut sink = WitnessSink::default();
    session
        .run(ShotCount::new(5), &mut sink)
        .expect("run with a byte-limited reusable batch");
    assert_eq!(sink.write_calls, 3);
    assert_eq!(sink.finish_calls, 1);

    let rejected =
        DemSamplerLimits::default().with_max_active_batch_bytes(active_scratch_bytes + 7);
    let error = sampler
        .session_with_limits(RandomPolicy::Seeded(Seed::new(3)), rejected)
        .expect_err("one record plus one packed shot exceeds the active byte budget");
    assert!(
        error
            .into_dem_error()
            .to_string()
            .contains("active batch bytes"),
        "caller active-byte rejection should retain typed DEM resource context"
    );
}

#[test]
fn sink_failures_preserve_progress_and_poison_sessions() {
    let sampler = compile_dem("error(0.5) D0\n");
    let mut session = sampler
        .session(RandomPolicy::Seeded(Seed::new(8)))
        .expect("write-failure session");
    let mut sink = CollectSink::failing_write(1);
    let error = session
        .run(ShotCount::new(65), &mut sink)
        .expect_err("second batch write must fail");
    assert!(error.to_string().contains("write-failure"), "{error}");
    assert_eq!(error.progress().committed_shots().get(), 64);
    assert_eq!(error.progress().attempted_batch_shots().get(), 1);
    assert_eq!(sink.finish_calls, 0);
    assert!(session.is_poisoned());

    let mut unused = CollectSink::default();
    let poisoned = session
        .run(ShotCount::new(1), &mut unused)
        .expect_err("poisoned session must reject");
    match poisoned {
        DemSamplingRunError::Engine {
            source: DemSamplingExecutionError::SessionPoisoned,
            progress,
        } => {
            assert_eq!(progress.committed_shots(), ShotCount::new(0));
            assert!(
                DemSamplingExecutionError::SessionPoisoned
                    .into_dem_error()
                    .to_string()
                    .contains("session is poisoned")
            );
        }
        other => panic!("expected poisoned execution error, got {other}"),
    }
    assert_eq!(unused.write_calls, 0);
    assert_eq!(unused.finish_calls, 0);

    let mut session = sampler
        .session(RandomPolicy::Seeded(Seed::new(8)))
        .expect("finish-failure session");
    let mut sink = CollectSink::failing_finish();
    let error = session
        .run(ShotCount::new(65), &mut sink)
        .expect_err("sink finalization must fail");
    assert!(error.to_string().contains("finish-failure"), "{error}");
    assert_eq!(error.progress().committed_shots().get(), 65);
    assert_eq!(error.progress().attempted_batch_shots().get(), 0);
    assert!(session.is_poisoned());
}

#[test]
fn zero_shots_touch_neither_sink_nor_seeded_rng() {
    let sampler = compile_dem("error(0.5) D0 L0\n");
    let mut session = sampler
        .session(RandomPolicy::Seeded(Seed::new(12)))
        .expect("zero-shot session");
    let mut zero_sink = CollectSink::default();
    let summary = session
        .run(ShotCount::new(0), &mut zero_sink)
        .expect("zero-shot run");
    assert!(summary.status().is_completed());
    assert_eq!(zero_sink.write_calls, 0);
    assert_eq!(zero_sink.finish_calls, 0);
    session
        .run_with_sampled_errors(ShotCount::new(0), &mut zero_sink)
        .expect("zero-shot sampled-error run");
    let mut replay = sampler
        .plan()
        .replay_session(ShotCount::new(0))
        .expect("zero-shot replay session");
    replay.run(&[], &mut zero_sink).expect("zero-shot replay");
    assert_eq!(zero_sink.write_calls, 0);
    assert_eq!(zero_sink.finish_calls, 0);

    let mut actual = CollectSink::default();
    session
        .run(ShotCount::new(65), &mut actual)
        .expect("run after zero shots");
    let mut control = sampler
        .session(RandomPolicy::Seeded(Seed::new(12)))
        .expect("control session");
    let mut expected = CollectSink::default();
    control
        .run(ShotCount::new(65), &mut expected)
        .expect("control run");
    assert_eq!(actual.detectors, expected.detectors);
    assert_eq!(actual.observables, expected.observables);
}

#[test]
fn work_limits_reject_before_rng_or_sink_and_leave_session_reusable() {
    let sampler = compile_dem("error(0.5) D0\n");
    let limits = DemSamplerLimits::default().with_max_sampled_error_applications(1);
    let mut session = sampler
        .session_with_limits(RandomPolicy::Seeded(Seed::new(22)), limits)
        .expect("limited session");
    let mut rejected = CollectSink::default();
    let error = session
        .run(ShotCount::new(2), &mut rejected)
        .expect_err("reject excessive detector-only work");
    assert_eq!(error.progress().committed_shots().get(), 0);
    assert_eq!(rejected.write_calls, 0);
    assert_eq!(rejected.finish_calls, 0);
    assert!(!session.is_poisoned());

    let mut accepted = CollectSink::default();
    session
        .run(ShotCount::new(1), &mut accepted)
        .expect("reuse after preflight rejection");
    let mut control = sampler
        .session_with_limits(RandomPolicy::Seeded(Seed::new(22)), limits)
        .expect("limited control session");
    let mut expected = CollectSink::default();
    control
        .run(ShotCount::new(1), &mut expected)
        .expect("control sample");
    assert_eq!(accepted.detectors, expected.detectors);

    let mut sampled_error_session = sampler
        .session_with_limits(RandomPolicy::Seeded(Seed::new(22)), limits)
        .expect("limited sampled-error session");
    let mut sampled_error_sink = CollectSink::default();
    sampled_error_session
        .run_with_sampled_errors(ShotCount::new(2), &mut sampled_error_sink)
        .expect_err("reject excessive sampled-error work");
    assert_eq!(sampled_error_sink.write_calls, 0);
    assert_eq!(sampled_error_sink.finish_calls, 0);
    assert!(!sampled_error_session.is_poisoned());
    let mut sampled_after_rejection = CollectSink::default();
    sampled_error_session
        .run_with_sampled_errors(ShotCount::new(1), &mut sampled_after_rejection)
        .expect("sample after sampled-error rejection");
    let mut sampled_control = sampler
        .session_with_limits(RandomPolicy::Seeded(Seed::new(22)), limits)
        .expect("sampled-error control session");
    let mut sampled_expected = CollectSink::default();
    sampled_control
        .run_with_sampled_errors(ShotCount::new(1), &mut sampled_expected)
        .expect("sampled-error control run");
    assert_eq!(
        sampled_after_rejection.sampled_errors,
        sampled_expected.sampled_errors
    );

    let replay_limits = DemSamplerLimits::default().with_max_replay_work_units(2);
    let replay_admission = sampler
        .plan()
        .validate_replay_with_limits(ShotCount::new(2), replay_limits)
        .expect_err("plan replay admission must reject excessive work");
    let DemError::ResourceLimit(resource) = replay_admission else {
        panic!("plan replay admission must expose typed resource context");
    };
    assert_eq!(resource.kind(), DemResourceKind::ReplayWorkUnits);
    assert_eq!(resource.actual(), 4);
    assert_eq!(resource.limit(), 2);
    let error = sampler
        .plan()
        .replay_session_with_limits(ShotCount::new(2), replay_limits)
        .expect_err("reject replay work before delivery");
    let DemSamplingExecutionError::InvalidRequest(DemError::ResourceLimit(resource)) = error else {
        panic!("replay session must expose typed resource context");
    };
    assert_eq!(resource.kind(), DemResourceKind::ReplayWorkUnits);
    assert_eq!((resource.actual(), resource.limit()), (4, 2));
}

#[test]
fn replay_session_admits_work_and_poisoning_before_record_widths() {
    let sampler = compile_dem("error(0.25) D0\nerror(0.5) D1\n");
    let limits = DemSamplerLimits::default().with_max_replay_work_units(2);
    let work_error = sampler
        .plan()
        .replay_session_with_limits(ShotCount::new(2), limits)
        .expect_err("work admission must precede record widths");
    let DemSamplingExecutionError::InvalidRequest(source) = work_error else {
        panic!("expected typed replay work request");
    };
    let DemError::ResourceLimit(resource) = source else {
        panic!("replay work error must expose typed resource context");
    };
    assert_eq!(resource.kind(), DemResourceKind::ReplayWorkUnits);
    assert_eq!(resource.actual(), 8);
    assert_eq!(resource.limit(), 2);
    let mut poisoned = sampler
        .plan()
        .replay_session(ShotCount::new(1))
        .expect("create replay session to poison");
    let mut failing = CollectSink::failing_finish();
    let mut transaction = poisoned
        .start_transaction(&mut failing)
        .expect("bind failing replay sink");
    transaction
        .write_batch(&[vec![false, false]])
        .expect("write replay record before finish failure");
    transaction
        .finish()
        .expect_err("poison session through finish failure");
    let mut second_untouched = CollectSink::default();
    let poison_error = poisoned
        .run(&[Vec::new()], &mut second_untouched)
        .expect_err("poison admission must precede record widths");
    assert!(matches!(
        poison_error,
        DemSamplingRunError::Engine {
            source: DemSamplingExecutionError::SessionPoisoned,
            ..
        }
    ));
    assert_eq!(second_untouched.write_calls, 0);
    assert_eq!(second_untouched.finish_calls, 0);
}

#[test]
fn post_warmup_session_execution_allocations_are_record_count_independent() {
    let sampler = compile_dem("repeat 3 {\n  error(0.25) D0 L0\n  shift_detectors 1\n}\n");
    let mut session = sampler
        .session(RandomPolicy::Seeded(Seed::new(71)))
        .expect("detector-only allocation session");
    let mut sink = WitnessSink::default();
    session
        .run(ShotCount::new(1), &mut sink)
        .expect("warm detector-only session");
    let allocations = allocation_counter::measure(|| {
        session
            .run(ShotCount::new(1_024), &mut sink)
            .expect("measure detector-only session");
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");

    let mut session = sampler
        .session(RandomPolicy::Seeded(Seed::new(71)))
        .expect("sampled-error allocation session");
    let mut sink = WitnessSink::default();
    session
        .run_with_sampled_errors(ShotCount::new(1), &mut sink)
        .expect("warm sampled-error session");
    let allocations = allocation_counter::measure(|| {
        session
            .run_with_sampled_errors(ShotCount::new(1_024), &mut sink)
            .expect("measure sampled-error session");
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");

    let replay_records = vec![vec![true, false, true]; 65];
    let mut replay_session = sampler
        .plan()
        .replay_session(ShotCount::new(65))
        .expect("replay allocation session");
    let mut replay_sink = WitnessSink::default();
    let mut transaction = replay_session
        .start_transaction(&mut replay_sink)
        .expect("bind allocation replay sink");
    transaction
        .write_batch(replay_records.get(..1).expect("warm replay record"))
        .expect("warm replay storage");
    let allocations = allocation_counter::measure(|| {
        transaction
            .write_batch(replay_records.get(1..).expect("measured replay records"))
            .expect("deliver measured replay");
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
    transaction.finish().expect("finish measured replay");
}
