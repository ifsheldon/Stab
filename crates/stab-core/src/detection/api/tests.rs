#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "focused detection lifecycle tests use compact fixture assertions"
)]

use std::convert::Infallible;

use super::*;
use crate::{
    Seed, convert_measurements_to_detection_events,
    convert_measurements_to_detection_events_with_sweep, sample_detection_events,
};

#[derive(Default)]
struct CollectSink {
    records: Vec<DetectionEventRecord>,
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
            self.records.push(DetectionEventRecord {
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
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
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
fn measurement_sink_adapter_preserves_sweep_semantics_and_sink_lifecycle() {
    let circuit =
        circuit("H 0\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(1) rec[-1]\n");
    let measurements = vec![vec![false], vec![false], vec![true], vec![true]];
    let sweeps = vec![vec![false], vec![true], vec![false], vec![true]];
    let expected = convert_measurements_to_detection_events_with_sweep(
        &circuit,
        &measurements,
        &sweeps,
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
    )
    .expect("materialize adapter oracle");
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile adapter plan");
    assert_eq!(plan.measurement_width().get(), 1);
    assert_eq!(plan.sweep_width().get(), 1);
    let measurement_batch = packed(&measurements, 1);
    let sweep_batch = packed(&sweeps, 1);
    let mut session = plan.session().expect("create adapter session");
    let mut sink = CollectSink::default();
    {
        let mut adapter = MeasurementToDetectionSinkAdapter::new(&mut session, &mut sink);
        let summary = adapter
            .write_batch_with_sweep(
                MeasurementBatchView::new(measurement_batch.view()),
                Some(MeasurementBatchView::new(sweep_batch.view())),
            )
            .expect("adapt measurement batch with sweep data");
        assert_eq!(summary.status(), DetectionRunStatus::Completed);
        adapter.finish().expect("finish adapted sink");
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
    let summary = session
        .write_batch(
            MeasurementBatchView::new(input.view()),
            None,
            &mut untouched,
        )
        .expect("cancel conversion batch");
    assert_eq!(summary.status(), DetectionRunStatus::Cancelled);
    assert_eq!(summary.committed_shots(), ShotCount::new(0));
    assert!(untouched.records.is_empty());
    assert!(!session.is_poisoned());

    cancellation.reset();
    session
        .write_batch(
            MeasurementBatchView::new(input.view()),
            None,
            &mut untouched,
        )
        .expect("resume conversion batch");
    session.finish(&mut untouched).expect("finish resumed sink");
    assert_eq!(untouched.records.len(), 1);
    assert_eq!(session.total_committed_shots(), ShotCount::new(1));
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
            DetectionConversionOptions {
                skip_reference_sample,
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
fn record_at_a_time_conversion_preserves_valid_prefix_and_preflight_reuse() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile conversion");
    let mut session = plan.session().expect("create conversion session");
    let valid = packed(&[vec![true]], 1);
    let invalid = packed(&[vec![true, false]], 2);
    let mut sink = CollectSink::default();

    session
        .write_batch(MeasurementBatchView::new(valid.view()), None, &mut sink)
        .expect("write valid prefix");
    let error = session
        .write_batch(MeasurementBatchView::new(invalid.view()), None, &mut sink)
        .expect_err("reject malformed later record");
    assert!(matches!(
        error,
        DetectionRunError::Engine {
            source: DetectionExecutionError::Conversion(_),
            ..
        }
    ));
    assert!(!session.is_poisoned());
    assert_eq!(sink.records.len(), 1);
    assert_eq!(session.total_committed_shots(), ShotCount::new(1));

    session
        .write_batch(MeasurementBatchView::new(valid.view()), None, &mut sink)
        .expect("reuse after preflight rejection");
    session.finish(&mut sink).expect("finish prefix sink");
    assert_eq!(sink.records.len(), 2);
    assert_eq!(sink.finish_count, 1);
}

#[test]
fn automatic_fused_and_direct_sampling_match_legacy_materialization() {
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
fn private_direct_and_fused_variants_agree_on_deterministic_circuit() {
    let circuit = circuit("R 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n");
    let compiler = DetectionSamplingCompiler::new();
    let direct = compiler
        .compile_direct_for_test(&circuit)
        .expect("compile direct variant");
    let fused = compiler
        .compile_fused_for_test(&circuit)
        .expect("compile fused variant");

    assert_eq!(
        run_plan(&direct, 65, 9).records,
        run_plan(&fused, 65, 9).records
    );
}

#[test]
fn seeded_same_session_partitioning_is_exact_for_both_private_variants() {
    let circuit = circuit("X_ERROR(0.375) 0\nM 0\nDETECTOR rec[-1]\n");
    let compiler = DetectionSamplingCompiler::new();
    for plan in [
        compiler
            .compile_direct_for_test(&circuit)
            .expect("compile direct variant"),
        compiler
            .compile_fused_for_test(&circuit)
            .expect("compile fused variant"),
    ] {
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
}

#[test]
fn cancellation_stops_between_bounded_batches_and_finalizes_sink() {
    let circuit = circuit("X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n");
    let compiler = DetectionSamplingCompiler::new();
    for plan in [
        compiler
            .compile_direct_for_test(&circuit)
            .expect("compile direct variant"),
        compiler
            .compile_fused_for_test(&circuit)
            .expect("compile fused variant"),
    ] {
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
            .into_circuit_error()
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
    assert!(
        conversion_error
            .into_circuit_error()
            .to_string()
            .contains("record width")
    );
    let sampling_error = DetectionSamplingCompiler::new()
        .limits(limits)
        .compile(&circuit)
        .expect_err("reject detection sampling plan beyond its record limit");
    assert!(matches!(
        &sampling_error,
        DetectionCompileError::InvalidCircuit(_)
    ));
    assert!(
        sampling_error
            .into_circuit_error()
            .to_string()
            .contains("record width")
    );
}

#[test]
fn warmed_conversion_reuses_width_and_batch_bounded_storage() {
    let circuit =
        circuit("H 0\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n");
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&circuit)
        .expect("compile sweep conversion");
    let mut session = plan.session().expect("create conversion session");
    let measurements = packed(&vec![vec![false]; 64], 1);
    let sweeps = packed(&vec![vec![true]; 64], 1);
    let measurement_view = MeasurementBatchView::new(measurements.view());
    let sweep_view = MeasurementBatchView::new(sweeps.view());
    let mut sink = NullSink::default();

    session
        .write_batch(measurement_view, Some(sweep_view), &mut sink)
        .expect("warm conversion scratch");
    let measured = allocation_counter::measure(|| {
        for _ in 0..128 {
            session
                .write_batch(measurement_view, Some(sweep_view), &mut sink)
                .expect("reuse conversion scratch");
        }
    });
    assert_eq!(
        measured.count_total, 0,
        "warmed conversion allocated while reusing bounded scratch: {measured:?}"
    );
    assert_eq!(sink.shots, 64 * 129);
}

#[test]
fn warmed_detection_variants_reuse_session_and_batch_storage() {
    let circuit = circuit("X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n");
    let compiler = DetectionSamplingCompiler::new();
    for plan in [
        compiler
            .compile_direct_for_test(&circuit)
            .expect("compile direct variant"),
        compiler
            .compile_fused_for_test(&circuit)
            .expect("compile fused variant"),
    ] {
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
}

#[test]
fn fused_detection_storage_admission_uses_the_combined_session_estimate() {
    let construction_started = Cell::new(false);
    let error = construct_fused_state_after_admission(
        u128::from(MAX_DETECTION_SESSION_STORAGE_BYTES) - 1,
        2,
        || {
            construction_started.set(true);
            Ok(())
        },
    )
    .expect_err("two individually admissible components must fail their combined envelope");
    assert_eq!(
        error,
        DetectionExecutionError::SessionStorageLimit {
            estimated_bytes: u128::from(MAX_DETECTION_SESSION_STORAGE_BYTES) + 1,
            limit_bytes: MAX_DETECTION_SESSION_STORAGE_BYTES,
        }
    );
    assert!(
        !construction_started.get(),
        "aggregate admission must reject before constructing either mutable component"
    );

    construct_fused_state_after_admission(
        u128::from(MAX_DETECTION_SESSION_STORAGE_BYTES) - 2,
        2,
        || {
            construction_started.set(true);
            Ok(())
        },
    )
    .expect("the exact combined storage maximum is admitted");
    assert!(
        construction_started.get(),
        "exact-limit admission must continue into component construction"
    );
}
