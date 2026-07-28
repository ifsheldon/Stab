#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "focused detection lifecycle tests use compact fixture assertions"
)]

use std::convert::Infallible;

use stab_records::MeasurementSink;

use super::*;
use crate::{
    ResourceKind, Seed, convert_measurements_to_detection_events,
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
        let mut adapter = session
            .start_delivery(&mut sink)
            .expect("start adapted delivery");
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
    let mut delivery = session
        .start_delivery(&mut untouched)
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

    let mut delivery = session
        .start_delivery(&mut sink)
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
        .start_delivery(&mut sink)
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
        .start_delivery(&mut sink)
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
            .start_delivery(&mut sink)
            .expect("start abandoned delivery");
        delivery
            .write_batch_with_sweep(MeasurementBatchView::new(input.view()), None)
            .expect("write abandoned prefix");
    }
    assert!(session.is_poisoned());
    assert_eq!(sink.finish_count, 0);
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
        .compile_direct_for_test(&tagged)
        .expect("compile tagged direct frame");
    let untagged_plan = compiler
        .compile_direct_for_test(&untagged)
        .expect("compile untagged direct frame");
    let exact_bytes = direct_compiled_bytes(&tagged_plan);
    assert_eq!(
        exact_bytes,
        direct_compiled_bytes(&untagged_plan),
        "nonsemantic tags must not be retained by the private executable"
    );

    DetectionSamplingCompiler::new()
        .limits(DetectionConversionLimits::default().with_max_compiled_bytes(exact_bytes))
        .compile_direct_for_test(&tagged)
        .expect("accept exact combined direct-plan byte boundary");
    let error = DetectionSamplingCompiler::new()
        .limits(DetectionConversionLimits::default().with_max_compiled_bytes(exact_bytes - 1))
        .compile_direct_for_test(&tagged)
        .expect_err("reject the first byte above the direct-plan boundary")
        .into_circuit_error();
    let resource = error
        .resource_limit_error()
        .expect("direct-plan byte rejection remains typed");
    assert_eq!(resource.resource(), ResourceKind::MaterializedBytes);
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
            .compile_direct_for_test(&baseline)
            .expect("compile rejection-overhead baseline"),
    );
    let baseline_allocations = rejected_direct_frame_allocations(&baseline, baseline_exact);

    for circuit in [&repeated, &filtered] {
        let exact_plan = DetectionSamplingCompiler::new()
            .compile_direct_for_test(circuit)
            .expect("compile exact-byte probe");
        let exact_bytes = direct_compiled_bytes(&exact_plan);
        DetectionSamplingCompiler::new()
            .limits(DetectionConversionLimits::default().with_max_compiled_bytes(exact_bytes))
            .compile_direct_for_test(circuit)
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
            .compile_direct_for_test(circuit)
            .is_err();
    });
    assert!(rejected, "reject first byte beyond retained-plan limit");
    measured
}

fn direct_compiled_bytes(plan: &DetectionSamplingPlan) -> u64 {
    let DetectionSamplingPlanKind::DirectDetectorFrame(direct) = &plan.inner.kind else {
        panic!("test compiler must select the direct detector frame");
    };
    direct
        .compiled_bytes()
        .expect("compute retained direct-plan bytes")
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

    let mut delivery = session
        .start_delivery(&mut sink)
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
