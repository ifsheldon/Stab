#![allow(
    clippy::expect_used,
    reason = "resource-contract tests use direct fixture setup assertions for focused failures"
)]

use std::assert_matches;

use stab_engine::{
    DetectionSamplingCompiler, MeasurementToDetectionCompiler, RandomPolicy, ReferenceSampleMode,
    SamplingCompiler, SamplingExecutionError, Seed,
};
use stab_model::Circuit;

const MAX_STIM_QUBIT: usize = (1 << 24) - 1;

fn sparse_single_measurement_circuit() -> Circuit {
    Circuit::from_stim_str(&format!(
        "M {MAX_STIM_QUBIT}\n\
         DETECTOR rec[-1]\n"
    ))
    .expect("parse maximum-id sparse measurement circuit")
}

fn assert_storage_limit(error: SamplingExecutionError) {
    assert_matches!(
        error,
        SamplingExecutionError::SessionStorageLimit {
            limit_bytes: 1_073_741_824,
            ..
        }
    );
}

/// Sampling and reference work size their state by the largest qubit index, so a single
/// maximum-index measurement is refused before allocation, exactly as detection refuses it.
#[test]
fn maximum_id_sampling_work_is_rejected_before_allocation() {
    let circuit = sparse_single_measurement_circuit();
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compilation retains only the sparse target list");

    assert_storage_limit(
        plan.try_reference_sample()
            .expect_err("reference sample must reject quadratic storage"),
    );
    for unknown_input in [false, true] {
        assert_storage_limit(
            plan.try_count_determined_measurements(unknown_input)
                .expect_err("determined count must reject quadratic storage"),
        );
    }
    for mode in [
        ReferenceSampleMode::UseReferenceSample,
        ReferenceSampleMode::SkipReferenceSample,
    ] {
        assert_storage_limit(
            plan.session_with_reference_mode(RandomPolicy::Seeded(Seed::new(1)), mode)
                .expect_err("the frame session must reject its frame storage"),
        );
    }
}

#[test]
fn maximum_id_detection_compilation_stays_constant_storage() {
    let circuit = sparse_single_measurement_circuit();
    let conversion = MeasurementToDetectionCompiler::new()
        .reference_sample_mode(ReferenceSampleMode::SkipReferenceSample)
        .compile(&circuit)
        .expect("compile sparse skip-reference converter");
    assert_eq!(conversion.measurement_width().get(), 1);
    assert_eq!(conversion.detector_width().get(), 1);

    let error = MeasurementToDetectionCompiler::new()
        .reference_sample_mode(ReferenceSampleMode::UseReferenceSample)
        .compile(&circuit)
        .expect_err("the reference sample must reject quadratic storage");
    assert!(
        error.to_string().contains("1073741824-byte memory budget"),
        "{error}"
    );

    let plan = DetectionSamplingCompiler::new()
        .compile(&circuit)
        .expect("compile sparse detector sampler");
    assert_eq!(plan.measurement_width().get(), 1);
    assert_eq!(plan.detector_width().get(), 1);
}

#[test]
fn sparse_reference_simulator_work_is_rejected_before_allocation() {
    const OVER_LIMIT_QUBIT: usize = 17_000;
    let circuit = Circuit::from_stim_str(&format!(
        "H {OVER_LIMIT_QUBIT}\n\
         M {OVER_LIMIT_QUBIT}\n"
    ))
    .expect("parse sparse over-limit circuit");
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile sparse over-limit plan");

    for error in [
        plan.try_reference_sample()
            .expect_err("reference sample must reject quadratic storage"),
        plan.try_count_determined_measurements(false)
            .expect_err("determined count must reject quadratic storage"),
    ] {
        assert_storage_limit(error);
    }
}
