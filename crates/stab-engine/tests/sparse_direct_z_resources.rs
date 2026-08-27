#![allow(
    clippy::expect_used,
    reason = "resource-contract tests use direct fixture setup assertions for focused failures"
)]

use stab_engine::{
    DetectionSamplingCompiler, MeasurementToDetectionCompiler, ReferenceSampleMode,
    ReferenceSampleTree, SamplingCompiler, SamplingExecutionError,
};
use stab_model::Circuit;

const MAX_STIM_QUBIT: usize = (1 << 24) - 1;

fn sparse_direct_z_circuit() -> Circuit {
    Circuit::from_stim_str(&format!(
        "M {MAX_STIM_QUBIT}\n\
         DETECTOR rec[-1]\n"
    ))
    .expect("parse maximum-id sparse Direct-Z circuit")
}

#[test]
fn maximum_id_direct_z_reference_operations_use_the_scalar_plan() {
    let circuit = sparse_direct_z_circuit();
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile sparse Direct-Z plan");

    assert_eq!(
        plan.try_reference_sample()
            .expect("compute sparse Direct-Z reference"),
        vec![false]
    );
    assert_eq!(
        plan.try_count_determined_measurements(false)
            .expect("count known-input measurements"),
        1
    );
    assert_eq!(
        plan.try_count_determined_measurements(true)
            .expect("count unknown-input measurements"),
        0
    );

    let tree = ReferenceSampleTree::from_circuit_reference_sample(&circuit)
        .expect("construct sparse Direct-Z reference tree");
    assert_eq!(
        tree.decompress().expect("decompress reference tree"),
        vec![false]
    );
}

#[test]
fn maximum_id_direct_z_detection_compilation_stays_constant_storage() {
    let circuit = sparse_direct_z_circuit();
    let conversion = MeasurementToDetectionCompiler::new()
        .reference_sample_mode(ReferenceSampleMode::UseReferenceSample)
        .compile(&circuit)
        .expect("compile sparse Direct-Z converter");
    assert_eq!(conversion.measurement_width().get(), 1);
    assert_eq!(conversion.detector_width().get(), 1);

    let plan = DetectionSamplingCompiler::new()
        .compile(&circuit)
        .expect("compile sparse Direct-Z detector sampler");
    assert_eq!(plan.measurement_width().get(), 1);
    assert_eq!(plan.detector_width().get(), 1);
}

#[test]
fn sparse_general_frame_reference_work_is_rejected_before_allocation() {
    const OVER_LIMIT_QUBIT: usize = 9_000;
    let circuit = Circuit::from_stim_str(&format!(
        "H {OVER_LIMIT_QUBIT}\n\
         M {OVER_LIMIT_QUBIT}\n"
    ))
    .expect("parse maximum-id sparse general-frame circuit");
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile sparse general-frame plan");

    for error in [
        plan.try_reference_sample()
            .expect_err("reference sample must reject quadratic storage"),
        plan.try_count_determined_measurements(false)
            .expect_err("determined count must reject quadratic storage"),
    ] {
        assert!(matches!(
            error,
            SamplingExecutionError::SessionStorageLimit {
                limit_bytes: 268_435_456,
                ..
            }
        ));
    }
}
