#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "canonical-owner tests use compact fixture setup and exact contract failures"
)]

use std::convert::Infallible;

use sha2::{Digest as _, Sha256};
use stab_engine::{
    DetectionCompileError, DetectionConversionLimits, DetectionError, DetectionResourceKind,
    DetectionResourceLimitError, DetectionSamplingCompiler, MeasurementToDetectionCompiler,
    MeasurementToDetectionPlan, PlanFingerprint, ReferenceSampleMode, SamplingCompiler,
    SamplingPlan, detection_record_width_with_limits, measurement_record_count_with_limits,
    validate_detection_sampling_circuit_with_limits,
};
use stab_model::{Circuit, MemoryBudget};
use stab_records::{DetectionBatchView, DetectionSink, MeasurementBatchView, PackedShotBatch};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit fixture")
}

fn compile_detection(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> Result<MeasurementToDetectionPlan, DetectionCompileError> {
    MeasurementToDetectionCompiler::new()
        .limits(limits.clone())
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

fn budget(bytes: u64) -> DetectionConversionLimits {
    DetectionConversionLimits::default().with_memory_budget(MemoryBudget::from_bytes(bytes))
}

fn compact_detection_plan_bytes(circuit: &Circuit) -> u64 {
    let mut limit = 0;
    loop {
        match compile_detection(circuit, budget(limit)) {
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

#[test]
fn detection_compiled_plan_admits_the_exact_budget_and_rejects_the_first_excess_byte() {
    let circuit = circuit(
        "M 0 1\n\
         REPEAT 3 {\n\
         DETECTOR rec[-1] rec[-2]\n\
         }\n",
    );
    let exact_bytes = compact_detection_plan_bytes(&circuit);
    let plan = compile_detection(&circuit, budget(exact_bytes))
        .expect("the exact compiled-plan budget must be admitted");
    assert_eq!(plan.detector_width().get(), 3);

    let resource = expect_detection_compile_resource(
        compile_detection(&circuit, budget(exact_bytes - 1))
            .expect_err("the first byte above the budget must reject"),
    );
    assert_eq!(resource.kind(), DetectionResourceKind::CompiledBytes);
    assert_eq!(
        (resource.actual(), resource.limit()),
        (exact_bytes, exact_bytes - 1)
    );
    assert!(resource.to_string().contains("memory budget"), "{resource}");
}

#[test]
fn every_detection_entry_point_applies_the_caller_budget() {
    let circuit = circuit("M 0\nDETECTOR rec[-1]\n");
    let limits = budget(0);

    for error in [
        measurement_record_count_with_limits(&circuit, limits.clone())
            .expect_err("measurement counting must apply the caller budget"),
        detection_record_width_with_limits(&circuit, limits.clone())
            .expect_err("detection-width counting must apply the caller budget"),
        validate_detection_sampling_circuit_with_limits(&circuit, limits.clone())
            .expect_err("frame validation must apply the caller budget"),
    ] {
        let resource = expect_detection_resource(error);
        assert_eq!(resource.kind(), DetectionResourceKind::CompiledBytes);
        assert_eq!(resource.limit(), 0);
    }

    for error in [
        compile_detection(&circuit, limits.clone()).expect_err("conversion compilation"),
        DetectionSamplingCompiler::new()
            .limits(limits.clone())
            .compile(&circuit)
            .expect_err("detection sampling compilation"),
    ] {
        let resource = expect_detection_compile_resource(error);
        assert_eq!(resource.kind(), DetectionResourceKind::CompiledBytes);
        assert_eq!(resource.limit(), 0);
    }
}

#[test]
fn record_width_is_charged_against_the_budget_and_converts_at_the_exact_budget() {
    let circuit = circuit("M 0 1\nDETECTOR rec[-1]\n");
    let exact_bytes = compact_detection_plan_bytes(&circuit);
    let plan = compile_detection(&circuit, budget(exact_bytes))
        .expect("the exact record and plan budget is admitted");
    let measurements = PackedShotBatch::from_records(&[vec![false, true]], 2)
        .expect("pack exact-width measurement record");
    // The session needs its own reusable storage, admitted against a budget that fits it.
    let plan_for_session =
        compile_detection(&circuit, DetectionConversionLimits::default()).expect("default budget");
    let mut session = plan_for_session
        .session()
        .expect("create conversion session");
    let mut sink = DetectionWitnessSink::default();
    session
        .run(
            MeasurementBatchView::new(measurements.view()),
            None,
            &mut sink,
        )
        .expect("execute conversion");
    assert_eq!(sink.detectors, vec![vec![true]]);
    assert_eq!(plan.measurement_width().get(), 2);

    // A compact circuit that declares an astronomically wide record is refused from its
    // arithmetic width before any record-sized storage is reserved.
    let wide = Circuit::from_stim_str("REPEAT 1000000000000000 {\nM 0\n}\nDETECTOR rec[-1]\n")
        .expect("parse wide-record fixture");
    let measured = allocation_counter::measure(|| {
        let resource = expect_detection_compile_resource(
            compile_detection(&wide, DetectionConversionLimits::default())
                .expect_err("a record far above the budget is refused"),
        );
        assert_eq!(resource.kind(), DetectionResourceKind::CompiledBytes);
        assert!(resource.actual() > 1_000_000_000_000_000);
    });
    assert!(measured.bytes_max < 1 << 20, "{measured:?}");
}

#[test]
fn expanded_detection_work_is_not_capped() {
    let compact = circuit(
        "REPEAT 1000000000000 {\n\
         TICK\n\
         H 0\n\
         }\n\
         REPEAT 2000000 {\n\
         REPEAT 2 {\n\
         M 0\n\
         }\n\
         }\n\
         DETECTOR rec[-1]\n",
    );
    let plan = compile_detection(&compact, DetectionConversionLimits::default())
        .expect("expanded work and repeat iterations far above the retired caps compile");
    assert_eq!(plan.measurement_width().get(), 4_000_000);
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
