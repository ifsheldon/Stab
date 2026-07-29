#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "public sampling contract tests use compact fixture setup"
)]

use std::convert::Infallible;
use std::thread;

use sha2::{Digest as _, Sha256};
use stab_core::advanced::{
    backend::{BackendPreference, SamplingBackend},
    compat::CompiledSampler,
    records::MeasurementCodecSink,
};
use stab_core::{
    Circuit, CircuitError, MeasurementBatchView, MeasurementSink, RandomPolicy, RecordFormat,
    RunError, SamplingCompileError, SamplingCompileErrorCode, SamplingCompiler,
    SamplingExecutionError, SamplingRunStatus, Seed, ShotCount, SinkFailurePhase,
};

#[derive(Debug, Default)]
struct CollectSink {
    records: Vec<Vec<bool>>,
    write_calls: usize,
    finish_calls: usize,
}

impl MeasurementSink for CollectSink {
    type Error = Infallible;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        self.write_calls += 1;
        for shot_index in 0..batch.shot_count() {
            let mut record = Vec::with_capacity(batch.width().get());
            for bit_index in 0..batch.width().get() {
                record.push(
                    batch
                        .get(shot_index, bit_index)
                        .expect("batch dimensions were validated"),
                );
            }
            self.records.push(record);
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finish_calls += 1;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestSinkError {
    Write,
    Finish,
}

#[derive(Debug)]
struct FailingSink {
    fail_write_call: Option<usize>,
    fail_finish: bool,
    write_calls: usize,
    finish_calls: usize,
}

impl MeasurementSink for FailingSink {
    type Error = TestSinkError;

    fn write_batch(&mut self, _batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        self.write_calls += 1;
        if self.fail_write_call == Some(self.write_calls) {
            return Err(TestSinkError::Write);
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finish_calls += 1;
        if self.fail_finish {
            return Err(TestSinkError::Finish);
        }
        Ok(())
    }
}

fn noisy_plan() -> stab_core::SamplingPlan {
    let circuit =
        Circuit::from_stim_str("H 0\nM 0\nCX rec[-1] 1\nM(0.125) 1\n").expect("parse circuit");
    SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile sampling plan")
}

fn collect(
    plan: &stab_core::SamplingPlan,
    seed: u64,
    shots: u64,
) -> (Vec<Vec<bool>>, usize, usize) {
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(seed)))
        .expect("construct session");
    let mut sink = CollectSink::default();
    let summary = session
        .run(ShotCount::new(shots), &mut sink)
        .expect("run sampling session");
    assert_eq!(summary.status(), SamplingRunStatus::Completed);
    assert_eq!(summary.committed_shots(), ShotCount::new(shots));
    (sink.records, sink.write_calls, sink.finish_calls)
}

#[test]
fn compiler_selects_only_registered_backends_and_fingerprints_the_plan() {
    let circuit = Circuit::from_stim_str("M 0\n").expect("parse circuit");
    let first = SamplingCompiler::new()
        .backend(BackendPreference::Auto)
        .compile(&circuit)
        .expect("compile automatic backend");
    let second = SamplingCompiler::new()
        .backend(BackendPreference::Scalar)
        .compile(&circuit)
        .expect("compile scalar backend");
    let legacy = CompiledSampler::compile(&circuit).expect("compile migration adapter");

    assert_eq!(first.backend(), SamplingBackend::Scalar);
    assert_eq!(second.backend(), SamplingBackend::Scalar);
    assert_eq!(stab_engine::REGISTERED_BACKENDS, &[SamplingBackend::Scalar]);
    assert_eq!(BackendPreference::Auto.as_str(), "auto");
    assert_eq!(BackendPreference::Scalar.as_str(), "scalar");
    assert_eq!(BackendPreference::PortableSimd.as_str(), "portable-simd");
    assert_eq!(SamplingBackend::Scalar.as_str(), "scalar");
    assert_eq!(first.request_fingerprint(), second.request_fingerprint());
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(legacy.plan().fingerprint(), first.fingerprint());
    assert_eq!(
        first.fingerprint().request_fingerprint(),
        first.request_fingerprint()
    );
    assert_eq!(first.fingerprint().backend(), SamplingBackend::Scalar);

    assert!(matches!(
        SamplingCompiler::new()
            .backend(BackendPreference::PortableSimd)
            .compile(&circuit),
        Err(SamplingCompileError::BackendUnavailable {
            requested: BackendPreference::PortableSimd
        })
    ));
}

#[test]
fn compiled_sampler_equality_preserves_lowered_execution_compatibility() {
    let plain = Circuit::from_stim_str("M 0\n").expect("parse plain circuit");
    let tagged = Circuit::from_stim_str("M[tag] 0\n").expect("parse tagged circuit");

    let plain = CompiledSampler::compile(&plain).expect("compile plain circuit");
    let tagged = CompiledSampler::compile(&tagged).expect("compile tagged circuit");

    assert_eq!(plain, tagged);
    assert_ne!(
        plain.plan().request_fingerprint(),
        tagged.plan().request_fingerprint()
    );
}

#[test]
fn plan_fingerprint_schema_one_has_an_independently_reconstructed_vector() {
    let circuit = Circuit::from_stim_str("M 0\n").expect("parse frozen circuit");
    let plan = SamplingCompiler::new()
        .backend(BackendPreference::Scalar)
        .compile(&circuit)
        .expect("compile frozen plan");
    let fingerprint = plan.fingerprint();

    assert_eq!(plan.measurement_width().get(), 1);
    assert_eq!(plan.qubit_count(), 1);
    assert_eq!(
        fingerprint.schema_version(),
        stab_core::PlanFingerprint::SCHEMA_VERSION
    );
    assert_eq!(stab_core::PlanFingerprint::ALGORITHM, "sha256");
    assert_eq!(
        fingerprint.executable_contract_schema_version(),
        stab_core::SamplingPlan::EXECUTABLE_CONTRACT_SCHEMA_VERSION
    );

    let mut executable = Sha256::new();
    executable.update(b"stab:sampling-executable-contract\0");
    executable.update(1_u16.to_be_bytes());
    executable.update([1_u8, 1_u8]);
    let executable_digest: [u8; 32] = executable.finalize().into();

    let mut reconstructed = Sha256::new();
    reconstructed.update(b"stab:plan-fingerprint\0");
    reconstructed.update(1_u16.to_be_bytes());
    reconstructed.update(1_u16.to_be_bytes());
    reconstructed.update(plan.request_fingerprint().digest());
    reconstructed.update([1_u8]);
    reconstructed.update(1_u16.to_be_bytes());
    reconstructed.update(executable_digest);
    let reconstructed_digest: [u8; 32] = reconstructed.finalize().into();

    assert_eq!(
        plan.request_fingerprint().digest_hex(),
        "f8b6f8896556955fd436ad8e1f1700eb031cd04bc910accbf549195102384e79"
    );
    assert_eq!(fingerprint.executable_contract_digest(), executable_digest);
    assert_eq!(
        fingerprint.executable_contract_digest_hex(),
        "825e33849503cf5a731547f393d47bb8405cc4d103ae4501db080ff8523fb47a"
    );
    assert_eq!(fingerprint.digest(), reconstructed_digest);
    assert_eq!(
        fingerprint.digest_hex(),
        "6211d411207f181cf93ee7a6cac4a862d3167bc9e7c471a2484e5f16b08909d8"
    );
}

#[test]
fn compilation_failures_keep_invalid_circuits_distinct_from_missing_backends() {
    let sweep_circuit =
        Circuit::from_stim_str("CX sweep[0] 0\nM 0\n").expect("parse sweep circuit");
    let invalid = SamplingCompiler::new()
        .compile(&sweep_circuit)
        .expect_err("ordinary sampling rejects sweep-controlled execution");
    assert_eq!(invalid.code(), SamplingCompileErrorCode::InvalidCircuit);
    assert_eq!(invalid.code().as_str(), "invalid-circuit");
    let SamplingCompileError::InvalidCircuit { message } = invalid.clone() else {
        panic!("invalid circuit must retain its engine diagnostic");
    };
    assert_eq!(message, "M8 sampler subset does not support CX");
    assert_eq!(
        CircuitError::from(invalid),
        CircuitError::InvalidSamplerCompilation {
            message: "M8 sampler subset does not support CX".to_owned()
        }
    );

    let ordinary = Circuit::from_stim_str("M 0\n").expect("parse ordinary circuit");
    let unavailable = SamplingCompiler::new()
        .backend(BackendPreference::PortableSimd)
        .compile(&ordinary)
        .expect_err("portable SIMD is registered in A6");
    assert_eq!(
        unavailable.code(),
        SamplingCompileErrorCode::BackendUnavailable
    );
    assert_eq!(unavailable.code().as_str(), "backend-unavailable");
    assert!(
        CircuitError::from(unavailable)
            .to_string()
            .contains("portable-simd")
    );
}

#[test]
fn plans_share_across_threads_while_sessions_keep_rng_state_isolated() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<stab_core::SamplingPlan>();

    let plan = noisy_plan();
    let left_plan = plan.clone();
    let right_plan = plan.clone();
    let left = thread::spawn(move || collect(&left_plan, 17, 96).0);
    let right = thread::spawn(move || collect(&right_plan, 17, 96).0);

    assert_eq!(
        left.join().expect("left worker"),
        right.join().expect("right worker")
    );
}

#[test]
fn same_seeded_session_chunking_matches_one_combined_run() {
    let plan = noisy_plan();
    let expected = collect(&plan, 23, 131).0;

    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(23)))
        .expect("construct chunked session");
    let mut first = CollectSink::default();
    let mut second = CollectSink::default();
    let first_summary = session
        .run(ShotCount::new(31), &mut first)
        .expect("first chunk");
    let second_summary = session
        .run(ShotCount::new(100), &mut second)
        .expect("second chunk");
    first.records.extend(second.records);

    assert_eq!(first.records, expected);
    assert_eq!(first_summary.committed_shots(), ShotCount::new(31));
    assert_eq!(first_summary.requested_shots(), ShotCount::new(31));
    assert_eq!(second_summary.total_committed_shots(), ShotCount::new(131));
}

#[test]
fn general_frame_seeded_stream_matches_the_pre_plan_frozen_vector() {
    let circuit =
        Circuit::from_stim_str("SWAP 0 1\nH 0\nM 0 1\n").expect("parse general-frame circuit");
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile general-frame plan");
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(43)))
        .expect("construct session");
    let mut sink = MeasurementCodecSink::try_new(RecordFormat::ZeroOne, plan.measurement_width())
        .expect("construct codec sink");

    session
        .run(ShotCount::new(32), &mut sink)
        .expect("sample frozen stream");

    assert_eq!(
        sink.into_bytes().expect("finalized bytes"),
        b"00\n10\n10\n00\n10\n10\n00\n00\n\
          10\n10\n10\n00\n10\n10\n00\n10\n\
          00\n10\n10\n10\n10\n10\n00\n00\n\
          10\n00\n00\n00\n00\n00\n00\n10\n"
    );
}

#[test]
fn direct_and_small_frame_reference_modes_match_pre_plan_frozen_vectors() {
    // Captured from clean pre-A4 revision 18099bf3. The 65th shot crosses the internal
    // 64-shot batch boundary introduced by the plan/session implementation.
    for (source, use_reference, skip_reference) in [
        (
            "X 0\nX_ERROR(0.25) 0\nM(0.125) 0\n",
            b"0\n1\n1\n0\n1\n1\n1\n0\n0\n1\n1\n1\n0\n1\n0\n0\n\
              1\n1\n1\n0\n1\n0\n1\n1\n0\n0\n1\n0\n1\n1\n1\n1\n\
              0\n0\n1\n1\n0\n1\n1\n1\n1\n1\n1\n1\n1\n0\n1\n1\n\
              0\n1\n1\n1\n1\n0\n1\n1\n1\n1\n0\n1\n0\n1\n1\n1\n1\n"
                .as_slice(),
            b"1\n0\n0\n1\n0\n0\n0\n1\n1\n0\n0\n0\n1\n0\n1\n1\n\
              0\n0\n0\n1\n0\n1\n0\n0\n1\n1\n0\n1\n0\n0\n0\n0\n\
              1\n1\n0\n0\n1\n0\n0\n0\n0\n0\n0\n0\n0\n1\n0\n0\n\
              1\n0\n0\n0\n0\n1\n0\n0\n0\n0\n1\n0\n1\n0\n0\n0\n0\n"
                .as_slice(),
        ),
        (
            "H 0\nCX 0 1\nM !0 !1\n",
            b"11\n00\n00\n11\n00\n00\n11\n11\n00\n00\n00\n11\n00\n00\n11\n00\n\
              11\n00\n00\n00\n00\n00\n11\n11\n00\n11\n11\n11\n11\n11\n11\n00\n\
              11\n11\n11\n00\n11\n00\n11\n11\n11\n00\n11\n11\n00\n00\n11\n11\n\
              11\n11\n11\n00\n00\n00\n11\n11\n00\n11\n00\n11\n11\n00\n00\n11\n11\n"
                .as_slice(),
            b"00\n11\n11\n00\n11\n11\n00\n00\n11\n11\n11\n00\n11\n11\n00\n11\n\
              00\n11\n11\n11\n11\n11\n00\n00\n11\n00\n00\n00\n00\n00\n00\n11\n\
              00\n00\n00\n11\n00\n11\n00\n00\n00\n11\n00\n00\n11\n11\n00\n00\n\
              00\n00\n00\n11\n11\n11\n00\n00\n11\n00\n11\n00\n00\n11\n11\n00\n00\n"
                .as_slice(),
        ),
    ] {
        let circuit = Circuit::from_stim_str(source).expect("parse frozen fixture");
        let plan = SamplingCompiler::new()
            .compile(&circuit)
            .expect("compile frozen fixture");

        for (reference_mode, expected) in [
            (
                stab_core::ReferenceSampleMode::UseReferenceSample,
                use_reference,
            ),
            (
                stab_core::ReferenceSampleMode::SkipReferenceSample,
                skip_reference,
            ),
        ] {
            let mut session = plan
                .session_with_reference_mode(RandomPolicy::Seeded(Seed::new(43)), reference_mode)
                .expect("construct frozen session");
            let mut sink =
                MeasurementCodecSink::try_new(RecordFormat::ZeroOne, plan.measurement_width())
                    .expect("construct frozen sink");
            session
                .run(ShotCount::new(65), &mut sink)
                .expect("sample frozen stream");

            assert_eq!(
                sink.into_bytes().expect("finalized frozen bytes"),
                expected,
                "{source:?} {reference_mode:?}"
            );
        }
    }
}

#[test]
fn chunked_codec_output_composes_from_fresh_finalized_sinks() {
    let plan = noisy_plan();
    for format in [
        RecordFormat::ZeroOne,
        RecordFormat::B8,
        RecordFormat::R8,
        RecordFormat::Hits,
        RecordFormat::Dets,
    ] {
        let mut combined_session = plan
            .session(RandomPolicy::Seeded(Seed::new(24)))
            .expect("construct combined session");
        let mut combined_sink = MeasurementCodecSink::try_new(format, plan.measurement_width())
            .expect("construct combined codec sink");
        combined_session
            .run(ShotCount::new(131), &mut combined_sink)
            .expect("combined run");
        let expected = combined_sink
            .into_bytes()
            .expect("finalized combined bytes");

        let mut chunked_session = plan
            .session(RandomPolicy::Seeded(Seed::new(24)))
            .expect("construct chunked session");
        let mut first_sink = MeasurementCodecSink::try_new(format, plan.measurement_width())
            .expect("construct first codec sink");
        chunked_session
            .run(ShotCount::new(31), &mut first_sink)
            .expect("first codec run");
        let mut actual = first_sink.into_bytes().expect("first finalized bytes");

        let mut second_sink = MeasurementCodecSink::try_new(format, plan.measurement_width())
            .expect("construct second codec sink");
        chunked_session
            .run(ShotCount::new(100), &mut second_sink)
            .expect("second codec run");
        actual.extend(second_sink.into_bytes().expect("second finalized bytes"));

        assert_eq!(actual, expected, "format {}", format.as_str());
    }

    let mut combined_session = plan
        .session(RandomPolicy::Seeded(Seed::new(25)))
        .expect("construct combined ptb64 session");
    let mut combined_sink =
        MeasurementCodecSink::try_new(RecordFormat::Ptb64, plan.measurement_width())
            .expect("construct combined ptb64 sink");
    combined_session
        .run(ShotCount::new(128), &mut combined_sink)
        .expect("combined ptb64 run");
    let expected = combined_sink
        .into_bytes()
        .expect("finalized combined ptb64 bytes");

    let mut chunked_session = plan
        .session(RandomPolicy::Seeded(Seed::new(25)))
        .expect("construct chunked ptb64 session");
    let mut first_sink =
        MeasurementCodecSink::try_new(RecordFormat::Ptb64, plan.measurement_width())
            .expect("construct first ptb64 sink");
    chunked_session
        .run(ShotCount::new(64), &mut first_sink)
        .expect("first ptb64 run");
    let mut actual = first_sink.into_bytes().expect("first ptb64 bytes");
    let mut second_sink =
        MeasurementCodecSink::try_new(RecordFormat::Ptb64, plan.measurement_width())
            .expect("construct second ptb64 sink");
    chunked_session
        .run(ShotCount::new(64), &mut second_sink)
        .expect("second ptb64 run");
    actual.extend(second_sink.into_bytes().expect("second ptb64 bytes"));

    assert_eq!(actual, expected, "ptb64");
}

#[test]
fn zero_shots_do_not_touch_the_sink_or_advance_the_rng() {
    let plan = noisy_plan();
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(29)))
        .expect("construct session");
    let mut empty_sink = CollectSink::default();
    let summary = session
        .run(ShotCount::new(0), &mut empty_sink)
        .expect("zero-shot run");

    assert_eq!(summary.status(), SamplingRunStatus::Completed);
    assert_eq!(summary.committed_shots(), ShotCount::new(0));
    assert_eq!(empty_sink.write_calls, 0);
    assert_eq!(empty_sink.finish_calls, 0);

    let mut after_zero = CollectSink::default();
    session
        .run(ShotCount::new(32), &mut after_zero)
        .expect("run after zero shots");
    assert_eq!(after_zero.records, collect(&plan, 29, 32).0);
}

#[test]
fn cancellation_stops_between_batches_and_leaves_the_session_resumable() {
    struct CancellingSink {
        inner: CollectSink,
        cancellation: stab_core::SamplingCancellation,
    }

    impl MeasurementSink for CancellingSink {
        type Error = Infallible;

        fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
            self.inner.write_batch(batch)?;
            self.cancellation.cancel();
            Ok(())
        }

        fn finish(&mut self) -> Result<(), Self::Error> {
            self.inner.finish()
        }
    }

    let plan = noisy_plan();
    let expected = collect(&plan, 31, 130).0;
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(31)))
        .expect("construct session");
    let cancellation = session.cancellation();
    let mut first = CancellingSink {
        inner: CollectSink::default(),
        cancellation: cancellation.clone(),
    };
    let first_summary = session
        .run(ShotCount::new(130), &mut first)
        .expect("cancelled run");

    assert_eq!(first_summary.status(), SamplingRunStatus::Cancelled);
    assert_eq!(first_summary.committed_shots(), ShotCount::new(64));
    assert_eq!(first.inner.write_calls, 1);
    assert_eq!(first.inner.finish_calls, 1);

    cancellation.reset();
    let mut second = CollectSink::default();
    session
        .run(ShotCount::new(66), &mut second)
        .expect("resumed run");
    first.inner.records.extend(second.records);
    assert_eq!(first.inner.records, expected);
}

#[test]
fn pre_cancelled_run_performs_no_work_and_remains_resumable() {
    let plan = noisy_plan();
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(35)))
        .expect("construct session");
    let cancellation = session.cancellation();
    cancellation.cancel();
    let mut cancelled_sink = CollectSink::default();

    let summary = session
        .run(ShotCount::new(65), &mut cancelled_sink)
        .expect("pre-cancelled run");
    assert_eq!(summary.status(), SamplingRunStatus::Cancelled);
    assert_eq!(summary.committed_shots(), ShotCount::new(0));
    assert_eq!(summary.total_committed_shots(), ShotCount::new(0));
    assert_eq!(cancelled_sink.write_calls, 0);
    assert_eq!(cancelled_sink.finish_calls, 1);
    assert!(!session.is_poisoned());

    cancellation.reset();
    let mut resumed_sink = CollectSink::default();
    session
        .run(ShotCount::new(65), &mut resumed_sink)
        .expect("resumed run");
    assert_eq!(resumed_sink.records, collect(&plan, 35, 65).0);
}

#[test]
fn pre_cancelled_finish_failure_reports_zero_progress_and_poisons() {
    let plan = noisy_plan();
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(36)))
        .expect("construct session");
    session.cancellation().cancel();
    let mut sink = FailingSink {
        fail_write_call: None,
        fail_finish: true,
        write_calls: 0,
        finish_calls: 0,
    };

    match session.run(ShotCount::new(65), &mut sink) {
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
        other => panic!("unexpected pre-cancelled finish result: {other:?}"),
    }
    assert_eq!(sink.write_calls, 0);
    assert_eq!(sink.finish_calls, 1);
    assert!(session.is_poisoned());
}

#[test]
fn compact_huge_measurement_count_returns_a_typed_storage_error() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 9223372036854775807 {\n\
             M 0\n\
         }\n",
    )
    .expect("parse compact circuit");
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile folded sampling plan");
    let legacy_error = CompiledSampler::compile(&circuit)
        .expect_err("legacy sampler must reject plans its infallible adapters cannot execute");
    assert!(
        legacy_error
            .to_string()
            .contains("exceeding the 268435456-byte safety limit"),
        "{legacy_error}"
    );

    let mut result = None;
    let allocations = allocation_counter::measure(|| {
        result = Some(plan.session(RandomPolicy::Seeded(Seed::new(1))));
    });
    match result.expect("session attempt was recorded") {
        Err(SamplingExecutionError::SessionStorageLimit {
            estimated_bytes,
            limit_bytes,
        }) => {
            assert_eq!(limit_bytes, 256 * 1024 * 1024);
            assert!(estimated_bytes > u128::from(limit_bytes));
        }
        other => panic!("expected pre-allocation session storage admission error, got {other:?}"),
    }
    assert_eq!(
        allocations.bytes_total, 0,
        "storage admission allocated before rejecting: {allocations:?}"
    );
}

#[test]
fn callback_adapters_stop_at_the_exact_visitor_error_across_batches() {
    let circuit = Circuit::from_stim_str("H 0\nM 0\n").expect("parse callback circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile callback sampler");

    let mut try_calls = 0_usize;
    let result =
        sampler.try_for_each_sample_with_seed_and_reference_mode(129, Some(41), false, |_| {
            try_calls += 1;
            if try_calls == 65 {
                Err("stop-at-second-batch")
            } else {
                Ok(())
            }
        });
    match result {
        Err(RunError::Sink {
            phase,
            source,
            progress,
        }) => {
            assert_eq!(phase, SinkFailurePhase::WriteBatch);
            assert_eq!(source, "stop-at-second-batch");
            assert_eq!(progress.committed_shots(), ShotCount::new(64));
            assert_eq!(progress.attempted_batch_shots(), ShotCount::new(64));
        }
        other => panic!("unexpected error-aware callback result: {other:?}"),
    }
    assert_eq!(try_calls, 65);

    let mut legacy_calls = 0_usize;
    let legacy_result =
        sampler.for_each_sample_with_seed_and_reference_mode(129, Some(41), false, |_| {
            legacy_calls += 1;
            if legacy_calls == 65 {
                Err("legacy-stop")
            } else {
                Ok(())
            }
        });
    assert_eq!(legacy_result, Err("legacy-stop"));
    assert_eq!(legacy_calls, 65);
}

#[test]
fn sink_write_and_finish_errors_poison_with_exact_progress() {
    let plan = noisy_plan();

    let mut write_session = plan
        .session(RandomPolicy::Seeded(Seed::new(37)))
        .expect("construct write-failure session");
    let mut write_sink = FailingSink {
        fail_write_call: Some(2),
        fail_finish: false,
        write_calls: 0,
        finish_calls: 0,
    };
    let write_result = write_session.run(ShotCount::new(70), &mut write_sink);
    assert_eq!(
        write_result
            .as_ref()
            .expect_err("second batch must fail")
            .progress()
            .committed_shots(),
        ShotCount::new(64)
    );
    match write_result {
        Err(RunError::Sink {
            phase,
            source,
            progress,
        }) => {
            assert_eq!(phase, SinkFailurePhase::WriteBatch);
            assert_eq!(phase.as_str(), "write-batch");
            assert_eq!(source, TestSinkError::Write);
            assert_eq!(progress.committed_shots(), ShotCount::new(64));
            assert_eq!(progress.attempted_batch_shots(), ShotCount::new(6));
        }
        other => panic!("unexpected write result: {other:?}"),
    }
    assert!(write_session.is_poisoned());
    assert_eq!(write_sink.write_calls, 2);
    assert_eq!(write_sink.finish_calls, 0);

    let mut unused = FailingSink {
        fail_write_call: None,
        fail_finish: false,
        write_calls: 0,
        finish_calls: 0,
    };
    assert!(matches!(
        write_session.run(ShotCount::new(1), &mut unused),
        Err(RunError::Engine {
            source: SamplingExecutionError::SessionPoisoned,
            ..
        })
    ));
    assert_eq!(unused.write_calls, 0);
    assert_eq!(
        CircuitError::from(SamplingExecutionError::SessionPoisoned).to_string(),
        CircuitError::InvalidSamplerCompilation {
            message: "sampling session is poisoned".to_owned()
        }
        .to_string()
    );

    let mut finish_session = plan
        .session(RandomPolicy::Seeded(Seed::new(41)))
        .expect("construct finish-failure session");
    let mut finish_sink = FailingSink {
        fail_write_call: None,
        fail_finish: true,
        write_calls: 0,
        finish_calls: 0,
    };
    match finish_session.run(ShotCount::new(5), &mut finish_sink) {
        Err(RunError::Sink {
            phase,
            source,
            progress,
        }) => {
            assert_eq!(phase, SinkFailurePhase::Finish);
            assert_eq!(phase.as_str(), "finish");
            assert_eq!(source, TestSinkError::Finish);
            assert_eq!(progress.committed_shots(), ShotCount::new(5));
            assert_eq!(progress.attempted_batch_shots(), ShotCount::new(0));
        }
        other => panic!("unexpected finish result: {other:?}"),
    }
    assert!(finish_session.is_poisoned());
}
