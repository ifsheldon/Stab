#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "cancellation tests use compact fixture setup and exact outcome assertions"
)]

//! Cooperative cancellation and sink-failure contracts of every engine session.
//!
//! Each workload repeats its body 10^12 times inside the memory budget, so it can only return by
//! observing its cancellation signal. The worker reports when it is about to start the call; the
//! test cancels shortly after, and the deadline only guards against a hang.

use std::assert_matches;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use stab_engine::{
    DemReplayBatchStatus, DemSamplerLimits, DemSamplingCompiler, DemSamplingExecutionError,
    DemSamplingRunError, DetectionCompileError, DetectionConversionLimits, DetectionError,
    DetectionExecutionError, DetectionRunError, DetectionRunStatus, DetectionSamplingCompiler,
    LaneWidth, MeasurementToDetectionCompiler, RandomPolicy, ReferenceSampleMode, RunError,
    SamplingCompiler, SamplingExecutionError, Seed, ShotCount, SinkFailurePhase,
};
use stab_model::{Cancellation, Cancelled, Circuit, DetectorErrorModel};
use stab_records::{
    DemSampleBatchView, DemSampleSink, DetectionBatchView, DetectionSink, MeasurementBatchView,
    MeasurementSink, PackedShotBatch,
};

const HANG_GUARD: Duration = Duration::from_secs(120);
const CANCEL_DELAY: Duration = Duration::from_millis(50);

/// Unitary work repeated far beyond any test's lifetime before the only measurement.
const ENDLESS_SAMPLING: &str = "REPEAT 1000000000000 {\n    H 0\n}\nM 0\nDETECTOR rec[-1]\n";

/// The same endless work for a reference run, which folds a loop whose state recurs: the
/// sweep-controlled Pauli makes every iteration depend on more than the state it starts from,
/// so the reference simulation must run each one.
const ENDLESS_REFERENCE: &str =
    "REPEAT 1000000000000 {\n    H 0\n    CX sweep[0] 1\n}\nM 0\nDETECTOR rec[-1]\n";

#[derive(Debug, Default)]
struct CountingSink {
    writes: usize,
    finishes: usize,
    fail_writes: bool,
}

#[derive(Debug, Eq, PartialEq)]
struct SinkFailed;

impl CountingSink {
    fn failing() -> Self {
        Self {
            fail_writes: true,
            ..Self::default()
        }
    }

    fn record_write(&mut self) -> Result<(), SinkFailed> {
        self.writes += 1;
        if self.fail_writes {
            Err(SinkFailed)
        } else {
            Ok(())
        }
    }
}

impl MeasurementSink for CountingSink {
    type Error = SinkFailed;

    fn write_batch(&mut self, _batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        self.record_write()
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finishes += 1;
        Ok(())
    }
}

impl DetectionSink for CountingSink {
    type Error = SinkFailed;

    fn write_batch(&mut self, _batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        self.record_write()
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finishes += 1;
        Ok(())
    }
}

impl DemSampleSink for CountingSink {
    type Error = SinkFailed;

    fn write_batch(&mut self, _batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        self.record_write()
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finishes += 1;
        Ok(())
    }
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit fixture")
}

fn dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).expect("parse DEM fixture")
}

/// Runs `work` on a worker thread, cancels `cancellation` once the worker signals that it is
/// about to start the cancellable call, and returns the worker's result.
fn cancel_while_running<T: Send + 'static>(
    cancellation: &Cancellation,
    work: impl FnOnce(&dyn Fn()) -> T + Send + 'static,
) -> T {
    let (started_tx, started_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    thread::spawn(move || {
        let started = move || {
            started_tx.send(()).ok();
        };
        drop(result_tx.send(work(&started)));
    });
    started_rx
        .recv_timeout(HANG_GUARD)
        .expect("worker never reached the cancellable call");
    thread::sleep(CANCEL_DELAY);
    cancellation.cancel();
    result_rx
        .recv_timeout(HANG_GUARD)
        .expect("cancellation was not observed before the hang guard")
}

#[test]
fn reference_sample_computation_stops_when_cancelled() {
    let cancellation = Cancellation::new();
    let plan = SamplingCompiler::new()
        .cancellation(cancellation.clone())
        .compile(&circuit(ENDLESS_SAMPLING))
        .expect("compile endless reference sample");
    // The plan computes its reference sample lazily, so a signal set after compilation is
    // observed only by the reference simulator's repeat interpretation.
    cancellation.cancel();
    assert_matches!(
        plan.session(RandomPolicy::Seeded(Seed::new(1))),
        Err(SamplingExecutionError::Cancelled(Cancelled))
    );
    assert_matches!(
        plan.try_reference_sample(),
        Err(SamplingExecutionError::Cancelled(Cancelled))
    );
    assert_matches!(
        SamplingCompiler::new()
            .cancellation(cancellation)
            .compile(&circuit(ENDLESS_SAMPLING)),
        Err(stab_engine::SamplingCompileError::Cancelled(Cancelled))
    );
}

#[test]
fn sampling_batch_stops_without_delivering_a_partial_batch() {
    for &lane_width in LaneWidth::ALL {
        let cancellation = Cancellation::new();
        let signal = cancellation.clone();
        let (result, sink, poisoned) = cancel_while_running(&cancellation, move |started| {
            let plan = SamplingCompiler::new()
                .cancellation(signal)
                .compile(&circuit(ENDLESS_SAMPLING))
                .expect("compile endless sampler");
            let mut session = plan
                .session_with_lane_width(
                    RandomPolicy::Seeded(Seed::new(3)),
                    ReferenceSampleMode::SkipReferenceSample,
                    lane_width,
                )
                .expect("skip-reference session needs no reference simulation");
            let mut sink = CountingSink::default();
            started();
            let result = session.run(ShotCount::new(1), &mut sink);
            (result, sink, session.is_poisoned())
        });
        match result {
            Err(RunError::Engine {
                source: SamplingExecutionError::Cancelled(Cancelled),
                progress,
            }) => assert_eq!(progress.committed_shots(), ShotCount::new(0)),
            other => panic!("expected an in-batch cancellation at {lane_width:?}, got {other:?}"),
        }
        assert_eq!((sink.writes, sink.finishes), (0, 0));
        assert!(
            poisoned,
            "the random stream advanced into the discarded batch"
        );
    }
}

#[test]
fn detection_sampling_batch_stops_without_delivering_a_partial_batch() {
    for &lane_width in LaneWidth::ALL {
        let cancellation = Cancellation::new();
        let signal = cancellation.clone();
        let (result, sink, poisoned) = cancel_while_running(&cancellation, move |started| {
            let plan = DetectionSamplingCompiler::new()
                .limits(DetectionConversionLimits::default().with_cancellation(signal))
                .compile(&circuit(ENDLESS_SAMPLING))
                .expect("compile endless detection sampler");
            let mut session = plan
                .session_with_lane_width(RandomPolicy::Seeded(Seed::new(5)), lane_width)
                .expect("create detection session");
            let mut sink = CountingSink::default();
            started();
            let result = session.run(ShotCount::new(1), &mut sink);
            (result, sink, session.is_poisoned())
        });
        match result {
            Err(DetectionRunError::Engine {
                source: DetectionExecutionError::Cancelled(Cancelled),
                progress,
            }) => assert_eq!(progress.committed_shots(), ShotCount::new(0)),
            other => panic!("expected an in-batch cancellation at {lane_width:?}, got {other:?}"),
        }
        assert_eq!((sink.writes, sink.finishes), (0, 0));
        assert!(poisoned);
    }
}

#[test]
fn measurement_to_detection_reference_sample_stops_when_cancelled() {
    let cancellation = Cancellation::new();
    let signal = cancellation.clone();
    let result = cancel_while_running(&cancellation, move |started| {
        let compiler = MeasurementToDetectionCompiler::new()
            .limits(DetectionConversionLimits::default().with_cancellation(signal));
        started();
        compiler.compile(&circuit(ENDLESS_REFERENCE)).map(drop)
    });
    assert_eq!(
        result,
        Err(DetectionCompileError::InvalidCircuit(
            DetectionError::Cancelled(Cancelled)
        ))
    );
}

#[test]
fn measurement_to_detection_sweep_correction_rejects_the_whole_batch_and_stays_resumable() {
    let cancellation = Cancellation::new();
    let signal = cancellation.clone();
    let (summary, sink, poisoned) = cancel_while_running(&cancellation, move |started| {
        let fixture =
            circuit("CX sweep[0] 0\nREPEAT 1000000000000 {\n    H 0\n}\nM 0\nDETECTOR rec[-1]\n");
        let plan = MeasurementToDetectionCompiler::new()
            .limits(DetectionConversionLimits::default().with_cancellation(signal))
            .reference_sample_mode(ReferenceSampleMode::SkipReferenceSample)
            .compile(&fixture)
            .expect("compile sweep-corrected conversion");
        let mut session = plan.session().expect("create conversion session");
        let measurements = PackedShotBatch::from_records(&[vec![true]], 1).expect("pack");
        let sweeps = PackedShotBatch::from_records(&[vec![true]], 1).expect("pack");
        let mut sink = CountingSink::default();
        started();
        let summary = session
            .run(
                MeasurementBatchView::new(measurements.view()),
                Some(MeasurementBatchView::new(sweeps.view())),
                &mut sink,
            )
            .expect("an in-batch conversion cancellation is not a failure");
        (summary, sink, session.is_poisoned())
    });
    assert_eq!(summary.status(), DetectionRunStatus::Cancelled);
    assert_eq!(summary.committed_shots(), ShotCount::new(0));
    assert_eq!(sink.writes, 0, "no partially converted batch is delivered");
    assert!(
        !poisoned,
        "conversion consumes no random stream, so it resumes"
    );
}

#[test]
fn dem_sampling_batch_stops_and_stays_resumable() {
    // A detector shift defeats the zero-shift fold, so every iteration samples its error.
    let model = dem("repeat 1000000000000 {\n    error(0.25) L0\n    shift_detectors 1\n}\n");
    let cancellation = Cancellation::new();
    let signal = cancellation.clone();
    let (summary, sink, poisoned, total) = cancel_while_running(&cancellation, move |started| {
        let plan = DemSamplingCompiler::new()
            .compile(&model)
            .expect("compile endless DEM sampler");
        let mut session = plan
            .session_with_limits(
                RandomPolicy::Seeded(Seed::new(7)),
                DemSamplerLimits::default().with_cancellation(signal),
            )
            .expect("create DEM session");
        let mut sink = CountingSink::default();
        started();
        let summary = session
            .run(ShotCount::new(1), &mut sink)
            .expect("an in-batch DEM cancellation is not a failure");
        (
            summary,
            sink,
            session.is_poisoned(),
            session.total_committed_shots(),
        )
    });
    assert!(summary.status().is_cancelled());
    assert_eq!(summary.committed_shots(), ShotCount::new(0));
    assert_eq!((sink.writes, sink.finishes), (0, 1));
    assert!(
        !poisoned,
        "counter-based DEM streams resume after cancellation"
    );
    assert_eq!(total, ShotCount::new(0));
}

#[test]
fn dem_replay_batch_stops_without_delivering_a_partial_batch() {
    // Replay work scales with the set bits of the supplied records, so the batch carries 16
    // records of 2^22 set errors each: far more work than the cancellation delay allows.
    const ERRORS: usize = 1 << 22;
    const SHOTS: usize = 16;
    let model = dem(&format!("repeat {ERRORS} {{\n    error(0.5) D0\n}}\n"));
    let cancellation = Cancellation::new();
    let signal = cancellation.clone();
    let (status, sink, poisoned) = cancel_while_running(&cancellation, move |started| {
        let plan = DemSamplingCompiler::new()
            .compile(&model)
            .expect("compile large replay");
        let mut records = PackedShotBatch::zeros(SHOTS, ERRORS).expect("allocate records");
        let all_set = vec![u8::MAX; ERRORS / 8];
        for shot in 0..SHOTS {
            records
                .copy_shot_from_b8(shot, &all_set)
                .expect("fill replay record");
        }
        let mut replay = plan
            .replay_session_with_limits(
                ShotCount::new(SHOTS as u64),
                DemSamplerLimits::default().with_cancellation(signal),
            )
            .expect("create replay session");
        let mut sink = CountingSink::default();
        let mut transaction = replay.start_transaction(&mut sink).expect("start replay");
        let view = records.view_prefix(SHOTS).expect("view replay records");
        started();
        let status = transaction
            .write_packed_batch(view)
            .expect("an in-batch replay cancellation is not a failure");
        let summary = transaction.finish().expect("finish cancelled replay");
        assert!(summary.status().is_cancelled());
        (status, sink, replay.is_poisoned())
    });
    assert_eq!(status, DemReplayBatchStatus::Cancelled);
    assert_eq!((sink.writes, sink.finishes), (0, 1));
    assert!(!poisoned);
}

#[test]
fn a_sink_error_stops_measurement_to_detection_within_one_batch() {
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit("M 0\nDETECTOR rec[-1]\n"))
        .expect("compile conversion");
    let mut session = plan.session().expect("create conversion session");
    let input = PackedShotBatch::from_records(&[vec![true]], 1).expect("pack");
    let mut sink = CountingSink::failing();
    let mut transaction = session.start_transaction(&mut sink).expect("start");
    let error = transaction
        .write_batch_with_sweep(MeasurementBatchView::new(input.view()), None)
        .expect_err("the sink rejects the first batch");
    assert_matches!(
        error,
        DetectionRunError::Sink {
            phase: SinkFailurePhase::WriteBatch,
            source: SinkFailed,
            ..
        }
    );
    assert_matches!(
        transaction.write_batch_with_sweep(MeasurementBatchView::new(input.view()), None),
        Err(DetectionRunError::Engine {
            source: DetectionExecutionError::SessionPoisoned,
            ..
        })
    );
    drop(transaction);
    assert_eq!((sink.writes, sink.finishes), (1, 0));
    assert!(session.is_poisoned());
}

#[test]
fn a_sink_error_stops_dem_replay_within_one_batch() {
    let plan = DemSamplingCompiler::new()
        .compile(&dem("error(0.5) D0\n"))
        .expect("compile replay");
    let records = vec![vec![true]; 200];
    let mut replay = plan
        .replay_session(ShotCount::new(200))
        .expect("create replay session");
    let mut sink = CountingSink::failing();
    match replay.run(&records, &mut sink) {
        Err(DemSamplingRunError::Sink {
            phase: SinkFailurePhase::WriteBatch,
            source: SinkFailed,
            progress,
        }) => assert_eq!(progress.committed_shots(), ShotCount::new(0)),
        other => panic!("expected the first replay batch to fail, got {other:?}"),
    }
    assert_eq!((sink.writes, sink.finishes), (1, 0));
    assert!(replay.is_poisoned());
    assert_matches!(
        replay.reset(),
        Err(DemSamplingExecutionError::SessionPoisoned)
    );
}
