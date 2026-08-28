#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "sampling lifecycle tests use fixed valid fixtures and explicit failure matching"
)]

use stab_engine::{
    RandomPolicy, RunError, SamplingCancellation, SamplingCompiler, SamplingExecutionError,
    SamplingPlan, SamplingRunStatus, Seed, ShotCount, SinkFailurePhase,
};
use stab_model::Circuit;
use stab_records::{MeasurementBatchView, MeasurementSink};

#[derive(Debug, Eq, PartialEq)]
enum TestSinkError {
    Finish,
}

#[derive(Debug, Default)]
struct ProbeSink {
    records: Vec<Vec<bool>>,
    write_calls: usize,
    finish_calls: usize,
    cancel_after_write: Option<SamplingCancellation>,
    fail_finish: bool,
}

impl MeasurementSink for ProbeSink {
    type Error = TestSinkError;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        self.write_calls += 1;
        for shot_index in 0..batch.shot_count() {
            let record = (0..batch.width().get())
                .map(|bit_index| {
                    batch
                        .get(shot_index, bit_index)
                        .expect("batch coordinates are within the reported dimensions")
                })
                .collect();
            self.records.push(record);
        }
        if let Some(cancellation) = &self.cancel_after_write {
            cancellation.cancel();
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finish_calls += 1;
        if self.fail_finish {
            Err(TestSinkError::Finish)
        } else {
            Ok(())
        }
    }
}

fn noisy_plan() -> SamplingPlan {
    let circuit = Circuit::from_stim_str("H 0\nM 0\nCX rec[-1] 1\nM(0.125) 1\n")
        .expect("parse noisy sampling circuit");
    SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile noisy sampling circuit")
}

fn clean_records(plan: &SamplingPlan, seed: u64, shots: u64) -> Vec<Vec<bool>> {
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(seed)))
        .expect("construct control session");
    let mut sink = ProbeSink::default();
    let summary = session
        .run(ShotCount::new(shots), &mut sink)
        .expect("run control session");
    assert_eq!(summary.status(), SamplingRunStatus::Completed);
    assert_eq!(summary.committed_shots(), ShotCount::new(shots));
    sink.records
}

#[test]
fn zero_shot_run_leaves_sink_and_random_stream_untouched() {
    let plan = noisy_plan();
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(29)))
        .expect("construct session");
    let mut empty_sink = ProbeSink::default();

    let summary = session
        .run(ShotCount::new(0), &mut empty_sink)
        .expect("run zero shots");

    assert_eq!(summary.status(), SamplingRunStatus::Completed);
    assert_eq!(summary.committed_shots(), ShotCount::new(0));
    assert_eq!(summary.total_committed_shots(), ShotCount::new(0));
    assert_eq!((empty_sink.write_calls, empty_sink.finish_calls), (0, 0));

    let mut resumed_sink = ProbeSink::default();
    session
        .run(ShotCount::new(32), &mut resumed_sink)
        .expect("run after zero-shot request");
    assert_eq!(resumed_sink.records, clean_records(&plan, 29, 32));
}

#[test]
fn cancellation_between_batches_preserves_the_resumable_random_stream() {
    let plan = noisy_plan();
    let requested = 386_u64;
    let expected = clean_records(&plan, 31, requested);
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(31)))
        .expect("construct cancellable session");
    let cancellation = session.cancellation();
    let mut first = ProbeSink {
        cancel_after_write: Some(cancellation.clone()),
        ..ProbeSink::default()
    };

    let summary = session
        .run(ShotCount::new(requested), &mut first)
        .expect("cancel after the first completed batch");
    let committed = summary.committed_shots().get();

    assert_eq!(summary.status(), SamplingRunStatus::Cancelled);
    assert!((1..requested).contains(&committed));
    assert_eq!(
        first.records.len(),
        usize::try_from(committed).expect("one bounded batch fits usize")
    );
    assert_eq!((first.write_calls, first.finish_calls), (1, 1));

    cancellation.reset();
    let mut resumed = ProbeSink::default();
    let resumed_summary = session
        .run(ShotCount::new(requested - committed), &mut resumed)
        .expect("resume the same session");
    first.records.extend(resumed.records);

    assert_eq!(resumed_summary.status(), SamplingRunStatus::Completed);
    assert_eq!(
        resumed_summary.total_committed_shots(),
        ShotCount::new(requested)
    );
    assert_eq!(first.records, expected);
    assert!(!session.is_poisoned());
}

#[test]
fn pre_cancelled_run_finalizes_without_work_and_remains_resumable() {
    let plan = noisy_plan();
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(35)))
        .expect("construct cancellable session");
    let cancellation = session.cancellation();
    cancellation.cancel();
    let mut cancelled_sink = ProbeSink::default();

    let summary = session
        .run(ShotCount::new(65), &mut cancelled_sink)
        .expect("run pre-cancelled request");

    assert_eq!(summary.status(), SamplingRunStatus::Cancelled);
    assert_eq!(summary.committed_shots(), ShotCount::new(0));
    assert_eq!(summary.total_committed_shots(), ShotCount::new(0));
    assert_eq!(
        (cancelled_sink.write_calls, cancelled_sink.finish_calls),
        (0, 1)
    );
    assert!(!session.is_poisoned());

    cancellation.reset();
    let mut resumed_sink = ProbeSink::default();
    session
        .run(ShotCount::new(65), &mut resumed_sink)
        .expect("resume after pre-cancellation");
    assert_eq!(resumed_sink.records, clean_records(&plan, 35, 65));
}

#[test]
fn pre_cancelled_finish_failure_poisons_and_rejects_the_session() {
    let plan = noisy_plan();
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(36)))
        .expect("construct cancellable session");
    session.cancellation().cancel();
    let mut failing_sink = ProbeSink {
        fail_finish: true,
        ..ProbeSink::default()
    };

    match session.run(ShotCount::new(65), &mut failing_sink) {
        Err(RunError::Sink {
            phase,
            source,
            progress,
        }) => {
            assert_eq!(phase, SinkFailurePhase::Finish);
            assert_eq!(source, TestSinkError::Finish);
            assert_eq!(progress.committed_shots(), ShotCount::new(0));
            assert_eq!(progress.attempted_batch_shots(), ShotCount::new(0));
        }
        other => panic!("expected pre-cancelled finish failure, got {other:?}"),
    }
    assert_eq!(
        (failing_sink.write_calls, failing_sink.finish_calls),
        (0, 1)
    );
    assert!(session.is_poisoned());
    assert_eq!(session.total_committed_shots(), ShotCount::new(0));

    let mut rejected_sink = ProbeSink::default();
    match session.run(ShotCount::new(1), &mut rejected_sink) {
        Err(RunError::Engine { source, progress }) => {
            assert_eq!(source, SamplingExecutionError::SessionPoisoned);
            assert_eq!(progress.committed_shots(), ShotCount::new(0));
            assert_eq!(progress.attempted_batch_shots(), ShotCount::new(0));
        }
        other => panic!("expected poisoned-session rejection, got {other:?}"),
    }
    assert_eq!(
        (rejected_sink.write_calls, rejected_sink.finish_calls),
        (0, 0)
    );
}
