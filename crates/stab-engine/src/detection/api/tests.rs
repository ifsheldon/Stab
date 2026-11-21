#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "focused detection lifecycle tests use compact fixture assertions"
)]

use std::assert_matches;
use std::convert::Infallible;

use stab_records::{BitIndex, MeasurementSink, PackedShotBatch, ShotIndex};

use super::super::test_support::convert_measurements_to_detection_events_with_sweep;
use super::*;
use crate::detection::DetectionRecordBuffer;
use crate::{DetectionResourceKind, RandomPolicy, Seed};

#[derive(Default)]
struct CollectSink {
    records: Vec<DetectionRecordBuffer>,
    batch_sizes: Vec<usize>,
    finish_count: usize,
}

impl DetectionSink for CollectSink {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        self.batch_sizes.push(batch.shot_count());
        for shot_index in 0..batch.shot_count() {
            let detectors = (0..batch.detector_width().get())
                .map(|bit_index| {
                    batch
                        .detectors()
                        .get(ShotIndex(shot_index), BitIndex(bit_index))
                        .expect("detector bit")
                })
                .collect();
            let observables = (0..batch.observable_width().get())
                .map(|bit_index| {
                    batch
                        .observables()
                        .get(ShotIndex(shot_index), BitIndex(bit_index))
                        .expect("observable bit")
                })
                .collect();
            self.records.push(DetectionRecordBuffer {
                detectors,
                observables,
            });
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finish_count += 1;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestSinkError {
    Write,
    Finish,
}

struct FailingSink {
    fail_write: bool,
    fail_finish: bool,
    writes: usize,
}

impl DetectionSink for FailingSink {
    type Error = TestSinkError;

    fn write_batch(&mut self, _batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        self.writes += 1;
        if self.fail_write {
            return Err(TestSinkError::Write);
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        if self.fail_finish {
            return Err(TestSinkError::Finish);
        }
        Ok(())
    }
}

struct CancellingSink {
    inner: CollectSink,
    cancellation: stab_model::Cancellation,
}

impl DetectionSink for CancellingSink {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        self.inner.write_batch(batch)?;
        self.cancellation.cancel();
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.inner.finish()
    }
}

#[derive(Default)]
struct NullSink {
    shots: usize,
}

impl DetectionSink for NullSink {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        self.shots += batch.shot_count();
        std::hint::black_box(batch.detectors().get(ShotIndex(0), BitIndex(0)));
        std::hint::black_box(batch.observables().get(ShotIndex(0), BitIndex(0)));
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse detection test circuit")
}

fn packed(records: &[Vec<bool>], width: usize) -> PackedShotBatch {
    PackedShotBatch::from_records(records, width).expect("pack measurement records")
}

fn run_plan(plan: &DetectionSamplingPlan, shots: u64, seed: u64) -> CollectSink {
    run_plan_at(plan, shots, seed, LaneWidth::default())
}

fn run_plan_at(
    plan: &DetectionSamplingPlan,
    shots: u64,
    seed: u64,
    lane_width: LaneWidth,
) -> CollectSink {
    let mut session = plan
        .session(&DetectionSamplingSessionOptions {
            random: RandomPolicy::Seeded(Seed::new(seed)),
            lane_width,
            ..Default::default()
        })
        .expect("create detection session");
    assert_eq!(session.lane_width(), lane_width);
    let mut sink = CollectSink::default();
    let summary = session
        .run(ShotCount::new(shots), &mut sink)
        .expect("run detection session");
    assert_eq!(summary.status(), RunStatus::Completed);
    assert_eq!(summary.requested_shots(), ShotCount::new(shots));
    assert_eq!(summary.committed_shots(), ShotCount::new(shots));
    assert_eq!(summary.total_committed_shots(), ShotCount::new(shots));
    sink
}

fn assert_send_sync<T: Send + Sync>() {}

mod m2d_oracle;
mod storage;

#[test]
fn measurement_sink_transaction_preserves_sweep_semantics_and_sink_lifecycle() {
    let circuit =
        circuit("H 0\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(1) rec[-1]\n");
    let measurements = vec![vec![false], vec![false], vec![true], vec![true]];
    let sweeps = vec![vec![false], vec![true], vec![false], vec![true]];
    let expected = convert_measurements_to_detection_events_with_sweep(
        &circuit,
        &measurements,
        &sweeps,
        ReferenceSampleMode::UseReferenceSample,
    )
    .expect("materialize transaction oracle");
    let plan = MeasurementToDetectionCompiler::default()
        .compile(&circuit)
        .expect("compile transaction plan");
    assert_eq!(plan.measurement_width().get(), 1);
    assert_eq!(plan.sweep_width().get(), 1);
    let measurement_batch = packed(&measurements, 1);
    let sweep_batch = packed(&sweeps, 1);
    let mut session = plan
        .session(&Default::default())
        .expect("create transaction session");
    let mut sink = CollectSink::default();
    {
        let mut transaction = session
            .start_transaction(&mut sink)
            .expect("start conversion transaction");
        let summary = transaction
            .write_batch_with_sweep(
                MeasurementBatchView::new(measurement_batch.view()),
                Some(MeasurementBatchView::new(sweep_batch.view())),
            )
            .expect("convert measurement batch with sweep data");
        assert_eq!(summary.status(), RunStatus::Completed);
        transaction.finish().expect("finish conversion sink");
    }
    assert_eq!(sink.records, expected.records);
    assert_eq!(sink.finish_count, 1);
}

#[test]
fn conversion_cancellation_rejects_a_whole_batch_and_is_resumable() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::default()
        .compile(&circuit)
        .expect("compile cancellable conversion");
    let input = packed(&[vec![true]], 1);
    let mut session = plan
        .session(&Default::default())
        .expect("create cancellable conversion");
    let cancellation = session.cancellation();
    cancellation.cancel();
    let mut untouched = CollectSink::default();
    let mut delivery = session
        .start_transaction(&mut untouched)
        .expect("start cancellable delivery");
    let summary = delivery
        .write_batch_with_sweep(MeasurementBatchView::new(input.view()), None)
        .expect("cancel conversion batch");
    assert_eq!(summary.status(), RunStatus::Cancelled);
    assert_eq!(summary.committed_shots(), ShotCount::new(0));

    cancellation.reset();
    delivery
        .write_batch_with_sweep(MeasurementBatchView::new(input.view()), None)
        .expect("resume conversion batch");
    delivery.finish().expect("finish resumed sink");
    assert_eq!(untouched.records.len(), 1);
    assert!(!session.is_poisoned());
    assert_eq!(session.total_committed_shots(), ShotCount::new(1));
}

#[test]
fn composed_conversion_cancellation_reports_the_committed_transaction_prefix() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::default()
        .compile(&circuit)
        .expect("compile composed conversion");
    let input = packed(&[vec![true]], 1);
    let mut session = plan
        .session(&Default::default())
        .expect("create composed conversion");
    let cancellation = session.cancellation();
    let mut sink = CollectSink::default();
    let mut transaction = session
        .start_transaction(&mut sink)
        .expect("start composed conversion");

    MeasurementSink::write_batch(&mut transaction, MeasurementBatchView::new(input.view()))
        .expect("commit first conversion batch");
    cancellation.cancel();
    let error =
        MeasurementSink::write_batch(&mut transaction, MeasurementBatchView::new(input.view()))
            .expect_err("cancel second conversion batch");

    assert_matches!(
        &error,
        RunError::Engine {
            source: DetectionExecutionError::CancelledComposition,
            ..
        }
    );
    assert_eq!(error.progress().committed_shots(), ShotCount::new(1));
    assert_eq!(error.progress().attempted_batch_shots(), ShotCount::new(1));
    cancellation.reset();
    transaction.finish().expect("finish committed prefix");
    assert_eq!(sink.records.len(), 1);
}

#[test]
fn sweep_batches_and_reference_modes_match_materialized_conversion() {
    let circuit =
        circuit("H 0\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(1) rec[-1]\n");
    let measurements = vec![vec![false], vec![false], vec![true], vec![true]];
    let sweeps = vec![vec![false], vec![true], vec![false], vec![true]];
    let measurement_batch = packed(&measurements, 1);
    let sweep_batch = packed(&sweeps, 1);

    for (mode, skip_reference_sample) in [
        (ReferenceSampleMode::UseReferenceSample, false),
        (ReferenceSampleMode::SkipReferenceSample, true),
    ] {
        let expected = convert_measurements_to_detection_events_with_sweep(
            &circuit,
            &measurements,
            &sweeps,
            if skip_reference_sample {
                ReferenceSampleMode::SkipReferenceSample
            } else {
                ReferenceSampleMode::UseReferenceSample
            },
        )
        .expect("materialized sweep conversion");
        let plan = MeasurementToDetectionCompiler {
            reference_sample_mode: mode,
            ..MeasurementToDetectionCompiler::default()
        }
        .compile(&circuit)
        .expect("compile sweep conversion");
        let mut session = plan
            .session(&Default::default())
            .expect("create sweep conversion session");
        let mut sink = CollectSink::default();
        session
            .run(
                MeasurementBatchView::new(measurement_batch.view()),
                Some(MeasurementBatchView::new(sweep_batch.view())),
                &mut sink,
            )
            .expect("stream sweep conversion");
        assert_eq!(sink.records, expected.records);
    }
}

#[test]
fn record_at_a_time_conversion_preserves_valid_prefix_and_preflight_reuse() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::default()
        .compile(&circuit)
        .expect("compile conversion");
    let mut session = plan
        .session(&Default::default())
        .expect("create conversion session");
    let valid = packed(&[vec![true]], 1);
    let invalid = packed(&[vec![true, false]], 2);
    let mut sink = CollectSink::default();

    let mut delivery = session
        .start_transaction(&mut sink)
        .expect("start prefix delivery");
    delivery
        .write_batch_with_sweep(MeasurementBatchView::new(valid.view()), None)
        .expect("write valid prefix");
    let error = delivery
        .write_batch_with_sweep(MeasurementBatchView::new(invalid.view()), None)
        .expect_err("reject malformed later record");
    assert_matches!(
        error,
        RunError::Engine {
            source: DetectionExecutionError::Conversion(_),
            ..
        }
    );
    assert_eq!(error.progress().committed_shots(), ShotCount::new(1));

    delivery
        .write_batch_with_sweep(MeasurementBatchView::new(valid.view()), None)
        .expect("reuse after preflight rejection");
    delivery.finish().expect("finish prefix sink");
    assert_eq!(sink.records.len(), 2);
    assert_eq!(sink.finish_count, 1);
    assert!(!session.is_poisoned());
    assert_eq!(session.total_committed_shots(), ShotCount::new(2));
}

#[test]
fn incremental_delivery_finalizes_once_and_rejects_post_finish_writes() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::default()
        .compile(&circuit)
        .expect("compile delivery lifecycle");
    let input = packed(&[vec![true]], 1);
    let mut session = plan
        .session(&Default::default())
        .expect("create delivery lifecycle session");
    let mut sink = CollectSink::default();
    let mut delivery = session
        .start_transaction(&mut sink)
        .expect("start delivery lifecycle");
    delivery
        .write_batch_with_sweep(MeasurementBatchView::new(input.view()), None)
        .expect("write delivery prefix");
    MeasurementSink::finish(&mut delivery).expect("finish delivery once");

    let repeated = MeasurementSink::finish(&mut delivery).expect_err("reject double finish");
    assert_matches!(
        repeated,
        RunError::Engine {
            source: DetectionExecutionError::DeliveryFinished,
            ..
        }
    );
    assert_eq!(repeated.progress().committed_shots(), ShotCount::new(1));
    let post_finish = delivery
        .write_batch_with_sweep(MeasurementBatchView::new(input.view()), None)
        .expect_err("reject write after finish");
    assert_matches!(
        post_finish,
        RunError::Engine {
            source: DetectionExecutionError::DeliveryFinished,
            ..
        }
    );
    assert_eq!(post_finish.progress().committed_shots(), ShotCount::new(1));
    drop(delivery);

    assert_eq!(sink.finish_count, 1);
    assert!(!session.is_poisoned());
    assert_eq!(session.total_committed_shots(), ShotCount::new(1));
}

#[test]
fn incremental_finish_failure_reports_committed_prefix_and_poisons_session() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::default()
        .compile(&circuit)
        .expect("compile finish failure lifecycle");
    let input = packed(&[vec![true]], 1);
    let mut session = plan
        .session(&Default::default())
        .expect("create finish failure lifecycle session");
    let mut sink = FailingSink {
        fail_write: false,
        fail_finish: true,
        writes: 0,
    };
    let mut delivery = session
        .start_transaction(&mut sink)
        .expect("start finish failure delivery");
    for _ in 0..2 {
        delivery
            .write_batch_with_sweep(MeasurementBatchView::new(input.view()), None)
            .expect("write committed prefix");
    }
    let error = delivery.finish().expect_err("surface finish failure");
    assert_matches!(
        error,
        RunError::Sink {
            phase: SinkFailurePhase::Finish,
            source: TestSinkError::Finish,
            ..
        }
    );
    assert_eq!(error.progress().committed_shots(), ShotCount::new(2));
    assert!(session.is_poisoned());
    assert_eq!(session.total_committed_shots(), ShotCount::new(2));
}

#[test]
fn dropping_a_committed_incremental_delivery_poisons_the_parent_session() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::default()
        .compile(&circuit)
        .expect("compile abandoned delivery");
    let input = packed(&[vec![true]], 1);
    let mut session = plan
        .session(&Default::default())
        .expect("create abandoned delivery session");
    let mut sink = CollectSink::default();
    {
        let mut delivery = session
            .start_transaction(&mut sink)
            .expect("start abandoned delivery");
        delivery
            .write_batch_with_sweep(MeasurementBatchView::new(input.view()), None)
            .expect("write abandoned prefix");
    }
    assert!(session.is_poisoned());
    assert_eq!(sink.finish_count, 0);
}

#[test]
#[allow(
    clippy::mem_forget,
    reason = "prove forgotten transactions fail closed instead of rebinding a sink"
)]
fn forgotten_incremental_transaction_cannot_rebind_another_sink() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::default()
        .compile(&circuit)
        .expect("compile forgotten transaction");
    let input = packed(&[vec![true]], 1);
    let mut session = plan
        .session(&Default::default())
        .expect("create forgotten transaction session");
    let mut first_sink = CollectSink::default();
    let mut transaction = session
        .start_transaction(&mut first_sink)
        .expect("start forgotten transaction");
    transaction
        .write_batch_with_sweep(MeasurementBatchView::new(input.view()), None)
        .expect("write forgotten transaction prefix");
    std::mem::forget(transaction);

    let mut second_sink = CollectSink::default();
    let error = session
        .start_transaction(&mut second_sink)
        .expect_err("an active forgotten transaction must block sink rebinding");
    assert_eq!(error, DetectionExecutionError::TransactionActive);
    assert_eq!(first_sink.finish_count, 0);
    assert_eq!(second_sink.finish_count, 0);
}

#[test]
fn direct_detection_sampling_reports_widths_and_bounded_batches() {
    for (circuit_text, measurements, detectors, observables) in [
        ("X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n", 1, 1, 0),
        ("RX 0\nZ_ERROR(0.25) 0\nOBSERVABLE_INCLUDE(0) X0\n", 0, 0, 1),
    ] {
        let plan = DetectionSamplingCompiler::default()
            .compile(&circuit(circuit_text))
            .expect("compile detection sampling");
        assert_eq!(plan.measurement_width().get(), measurements);
        assert_eq!(plan.detector_width().get(), detectors);
        assert_eq!(plan.observable_width().get(), observables);
        let actual = run_plan(&plan, 513, 31);
        assert_eq!(actual.records.len(), 513);
        assert_eq!(actual.batch_sizes, vec![256, 256, 1]);
        assert_eq!(actual.finish_count, 1);
        let narrow = run_plan_at(&plan, 129, 31, LaneWidth::Lanes64);
        assert_eq!(narrow.records.len(), 129);
        assert_eq!(narrow.batch_sizes, vec![64, 64, 1]);
        assert_eq!(narrow.finish_count, 1);
    }
}

#[test]
fn noiseless_detection_is_exact_at_every_batch_boundary_and_partition() {
    // Resets and measurements randomize the frame lanes, but every detector and observable is
    // deterministic, so every record must be all false however the shots split across batches.
    let plan = DetectionSamplingCompiler::default()
        .compile(&circuit(
            "RX 0\nH 0\nM 0\nDETECTOR rec[-1]\nX 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]\n\
             MR 1 1\nDETECTOR rec[-1] rec[-2]\nMX 2\nMX 2\nDETECTOR rec[-1] rec[-2]\n",
        ))
        .expect("compile noiseless detection plan");
    let zero = DetectionRecordBuffer {
        detectors: vec![false; 3],
        observables: vec![false],
    };
    for &lane_width in LaneWidth::ALL {
        for shots in [1_u64, 63, 64, 65, 255, 256, 257, 1000] {
            let whole = run_plan_at(&plan, shots, 3, lane_width);
            assert_eq!(whole.records.len() as u64, shots);
            assert!(
                whole.records.iter().all(|record| *record == zero),
                "{shots} shots at {lane_width:?}"
            );
            assert!(
                whole
                    .batch_sizes
                    .iter()
                    .all(|size| *size <= lane_width.shots_per_batch())
            );

            let mut session = plan
                .session(&DetectionSamplingSessionOptions {
                    random: RandomPolicy::Seeded(Seed::new(3)),
                    lane_width,
                    ..Default::default()
                })
                .expect("create partitioned session");
            let mut records = 0_usize;
            for part in [shots / 3, shots - shots / 3] {
                let mut sink = CollectSink::default();
                session
                    .run(ShotCount::new(part), &mut sink)
                    .expect("run partition");
                assert!(sink.records.iter().all(|record| *record == zero));
                records += sink.records.len();
            }
            assert_eq!(records as u64, shots);
        }
    }
}

#[test]
fn seeded_same_session_partitioning_is_exact() {
    let circuit = circuit("X_ERROR(0.375) 0\nM 0\nDETECTOR rec[-1]\n");
    let plan = DetectionSamplingCompiler::default()
        .compile(&circuit)
        .expect("compile detection plan");
    let mut streams = Vec::new();
    for &lane_width in LaneWidth::ALL {
        let combined = run_plan_at(&plan, 600, 44, lane_width).records;
        let mut session = plan
            .session(&DetectionSamplingSessionOptions {
                random: RandomPolicy::Seeded(Seed::new(44)),
                lane_width,
                ..Default::default()
            })
            .expect("create partitioned session");
        let mut partitioned = Vec::new();
        for shots in [17, 47, 1, 63, 300, 1, 171] {
            let mut sink = CollectSink::default();
            session
                .run(ShotCount::new(shots), &mut sink)
                .expect("run partition");
            partitioned.extend(sink.records);
        }
        assert_eq!(partitioned, combined, "{lane_width:?}");
        assert_eq!(session.total_committed_shots(), ShotCount::new(600));
        streams.push(combined);
    }
    // The seeded stream is a function of the lane width as well as the seed.
    assert_eq!(streams.len(), LaneWidth::ALL.len());
    assert_ne!(streams.first(), streams.last());
}

#[test]
fn cancellation_stops_between_bounded_batches_and_finalizes_sink() {
    let circuit = circuit("X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n");
    let plan = DetectionSamplingCompiler::default()
        .compile(&circuit)
        .expect("compile detection plan");
    for &lane_width in LaneWidth::ALL {
        let batch = lane_width.shots_per_batch();
        let batch_shots = ShotCount::new(batch as u64);
        let mut session = plan
            .session(&DetectionSamplingSessionOptions {
                random: RandomPolicy::Seeded(Seed::new(7)),
                lane_width,
                ..Default::default()
            })
            .expect("create cancellable session");
        let cancellation = session.cancellation();
        let mut sink = CancellingSink {
            inner: CollectSink::default(),
            cancellation: cancellation.clone(),
        };
        let summary = session
            .run(ShotCount::new(600), &mut sink)
            .expect("cancel detection run");
        assert_eq!(summary.status(), RunStatus::Cancelled);
        assert_eq!(summary.requested_shots(), ShotCount::new(600));
        assert_eq!(summary.committed_shots(), batch_shots);
        assert_eq!(summary.total_committed_shots(), batch_shots);
        assert_eq!(sink.inner.batch_sizes, vec![batch]);
        assert_eq!(sink.inner.finish_count, 1);
        assert!(!session.is_poisoned());
        cancellation.reset();
        let mut resumed = CollectSink::default();
        session
            .run(ShotCount::new(1), &mut resumed)
            .expect("resume cancelled session");
        assert_eq!(resumed.records.len(), 1);
        assert_eq!(
            session.total_committed_shots(),
            ShotCount::new(batch as u64 + 1)
        );
    }
}

#[test]
fn sink_failures_preserve_first_error_progress_and_poison_sessions() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = DetectionSamplingCompiler::default()
        .compile(&circuit)
        .expect("compile detection sampling");
    let mut session = plan
        .session(&DetectionSamplingSessionOptions {
            random: RandomPolicy::Seeded(Seed::new(5)),
            ..Default::default()
        })
        .expect("create detection session");
    let mut sink = FailingSink {
        fail_write: true,
        fail_finish: false,
        writes: 0,
    };
    let error = session
        .run(ShotCount::new(257), &mut sink)
        .expect_err("surface write failure");
    assert_eq!(error.progress().committed_shots(), ShotCount::new(0));
    assert_eq!(
        error.progress().attempted_batch_shots(),
        ShotCount::new(256)
    );
    match error {
        RunError::Sink {
            phase,
            source,
            progress,
        } => {
            assert_eq!(phase, SinkFailurePhase::WriteBatch);
            assert_eq!(source, TestSinkError::Write);
            assert_eq!(progress.committed_shots(), ShotCount::new(0));
            assert_eq!(progress.attempted_batch_shots(), ShotCount::new(256));
        }
        RunError::Engine { .. } => panic!("expected sink error"),
    }
    assert!(session.is_poisoned());
    let mut untouched = FailingSink {
        fail_write: false,
        fail_finish: false,
        writes: 0,
    };
    assert_matches!(
        session.run(ShotCount::new(1), &mut untouched),
        Err(RunError::Engine {
            source: DetectionExecutionError::SessionPoisoned,
            ..
        })
    );
    assert!(
        DetectionExecutionError::SessionPoisoned
            .to_string()
            .contains("session is poisoned")
    );
    assert_eq!(untouched.writes, 0);

    let mut conversion = MeasurementToDetectionCompiler::default()
        .compile(&circuit)
        .expect("compile conversion")
        .session(&Default::default())
        .expect("create conversion session");
    let input = packed(&[vec![false]], 1);
    let mut finish_failure = FailingSink {
        fail_write: false,
        fail_finish: true,
        writes: 0,
    };
    let error = conversion
        .run(
            MeasurementBatchView::new(input.view()),
            None,
            &mut finish_failure,
        )
        .expect_err("surface finish failure");
    match error {
        RunError::Sink {
            phase,
            source,
            progress,
        } => {
            assert_eq!(phase, SinkFailurePhase::Finish);
            assert_eq!(source, TestSinkError::Finish);
            assert_eq!(progress.committed_shots(), ShotCount::new(1));
            assert_eq!(progress.attempted_batch_shots(), ShotCount::new(0));
        }
        RunError::Engine { .. } => panic!("expected finish error"),
    }
    assert!(conversion.is_poisoned());
}

#[test]
fn zero_shots_do_not_touch_sink_or_advance_seeded_stream() {
    let circuit = circuit("X_ERROR(0.5) 0\nM 0\nDETECTOR rec[-1]\n");
    let plan = DetectionSamplingCompiler::default()
        .compile(&circuit)
        .expect("compile detection sampling");
    let mut with_zero = plan
        .session(&DetectionSamplingSessionOptions {
            random: RandomPolicy::Seeded(Seed::new(123)),
            ..Default::default()
        })
        .expect("create session");
    let mut untouched = CollectSink::default();
    let zero = with_zero
        .run(ShotCount::new(0), &mut untouched)
        .expect("run zero shots");
    assert_eq!(zero.committed_shots(), ShotCount::new(0));
    assert!(untouched.records.is_empty());
    assert!(untouched.batch_sizes.is_empty());
    assert_eq!(untouched.finish_count, 0);

    let mut after_zero = CollectSink::default();
    with_zero
        .run(ShotCount::new(16), &mut after_zero)
        .expect("run after zero");
    assert_eq!(after_zero.records, run_plan(&plan, 16, 123).records);

    let conversion_plan = MeasurementToDetectionCompiler::default()
        .compile(&circuit)
        .expect("compile zero-shot conversion");
    let mut conversion = conversion_plan
        .session(&Default::default())
        .expect("create zero-shot conversion session");
    let empty = PackedShotBatch::zeros(0, conversion_plan.measurement_width().get())
        .expect("create empty measurement batch");
    let mut untouched_conversion_sink = CollectSink::default();
    conversion
        .run(
            MeasurementBatchView::new(empty.view()),
            None,
            &mut untouched_conversion_sink,
        )
        .expect("convert zero records");
    assert!(untouched_conversion_sink.batch_sizes.is_empty());
    assert_eq!(untouched_conversion_sink.finish_count, 0);
}

#[test]
fn direct_frame_rejects_anti_hermitian_spp_during_compilation() {
    let circuit = circuit("SPP X0*Z0\nM 0\nDETECTOR rec[-1]\n");
    let error = DetectionSamplingCompiler::default()
        .compile(&circuit)
        .expect_err("reject anti-Hermitian SPP before a session exists");

    assert_matches!(error, DetectionCompileError::InvalidCircuit(_));
    assert!(error.to_string().contains("anti-Hermitian"));
}

/// Detectors and observables wider than the short-parity charge, and wider than one charged
/// chunk of terms, convert to the parity of all their terms.
#[test]
fn wide_detectors_and_observables_convert_to_the_parity_of_every_term() {
    let repeat = |term: &str, count: usize| vec![term; count].join(" ");
    let text = format!(
        "M 0 1 2\nDETECTOR {} {} rec[-3]\nDETECTOR {} rec[-2]\nOBSERVABLE_INCLUDE(0) {} rec[-3]\n",
        repeat("rec[-1]", 4099),
        repeat("rec[-2]", 100),
        repeat("rec[-1]", 64),
        repeat("rec[-2]", 5001),
    );
    let plan = MeasurementToDetectionCompiler::default()
        .compile(&circuit(&text))
        .expect("compile wide conversion");
    let mut session = plan
        .session(&Default::default())
        .expect("create conversion session");
    let records: Vec<Vec<bool>> = (0..8_u8)
        .map(|bits| (0..3).map(|bit| bits >> bit & 1 == 1).collect())
        .collect();
    let input = packed(&records, 3);
    let mut sink = CollectSink::default();
    let summary = session
        .run(MeasurementBatchView::new(input.view()), None, &mut sink)
        .expect("convert wide parities");
    assert_eq!(summary.status(), RunStatus::Completed);
    for (record, converted) in records.iter().zip(&sink.records) {
        let [m0, m1, m2] = <[bool; 3]>::try_from(record.as_slice()).expect("three measurements");
        assert_eq!(converted.detectors, vec![m2 ^ m0, m1]);
        assert_eq!(converted.observables, vec![m1 ^ m0]);
    }
    assert_eq!(sink.records.len(), records.len());
}
