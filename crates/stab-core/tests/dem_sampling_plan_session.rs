#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "DEM session tests use exact fixture failures for compact diagnostics"
)]

use std::fmt;

use stab_core::{
    CompiledDemSampler, DemSampleBatchView, DemSampleSink, DemSamplerLimits, DetectorErrorModel,
    RandomPolicy, ResourceKind, ResourceOperation, Seed, ShotCount,
    execution::{DemSamplingCompiler, DemSamplingExecutionError, DemSamplingRunError},
};

fn compile_dem(text: &str) -> CompiledDemSampler {
    let model = DetectorErrorModel::from_dem_str(text).expect("parse DEM fixture");
    CompiledDemSampler::compile(&model).expect("compile DEM sampler")
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
    records: stab_core::PackedShotBatchView<'_>,
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

fn witness(records: stab_core::PackedShotBatchView<'_>, shot_count: usize) -> u64 {
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
fn public_compiler_and_compatibility_facade_share_one_plan_contract() {
    let model =
        DetectorErrorModel::from_dem_str("error(0.25) D0 D2 L3\n").expect("parse compiler fixture");
    let plan = DemSamplingCompiler::new()
        .compile(&model)
        .expect("compile public DEM plan");
    let compatibility = CompiledDemSampler::compile(&model).expect("compile compatibility facade");
    assert_eq!(plan, compatibility.plan());
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
            .sample_detection_events_with_seed(65, Some(7))
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
            .sample_detection_events_and_errors_with_seed(65, Some(7))
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
                .into_iter()
                .map(Some)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn seeded_sessions_partition_exactly_for_both_sampling_algorithms() {
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
                .run_with_sampled_errors(ShotCount::new(48), &mut second)
                .expect("second sampled-error partition");
        } else {
            partitioned
                .run(ShotCount::new(17), &mut first)
                .expect("first detector-only partition");
            partitioned
                .run(ShotCount::new(48), &mut second)
                .expect("second detector-only partition");
        }

        let mut whole = sampler
            .session(RandomPolicy::Seeded(Seed::new(41)))
            .expect("whole session");
        let mut expected = CollectSink::default();
        if sampled_errors {
            whole
                .run_with_sampled_errors(ShotCount::new(65), &mut expected)
                .expect("whole sampled-error run");
        } else {
            whole
                .run(ShotCount::new(65), &mut expected)
                .expect("whole detector-only run");
        }

        first.detectors.extend(second.detectors);
        first.observables.extend(second.observables);
        first.sampled_errors.extend(second.sampled_errors);
        assert_eq!(first.detectors, expected.detectors);
        assert_eq!(first.observables, expected.observables);
        assert_eq!(first.sampled_errors, expected.sampled_errors);
        assert_eq!(partitioned.total_committed_shots().get(), 65);
    }
}

#[test]
fn incremental_replay_owns_one_sink_lifecycle_and_does_not_advance_rng() {
    let sampler = compile_dem("error(0.25) D0 L0\nerror(0.6) D1\n");
    let (_, error_records) = sampler
        .sample_detection_events_and_errors_with_seed(65, Some(13))
        .expect("sample replay records");
    let expected = sampler
        .sample_detection_events_from_error_records(&error_records)
        .expect("materialized replay");

    let mut session = sampler
        .session(RandomPolicy::Seeded(Seed::new(99)))
        .expect("replay session");
    let mut before = CollectSink::default();
    session
        .run(ShotCount::new(9), &mut before)
        .expect("sample before replay");
    let mut replayed = CollectSink::default();
    {
        let mut replay = session
            .start_replay(ShotCount::new(65), &mut replayed)
            .expect("start replay delivery");
        assert!(
            replay
                .write_batch(error_records.get(..1).expect("first replay record"),)
                .expect("write first replay batch")
                .is_accepted()
        );
        for records in error_records
            .get(1..)
            .expect("remaining replay records")
            .chunks(11)
        {
            replay
                .write_batch(records)
                .expect("write incremental replay batch");
        }
        let summary = replay.finish().expect("finish replay");
        assert_eq!(summary.committed_shots().get(), 65);
    }
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

    let mut after = CollectSink::default();
    session
        .run(ShotCount::new(9), &mut after)
        .expect("sample after replay");
    let mut control = sampler
        .session(RandomPolicy::Seeded(Seed::new(99)))
        .expect("control session");
    let mut control_before = CollectSink::default();
    let mut control_after = CollectSink::default();
    control
        .run(ShotCount::new(9), &mut control_before)
        .expect("control prefix");
    control
        .run(ShotCount::new(9), &mut control_after)
        .expect("control suffix");
    assert_eq!(before.detectors, control_before.detectors);
    assert_eq!(before.observables, control_before.observables);
    assert_eq!(after.detectors, control_after.detectors);
    assert_eq!(after.observables, control_after.observables);
}

#[test]
fn replay_batch_validation_is_retryable_but_abandoned_output_poisons() {
    let sampler = compile_dem("error(1) D0\n");
    let valid = vec![vec![true], vec![false]];
    let invalid = vec![Vec::new()];

    let mut session = sampler
        .session(RandomPolicy::Seeded(Seed::new(1)))
        .expect("retryable replay session");
    let mut sink = CollectSink::default();
    {
        let mut replay = session
            .start_replay(ShotCount::new(2), &mut sink)
            .expect("start retryable replay");
        let error = replay
            .write_batch(&invalid)
            .expect_err("reject invalid replay width");
        assert_eq!(error.progress().committed_shots().get(), 0);
        replay
            .write_batch(&valid)
            .expect("retry valid replay delivery");
        replay.finish().expect("finish retried replay");
    }
    assert!(!session.is_poisoned());
    assert_eq!(sink.write_calls, 1);
    assert_eq!(sink.finish_calls, 1);

    let mut session = sampler
        .session(RandomPolicy::Seeded(Seed::new(1)))
        .expect("prefix-progress replay session");
    let mut sink = CollectSink::default();
    {
        let mut replay = session
            .start_replay(ShotCount::new(2), &mut sink)
            .expect("start prefix-progress replay");
        replay
            .write_batch(valid.get(..1).expect("first valid replay record"))
            .expect("commit valid replay prefix");
        let error = replay
            .write_batch(&invalid)
            .expect_err("reject invalid replay width after a valid prefix");
        assert_eq!(error.progress().committed_shots(), ShotCount::new(1));
        assert_eq!(error.progress().attempted_batch_shots(), ShotCount::new(1));
        replay
            .write_batch(valid.get(1..).expect("second valid replay record"))
            .expect("retry the second replay record");
        replay.finish().expect("finish prefix-progress replay");
    }
    assert!(!session.is_poisoned());
    assert_eq!(sink.write_calls, 2);
    assert_eq!(sink.finish_calls, 1);

    let mut session = sampler
        .session(RandomPolicy::Seeded(Seed::new(1)))
        .expect("abandoned replay session");
    let mut sink = CollectSink::default();
    {
        let mut replay = session
            .start_replay(ShotCount::new(2), &mut sink)
            .expect("start abandoned replay");
        replay
            .write_batch(valid.get(..1).expect("first valid replay record"))
            .expect("commit replay prefix");
    }
    assert!(session.is_poisoned());
    assert_eq!(sink.write_calls, 1);
    assert_eq!(sink.finish_calls, 0);
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
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .expect("cancellable replay session");
    let replay_cancellation = replay_session.cancellation();
    replay_cancellation.cancel();
    let replay_records = vec![vec![true]; 64];
    let mut replay_sink = CollectSink::default();
    let replay_summary = {
        let mut replay = replay_session
            .start_replay(ShotCount::new(65), &mut replay_sink)
            .expect("start cancelled replay");
        let status = replay
            .write_batch(&replay_records)
            .expect("observe replay cancellation");
        assert!(status.is_cancelled());
        replay.finish().expect("finish cancelled replay")
    };
    assert!(replay_summary.status().is_cancelled());
    assert_eq!(replay_summary.committed_shots(), ShotCount::new(0));
    assert_eq!(replay_sink.write_calls, 0);
    assert_eq!(replay_sink.finish_calls, 1);
    assert!(!replay_session.is_poisoned());

    replay_cancellation.reset();
    let cancellation_for_sink = replay_cancellation.clone();
    let mut replay_sink = CollectSink::after_write(move || {
        cancellation_for_sink.cancel();
    });
    let replay_summary = {
        let mut replay = replay_session
            .start_replay(ShotCount::new(2), &mut replay_sink)
            .expect("start replay cancelled before finish");
        replay
            .write_batch(&[vec![true], vec![false]])
            .expect("commit replay batch before cancellation");
        replay
            .finish()
            .expect("finish observes cooperative cancellation")
    };
    assert!(replay_summary.status().is_cancelled());
    assert_eq!(replay_summary.committed_shots(), ShotCount::new(2));
    assert_eq!(replay_sink.write_calls, 1);
    assert_eq!(replay_sink.finish_calls, 1);
    assert!(!replay_session.is_poisoned());

    replay_cancellation.reset();
    let mut resumed_sink = CollectSink::default();
    replay_session
        .run(ShotCount::new(1), &mut resumed_sink)
        .expect("session remains reusable after replay cancellation at finish");
}

#[test]
fn session_batch_capacity_obeys_the_caller_active_byte_budget() {
    let sampler = compile_dem("error(1) D0\n");
    let detector_record_bytes = std::mem::size_of::<stab_core::DetectionEventRecord>() + 1;
    let limits =
        DemSamplerLimits::default().with_max_materialized_bytes(detector_record_bytes + 16);
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
        DemSamplerLimits::default().with_max_materialized_bytes(detector_record_bytes + 7);
    let error = sampler
        .session_with_limits(RandomPolicy::Seeded(Seed::new(3)), rejected)
        .expect_err("one record plus one packed shot exceeds the active byte budget");
    assert!(
        error
            .into_circuit_error()
            .to_string()
            .contains("materialized bytes"),
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
                    .into_circuit_error()
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
    session
        .replay(&[], &mut zero_sink)
        .expect("zero-shot replay");
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
    let resource = replay_admission
        .resource_limit_error()
        .expect("plan replay admission exposes typed context");
    assert_eq!(
        resource.operation(),
        ResourceOperation::DetectorErrorModelSampling
    );
    assert_eq!(resource.resource(), ResourceKind::ReplayWorkUnits);
    assert_eq!(resource.actual(), 4);
    assert_eq!(resource.limit(), 2);
    let mut replay_session = sampler
        .session_with_limits(RandomPolicy::Seeded(Seed::new(22)), replay_limits)
        .expect("limited replay session");
    let mut replay_sink = CollectSink::default();
    let error = replay_session
        .start_replay(ShotCount::new(2), &mut replay_sink)
        .expect_err("reject replay work before delivery");
    assert_eq!(error.progress().committed_shots().get(), 0);
    assert_eq!(replay_sink.write_calls, 0);
    assert_eq!(replay_sink.finish_calls, 0);
    assert!(!replay_session.is_poisoned());
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

    let replay_records = vec![vec![true, false, true]; 64];
    let mut replay_session = sampler
        .session(RandomPolicy::Seeded(Seed::new(71)))
        .expect("replay allocation session");
    let mut replay_sink = WitnessSink::default();
    {
        let mut replay = replay_session
            .start_replay(ShotCount::new(1), &mut replay_sink)
            .expect("warm replay");
        replay
            .write_batch(replay_records.get(..1).expect("warm replay record"))
            .expect("deliver warm replay");
        replay.finish().expect("finish warm replay");
    }
    let allocations = allocation_counter::measure(|| {
        let mut replay = replay_session
            .start_replay(ShotCount::new(64), &mut replay_sink)
            .expect("start measured replay");
        replay
            .write_batch(&replay_records)
            .expect("deliver measured replay");
        replay.finish().expect("finish measured replay");
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
}
