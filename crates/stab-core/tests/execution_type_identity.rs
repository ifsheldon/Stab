use std::any::TypeId;

macro_rules! assert_same_type {
    ($facade:ty, $canonical:ty) => {
        assert_eq!(TypeId::of::<$facade>(), TypeId::of::<$canonical>());
    };
}

#[test]
fn execution_facade_uses_canonical_engine_type_identities() {
    assert_same_type!(
        stab_core::execution::MeasurementToDetectionCompiler,
        stab_engine::MeasurementToDetectionCompiler
    );
    assert_same_type!(
        stab_core::execution::MeasurementToDetectionPlan,
        stab_engine::MeasurementToDetectionPlan
    );
    assert_same_type!(
        stab_core::execution::MeasurementToDetectionSession,
        stab_engine::MeasurementToDetectionSession
    );
    assert_same_type!(
        stab_core::execution::DetectionSamplingCompiler,
        stab_engine::DetectionSamplingCompiler
    );
    assert_same_type!(
        stab_core::execution::DetectionSamplingPlan,
        stab_engine::DetectionSamplingPlan
    );
    assert_same_type!(
        stab_core::execution::DetectionSamplingSession,
        stab_engine::DetectionSamplingSession
    );
    assert_same_type!(
        stab_core::execution::DemSamplingCompiler,
        stab_engine::DemSamplingCompiler
    );
    assert_same_type!(
        stab_core::execution::DemSamplingPlan,
        stab_engine::DemSamplingPlan
    );
    assert_same_type!(
        stab_core::execution::DemSamplingSession,
        stab_engine::DemSamplingSession
    );
}
