#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "canonical-owner tests use compact fixture setup and exact contract failures"
)]

use std::convert::Infallible;
use std::fmt;
use std::thread;

use sha2::{Digest as _, Sha256};
use stab_engine::{
    BackendPreference, CompiledDetectionConverter, DemError, DemResourceKind, DemSamplerLimits,
    DemSamplingCompiler, DemSamplingExecutionError, DemSamplingRunError, DetectionCompileError,
    DetectionConversionLimits, DetectionConversionOptions, DetectionError,
    DetectionRecordLimitSubject, DetectionResourceKind, DetectionResourceLimitError,
    DetectionSamplingCompiler, MeasurementToDetectionCompiler, PlanFingerprint, RandomPolicy,
    ReferenceSampleMode, RunError, SamplingBackend, SamplingCompileError, SamplingCompileErrorCode,
    SamplingCompiler, SamplingExecutionError, SamplingPlan, SamplingRunStatus, Seed, ShotCount,
    SinkFailurePhase, detection_record_width_with_limits, measurement_record_count_with_limits,
    validate_detection_sampling_circuit_with_limits,
};
use stab_model::{Circuit, DetectorErrorModel};
use stab_records::{DemSampleBatchView, DemSampleSink, MeasurementBatchView, MeasurementSink};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit fixture")
}

fn dem_plan(text: &str) -> stab_engine::DemSamplingPlan {
    let model = DetectorErrorModel::from_dem_str(text).expect("parse DEM fixture");
    DemSamplingCompiler::new()
        .compile(&model)
        .expect("compile DEM sampling plan")
}

fn skip_reference_sample() -> DetectionConversionOptions {
    DetectionConversionOptions {
        skip_reference_sample: true,
    }
}

fn expect_dem_resource(error: DemError) -> stab_engine::DemResourceLimitError {
    match error {
        DemError::ResourceLimit(resource) => resource,
        other => panic!("expected DEM resource-limit error, got {other:?}"),
    }
}

fn expect_detection_resource(error: DetectionError) -> DetectionResourceLimitError {
    match error {
        DetectionError::ResourceLimit(resource) => resource,
        other => panic!("expected detection resource-limit error, got {other:?}"),
    }
}

fn expect_detection_compile_resource(error: DetectionCompileError) -> DetectionResourceLimitError {
    match error {
        DetectionCompileError::InvalidCircuit(DetectionError::ResourceLimit(resource)) => resource,
        other => panic!("expected detection compile resource-limit error, got {other:?}"),
    }
}

#[derive(Default)]
struct DemWitnessSink {
    detector_zero: Vec<bool>,
    shots: usize,
    sampled_error_shots: usize,
    write_calls: usize,
    finish_calls: usize,
}

impl DemSampleSink for DemWitnessSink {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        self.write_calls += 1;
        let detection = batch.detection();
        self.shots += detection.shot_count();
        for shot in 0..detection.shot_count() {
            if let Some(bit) = detection.detectors().get(shot, 0) {
                self.detector_zero.push(bit);
            }
        }
        if batch.sampled_errors().is_some() {
            self.sampled_error_shots += detection.shot_count();
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finish_calls += 1;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct MeasurementCollector {
    records: Vec<Vec<bool>>,
    write_calls: usize,
    finish_calls: usize,
}

impl MeasurementSink for MeasurementCollector {
    type Error = Infallible;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        self.write_calls += 1;
        for shot in 0..batch.shot_count() {
            let record = (0..batch.width().get())
                .map(|bit| {
                    batch
                        .get(shot, bit)
                        .expect("validated batch coordinates must be readable")
                })
                .collect();
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

impl fmt::Display for TestSinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write => formatter.write_str("write failure"),
            Self::Finish => formatter.write_str("finish failure"),
        }
    }
}

impl std::error::Error for TestSinkError {}

#[derive(Debug)]
struct FailingMeasurementSink {
    fail_write_call: Option<usize>,
    fail_finish: bool,
    write_calls: usize,
    finish_calls: usize,
}

impl MeasurementSink for FailingMeasurementSink {
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

fn noisy_sampling_plan() -> SamplingPlan {
    SamplingCompiler::new()
        .compile(&circuit("H 0\nM 0\nCX rec[-1] 1\nM(0.125) 1\n"))
        .expect("compile noisy sampling plan")
}

fn collect_measurements(plan: &SamplingPlan, seed: u64, shots: u64) -> MeasurementCollector {
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(seed)))
        .expect("construct sampling session");
    let mut sink = MeasurementCollector::default();
    let summary = session
        .run(ShotCount::new(shots), &mut sink)
        .expect("run sampling session");
    assert_eq!(summary.status(), SamplingRunStatus::Completed);
    assert_eq!(summary.committed_shots(), ShotCount::new(shots));
    sink
}

#[test]
fn a2_dem_sampler_byte_policy_admission() {
    let plan = dem_plan("");
    let bytes_per_shot = plan
        .materialized_bytes_per_shot(false)
        .expect("compute exact materialized byte charge");
    let exact_bytes = bytes_per_shot
        .checked_mul(3)
        .expect("small fixture byte count must fit");
    let limits = DemSamplerLimits::default().with_max_materialized_bytes(exact_bytes);

    plan.validate_materialized_bytes_with_limits(3, false, limits)
        .expect("the exact materialized-byte maximum must be admitted");
    let resource = expect_dem_resource(
        plan.validate_materialized_bytes_with_limits(4, false, limits)
            .expect_err("the first byte above the limit must be rejected"),
    );
    assert_eq!(resource.kind(), DemResourceKind::MaterializedBytes);
    assert_eq!(
        resource.actual(),
        u64::try_from(bytes_per_shot * 4).expect("fixture byte count fits u64")
    );
    assert_eq!(
        resource.limit(),
        u64::try_from(exact_bytes).expect("fixture byte limit fits u64")
    );
}

#[test]
fn a2_dem_sampler_replay_work_policy_admission() {
    let plan = dem_plan("error(1) D0\n");
    let records = vec![vec![true], vec![false]];
    let exact = DemSamplerLimits::default().with_max_replay_work_units(4);
    plan.validate_replay_with_limits(ShotCount::new(2), exact)
        .expect("two replay records reach the exact four-unit work limit");

    let mut accepted_session = plan
        .session_with_limits(RandomPolicy::Seeded(Seed::new(7)), exact)
        .expect("construct exact-limit replay session");
    let mut accepted_sink = DemWitnessSink::default();
    let summary = accepted_session
        .replay(&records, &mut accepted_sink)
        .expect("replay at exact work limit");
    assert_eq!(summary.committed_shots(), ShotCount::new(2));
    assert_eq!(accepted_sink.detector_zero, vec![true, false]);
    assert_eq!(accepted_sink.sampled_error_shots, 2);

    let rejected = exact.with_max_replay_work_units(3);
    let mut rejected_session = plan
        .session_with_limits(RandomPolicy::Seeded(Seed::new(7)), rejected)
        .expect("construct below-limit replay session");
    let mut rejected_sink = DemWitnessSink::default();
    let error = rejected_session
        .replay(&records, &mut rejected_sink)
        .expect_err("the first excess replay-work unit must fail before output");
    let resource = match error {
        DemSamplingRunError::Engine {
            source: DemSamplingExecutionError::InvalidRequest(DemError::ResourceLimit(resource)),
            ..
        } => resource,
        other => panic!("expected replay resource rejection, got {other:?}"),
    };
    assert_eq!(resource.kind(), DemResourceKind::ReplayWorkUnits);
    assert_eq!((resource.actual(), resource.limit()), (4, 3));
    assert_eq!(rejected_sink.write_calls, 0);
    assert_eq!(rejected_sink.finish_calls, 0);
}

#[test]
fn a2_dem_sampler_unit_policy_admission() {
    let plan = dem_plan("");
    let exact = DemSamplerLimits::default().with_max_materialized_units(3);

    plan.validate_sample_buffer_units_with_limits(3, false, exact)
        .expect("three empty records reach the exact ownership-unit maximum");
    let resource = expect_dem_resource(
        plan.validate_sample_buffer_units_with_limits(4, false, exact)
            .expect_err("the first excess ownership unit must be rejected"),
    );
    assert_eq!(resource.kind(), DemResourceKind::MaterializedUnits);
    assert_eq!((resource.actual(), resource.limit()), (4, 3));

    let error_plan = dem_plan("error(0.5) D0\n");
    let exact_with_errors = DemSamplerLimits::default().with_max_materialized_units(2);
    error_plan
        .validate_sample_buffer_units_with_limits(1, true, exact_with_errors)
        .expect("one detector and one sampled-error bit reach two units");
    let resource = expect_dem_resource(
        error_plan
            .validate_sample_buffer_units_with_limits(2, true, exact_with_errors)
            .expect_err("a second materialized error record must exceed the unit budget"),
    );
    assert_eq!(resource.kind(), DemResourceKind::MaterializedUnits);
    assert_eq!((resource.actual(), resource.limit()), (4, 2));
}

#[test]
fn a2_dem_sampler_work_policy_admission() {
    let plan = dem_plan("error(0.5) D0\n");
    let limits = DemSamplerLimits::default().with_max_sampled_error_applications(3);
    plan.validate_sampled_error_work_units_with_limits(3, limits)
        .expect("three shots reach the exact sampled-error work limit");

    let mut accepted_session = plan
        .session_with_limits(RandomPolicy::Seeded(Seed::new(11)), limits)
        .expect("construct exact-work session");
    let mut accepted_sink = DemWitnessSink::default();
    accepted_session
        .run_with_sampled_errors(ShotCount::new(3), &mut accepted_sink)
        .expect("run at exact sampled-error work maximum");
    assert_eq!(accepted_sink.shots, 3);
    assert_eq!(accepted_sink.sampled_error_shots, 3);

    let mut rejected_session = plan
        .session_with_limits(RandomPolicy::Seeded(Seed::new(11)), limits)
        .expect("construct rejected-work session");
    let mut rejected_sink = DemWitnessSink::default();
    let error = rejected_session
        .run_with_sampled_errors(ShotCount::new(4), &mut rejected_sink)
        .expect_err("the first excess sampled-error application must reject");
    let resource = match error {
        DemSamplingRunError::Engine {
            source: DemSamplingExecutionError::InvalidRequest(DemError::ResourceLimit(resource)),
            ..
        } => resource,
        other => panic!("expected sampled-work resource rejection, got {other:?}"),
    };
    assert_eq!(resource.kind(), DemResourceKind::SampledErrorApplications);
    assert_eq!((resource.actual(), resource.limit()), (4, 3));
    assert_eq!(rejected_sink.write_calls, 0);
    assert_eq!(rejected_sink.finish_calls, 0);
}

#[test]
fn a2_detection_compiled_plan_policy() {
    let circuit = circuit(
        "M 0 1\n\
         REPEAT 3 {\n\
         DETECTOR rec[-1] rec[-2]\n\
         }\n",
    );
    let vector_bytes = u64::try_from(std::mem::size_of::<Vec<usize>>()).expect("Vec size fits u64");
    let usize_bytes = u64::try_from(std::mem::size_of::<usize>()).expect("usize size fits u64");
    let exact_bytes = 3 * vector_bytes + 6 * usize_bytes;
    let exact = DetectionConversionLimits::default()
        .with_max_repeat_unroll(3)
        .with_max_repeat_iterations(3)
        .with_max_expanded_instructions(4)
        .with_max_compiled_terms(6)
        .with_max_compiled_bytes(exact_bytes);
    let converter =
        CompiledDetectionConverter::compile_with_limits(&circuit, skip_reference_sample(), exact)
            .expect("exact compiled-plan boundaries must be admitted");
    assert_eq!(converter.detector_count(), 3);

    for (limits, expected_kind, actual, limit) in [
        (
            exact.with_max_compiled_terms(5),
            DetectionResourceKind::CompiledTerms,
            6,
            5,
        ),
        (
            exact.with_max_compiled_bytes(exact_bytes - 1),
            DetectionResourceKind::CompiledBytes,
            exact_bytes,
            exact_bytes - 1,
        ),
    ] {
        let resource = expect_detection_resource(
            CompiledDetectionConverter::compile_with_limits(
                &circuit,
                skip_reference_sample(),
                limits,
            )
            .expect_err("the first compiled-plan unit above its limit must reject"),
        );
        assert_eq!(resource.kind(), expected_kind);
        assert_eq!((resource.actual(), resource.limit()), (actual, limit));
    }
}

#[test]
fn a2_detection_entry_policy_propagation() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let limits = DetectionConversionLimits::default().with_max_expanded_instructions(0);

    for error in [
        measurement_record_count_with_limits(&circuit, limits)
            .expect_err("measurement counting must apply caller limits"),
        detection_record_width_with_limits(&circuit, limits)
            .expect_err("detection-width counting must apply caller limits"),
    ] {
        let resource = expect_detection_resource(error);
        assert_eq!(resource.kind(), DetectionResourceKind::ExpandedInstructions);
        assert_eq!((resource.actual(), resource.limit()), (1, 0));
    }

    let resource = expect_detection_compile_resource(
        MeasurementToDetectionCompiler::new()
            .limits(limits)
            .reference_sample_mode(ReferenceSampleMode::SkipReferenceSample)
            .compile(&circuit)
            .expect_err("streaming conversion compilation must apply caller limits"),
    );
    assert_eq!(resource.kind(), DetectionResourceKind::ExpandedInstructions);
    assert_eq!((resource.actual(), resource.limit()), (1, 0));
}

#[test]
fn a2_detection_frame_policy_propagation() {
    let circuit = circuit(
        "REPEAT 2 {\n\
         MPAD 0\n\
         OBSERVABLE_INCLUDE(0) Z0\n\
         }\n",
    );
    let limits = DetectionConversionLimits::default().with_max_repeat_unroll(1);

    let validation = expect_detection_resource(
        validate_detection_sampling_circuit_with_limits(&circuit, limits)
            .expect_err("frame validation must apply repeat limits"),
    );
    let compilation = expect_detection_compile_resource(
        DetectionSamplingCompiler::new()
            .limits(limits)
            .compile(&circuit)
            .expect_err("detection sampling compilation must apply repeat limits"),
    );
    for resource in [validation, compilation] {
        assert_eq!(resource.kind(), DetectionResourceKind::RepeatCount);
        assert_eq!((resource.actual(), resource.limit()), (2, 1));
    }
}

#[test]
fn a2_detection_record_policy_admission() {
    let circuit = circuit("M 0 1\nDETECTOR rec[-1]\n");
    let exact = DetectionConversionLimits::default().with_max_record_bits(2);
    let converter =
        CompiledDetectionConverter::compile_with_limits(&circuit, skip_reference_sample(), exact)
            .expect("two measurement bits reach the exact record-width limit");
    let output = converter
        .convert_record(&[false, true])
        .expect("execute exact-width conversion");
    assert_eq!(output.detectors, vec![true]);

    let resource = expect_detection_resource(
        CompiledDetectionConverter::compile_with_limits(
            &circuit,
            skip_reference_sample(),
            exact.with_max_record_bits(1),
        )
        .expect_err("the second measurement bit must exceed the record-width limit"),
    );
    assert_eq!(
        resource.kind(),
        DetectionResourceKind::RecordBits(DetectionRecordLimitSubject::MeasurementRecord)
    );
    assert_eq!((resource.actual(), resource.limit()), (2, 1));
}

#[test]
fn a2_detection_repeat_policy_admission() {
    let circuit = circuit(
        "REPEAT 2 {\n\
         REPEAT 2 {\n\
         M 0\n\
         }\n\
         }\n",
    );
    let exact = DetectionConversionLimits::default()
        .with_max_repeat_unroll(2)
        .with_max_repeat_iterations(6)
        .with_max_expanded_instructions(100);
    CompiledDetectionConverter::compile_with_limits(&circuit, skip_reference_sample(), exact)
        .expect("the exact aggregate repeat-iteration budget must be admitted");

    let resource = expect_detection_resource(
        CompiledDetectionConverter::compile_with_limits(
            &circuit,
            skip_reference_sample(),
            exact.with_max_repeat_iterations(5),
        )
        .expect_err("nested repeats must share one aggregate iteration budget"),
    );
    assert_eq!(resource.kind(), DetectionResourceKind::RepeatIterations);
    assert_eq!((resource.actual(), resource.limit()), (6, 5));
}

#[test]
fn a2_detection_work_policy_admission() {
    let circuit = circuit("REPEAT 3 {\nM 0\n}\n");
    let exact = DetectionConversionLimits::default()
        .with_max_repeat_unroll(3)
        .with_max_repeat_iterations(3)
        .with_max_expanded_instructions(3);
    CompiledDetectionConverter::compile_with_limits(&circuit, skip_reference_sample(), exact)
        .expect("exact repeat and expanded-work maxima must be admitted");

    let repeat_resource = expect_detection_resource(
        CompiledDetectionConverter::compile_with_limits(
            &circuit,
            skip_reference_sample(),
            exact.with_max_repeat_unroll(2),
        )
        .expect_err("per-repeat work must have an independent limit"),
    );
    assert_eq!(repeat_resource.kind(), DetectionResourceKind::RepeatCount);
    assert_eq!((repeat_resource.actual(), repeat_resource.limit()), (3, 2));

    let expanded_resource = expect_detection_resource(
        CompiledDetectionConverter::compile_with_limits(
            &circuit,
            skip_reference_sample(),
            exact.with_max_expanded_instructions(2),
        )
        .expect_err("expanded instruction work must have an independent limit"),
    );
    assert_eq!(
        expanded_resource.kind(),
        DetectionResourceKind::ExpandedInstructions
    );
    assert_eq!(
        (expanded_resource.actual(), expanded_resource.limit()),
        (3, 2)
    );
}

#[test]
fn a4_sampling_compile_diagnostic_contract() {
    let sweep_circuit = circuit("CX sweep[0] 0\nM 0\n");
    let invalid = SamplingCompiler::new()
        .compile(&sweep_circuit)
        .expect_err("ordinary sampling must reject sweep-controlled execution");
    assert_eq!(invalid.code(), SamplingCompileErrorCode::InvalidCircuit);
    match invalid {
        SamplingCompileError::InvalidCircuit { message } => {
            assert_eq!(message, "M8 sampler subset does not support CX");
        }
        other => panic!("expected an invalid-circuit diagnostic, got {other:?}"),
    }

    let unavailable = SamplingCompiler::new()
        .backend(BackendPreference::PortableSimd)
        .compile(&circuit("M 0\n"))
        .expect_err("unregistered portable-SIMD backend must remain distinct");
    assert_eq!(
        unavailable.code(),
        SamplingCompileErrorCode::BackendUnavailable
    );
    assert!(matches!(
        unavailable,
        SamplingCompileError::BackendUnavailable {
            requested: BackendPreference::PortableSimd
        }
    ));
}

#[test]
fn a4_sampling_compiler_backend_contract() {
    let circuit = circuit("H 0\nM(0.125) 0\n");
    let automatic = SamplingCompiler::new()
        .backend(BackendPreference::Auto)
        .compile(&circuit)
        .expect("compile automatic backend");
    let scalar = SamplingCompiler::new()
        .backend(BackendPreference::Scalar)
        .compile(&circuit)
        .expect("compile scalar backend");

    assert_eq!(automatic.backend(), SamplingBackend::Scalar);
    assert_eq!(scalar.backend(), SamplingBackend::Scalar);
    assert_eq!(stab_engine::REGISTERED_BACKENDS, &[SamplingBackend::Scalar]);
    assert_eq!(automatic.fingerprint(), scalar.fingerprint());
    assert_eq!(
        collect_measurements(&automatic, 17, 129).records,
        collect_measurements(&scalar, 17, 129).records
    );

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
fn a4_sampling_plan_fingerprint_contract() {
    let plan = SamplingCompiler::new()
        .backend(BackendPreference::Scalar)
        .compile(&circuit("M 0\n"))
        .expect("compile frozen fingerprint fixture");
    let fingerprint = plan.fingerprint();

    let mut executable = Sha256::new();
    executable.update(b"stab:sampling-executable-contract\0");
    executable.update(1_u16.to_be_bytes());
    executable.update([1_u8, 1_u8]);
    let executable_digest: [u8; 32] = executable.finalize().into();

    let mut reconstructed = Sha256::new();
    reconstructed.update(b"stab:plan-fingerprint\0");
    reconstructed.update(PlanFingerprint::SCHEMA_VERSION.to_be_bytes());
    reconstructed.update(plan.request_fingerprint().schema_version().to_be_bytes());
    reconstructed.update(plan.request_fingerprint().digest());
    reconstructed.update([1_u8]);
    reconstructed.update(SamplingPlan::EXECUTABLE_CONTRACT_SCHEMA_VERSION.to_be_bytes());
    reconstructed.update(executable_digest);
    let reconstructed_digest: [u8; 32] = reconstructed.finalize().into();

    assert_eq!(
        plan.request_fingerprint().digest_hex(),
        "f8b6f8896556955fd436ad8e1f1700eb031cd04bc910accbf549195102384e79"
    );
    assert_eq!(fingerprint.executable_contract_digest(), executable_digest);
    assert_eq!(fingerprint.digest(), reconstructed_digest);
    assert_eq!(
        fingerprint.digest_hex(),
        "6211d411207f181cf93ee7a6cac4a862d3167bc9e7c471a2484e5f16b08909d8"
    );
}

#[test]
fn a4_sampling_plan_sharing_contract() {
    let plan = noisy_sampling_plan();
    let left_plan = plan.clone();
    let right_plan = plan.clone();
    let left = thread::spawn(move || collect_measurements(&left_plan, 19, 130).records);
    let right = thread::spawn(move || collect_measurements(&right_plan, 19, 130).records);

    let left = left.join().expect("left sampling worker");
    let right = right.join().expect("right sampling worker");
    assert_eq!(left, right);
    assert_eq!(left, collect_measurements(&plan, 19, 130).records);
}

#[test]
fn a4_sampling_session_cancellation_contract() {
    struct CancellingSink {
        inner: MeasurementCollector,
        cancellation: stab_engine::SamplingCancellation,
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

    let plan = noisy_sampling_plan();
    let expected = collect_measurements(&plan, 31, 130).records;
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(31)))
        .expect("construct cancellable session");
    let cancellation = session.cancellation();
    let mut first = CancellingSink {
        inner: MeasurementCollector::default(),
        cancellation: cancellation.clone(),
    };
    let summary = session
        .run(ShotCount::new(130), &mut first)
        .expect("cooperatively cancel after the first batch");
    assert_eq!(summary.status(), SamplingRunStatus::Cancelled);
    assert_eq!(summary.committed_shots(), ShotCount::new(64));
    assert_eq!(first.inner.write_calls, 1);
    assert_eq!(first.inner.finish_calls, 1);

    cancellation.reset();
    let mut resumed = MeasurementCollector::default();
    session
        .run(ShotCount::new(66), &mut resumed)
        .expect("resume the same random stream");
    first.inner.records.extend(resumed.records);
    assert_eq!(first.inner.records, expected);
    assert!(!session.is_poisoned());
}

#[test]
fn a4_sampling_session_chunking_contract() {
    let plan = noisy_sampling_plan();
    let expected = collect_measurements(&plan, 23, 131).records;
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(23)))
        .expect("construct chunked sampling session");
    let mut first = MeasurementCollector::default();
    let mut second = MeasurementCollector::default();

    let first_summary = session
        .run(ShotCount::new(31), &mut first)
        .expect("run first chunk");
    let second_summary = session
        .run(ShotCount::new(100), &mut second)
        .expect("run second chunk");
    first.records.extend(second.records);

    assert_eq!(first.records, expected);
    assert_eq!(first_summary.committed_shots(), ShotCount::new(31));
    assert_eq!(first_summary.total_committed_shots(), ShotCount::new(31));
    assert_eq!(second_summary.committed_shots(), ShotCount::new(100));
    assert_eq!(second_summary.total_committed_shots(), ShotCount::new(131));
}

#[test]
fn a4_sampling_session_failure_contract() {
    let plan = noisy_sampling_plan();
    let mut write_session = plan
        .session(RandomPolicy::Seeded(Seed::new(37)))
        .expect("construct write-failure session");
    let mut write_sink = FailingMeasurementSink {
        fail_write_call: Some(2),
        fail_finish: false,
        write_calls: 0,
        finish_calls: 0,
    };
    match write_session.run(ShotCount::new(70), &mut write_sink) {
        Err(RunError::Sink {
            phase,
            source,
            progress,
        }) => {
            assert_eq!(phase, SinkFailurePhase::WriteBatch);
            assert_eq!(source, TestSinkError::Write);
            assert_eq!(progress.committed_shots(), ShotCount::new(64));
            assert_eq!(progress.attempted_batch_shots(), ShotCount::new(6));
        }
        other => panic!("expected second-batch sink failure, got {other:?}"),
    }
    assert!(write_session.is_poisoned());
    assert_eq!(write_sink.write_calls, 2);
    assert_eq!(write_sink.finish_calls, 0);

    let mut untouched = FailingMeasurementSink {
        fail_write_call: None,
        fail_finish: false,
        write_calls: 0,
        finish_calls: 0,
    };
    assert!(matches!(
        write_session.run(ShotCount::new(1), &mut untouched),
        Err(RunError::Engine {
            source: SamplingExecutionError::SessionPoisoned,
            ..
        })
    ));
    assert_eq!((untouched.write_calls, untouched.finish_calls), (0, 0));

    let mut finish_session = plan
        .session(RandomPolicy::Seeded(Seed::new(41)))
        .expect("construct finish-failure session");
    let mut finish_sink = FailingMeasurementSink {
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
            assert_eq!(source, TestSinkError::Finish);
            assert_eq!(progress.committed_shots(), ShotCount::new(5));
            assert_eq!(progress.attempted_batch_shots(), ShotCount::new(0));
        }
        other => panic!("expected finish failure, got {other:?}"),
    }
    assert!(finish_session.is_poisoned());
}
