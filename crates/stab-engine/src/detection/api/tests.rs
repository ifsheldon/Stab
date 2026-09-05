#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "focused detection lifecycle tests use compact fixture assertions"
)]

use std::convert::Infallible;

use stab_records::{BitPlane64Batch, MeasurementSink};

use super::super::test_support::{
    convert_measurements_to_detection_events, convert_measurements_to_detection_events_with_sweep,
    sample_detection_events,
};
use super::*;
use crate::detection::DetectionRecordBuffer;
use crate::{DetectionResourceKind, Seed};

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
                        .get(shot_index, bit_index)
                        .expect("detector bit")
                })
                .collect();
            let observables = (0..batch.observable_width().get())
                .map(|bit_index| {
                    batch
                        .observables()
                        .get(shot_index, bit_index)
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
    cancellation: SamplingCancellation,
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
        std::hint::black_box(batch.detectors().get(0, 0));
        std::hint::black_box(batch.observables().get(0, 0));
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
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(seed)))
        .expect("create detection session");
    let mut sink = CollectSink::default();
    let summary = session
        .run(ShotCount::new(shots), &mut sink)
        .expect("run detection session");
    assert_eq!(summary.status(), DetectionRunStatus::Completed);
    assert_eq!(summary.requested_shots(), ShotCount::new(shots));
    assert_eq!(summary.committed_shots(), ShotCount::new(shots));
    assert_eq!(summary.total_committed_shots(), ShotCount::new(shots));
    sink
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn plans_are_shareable_and_streamed_conversion_matches_materialized_output() {
    assert_send_sync::<MeasurementToDetectionPlan>();
    assert_send_sync::<DetectionSamplingPlan>();

    let circuit =
        circuit("X 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\n");
    let measurements = vec![
        vec![false, false],
        vec![false, true],
        vec![true, false],
        vec![true, true],
    ];
    let expected = convert_measurements_to_detection_events(
        &circuit,
        &measurements,
        ReferenceSampleMode::UseReferenceSample,
    )
    .expect("materialize detection conversion");
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile conversion plan");
    let cloned = plan.clone();
    assert_eq!(cloned.measurement_width().get(), 2);
    assert_eq!(cloned.sweep_width().get(), 0);
    assert_eq!(cloned.detector_width().get(), expected.detector_count);
    assert_eq!(cloned.observable_width().get(), expected.observable_count);

    let input = packed(&measurements, 2);
    let mut session = plan.session().expect("create conversion session");
    let mut sink = CollectSink::default();
    let summary = session
        .run(MeasurementBatchView::new(input.view()), None, &mut sink)
        .expect("stream conversion");

    assert_eq!(summary.committed_shots(), ShotCount::new(4));
    assert_eq!(sink.records, expected.records);
    assert_eq!(sink.batch_sizes, vec![4]);
    assert_eq!(sink.finish_count, 1);
}

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
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile transaction plan");
    assert_eq!(plan.measurement_width().get(), 1);
    assert_eq!(plan.sweep_width().get(), 1);
    let measurement_batch = packed(&measurements, 1);
    let sweep_batch = packed(&sweeps, 1);
    let mut session = plan.session().expect("create transaction session");
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
        assert_eq!(summary.status(), DetectionRunStatus::Completed);
        transaction.finish().expect("finish conversion sink");
    }
    assert_eq!(sink.records, expected.records);
    assert_eq!(sink.finish_count, 1);
}

#[test]
fn conversion_cancellation_rejects_a_whole_batch_and_is_resumable() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile cancellable conversion");
    let input = packed(&[vec![true]], 1);
    let mut session = plan.session().expect("create cancellable conversion");
    let cancellation = session.cancellation();
    cancellation.cancel();
    let mut untouched = CollectSink::default();
    let mut delivery = session
        .start_transaction(&mut untouched)
        .expect("start cancellable delivery");
    let summary = delivery
        .write_batch_with_sweep(MeasurementBatchView::new(input.view()), None)
        .expect("cancel conversion batch");
    assert_eq!(summary.status(), DetectionRunStatus::Cancelled);
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
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile composed conversion");
    let input = packed(&[vec![true]], 1);
    let mut session = plan.session().expect("create composed conversion");
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

    assert!(matches!(
        &error,
        DetectionRunError::Engine {
            source: DetectionExecutionError::CancelledComposition,
            ..
        }
    ));
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
        let plan = MeasurementToDetectionCompiler::new()
            .reference_sample_mode(mode)
            .compile(&circuit)
            .expect("compile sweep conversion");
        let mut session = plan.session().expect("create sweep conversion session");
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
fn packed_and_bit_plane_batches_match_scalar_conversion_at_batch_boundaries() {
    let circuit = circuit(
        "X 1\nCX sweep[0] 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2] rec[-1]\n",
    );
    for shot_count in [1, 17, 63, 64] {
        let measurements = (0..shot_count)
            .map(|shot| vec![shot % 3 == 0, shot % 5 < 2])
            .collect::<Vec<_>>();
        let sweeps = (0..shot_count)
            .map(|shot| vec![shot % 2 == 0])
            .collect::<Vec<_>>();
        let packed_measurements = packed(&measurements, 2);
        let packed_sweeps = packed(&sweeps, 1);
        let plane_measurements = BitPlane64Batch::from_shot_major(packed_measurements.view())
            .expect("transpose measurements");
        let plane_sweeps =
            BitPlane64Batch::from_shot_major(packed_sweeps.view()).expect("transpose sweeps");

        for mode in [
            ReferenceSampleMode::UseReferenceSample,
            ReferenceSampleMode::SkipReferenceSample,
        ] {
            let expected = convert_measurements_to_detection_events_with_sweep(
                &circuit,
                &measurements,
                &sweeps,
                mode,
            )
            .expect("materialize scalar conversion");
            let plan = MeasurementToDetectionCompiler::new()
                .reference_sample_mode(mode)
                .compile(&circuit)
                .expect("compile batch conversion");

            let mut packed_session = plan.session().expect("create packed session");
            let mut packed_sink = CollectSink::default();
            packed_session
                .run(
                    MeasurementBatchView::new(packed_measurements.view()),
                    Some(MeasurementBatchView::new(packed_sweeps.view())),
                    &mut packed_sink,
                )
                .expect("convert packed batch");
            assert_eq!(packed_sink.records, expected.records);

            let mut plane_session = plan.session().expect("create bit-plane session");
            let mut plane_sink = CollectSink::default();
            plane_session
                .run(
                    MeasurementBatchView::from_bit_planes(plane_measurements.view()),
                    Some(MeasurementBatchView::from_bit_planes(plane_sweeps.view())),
                    &mut plane_sink,
                )
                .expect("convert bit-plane batch");
            assert_eq!(plane_sink.records, expected.records);
        }
    }
}

#[test]
fn record_at_a_time_conversion_preserves_valid_prefix_and_preflight_reuse() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile conversion");
    let mut session = plan.session().expect("create conversion session");
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
    assert!(matches!(
        error,
        DetectionRunError::Engine {
            source: DetectionExecutionError::Conversion(_),
            ..
        }
    ));
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
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile delivery lifecycle");
    let input = packed(&[vec![true]], 1);
    let mut session = plan.session().expect("create delivery lifecycle session");
    let mut sink = CollectSink::default();
    let mut delivery = session
        .start_transaction(&mut sink)
        .expect("start delivery lifecycle");
    delivery
        .write_batch_with_sweep(MeasurementBatchView::new(input.view()), None)
        .expect("write delivery prefix");
    MeasurementSink::finish(&mut delivery).expect("finish delivery once");

    let repeated = MeasurementSink::finish(&mut delivery).expect_err("reject double finish");
    assert!(matches!(
        repeated,
        DetectionRunError::Engine {
            source: DetectionExecutionError::DeliveryFinished,
            ..
        }
    ));
    assert_eq!(repeated.progress().committed_shots(), ShotCount::new(1));
    let post_finish = delivery
        .write_batch_with_sweep(MeasurementBatchView::new(input.view()), None)
        .expect_err("reject write after finish");
    assert!(matches!(
        post_finish,
        DetectionRunError::Engine {
            source: DetectionExecutionError::DeliveryFinished,
            ..
        }
    ));
    assert_eq!(post_finish.progress().committed_shots(), ShotCount::new(1));
    drop(delivery);

    assert_eq!(sink.finish_count, 1);
    assert!(!session.is_poisoned());
    assert_eq!(session.total_committed_shots(), ShotCount::new(1));
}

#[test]
fn incremental_finish_failure_reports_committed_prefix_and_poisons_session() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile finish failure lifecycle");
    let input = packed(&[vec![true]], 1);
    let mut session = plan
        .session()
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
    assert!(matches!(
        error,
        DetectionRunError::Sink {
            phase: SinkFailurePhase::Finish,
            source: TestSinkError::Finish,
            ..
        }
    ));
    assert_eq!(error.progress().committed_shots(), ShotCount::new(2));
    assert!(session.is_poisoned());
    assert_eq!(session.total_committed_shots(), ShotCount::new(2));
}

#[test]
fn dropping_a_committed_incremental_delivery_poisons_the_parent_session() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile abandoned delivery");
    let input = packed(&[vec![true]], 1);
    let mut session = plan.session().expect("create abandoned delivery session");
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
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile forgotten transaction");
    let input = packed(&[vec![true]], 1);
    let mut session = plan
        .session()
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
fn direct_detection_sampling_matches_materialized_conversion() {
    for (circuit_text, measurements, detectors, observables) in [
        ("X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n", 1, 1, 0),
        ("RX 0\nZ_ERROR(0.25) 0\nOBSERVABLE_INCLUDE(0) X0\n", 0, 0, 1),
    ] {
        let circuit = circuit(circuit_text);
        let expected =
            sample_detection_events(&circuit, 129, Some(31)).expect("legacy detection sample");
        let plan = DetectionSamplingCompiler::new()
            .compile(&circuit)
            .expect("compile detection sampling");
        assert_eq!(plan.measurement_width().get(), measurements);
        assert_eq!(plan.detector_width().get(), detectors);
        assert_eq!(plan.observable_width().get(), observables);
        let actual = run_plan(&plan, 129, 31);
        assert_eq!(actual.records, expected.records);
        assert_eq!(actual.batch_sizes, vec![64, 64, 1]);
        assert_eq!(actual.finish_count, 1);
    }
}

#[test]
fn seeded_same_session_partitioning_is_exact() {
    let circuit = circuit("X_ERROR(0.375) 0\nM 0\nDETECTOR rec[-1]\n");
    let plan = DetectionSamplingCompiler::new()
        .compile(&circuit)
        .expect("compile detection plan");
    let combined = run_plan(&plan, 130, 44).records;
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(44)))
        .expect("create partitioned session");
    let mut first = CollectSink::default();
    let mut second = CollectSink::default();
    session
        .run(ShotCount::new(17), &mut first)
        .expect("run first partition");
    session
        .run(ShotCount::new(113), &mut second)
        .expect("run second partition");
    first.records.extend(second.records);
    assert_eq!(first.records, combined);
    assert_eq!(session.total_committed_shots(), ShotCount::new(130));
}

#[test]
fn cancellation_stops_between_bounded_batches_and_finalizes_sink() {
    let circuit = circuit("X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n");
    let plan = DetectionSamplingCompiler::new()
        .compile(&circuit)
        .expect("compile detection plan");
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(7)))
        .expect("create cancellable session");
    let cancellation = session.cancellation();
    let mut sink = CancellingSink {
        inner: CollectSink::default(),
        cancellation: cancellation.clone(),
    };
    let summary = session
        .run(ShotCount::new(130), &mut sink)
        .expect("cancel detection run");
    assert_eq!(summary.status(), DetectionRunStatus::Cancelled);
    assert_eq!(summary.requested_shots(), ShotCount::new(130));
    assert_eq!(summary.committed_shots(), ShotCount::new(64));
    assert_eq!(summary.total_committed_shots(), ShotCount::new(64));
    assert_eq!(sink.inner.batch_sizes, vec![64]);
    assert_eq!(sink.inner.finish_count, 1);
    assert!(!session.is_poisoned());
    cancellation.reset();
    let mut resumed = CollectSink::default();
    session
        .run(ShotCount::new(1), &mut resumed)
        .expect("resume cancelled session");
    assert_eq!(resumed.records.len(), 1);
    assert_eq!(session.total_committed_shots(), ShotCount::new(65));
}

#[test]
fn sink_failures_preserve_first_error_progress_and_poison_sessions() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = DetectionSamplingCompiler::new()
        .compile(&circuit)
        .expect("compile detection sampling");
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .expect("create detection session");
    let mut sink = FailingSink {
        fail_write: true,
        fail_finish: false,
        writes: 0,
    };
    let error = session
        .run(ShotCount::new(65), &mut sink)
        .expect_err("surface write failure");
    assert_eq!(error.progress().committed_shots(), ShotCount::new(0));
    assert_eq!(error.progress().attempted_batch_shots(), ShotCount::new(64));
    match error {
        DetectionRunError::Sink {
            phase,
            source,
            progress,
        } => {
            assert_eq!(phase, SinkFailurePhase::WriteBatch);
            assert_eq!(source, TestSinkError::Write);
            assert_eq!(progress.committed_shots(), ShotCount::new(0));
            assert_eq!(progress.attempted_batch_shots(), ShotCount::new(64));
        }
        DetectionRunError::Engine { .. } => panic!("expected sink error"),
    }
    assert!(session.is_poisoned());
    let mut untouched = FailingSink {
        fail_write: false,
        fail_finish: false,
        writes: 0,
    };
    assert!(matches!(
        session.run(ShotCount::new(1), &mut untouched),
        Err(DetectionRunError::Engine {
            source: DetectionExecutionError::SessionPoisoned,
            ..
        })
    ));
    assert!(
        DetectionExecutionError::SessionPoisoned
            .to_string()
            .contains("session is poisoned")
    );
    assert_eq!(untouched.writes, 0);

    let mut conversion = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile conversion")
        .session()
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
        DetectionRunError::Sink {
            phase,
            source,
            progress,
        } => {
            assert_eq!(phase, SinkFailurePhase::Finish);
            assert_eq!(source, TestSinkError::Finish);
            assert_eq!(progress.committed_shots(), ShotCount::new(1));
            assert_eq!(progress.attempted_batch_shots(), ShotCount::new(0));
        }
        DetectionRunError::Engine { .. } => panic!("expected finish error"),
    }
    assert!(conversion.is_poisoned());
}

#[test]
fn zero_shots_do_not_touch_sink_or_advance_seeded_stream() {
    let circuit = circuit("X_ERROR(0.5) 0\nM 0\nDETECTOR rec[-1]\n");
    let plan = DetectionSamplingCompiler::new()
        .compile(&circuit)
        .expect("compile detection sampling");
    let mut with_zero = plan
        .session(RandomPolicy::Seeded(Seed::new(123)))
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

    let conversion_plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile zero-shot conversion");
    let mut conversion = conversion_plan
        .session()
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
fn compilation_rejects_limits_before_session_or_sink_work() {
    let circuit = circuit("M 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n");
    let limits = DetectionConversionLimits::default().with_max_record_bits(1);
    let conversion_error = MeasurementToDetectionCompiler::new()
        .limits(limits)
        .compile(&circuit)
        .expect_err("reject conversion plan beyond its record limit");
    assert!(matches!(
        &conversion_error,
        DetectionCompileError::InvalidCircuit(_)
    ));
    assert!(conversion_error.to_string().contains("record width"));
    let sampling_error = DetectionSamplingCompiler::new()
        .limits(limits)
        .compile(&circuit)
        .expect_err("reject detection sampling plan beyond its record limit");
    assert!(matches!(
        &sampling_error,
        DetectionCompileError::InvalidCircuit(_)
    ));
    assert!(sampling_error.to_string().contains("record width"));
}

#[test]
fn direct_frame_compilation_charges_executable_targets_before_materialization() {
    let targets = (0..256)
        .map(|qubit| format!("X{qubit}"))
        .collect::<Vec<_>>()
        .join(" ");
    let metadata = "tag".repeat(1_024);
    let tagged = circuit(&format!("OBSERVABLE_INCLUDE[{metadata}](0) {targets}\n"));
    let untagged = circuit(&format!("OBSERVABLE_INCLUDE(0) {targets}\n"));
    let compiler = DetectionSamplingCompiler::new();
    let tagged_plan = compiler
        .compile(&tagged)
        .expect("compile tagged direct frame");
    let untagged_plan = compiler
        .compile(&untagged)
        .expect("compile untagged direct frame");
    let exact_bytes = direct_compiled_bytes(&tagged_plan);
    assert_eq!(
        exact_bytes,
        direct_compiled_bytes(&untagged_plan),
        "nonsemantic tags must not be retained by the private executable"
    );

    DetectionSamplingCompiler::new()
        .limits(DetectionConversionLimits::default().with_max_compiled_bytes(exact_bytes))
        .compile(&tagged)
        .expect("accept exact combined direct-plan byte boundary");
    let error = DetectionSamplingCompiler::new()
        .limits(DetectionConversionLimits::default().with_max_compiled_bytes(exact_bytes - 1))
        .compile(&tagged)
        .expect_err("reject the first byte above the direct-plan boundary");
    let DetectionCompileError::InvalidCircuit(DetectionError::ResourceLimit(resource)) = error
    else {
        panic!("direct-plan byte rejection must remain typed");
    };
    assert_eq!(resource.kind(), DetectionResourceKind::CompiledBytes);
    assert_eq!(resource.actual(), exact_bytes);
    assert_eq!(resource.limit(), exact_bytes - 1);
}

#[test]
fn direct_frame_compilation_admits_repeats_and_filtered_targets_before_materialization() {
    let repeated =
        circuit("REPEAT 4096 {\n M 0\n DETECTOR rec[-1]\n OBSERVABLE_INCLUDE(0) X0 rec[-1]\n}\n");
    let filtered_targets = (0..512)
        .flat_map(|index| {
            let qubit = index * 3;
            [
                format!("{qubit} sweep[{index}]"),
                format!("{} {}", qubit + 1, qubit + 2),
            ]
        })
        .collect::<Vec<_>>()
        .join(" ");
    let filtered = circuit(&format!(
        "XCZ {filtered_targets}\nOBSERVABLE_INCLUDE(0) X0\n"
    ));

    let baseline = circuit("OBSERVABLE_INCLUDE(0) X0\n");
    let baseline_exact = direct_compiled_bytes(
        &DetectionSamplingCompiler::new()
            .compile(&baseline)
            .expect("compile rejection-overhead baseline"),
    );
    let baseline_allocations = rejected_direct_frame_allocations(&baseline, baseline_exact);

    for circuit in [&repeated, &filtered] {
        let exact_plan = DetectionSamplingCompiler::new()
            .compile(circuit)
            .expect("compile exact-byte probe");
        let exact_bytes = direct_compiled_bytes(&exact_plan);
        DetectionSamplingCompiler::new()
            .limits(DetectionConversionLimits::default().with_max_compiled_bytes(exact_bytes))
            .compile(circuit)
            .expect("accept exact combined direct-plan byte boundary");

        let measured = rejected_direct_frame_allocations(circuit, exact_bytes);
        assert_eq!(
            measured.count_max, baseline_allocations.count_max,
            "aggregate byte rejection retained plan allocations concurrently"
        );
        assert_eq!(
            measured.bytes_max, baseline_allocations.bytes_max,
            "aggregate byte rejection allocated a retained plan buffer"
        );
    }
}

fn rejected_direct_frame_allocations(
    circuit: &Circuit,
    exact_bytes: u64,
) -> allocation_counter::AllocationInfo {
    let mut rejected = false;
    let measured = allocation_counter::measure(|| {
        rejected = DetectionSamplingCompiler::new()
            .limits(DetectionConversionLimits::default().with_max_compiled_bytes(exact_bytes - 1))
            .compile(circuit)
            .is_err();
    });
    assert!(rejected, "reject first byte beyond retained-plan limit");
    measured
}

fn direct_compiled_bytes(plan: &DetectionSamplingPlan) -> u64 {
    plan.inner
        .direct
        .compiled_bytes()
        .expect("compute retained direct-plan bytes")
}

#[test]
fn warmed_conversion_reuses_width_and_batch_bounded_storage() {
    let circuit = circuit(
        "H 0\nCX sweep[0] 0\nSPP X0*Z1\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile sweep conversion");
    let mut session = plan.session().expect("create conversion session");
    let measurements = packed(&vec![vec![false]; 64], 1);
    let sweeps = packed(&vec![vec![true]; 64], 1);
    let measurement_view = MeasurementBatchView::new(measurements.view());
    let sweep_view = MeasurementBatchView::new(sweeps.view());
    let mut sink = NullSink::default();

    let mut delivery = session
        .start_transaction(&mut sink)
        .expect("start allocation delivery");
    delivery
        .write_batch_with_sweep(measurement_view, Some(sweep_view))
        .expect("warm conversion scratch");
    let measured = allocation_counter::measure(|| {
        for _ in 0..128 {
            delivery
                .write_batch_with_sweep(measurement_view, Some(sweep_view))
                .expect("reuse conversion scratch");
        }
    });
    delivery.finish().expect("finish allocation delivery");
    assert_eq!(
        measured.count_total, 0,
        "warmed conversion allocated while reusing bounded scratch: {measured:?}"
    );
    assert_eq!(sink.shots, 64 * 129);
}

#[test]
fn session_storage_estimates_match_retained_allocations_at_word_boundaries() {
    for (detectors, observables) in [(0, 0), (1, 1), (63, 64), (64, 63), (65, 65)] {
        let mut text = String::from("M 0\n");
        for _ in 0..detectors {
            text.push_str("DETECTOR rec[-1]\n");
        }
        if observables > 0 {
            text.push_str(&format!(
                "OBSERVABLE_INCLUDE({}) rec[-1]\n",
                observables - 1
            ));
        }
        let direct = DetectionSamplingCompiler::new()
            .compile(&circuit(&text))
            .expect("compile direct allocation fixture");
        let mut session = None;
        let allocated = allocation_counter::measure(|| {
            session = Some(
                direct
                    .session(RandomPolicy::Seeded(Seed::new(1)))
                    .expect("allocate direct session"),
            );
        });
        assert_eq!(
            execution::direct_session_storage_bytes(&direct.inner.direct),
            u128::try_from(allocated.bytes_current).expect("retained direct allocation"),
            "direct widths {detectors}/{observables}"
        );

        for sweep in [false, true] {
            let text = if sweep {
                format!("CX sweep[0] 0\n{text}")
            } else {
                text.clone()
            };
            let conversion = MeasurementToDetectionCompiler::new()
                .compile(&circuit(&text))
                .expect("compile conversion allocation fixture");
            let mut session = None;
            let allocated = allocation_counter::measure(|| {
                session = Some(conversion.session().expect("allocate conversion session"));
            });
            assert_eq!(
                execution::conversion_session_storage_bytes(&conversion),
                u128::try_from(allocated.bytes_current).expect("retained conversion allocation"),
                "conversion widths {detectors}/{observables}, sweep={sweep}"
            );
        }
    }
}

#[test]
fn direct_session_storage_rejects_first_excess_before_allocation() {
    // One measurement and detector add 16 plane bytes and two 512-byte packed batches.
    // This sparse qubit identifier puts the admitted backing storage exactly at 256 MiB.
    let at_limit = DetectionSamplingCompiler::new()
        .compile(&circuit("M 16777150\nDETECTOR rec[-1]\n"))
        .expect("compile exact storage boundary");
    execution::validate_direct_session_storage(&at_limit.inner.direct)
        .expect("admit the exact boundary without allocating its state");
    assert_eq!(
        execution::direct_session_storage_bytes(&at_limit.inner.direct),
        u128::from(MAX_DETECTION_SESSION_STORAGE_BYTES)
    );

    let over_limit = DetectionSamplingCompiler::new()
        .compile(&circuit("M 16777151\nDETECTOR rec[-1]\n"))
        .expect("compile the first excess qubit");
    // Check admission first so a regression cannot allocate the large frame in this test.
    execution::validate_direct_session_storage(&over_limit.inner.direct)
        .expect_err("reject the first excess qubit before attempting construction");
    let mut rejected = None;
    let allocated = allocation_counter::measure(|| {
        rejected = Some(
            over_limit
                .session(RandomPolicy::Seeded(Seed::new(1)))
                .expect_err("public construction must use the same admission"),
        );
    });
    assert_eq!(allocated.bytes_total, 0);
    assert_eq!(
        rejected,
        Some(DetectionExecutionError::SessionStorageLimit {
            estimated_bytes: u128::from(MAX_DETECTION_SESSION_STORAGE_BYTES) + 16,
            limit_bytes: MAX_DETECTION_SESSION_STORAGE_BYTES,
        })
    );
}

#[test]
fn warmed_detection_session_reuses_batch_storage() {
    let spp = (0..32)
        .map(|qubit| format!("X{qubit}"))
        .collect::<Vec<_>>()
        .join("*");
    let circuit = circuit(&format!(
        "SPP {spp}\nX_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n"
    ));
    let plan = DetectionSamplingCompiler::new()
        .compile(&circuit)
        .expect("compile detection plan");
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(91)))
        .expect("create reusable detection session");
    let mut sink = NullSink::default();
    session
        .run(ShotCount::new(64), &mut sink)
        .expect("warm detection session");
    let measured = allocation_counter::measure(|| {
        for _ in 0..128 {
            session
                .run(ShotCount::new(64), &mut sink)
                .expect("reuse detection session");
        }
    });
    assert_eq!(
        measured.count_total, 0,
        "warmed detection session allocated while reusing bounded state: {measured:?}"
    );
    assert_eq!(sink.shots, 64 * 129);
}

#[test]
fn direct_frame_rejects_anti_hermitian_spp_during_compilation() {
    let circuit = circuit("SPP X0*Z0\nM 0\nDETECTOR rec[-1]\n");
    let error = DetectionSamplingCompiler::new()
        .compile(&circuit)
        .expect_err("reject anti-Hermitian SPP before a session exists");

    assert!(matches!(error, DetectionCompileError::InvalidCircuit(_)));
    assert!(error.to_string().contains("anti-Hermitian"));
}
