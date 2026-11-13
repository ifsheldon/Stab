#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "canonical-owner tests use compact fixture setup and exact contract failures"
)]

use std::convert::Infallible;

use sha2::{Digest as _, Sha256};
use stab_engine::{
    DetectionCompileError, DetectionConversionLimits, DetectionError, DetectionRecordLimitSubject,
    DetectionResourceKind, DetectionResourceLimitError, DetectionSamplingCompiler,
    MeasurementToDetectionCompiler, MeasurementToDetectionPlan, PlanFingerprint,
    ReferenceSampleMode, SamplingCompiler, SamplingPlan, detection_record_width_with_limits,
    measurement_record_count_with_limits, validate_detection_sampling_circuit_with_limits,
};
use stab_model::Circuit;
use stab_records::{DetectionBatchView, DetectionSink, MeasurementBatchView, PackedShotBatch};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit fixture")
}

fn compile_detection(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> Result<MeasurementToDetectionPlan, DetectionCompileError> {
    MeasurementToDetectionCompiler::new()
        .limits(limits)
        .reference_sample_mode(ReferenceSampleMode::SkipReferenceSample)
        .compile(circuit)
}

#[derive(Default)]
struct DetectionWitnessSink {
    detectors: Vec<Vec<bool>>,
}

impl DetectionSink for DetectionWitnessSink {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        for shot in 0..batch.shot_count() {
            self.detectors.push(
                (0..batch.detector_width().get())
                    .map(|bit| {
                        batch
                            .detectors()
                            .get(shot, bit)
                            .expect("validated detection coordinate")
                    })
                    .collect(),
            );
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
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

fn compact_detection_plan_bytes(circuit: &Circuit) -> u64 {
    let mut limit = 0;
    loop {
        match compile_detection(
            circuit,
            DetectionConversionLimits::default().with_max_compiled_bytes(limit),
        ) {
            Ok(_) => return limit,
            Err(error) => {
                let resource = expect_detection_compile_resource(error);
                assert_eq!(resource.kind(), DetectionResourceKind::CompiledBytes);
                assert!(resource.actual() > limit);
                limit = resource.actual();
            }
        }
    }
}

fn detection_compiled_plan_policy() {
    let circuit = circuit(
        "M 0 1\n\
         REPEAT 3 {\n\
         DETECTOR rec[-1] rec[-2]\n\
         }\n",
    );
    let exact_bytes = compact_detection_plan_bytes(&circuit);
    let exact = DetectionConversionLimits::default()
        .with_max_repeat_iterations(3)
        .with_max_expanded_instructions(4)
        .with_max_compiled_terms(2)
        .with_max_compiled_bytes(exact_bytes);
    let plan = compile_detection(&circuit, exact)
        .expect("exact compiled-plan boundaries must be admitted");
    assert_eq!(plan.detector_width().get(), 3);

    for (limits, expected_kind, actual, limit) in [
        (
            exact.with_max_compiled_terms(1),
            DetectionResourceKind::CompiledTerms,
            2,
            1,
        ),
        (
            exact.with_max_compiled_bytes(exact_bytes - 1),
            DetectionResourceKind::CompiledBytes,
            exact_bytes,
            exact_bytes - 1,
        ),
    ] {
        let resource = expect_detection_compile_resource(
            compile_detection(&circuit, limits)
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
    let limits = DetectionConversionLimits::default().with_max_repeat_iterations(1);

    let validation = expect_detection_resource(
        validate_detection_sampling_circuit_with_limits(&circuit, limits)
            .expect_err("frame validation must apply aggregate repeat-work limits"),
    );
    let compilation = expect_detection_compile_resource(
        DetectionSamplingCompiler::new()
            .limits(limits)
            .compile(&circuit)
            .expect_err("detection sampling compilation must apply aggregate repeat-work limits"),
    );
    for resource in [validation, compilation] {
        assert_eq!(resource.kind(), DetectionResourceKind::RepeatIterations);
        assert_eq!((resource.actual(), resource.limit()), (2, 1));
    }
}

fn detection_record_policy_admission() {
    let circuit = circuit("M 0 1\nDETECTOR rec[-1]\n");
    let exact = DetectionConversionLimits::default().with_max_record_bits(2);
    let plan = compile_detection(&circuit, exact)
        .expect("two measurement bits reach the exact record-width limit");
    let measurements = PackedShotBatch::from_records(&[vec![false, true]], 2)
        .expect("pack exact-width measurement record");
    let mut session = plan.session().expect("create conversion session");
    let mut sink = DetectionWitnessSink::default();
    session
        .run(
            MeasurementBatchView::new(measurements.view()),
            None,
            &mut sink,
        )
        .expect("execute exact-width conversion");
    assert_eq!(sink.detectors, vec![vec![true]]);

    let resource = expect_detection_compile_resource(
        compile_detection(&circuit, exact.with_max_record_bits(1))
            .expect_err("the second measurement bit must exceed the record-width limit"),
    );
    assert_eq!(
        resource.kind(),
        DetectionResourceKind::RecordBits(DetectionRecordLimitSubject::MeasurementRecord)
    );
    assert_eq!((resource.actual(), resource.limit()), (2, 1));
}

fn detection_repeat_policy_admission() {
    let circuit = circuit(
        "REPEAT 2 {\n\
         REPEAT 2 {\n\
         M 0\n\
         }\n\
         }\n",
    );
    let exact = DetectionConversionLimits::default()
        .with_max_repeat_iterations(6)
        .with_max_expanded_instructions(100);
    compile_detection(&circuit, exact)
        .expect("the exact aggregate repeat-iteration budget must be admitted");

    let resource = expect_detection_compile_resource(
        compile_detection(&circuit, exact.with_max_repeat_iterations(5))
            .expect_err("nested repeats must share one aggregate iteration budget"),
    );
    assert_eq!(resource.kind(), DetectionResourceKind::RepeatIterations);
    assert_eq!((resource.actual(), resource.limit()), (6, 5));
}

fn detection_work_policy_admission() {
    let repeated_measurement = circuit("REPEAT 3 {\nM 0\n}\n");
    let exact = DetectionConversionLimits::default()
        .with_max_repeat_iterations(3)
        .with_max_expanded_instructions(3);
    compile_detection(&repeated_measurement, exact)
        .expect("exact aggregate repeat and expanded-work maxima must be admitted");

    let iteration_resource = expect_detection_compile_resource(
        compile_detection(&repeated_measurement, exact.with_max_repeat_iterations(2))
            .expect_err("aggregate repeat work must reject its first excess iteration"),
    );
    assert_eq!(
        iteration_resource.kind(),
        DetectionResourceKind::RepeatIterations
    );
    assert_eq!(
        (iteration_resource.actual(), iteration_resource.limit()),
        (3, 2)
    );

    let expanded_resource = expect_detection_compile_resource(
        compile_detection(
            &repeated_measurement,
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

    let above_old_syntax_cap = circuit("REPEAT 100001 {\nTICK\n}\n");
    compile_detection(
        &above_old_syntax_cap,
        DetectionConversionLimits::default()
            .with_max_repeat_iterations(100_001)
            .with_max_expanded_instructions(100_001),
    )
    .expect("repeat counts above the obsolete syntax cap remain compact and admissible");
}

#[test]
fn a2_detection_compile_limits_are_independent_and_atomic() {
    detection_compiled_plan_policy();
    detection_record_policy_admission();
    detection_repeat_policy_admission();
    detection_work_policy_admission();
}

#[test]
fn a4_sampling_plan_fingerprint_contract() {
    let plan = SamplingCompiler::new()
        .compile(&circuit("M 0\n"))
        .expect("compile frozen fingerprint fixture");
    let fingerprint = plan.fingerprint();

    let mut executable = Sha256::new();
    executable.update(b"stab:sampling-executable-contract\0");
    executable.update(6_u16.to_be_bytes());
    // Scalar backend, bit-plane Pauli frame, fold policy.
    executable.update([1_u8, 4_u8, 1_u8]);
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
        "3eed620c6e6811fe2808572d8cd39917731ceaf3a99a4c50cc60507ed77716af"
    );
    assert_eq!(fingerprint.executable_contract_digest(), executable_digest);
    assert_eq!(fingerprint.digest(), reconstructed_digest);
    assert_eq!(
        fingerprint.digest_hex(),
        "9aaeadd0765842bb713dfc254564cc4fb67ec0fd11b65ec70e0d13a99df667fd"
    );
}
