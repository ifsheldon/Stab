#![allow(
    clippy::expect_used,
    reason = "facade tests use compact fixture assertions"
)]

use std::convert::Infallible;

use stab_core::advanced::compat::CompiledDemSampler;
use stab_core::{
    DemSampleBatchView, DemSampleSink, DemSamplerLimits, DetectorErrorModel, RandomPolicy, Seed,
    ShotCount,
};

#[derive(Default)]
struct CollectSink {
    detectors: Vec<Vec<bool>>,
    observables: Vec<Vec<bool>>,
    sampled_errors: Vec<Vec<bool>>,
}

impl DemSampleSink for CollectSink {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        let detection = batch.detection();
        let sampled_errors = batch
            .sampled_errors()
            .expect("sampled-error run must provide its error plane");
        for shot_index in 0..detection.shot_count() {
            self.detectors.push(
                (0..detection.detector_width().get())
                    .map(|bit| {
                        detection
                            .detectors()
                            .get(shot_index, bit)
                            .expect("detector bit")
                    })
                    .collect(),
            );
            self.observables.push(
                (0..detection.observable_width().get())
                    .map(|bit| {
                        detection
                            .observables()
                            .get(shot_index, bit)
                            .expect("observable bit")
                    })
                    .collect(),
            );
            self.sampled_errors.push(
                (0..sampled_errors.bits_per_shot())
                    .map(|bit| {
                        sampled_errors
                            .get(shot_index, bit)
                            .expect("sampled-error bit")
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

#[test]
fn compatibility_materializer_matches_the_engine_owned_session() {
    let model = DetectorErrorModel::from_dem_str(
        "repeat 3 {\n  error(0.25) D0 L1\n  shift_detectors 1\n}\n",
    )
    .expect("parse DEM");
    let facade = CompiledDemSampler::compile(&model).expect("compile facade");
    let (materialized, materialized_errors) = facade
        .sample_detection_events_and_errors_with_seed(65, Some(7))
        .expect("materialize facade samples");

    let engine_plan = stab_engine::DemSamplingCompiler::new()
        .compile(&model)
        .expect("compile engine plan");
    let mut engine_session = engine_plan
        .session(RandomPolicy::Seeded(Seed::new(7)))
        .expect("create engine session");
    let mut engine = CollectSink::default();
    engine_session
        .run_with_sampled_errors(ShotCount::new(65), &mut engine)
        .expect("run engine session");

    assert_eq!(
        materialized
            .records
            .iter()
            .map(|record| record.detectors.clone())
            .collect::<Vec<_>>(),
        engine.detectors
    );
    assert_eq!(
        materialized
            .records
            .iter()
            .map(|record| record.observables.clone())
            .collect::<Vec<_>>(),
        engine.observables
    );
    assert_eq!(materialized_errors, engine.sampled_errors);
    assert_eq!(facade.plan().detector_width(), engine_plan.detector_width());
    assert_eq!(
        facade.plan().observable_width(),
        engine_plan.observable_width()
    );
    assert_eq!(
        facade.plan().sampled_error_width(),
        engine_plan.sampled_error_width()
    );
}

#[test]
fn facade_preserves_typed_dem_resource_context() {
    let model = DetectorErrorModel::from_dem_str("error(0.25) D0\n").expect("parse DEM");
    let facade = CompiledDemSampler::compile(&model).expect("compile facade");
    let error = facade
        .plan()
        .validate_replay_with_limits(
            ShotCount::new(2),
            DemSamplerLimits::default().with_max_replay_work_units(1),
        )
        .expect_err("reject replay work");
    assert!(
        matches!(&error, stab_engine::DemError::ResourceLimit(_)),
        "canonical plan must retain its engine resource error"
    );
    if let stab_engine::DemError::ResourceLimit(resource) = error {
        assert_eq!(
            resource.kind(),
            stab_engine::DemResourceKind::ReplayWorkUnits
        );
        assert_eq!(resource.actual(), 4);
        assert_eq!(resource.limit(), 1);
    }
}
